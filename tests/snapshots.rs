use deedwake_core::{
    ActionId, Availability, Content, Core, CoreSnapshot, DeliveryMode, DeliveryRejection,
    DeliveryState, Entity, EntityState, Error, Event, InputError, InputId, LifecycleError,
    Proposal, RecordId, Residency, SnapshotAppliedInput, SnapshotContent, SnapshotDeliveryState,
    SnapshotEvent, SnapshotLimits, SnapshotStatus, Status,
};

const A: Entity = Entity(1);
const B: Entity = Entity(2);
const C: Entity = Entity(3);
const D: Entity = Entity(4);
const LEGACY: Entity = Entity(5);

fn limits() -> SnapshotLimits {
    SnapshotLimits::default()
}

fn save(core: &Core) -> CoreSnapshot {
    core.snapshot(&limits()).unwrap()
}

fn restore(core: &Core) -> Core {
    Core::from_snapshot(save(core), &limits()).unwrap()
}

fn propose(core: &mut Core, actor: Entity, cause: RecordId) -> ActionId {
    core.evaluate(actor, |_| {
        Some(Proposal {
            request: "repair the reported finding".into(),
            causes: vec![cause],
            rule: "A known account permits a request".into(),
        })
    })
    .unwrap()
    .unwrap()
}

fn visible(core: &Core, entity: Entity) -> Vec<RecordId> {
    core.perspective(entity)
        .records()
        .iter()
        .map(|record| record.id)
        .collect()
}

#[test]
fn full_state_restores_and_continues_through_delivery_and_confirmation() {
    let mut original = Core::with_lifecycle_capacity(128, 8, 3);
    for entity in [B, C, D] {
        original
            .register_entity(entity, Residency::Loaded, Availability::Available)
            .unwrap();
    }
    original
        .set_entity_state(
            C,
            EntityState::Present {
                residency: Residency::Unloaded,
                availability: Availability::Available,
            },
        )
        .unwrap();
    let observation = original
        .observe_identified(InputId(1), A, "private finding".into())
        .unwrap()
        .value;
    original.advance_to(2).unwrap();
    let account = original
        .account_identified(InputId(2), A, observation, "partial account".into())
        .unwrap()
        .value;
    original.schedule(A, B, account, 5).unwrap();
    original
        .schedule_identified(InputId(3), A, LEGACY, account, 5)
        .unwrap();
    for (input, recipient, mode) in [
        (4, B, DeliveryMode::LoadedRecipient),
        (5, C, DeliveryMode::AbstractReceipt),
        (6, D, DeliveryMode::LoadedRecipient),
    ] {
        original
            .schedule_managed_identified(InputId(input), A, recipient, account, 5, mode)
            .unwrap();
    }
    original.set_entity_state(D, EntityState::Retired).unwrap();
    let proposed = propose(&mut original, A, account);
    let accepted = propose(&mut original, A, account);
    original.accept(accepted).unwrap();
    let rejected = propose(&mut original, A, account);
    original.reject(rejected, "host declined".into()).unwrap();
    let confirmed = propose(&mut original, A, account);
    original.accept(confirmed).unwrap();
    let outcome = original
        .confirm_identified(InputId(7), confirmed, "first repair completed".into())
        .unwrap()
        .value;
    let saved_trace = original.trace().clone();
    let mut restored = restore(&original);

    assert_eq!(restored.time(), 2);
    assert_eq!(restored.trace(), &saved_trace);
    assert_eq!(restored.action(proposed).unwrap().status, Status::Proposed);
    assert_eq!(restored.action(accepted).unwrap().status, Status::Accepted);
    assert_eq!(
        restored.action(rejected).unwrap().status,
        Status::Rejected("host declined".into())
    );
    assert_eq!(
        restored.action(confirmed).unwrap().status,
        Status::Confirmed(outcome)
    );
    assert_eq!(visible(&restored, A), vec![observation, account, outcome]);
    for entity in [B, C, D, LEGACY] {
        assert!(visible(&restored, entity).is_empty());
    }
    assert_eq!(restored.entity_state(A), None);
    assert_eq!(restored.entity_state(D), Some(EntityState::Retired));
    assert_eq!(
        restored.delivery_receipt(InputId(4)).unwrap().state,
        DeliveryState::Pending
    );

    for core in [&mut original, &mut restored] {
        core.advance_to(8).unwrap();
        assert_eq!(core.time(), 8);
        for entity in [B, C, LEGACY] {
            assert_eq!(visible(core, entity), vec![account]);
            assert_eq!(
                core.account(entity, observation, "hidden basis".into()),
                Err(Error::NotKnown)
            );
        }
        assert!(visible(core, D).is_empty());
        assert_eq!(
            core.delivery_receipt(InputId(6)).unwrap().state,
            DeliveryState::Rejected {
                at: 5,
                reason: DeliveryRejection::Retired,
            }
        );
        let arrivals: Vec<_> = core
            .trace()
            .iter()
            .filter_map(|entry| match &entry.event {
                Event::Delivered(delivery) => Some((entry.time, delivery.to, delivery.order)),
                _ => None,
            })
            .collect();
        assert_eq!(
            arrivals,
            vec![(5, B, 0), (5, LEGACY, 1), (5, B, 2), (5, C, 3)]
        );
        let action = propose(core, B, account);
        core.accept_guarded(action).unwrap();
        let completion = core
            .confirm_identified(InputId(8), action, "recipient repair completed".into())
            .unwrap();
        assert!(!completion.duplicate);
        assert_eq!(
            core.record(completion.value).unwrap().content,
            Content::ConfirmedOutcome {
                description: "recipient repair completed".into(),
                action,
            }
        );
        assert_eq!(core.record(observation).unwrap().time, 0);
        assert_eq!(core.record(account).unwrap().time, 2);
        assert_eq!(core.record(completion.value).unwrap().time, 8);
        let before = save(core);
        let retried = core
            .observe_identified(InputId(1), A, "private finding".into())
            .unwrap();
        assert!(retried.duplicate);
        assert_eq!(retried.value, observation);
        let retried = core
            .account_identified(InputId(2), A, observation, "partial account".into())
            .unwrap();
        assert!(retried.duplicate);
        assert_eq!(retried.value, account);
        assert!(
            core.schedule_identified(InputId(3), A, LEGACY, account, 5)
                .unwrap()
                .duplicate
        );
        for (input, recipient, mode) in [
            (4, B, DeliveryMode::LoadedRecipient),
            (5, C, DeliveryMode::AbstractReceipt),
            (6, D, DeliveryMode::LoadedRecipient),
        ] {
            assert!(
                core.schedule_managed_identified(InputId(input), A, recipient, account, 5, mode)
                    .unwrap()
                    .duplicate
            );
        }
        for (input, action, description, expected) in [
            (7, confirmed, "first repair completed", outcome),
            (8, action, "recipient repair completed", completion.value),
        ] {
            let retry = core
                .confirm_identified(InputId(input), action, description.into())
                .unwrap();
            assert!(retry.duplicate);
            assert_eq!(retry.value, expected);
        }
        assert_eq!(
            core.observe_identified(InputId(9), A, "new input".into()),
            Err(InputError::CapacityReached)
        );
        assert_eq!(
            core.observe_identified(InputId(1), A, "changed input".into()),
            Err(InputError::Conflict)
        );
        assert_eq!(save(core), before);
    }
    assert_eq!(save(&restored), save(&original));
}

#[test]
fn trace_loss_keeps_terminal_receipts_and_lifecycle_tombstones() {
    for trace_capacity in [0, 1] {
        let mut core = Core::with_lifecycle_capacity(trace_capacity, 3, 2);
        for entity in [B, C] {
            core.register_entity(entity, Residency::Loaded, Availability::Available)
                .unwrap();
        }
        let record = core
            .observe_identified(InputId(1), A, "retained outside trace".into())
            .unwrap()
            .value;
        for (input, to) in [(2, B), (3, C)] {
            core.schedule_managed_identified(
                InputId(input),
                A,
                to,
                record,
                1,
                DeliveryMode::LoadedRecipient,
            )
            .unwrap();
        }
        core.set_entity_state(
            C,
            EntityState::Present {
                residency: Residency::Loaded,
                availability: Availability::Unavailable,
            },
        )
        .unwrap();
        core.advance_to(3).unwrap();
        core.set_entity_state(B, EntityState::Retired).unwrap();
        core.set_entity_state(
            C,
            EntityState::Present {
                residency: Residency::Loaded,
                availability: Availability::Available,
            },
        )
        .unwrap();
        let before = save(&core);
        let mut restored = restore(&core);
        assert_eq!(restored.trace().len(), trace_capacity);
        assert_eq!(restored.dropped_trace(), core.dropped_trace());
        assert!(restored.dropped_trace() > 0);
        assert_eq!(
            restored.delivery_receipt(InputId(2)).unwrap().state,
            DeliveryState::Delivered { at: 1 }
        );
        assert_eq!(
            restored.delivery_receipt(InputId(3)).unwrap().state,
            DeliveryState::Rejected {
                at: 1,
                reason: DeliveryRejection::Unavailable,
            }
        );
        assert_eq!(visible(&restored, B), vec![record]);
        assert!(visible(&restored, C).is_empty());
        assert_eq!(
            restored.register_entity(B, Residency::Loaded, Availability::Available),
            Err(LifecycleError::AlreadyRegistered)
        );
        assert_eq!(
            restored.register_entity(D, Residency::Loaded, Availability::Available),
            Err(LifecycleError::CapacityReached)
        );
        assert_eq!(
            restored.set_entity_state(
                B,
                EntityState::Present {
                    residency: Residency::Loaded,
                    availability: Availability::Available,
                }
            ),
            Err(LifecycleError::Retired)
        );
        for (input, to) in [(2, B), (3, C)] {
            assert!(
                restored
                    .schedule_managed_identified(
                        InputId(input),
                        A,
                        to,
                        record,
                        1,
                        DeliveryMode::LoadedRecipient,
                    )
                    .unwrap()
                    .duplicate
            );
        }
        assert_eq!(save(&restored), before);
        restored.advance_to(9).unwrap();
        assert_eq!(visible(&restored, B), vec![record]);
        assert!(visible(&restored, C).is_empty());
        assert_eq!(restored.trace(), core.trace());
        assert_eq!(restored.dropped_trace(), core.dropped_trace());
    }
}

#[test]
fn every_lifecycle_state_and_managed_terminal_result_survives_restoration() {
    let states = [Residency::Loaded, Residency::Unloaded]
        .into_iter()
        .flat_map(|residency| {
            [
                Availability::Available,
                Availability::Unavailable,
                Availability::Incapable,
            ]
            .map(|availability| EntityState::Present {
                residency,
                availability,
            })
        })
        .chain([EntityState::Retired]);
    for state in states {
        for mode in [DeliveryMode::LoadedRecipient, DeliveryMode::AbstractReceipt] {
            let mut core = Core::with_lifecycle_capacity(0, 1, 1);
            core.register_entity(B, Residency::Loaded, Availability::Available)
                .unwrap();
            let record = core.observe(A, "managed delivery".into());
            core.schedule_managed_identified(InputId(1), A, B, record, 4, mode)
                .unwrap();
            core.set_entity_state(B, state).unwrap();
            let mut restored = restore(&core);
            assert_eq!(restored.entity_state(B), Some(state));
            assert_eq!(restored.time(), 0);
            assert!(visible(&restored, B).is_empty());
            restored.advance_to(9).unwrap();
            let reason = match state {
                EntityState::Retired => Some(DeliveryRejection::Retired),
                EntityState::Present {
                    availability: Availability::Unavailable,
                    ..
                } => Some(DeliveryRejection::Unavailable),
                EntityState::Present {
                    availability: Availability::Incapable,
                    ..
                } => Some(DeliveryRejection::Incapable),
                EntityState::Present {
                    residency: Residency::Unloaded,
                    availability: Availability::Available,
                } if mode == DeliveryMode::LoadedRecipient => Some(DeliveryRejection::Unloaded),
                _ => None,
            };
            let expected = match reason {
                Some(reason) => DeliveryState::Rejected { at: 4, reason },
                None => DeliveryState::Delivered { at: 4 },
            };
            let mut terminal = restore(&restored);
            assert_eq!(terminal.entity_state(B), Some(state));
            assert_eq!(
                terminal.delivery_receipt(InputId(1)).unwrap().state,
                expected
            );
            assert_eq!(terminal.time(), 9);
            assert_eq!(
                visible(&terminal, B),
                if reason.is_none() {
                    vec![record]
                } else {
                    vec![]
                }
            );
            let before = save(&terminal);
            assert!(
                terminal
                    .schedule_managed_identified(InputId(1), A, B, record, 4, mode)
                    .unwrap()
                    .duplicate
            );
            assert_eq!(save(&terminal), before);
            terminal.advance_to(12).unwrap();
            assert_eq!(
                terminal.delivery_receipt(InputId(1)).unwrap().state,
                expected
            );
        }
    }
}

#[test]
fn due_now_queues_wait_for_the_host_and_preserve_equal_time_order() {
    let mut core = Core::with_lifecycle_capacity(32, 1, 1);
    core.register_entity(B, Residency::Loaded, Availability::Available)
        .unwrap();
    core.advance_to(7).unwrap();
    let first = core.observe(A, "first".into());
    let second = core.observe(A, "second".into());
    let third = core.observe(A, "third".into());
    core.schedule(A, B, first, 7).unwrap();
    core.schedule_managed_identified(InputId(1), A, B, second, 7, DeliveryMode::LoadedRecipient)
        .unwrap();
    core.schedule(A, B, third, 7).unwrap();
    let mut restored = restore(&core);
    assert_eq!(restored.time(), 7);
    assert!(visible(&restored, B).is_empty());
    assert_eq!(
        restored.delivery_receipt(InputId(1)).unwrap().state,
        DeliveryState::Pending
    );
    let before = save(&restored);
    assert_eq!(restored.advance_to(6), Err(Error::PastTime));
    assert_eq!(save(&restored), before);
    restored.advance_to(7).unwrap();
    assert_eq!(visible(&restored, B), vec![first, second, third]);
    let delivered: Vec<_> = restored
        .trace()
        .iter()
        .filter_map(|entry| match &entry.event {
            Event::Delivered(delivery) => Some((entry.time, delivery.record, delivery.order)),
            _ => None,
        })
        .collect();
    assert_eq!(
        delivered,
        vec![(7, first, 0), (7, second, 1), (7, third, 2)]
    );
    restored.schedule(A, C, first, 7).unwrap();
    restored.advance_to(7).unwrap();
    assert!(matches!(
        &restored.trace().back().unwrap().event,
        Event::Delivered(delivery) if delivery.order == 3 && delivery.to == C
    ));
    let before = save(&restored);
    restored.advance_to(7).unwrap();
    assert_eq!(save(&restored), before);
}

#[test]
fn unregistered_legacy_actors_and_empty_causes_remain_valid() {
    let mut core = Core::new(32);
    let cause = core.observe(A, "legacy finding".into());
    let action = core
        .evaluate(A, |_| {
            Some(Proposal {
                request: "host supplied request".into(),
                causes: vec![],
                rule: "Host authorization".into(),
            })
        })
        .unwrap()
        .unwrap();
    core.accept(action).unwrap();
    let mut restored = restore(&core);
    assert_eq!(restored.entity_state(A), None);
    assert_eq!(visible(&restored, A), vec![cause]);
    assert_eq!(restored.action(action).unwrap().status, Status::Accepted);
    assert!(restored.action(action).unwrap().proposal.causes.is_empty());
    restored.schedule(A, LEGACY, cause, 0).unwrap();
    restored.advance_to(0).unwrap();
    assert_eq!(visible(&restored, LEGACY), vec![cause]);
    let outcome = restored.confirm(action, "host executed".into()).unwrap();
    assert_eq!(
        restored.action(action).unwrap().status,
        Status::Confirmed(outcome)
    );
    assert_eq!(restored.entity_state(LEGACY), None);
    assert_eq!(
        restored.observe_identified(InputId(1), A, "disabled ledger".into()),
        Err(InputError::CapacityReached)
    );
    assert_eq!(
        restored.register_entity(A, Residency::Loaded, Availability::Available),
        Err(LifecycleError::CapacityReached)
    );
}

fn validation_fixture() -> Core {
    let mut core = Core::with_lifecycle_capacity(32, 2, 1);
    core.register_entity(B, Residency::Loaded, Availability::Available)
        .unwrap();
    let observation = core
        .observe_identified(InputId(1), A, "private finding".into())
        .unwrap()
        .value;
    let account = core.account(A, observation, "account".into()).unwrap();
    core.schedule_managed_identified(InputId(2), A, B, account, 3, DeliveryMode::LoadedRecipient)
        .unwrap();
    let action = propose(&mut core, A, account);
    core.accept(action).unwrap();
    core.confirm(action, "completed".into()).unwrap();
    core.advance_to(1).unwrap();
    core
}

#[track_caller]
fn reject_change(snapshot: &CoreSnapshot, change: impl FnOnce(&mut CoreSnapshot)) {
    let mut corrupted = snapshot.clone();
    change(&mut corrupted);
    assert!(Core::from_snapshot(corrupted, &limits()).is_err());
}

#[test]
fn malformed_references_duplicates_and_counters_are_rejected_without_affecting_the_source() {
    let core = validation_fixture();
    let valid = save(&core);
    reject_change(&valid, |s| s.version += 1);
    reject_change(&valid, |s| s.records[0].id = 99);
    reject_change(&valid, |s| s.records[0].time = s.time + 1);
    reject_change(&valid, |s| s.records[0].source = B.0);
    reject_change(&valid, |s| s.known[0].records.push(u64::MAX));
    reject_change(&valid, |s| {
        let duplicate = s.known[0].records[0];
        s.known[0].records.push(duplicate);
    });
    reject_change(&valid, |s| s.known.push(s.known[0].clone()));
    reject_change(&valid, |s| s.known.clear());
    reject_change(&valid, |s| s.pending[0].delivery.record = u64::MAX);
    reject_change(&valid, |s| s.pending[0].delivery.from = B.0);
    reject_change(&valid, |s| s.pending[0].delivery.due = 0);
    reject_change(&valid, |s| s.pending[0].managed_input = Some(999));
    reject_change(&valid, |s| s.pending[0].managed_input = None);
    reject_change(&valid, |s| s.pending.clear());
    reject_change(&valid, |s| s.pending.push(s.pending[0].clone()));
    reject_change(&valid, |s| s.next_delivery = 0);
    reject_change(&valid, |s| s.inputs.push(s.inputs[0].clone()));
    reject_change(&valid, |s| s.input_capacity = 1);
    reject_change(&valid, |s| s.entities.push(s.entities[0].clone()));
    reject_change(&valid, |s| s.entities.clear());
    reject_change(&valid, |s| s.entity_capacity = 0);
    reject_change(&valid, |s| s.actions[0].actor = B.0);
    reject_change(&valid, |s| s.actions[0].proposal.causes = vec![u64::MAX]);
    reject_change(&valid, |s| s.actions[0].proposal.rule = " \n ".into());
    reject_change(&valid, |s| {
        s.records[1].content = SnapshotContent::Account {
            summary: "account".into(),
            basis: 1,
        };
    });
    reject_change(&valid, |s| {
        s.records[2].content = SnapshotContent::ConfirmedOutcome {
            description: "completed".into(),
            action: u64::MAX,
        };
    });
    reject_change(&valid, |s| {
        s.actions[0].status = SnapshotStatus::Confirmed(0)
    });
    reject_change(&valid, |s| s.actions[0].status = SnapshotStatus::Accepted);
    reject_change(&valid, |s| {
        s.inputs[0].applied = SnapshotAppliedInput::Record(u64::MAX)
    });
    reject_change(&valid, |s| {
        let SnapshotAppliedInput::ManagedSchedule(receipt) = &mut s.inputs[1].applied else {
            panic!("fixture must contain a managed receipt");
        };
        receipt.delivery.to = C.0;
    });
    reject_change(&valid, |s| {
        let SnapshotAppliedInput::ManagedSchedule(receipt) = &mut s.inputs[1].applied else {
            panic!("fixture must contain a managed receipt");
        };
        receipt.state = SnapshotDeliveryState::Delivered { at: 3 };
    });
    reject_change(&valid, |s| s.trace_capacity = 0);
    reject_change(&valid, |s| s.trace[0].time = s.time + 1);
    reject_change(&valid, |s| s.trace[0].causes.push(u64::MAX));
    reject_change(&valid, |s| {
        s.trace[0].event = SnapshotEvent::Recorded(u64::MAX)
    });
    assert_eq!(save(&core), valid);
    let mut restored = Core::from_snapshot(valid, &limits()).unwrap();
    restored.advance_to(3).unwrap();
    assert_eq!(visible(&restored, B).len(), 1);
    assert_eq!(
        restored.delivery_receipt(InputId(2)).unwrap().state,
        DeliveryState::Delivered { at: 3 }
    );
}

fn rejects_limit(core: &Core, change: impl FnOnce(&mut SnapshotLimits)) {
    let snapshot = save(core);
    let mut bounded = limits();
    change(&mut bounded);
    assert!(core.snapshot(&bounded).is_err());
    assert!(Core::from_snapshot(snapshot.clone(), &bounded).is_err());
    assert_eq!(save(core), snapshot);
}

#[test]
fn import_and_export_enforce_host_limits_including_saved_capacities_and_strings() {
    let core = validation_fixture();
    rejects_limit(&core, |l| l.max_records = 2);
    rejects_limit(&core, |l| l.max_pending = 0);
    rejects_limit(&core, |l| l.max_inputs = 1);
    rejects_limit(&core, |l| l.max_actions = 0);
    rejects_limit(&core, |l| l.max_entities = 0);
    rejects_limit(&core, |l| l.max_trace_capacity = 31);
    rejects_limit(&core, |l| l.max_known_owners = 0);
    rejects_limit(&core, |l| l.max_known_edges = 2);
    rejects_limit(&core, |l| l.max_string_bytes = 0);
    rejects_limit(&core, |l| l.max_total_string_bytes = 0);

    let mut bounded = limits();
    bounded.max_records = 0;
    bounded.max_pending = 0;
    bounded.max_inputs = 0;
    bounded.max_actions = 0;
    bounded.max_entities = 0;
    bounded.max_trace_capacity = 0;
    bounded.max_known_owners = 0;
    bounded.max_known_edges = 0;
    bounded.max_string_bytes = 0;
    bounded.max_total_string_bytes = 0;
    let empty = Core::new(0).snapshot(&bounded).unwrap();
    assert_eq!(Core::from_snapshot(empty, &bounded).unwrap().time(), 0);

    let mut unicode = Core::new(0);
    let record = unicode.observe(A, "éé".into());
    bounded.max_records = 1;
    bounded.max_known_owners = 1;
    bounded.max_known_edges = 1;
    bounded.max_string_bytes = 4;
    bounded.max_total_string_bytes = 4;
    let snapshot = unicode.snapshot(&bounded).unwrap();
    let restored = Core::from_snapshot(snapshot.clone(), &bounded).unwrap();
    assert_eq!(
        restored.record(record).unwrap().content,
        Content::Observation("éé".into())
    );
    bounded.max_string_bytes = 3;
    assert!(unicode.snapshot(&bounded).is_err());
    assert!(Core::from_snapshot(snapshot.clone(), &bounded).is_err());
    bounded.max_string_bytes = 4;
    bounded.max_total_string_bytes = 3;
    assert!(unicode.snapshot(&bounded).is_err());
    assert!(Core::from_snapshot(snapshot, &bounded).is_err());
}

#[test]
fn exhausted_delivery_counter_remains_exhausted_after_restoration() {
    let mut core = Core::new(0);
    let record = core.observe(A, "counter boundary".into());
    let mut snapshot = save(&core);
    snapshot.next_delivery = u64::try_from(usize::MAX).unwrap();
    snapshot.dropped_trace = u64::MAX;
    let mut restored = Core::from_snapshot(snapshot, &limits()).unwrap();
    let before = save(&restored);
    assert_eq!(
        restored.schedule(A, B, record, 0),
        Err(Error::SequenceExhausted)
    );
    assert_eq!(save(&restored), before);
    assert_eq!(visible(&restored, A), vec![record]);
    assert!(visible(&restored, B).is_empty());
    restored.observe(A, "trace counter stays saturated".into());
    assert_eq!(restored.dropped_trace(), u64::MAX);
}
