//! Host lifecycle checks for the configured scene; legacy fixtures stay unmanaged.
use super::*;
use deedwake_core::{DeliveryRejection, DeliveryState, InputError};

const KINDS: [ProviderDelivery; 3] = [
    ProviderDelivery::Report,
    ProviderDelivery::Routine,
    ProviderDelivery::Intermediary,
];

fn present(residency: Residency, availability: Availability) -> EntityState {
    EntityState::Present {
        residency,
        availability,
    }
}
fn loaded() -> EntityState {
    present(Residency::Loaded, Availability::Available)
}
fn configured(trace: u64, confidence: &str, reaction: &str, delays: [u64; 3]) -> Convoy {
    Convoy::with_config(
        config::validate(config::Input {
            trace_capacity: trace,
            sela_confidence: confidence,
            reaction,
            initial_delivery_ticks: delays[0],
            routine_delivery_ticks: delays[1],
            intermediary_delivery_ticks: delays[2],
            ..config::DEFAULT_INPUT
        })
        .unwrap(),
    )
}
fn perform(scene: &mut Convoy, actions: &[u32]) {
    for action in actions {
        assert_eq!(scene.act(*action), 0, "action {action}");
    }
}
fn operations(scene: &Convoy) -> Vec<(u64, ServiceOperation)> {
    scene
        .core
        .perspective(PROVIDER)
        .records()
        .iter()
        .filter_map(|record| match received_fact(&scene.facts, record) {
            Some(Fact::Service { operation, .. }) => Some((record.time, operation)),
            _ => None,
        })
        .collect()
}
fn knows(scene: &Convoy, record: RecordId) -> bool {
    scene
        .core
        .perspective(PROVIDER)
        .records()
        .iter()
        .any(|known| known.id == record)
}
// All observable effects except the deliberately changed lifecycle registration.
fn effects(scene: &Convoy) -> String {
    let records: BTreeMap<_, _> = [PLAYER, WITNESS, PROVIDER, INTERMEDIARY]
        .into_iter()
        .flat_map(|entity| {
            scene
                .core
                .perspective(entity)
                .records()
                .iter()
                .map(|record| (record.id, (*record).clone()))
                .collect::<Vec<_>>()
        })
        .collect();
    let actions: Vec<_> = records
        .values()
        .filter_map(|record| match record.content {
            Content::ConfirmedOutcome { action, .. } => scene.core.action(action),
            _ => None,
        })
        .collect();
    let views: Vec<_> = [PLAYER, WITNESS, PROVIDER, INTERMEDIARY]
        .into_iter()
        .map(|entity| {
            scene
                .core
                .perspective(entity)
                .records()
                .iter()
                .map(|record| record.id)
                .collect::<Vec<_>>()
        })
        .collect();
    format!(
        "{:?}",
        (
            scene.core.time(),
            records,
            actions,
            views,
            scene.core.trace(),
            scene.core.dropped_trace(),
            KINDS.map(|kind| scene.provider_delivery_receipt(kind)),
            &scene.journal,
            &scene.dialogue,
            &scene.facts,
            scene.access,
            scene.refused,
        )
    )
}

#[test]
fn host_updates_and_inspections_are_quiet_and_legacy_scenes_are_unsupported() {
    let mut scene = configured(128, "trusted", "restrict_until_correction", [3, 3, 1]);
    for entity in [PLAYER, WITNESS, PROVIDER, INTERMEDIARY] {
        assert_eq!(scene.core.entity_state(entity), Some(loaded()));
    }
    perform(&mut scene, &[1]);
    let before = effects(&scene);
    for state in [
        present(Residency::Unloaded, Availability::Available),
        present(Residency::Loaded, Availability::Unavailable),
        present(Residency::Unloaded, Availability::Incapable),
        loaded(),
        EntityState::Retired,
        EntityState::Retired,
    ] {
        scene.set_provider_state(state).unwrap();
        for _ in 0..3 {
            assert_eq!(scene.provider_state(), Some(state));
            for kind in KINDS {
                let _ = scene.provider_delivery_receipt(kind);
            }
        }
        assert_eq!(effects(&scene), before);
    }
    assert_eq!(
        scene.set_provider_state(loaded()),
        Err(LifecycleError::Retired)
    );
    assert_eq!(effects(&scene), before);
    let legacy = [
        Convoy::new(),
        Convoy::with_condition(Condition::Control),
        Convoy::with_study(Study::PreciseImmediate),
        Convoy::with_study(Study::IncompleteImmediate),
        Convoy::with_study(Study::PreciseScheduled),
        Convoy::with_study(Study::IncompleteScheduled),
    ];
    for mut scene in legacy {
        let before = effects(&scene);
        assert_eq!(scene.provider_state(), None);
        assert_eq!(
            scene.set_provider_state(loaded()),
            Err(LifecycleError::UnknownEntity)
        );
        assert_eq!(effects(&scene), before);
        perform(&mut scene, &[1, 3, 4, 6, 6, 6]);
        assert!(
            KINDS
                .into_iter()
                .all(|kind| scene.provider_delivery_receipt(kind).is_none())
        );
    }
}

#[test]
fn unloaded_arrivals_wait_for_an_explicit_loaded_step_at_the_actual_hour() {
    for trace in [0, 2, 128] {
        let mut scene = configured(trace, "trusted", "restrict_until_correction", [3, 3, 1]);
        perform(&mut scene, &[1]);
        scene
            .set_provider_state(present(Residency::Unloaded, Availability::Available))
            .unwrap();
        perform(&mut scene, &[6, 6, 6, 6, 6]);
        assert!(knows(&scene, scene.report.unwrap()));
        assert_eq!(
            scene
                .provider_delivery_receipt(ProviderDelivery::Report)
                .unwrap()
                .state,
            DeliveryState::Delivered { at: 3 }
        );
        assert!(operations(&scene).is_empty());
        assert_eq!(scene.service_decision, None);
        assert!(!scene.core.trace().iter().any(|entry| matches!(entry.event,
            deedwake_core::Event::Proposed(id) if scene.core.action(id).unwrap().actor == PROVIDER)));
        assert!(scene.enabled(5));
        let before = effects(&scene);
        assert_eq!(scene.act(5), 1);
        assert_eq!(effects(&scene), before);
        scene.set_provider_state(loaded()).unwrap();
        assert_eq!(effects(&scene), before);
        perform(&mut scene, &[5]);
        assert_eq!(scene.time(), 5);
        assert_eq!(operations(&scene), vec![(5, ServiceOperation::Restrict)]);
        assert!(scene.dialogue.contains("At hour 5"));
        assert!(!scene.access);
        assert_eq!(operations(&scene).len(), 1);
    }
}

#[test]
fn corrections_received_while_unloaded_preserve_history_until_reload() {
    for confidence in ["trusted", "unverified"] {
        for reaction in ["restrict_until_correction", "warn_and_continue"] {
            for trace in [0, 2, 128] {
                let mut scene = configured(trace, confidence, reaction, [0, 3, 1]);
                perform(&mut scene, &[1, 5]);
                let old_response = scene.dialogue.clone();
                let old_report = scene.core.record(scene.report.unwrap()).unwrap().clone();
                let old_decision_id = scene.service_decision.unwrap();
                let old_decision = scene.core.action(old_decision_id).unwrap().clone();
                scene
                    .set_provider_state(present(Residency::Unloaded, Availability::Available))
                    .unwrap();
                perform(&mut scene, &[3, 4, 6, 6]);
                assert_eq!(operations(&scene).len(), 1);
                assert!(knows(&scene, scene.verification.unwrap()));
                assert!(!knows(&scene, scene.release.unwrap()));
                scene.set_provider_state(loaded()).unwrap();
                perform(&mut scene, &[5]);
                assert_eq!(
                    operations(&scene).len(),
                    if confidence == "trusted" { 2 } else { 1 }
                );
                if confidence == "trusted" {
                    assert_eq!(operations(&scene)[1].0, 2);
                }
                perform(&mut scene, &[6, 5]);
                assert_eq!(operations(&scene).len(), 2);
                assert_eq!(
                    operations(&scene)[1].0,
                    if confidence == "trusted" { 2 } else { 3 }
                );
                assert!(scene.access);
                assert!(scene.journal.contains(&old_response));
                assert_eq!(scene.core.record(old_report.id), Some(&old_report));
                assert_eq!(scene.core.action(old_decision_id), Some(&old_decision));
                assert!(scene.refused == (reaction == "restrict_until_correction"));
                for kind in KINDS {
                    assert!(matches!(
                        scene.provider_delivery_receipt(kind).unwrap().state,
                        DeliveryState::Delivered { .. }
                    ));
                }
            }
        }
    }
}

#[test]
fn temporary_unavailability_and_retirement_terminally_reject_all_queued_inputs() {
    for (state, reason) in [
        (
            present(Residency::Loaded, Availability::Unavailable),
            DeliveryRejection::Unavailable,
        ),
        (
            present(Residency::Unloaded, Availability::Incapable),
            DeliveryRejection::Incapable,
        ),
        (EntityState::Retired, DeliveryRejection::Retired),
    ] {
        for trace in [0, 2, 128] {
            let mut scene = configured(trace, "trusted", "restrict_until_correction", [3, 3, 1]);
            perform(&mut scene, &[1, 3, 4]);
            scene.set_provider_state(state).unwrap();
            let before = effects(&scene);
            assert_eq!(scene.act(5), 1);
            assert_eq!(effects(&scene), before);
            perform(&mut scene, &[6, 6, 6]);
            for (kind, at) in [
                (ProviderDelivery::Report, 3),
                (ProviderDelivery::Routine, 3),
                (ProviderDelivery::Intermediary, 1),
            ] {
                let receipt = scene.provider_delivery_receipt(kind).unwrap();
                assert_eq!(receipt.state, DeliveryState::Rejected { at, reason });
                assert!(!knows(&scene, receipt.delivery.record));
            }
            assert!(operations(&scene).is_empty());
            assert_eq!(scene.service_decision, None);
            assert!(!scene.core.trace().iter().any(|entry| matches!(entry.event,
                deedwake_core::Event::Proposed(id) if scene.core.action(id).unwrap().actor == PROVIDER)));
            if state != EntityState::Retired {
                let before = effects(&scene);
                scene.set_provider_state(loaded()).unwrap();
                assert_eq!(effects(&scene), before);
                perform(&mut scene, &[6, 5]);
                assert!(operations(&scene).is_empty());
                assert!(scene.access);
                assert!(KINDS.into_iter().all(|kind| matches!(
                    scene.provider_delivery_receipt(kind).unwrap().state,
                    DeliveryState::Rejected { .. }
                )));
            }
        }
    }
}

#[test]
fn lifecycle_changes_do_not_execute_received_evidence_or_rewrite_service_history() {
    let mut scene = configured(128, "trusted", "restrict_until_correction", [0, 0, 0]);
    scene
        .set_provider_state(present(Residency::Unloaded, Availability::Available))
        .unwrap();
    perform(&mut scene, &[1]);
    assert!(knows(&scene, scene.report.unwrap()));
    for state in [
        loaded(),
        present(Residency::Loaded, Availability::Unavailable),
        loaded(),
    ] {
        let before = effects(&scene);
        scene.set_provider_state(state).unwrap();
        assert_eq!(effects(&scene), before);
    }
    perform(&mut scene, &[5]);
    let original = operations(&scene);
    scene
        .set_provider_state(present(Residency::Unloaded, Availability::Available))
        .unwrap();
    perform(&mut scene, &[3, 4, 6, 6]);
    assert_eq!(operations(&scene), original);
    scene.set_provider_state(EntityState::Retired).unwrap();
    perform(&mut scene, &[6]);
    assert_eq!(operations(&scene), original);
    assert!(!scene.access);
    assert!(knows(&scene, scene.release.unwrap()));
}

#[test]
fn fixed_delivery_ids_acknowledge_exact_retries_without_resending_and_never_evict() {
    for trace in [0, 2] {
        for rejected in [false, true] {
            let mut scene = configured(trace, "trusted", "restrict_until_correction", [3, 3, 1]);
            perform(&mut scene, &[1, 3, 4]);
            // Pending and terminal retries both preserve all state.
            for terminal in [false, true] {
                if terminal {
                    if rejected {
                        scene.set_provider_state(EntityState::Retired).unwrap();
                    }
                    perform(&mut scene, &[6, 6, 6]);
                }
                for kind in KINDS {
                    let receipt = scene.provider_delivery_receipt(kind).unwrap().clone();
                    let delivery = &receipt.delivery;
                    let before = format!("{:?}", scene.core);
                    let ack = scene
                        .core
                        .schedule_managed_identified(
                            kind.input(),
                            delivery.from,
                            delivery.to,
                            delivery.record,
                            delivery.due,
                            receipt.mode,
                        )
                        .unwrap();
                    assert!(ack.duplicate);
                    assert_eq!(format!("{:?}", scene.core), before);
                    assert_eq!(
                        scene.core.schedule_managed_identified(
                            kind.input(),
                            delivery.from,
                            delivery.to,
                            delivery.record,
                            delivery.due + 1,
                            receipt.mode
                        ),
                        Err(DeliveryError::Input(InputError::Conflict))
                    );
                    assert_eq!(format!("{:?}", scene.core), before);
                }
            }
            let report = scene.report.unwrap();
            let before = format!("{:?}", scene.core);
            assert_eq!(
                scene.core.schedule_managed_identified(
                    InputId(4),
                    scene.observer(),
                    PROVIDER,
                    report,
                    scene.core.time(),
                    DeliveryMode::AbstractReceipt
                ),
                Err(DeliveryError::Input(InputError::CapacityReached))
            );
            assert_eq!(format!("{:?}", scene.core), before);
            assert!(scene.core.trace().len() <= trace as usize);
            assert!(scene.core.dropped_trace() > 0);
        }
    }
}

#[test]
fn retiring_before_scheduling_does_not_block_player_actions_or_create_deliveries() {
    for retired_before in 0..3 {
        let mut scene = configured(128, "trusted", "restrict_until_correction", [3, 3, 1]);
        let mut control = configured(128, "trusted", "restrict_until_correction", [3, 3, 1]);
        for (index, action) in [1, 3, 4].into_iter().enumerate() {
            if index == retired_before {
                scene.set_provider_state(EntityState::Retired).unwrap();
            }
            assert_eq!(scene.enabled(action), control.enabled(action));
            assert_eq!(scene.act(action), control.act(action));
            assert_eq!(scene.dialogue, control.dialogue);
            assert_eq!(scene.journal, control.journal);
            assert_eq!(scene.time(), control.time());
        }
        let records = [
            scene.report.unwrap(),
            scene.release.unwrap(),
            scene.verification.unwrap(),
        ];
        for (index, kind) in KINDS.into_iter().enumerate() {
            assert_eq!(
                scene.provider_delivery_receipt(kind).is_none(),
                index >= retired_before
            );
            if index >= retired_before {
                assert!(!scene.core.trace().iter().any(|entry| matches!(&entry.event,
                    deedwake_core::Event::Scheduled(delivery) if delivery.to == PROVIDER && delivery.record == records[index])));
            }
            assert!(scene.core.record(records[index]).is_some());
            assert!(scene.facts.contains_key(&records[index]));
        }
        perform(&mut scene, &[2, 6, 6, 6]);
        assert!(scene.core.perspective(PROVIDER).records().is_empty());
        assert!(operations(&scene).is_empty());
    }
}

#[test]
fn restart_resets_provider_state_and_receipts_but_keeps_configuration() {
    let mut scene = configured(2, "unverified", "warn_and_continue", [3, 3, 1]);
    perform(&mut scene, &[1, 3, 4]);
    scene.set_provider_state(EntityState::Retired).unwrap();
    perform(&mut scene, &[6, 6, 6]);
    let config = scene.config().unwrap().clone();
    perform(&mut scene, &[0]);
    assert_eq!(scene.config(), Some(&config));
    assert_eq!(scene.provider_state(), Some(loaded()));
    assert_eq!(scene.time(), 0);
    assert!(scene.journal.is_empty());
    assert!(
        KINDS
            .into_iter()
            .all(|kind| scene.provider_delivery_receipt(kind).is_none())
    );
    perform(&mut scene, &[1, 6, 6, 6]);
    assert_eq!(operations(&scene), vec![(3, ServiceOperation::Warn)]);
}
