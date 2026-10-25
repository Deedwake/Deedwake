use deedwake_core::{
    Availability, Content, Core, DeliveryError, DeliveryMode, DeliveryRejection, DeliveryState,
    Entity, EntityState, Error, Event, InputError, InputId, Proposal, Residency,
};

const A: Entity = Entity(1);
const B: Entity = Entity(2);
const C: Entity = Entity(3);
const D: Entity = Entity(4);

fn present(residency: Residency, availability: Availability) -> EntityState {
    EntityState::Present {
        residency,
        availability,
    }
}

#[test]
fn pending_account_arrives_at_its_due_time_without_exposing_or_mutating_its_basis() {
    let mut core = Core::with_lifecycle_capacity(32, 1, 1);
    core.register_entity(B, Residency::Loaded, Availability::Available)
        .unwrap();
    let basis = core.observe(A, "private inspection".into());
    core.advance_to(2).unwrap();
    let account = core.account(A, basis, "partial account".into()).unwrap();
    let saved_record = core.record(account).unwrap().clone();
    let accepted = core
        .schedule_managed_identified(InputId(1), A, B, account, 5, DeliveryMode::LoadedRecipient)
        .unwrap();
    assert_eq!(accepted.value, ());
    assert!(!accepted.duplicate);
    let pending = core.delivery_receipt(InputId(1)).unwrap().clone();
    assert_eq!(pending.state, DeliveryState::Pending);
    assert_eq!(pending.mode, DeliveryMode::LoadedRecipient);
    assert_eq!(pending.delivery.record, account);
    assert_eq!(pending.delivery.from, A);
    assert_eq!(pending.delivery.to, B);
    assert_eq!(pending.delivery.due, 5);
    assert_eq!(pending.delivery.order, 0);
    assert!(core.perspective(B).records().is_empty());
    core.account(A, basis, "later correction".into()).unwrap();
    core.advance_to(4).unwrap();
    assert_eq!(core.delivery_receipt(InputId(1)), Some(&pending));
    assert!(core.perspective(B).records().is_empty());
    core.advance_to(9).unwrap();
    assert_eq!(core.time(), 9);
    assert_eq!(
        core.delivery_receipt(InputId(1)).unwrap().state,
        DeliveryState::Delivered { at: 5 }
    );
    assert_eq!(core.record(account), Some(&saved_record));
    assert_eq!(saved_record.time, 2);
    assert_eq!(
        saved_record.content,
        Content::Account {
            summary: "partial account".into(),
            basis,
        }
    );
    assert_eq!(core.perspective(B).records(), &[&saved_record]);
    assert_eq!(
        core.account(B, basis, "hidden basis".into()),
        Err(Error::NotKnown)
    );
    let arrivals: Vec<_> = core
        .trace()
        .iter()
        .filter(|entry| matches!(entry.event, Event::Delivered(_)))
        .collect();
    assert_eq!(arrivals.len(), 1);
    assert_eq!(arrivals[0].time, 5);
    assert_eq!(arrivals[0].event, Event::Delivered(pending.delivery));
    assert_eq!(arrivals[0].causes, vec![account]);

    let terminal = core.delivery_receipt(InputId(1)).unwrap().clone();
    core.set_entity_state(B, EntityState::Retired).unwrap();
    let before = core.trace().clone();
    let dropped = core.dropped_trace();
    assert!(
        core.schedule_managed_identified(
            InputId(1),
            A,
            B,
            account,
            5,
            DeliveryMode::LoadedRecipient
        )
        .unwrap()
        .duplicate
    );
    core.advance_to(10).unwrap();
    assert_eq!(core.delivery_receipt(InputId(1)), Some(&terminal));
    assert_eq!(core.trace(), &before);
    assert_eq!(core.dropped_trace(), dropped);
    assert_eq!(core.perspective(B).records(), &[&saved_record]);
}

#[test]
fn arrival_uses_the_current_recipient_state_for_every_mode_and_admission_state() {
    let present_states = [
        present(Residency::Loaded, Availability::Available),
        present(Residency::Unloaded, Availability::Available),
        present(Residency::Loaded, Availability::Unavailable),
        present(Residency::Unloaded, Availability::Unavailable),
        present(Residency::Loaded, Availability::Incapable),
        present(Residency::Unloaded, Availability::Incapable),
    ];
    for admission in present_states {
        for arrival in present_states.into_iter().chain([EntityState::Retired]) {
            for mode in [DeliveryMode::LoadedRecipient, DeliveryMode::AbstractReceipt] {
                let mut core = Core::with_lifecycle_capacity(16, 1, 1);
                core.register_entity(B, Residency::Loaded, Availability::Available)
                    .unwrap();
                core.set_entity_state(B, admission).unwrap();
                let record = core.observe(A, "account to deliver".into());
                core.schedule_managed_identified(InputId(1), A, B, record, 3, mode)
                    .unwrap();
                assert_eq!(
                    core.delivery_receipt(InputId(1)).unwrap().state,
                    DeliveryState::Pending
                );
                core.set_entity_state(B, arrival).unwrap();
                let before = core.trace().clone();
                let rejection = match arrival {
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
                        ..
                    } if mode == DeliveryMode::LoadedRecipient => Some(DeliveryRejection::Unloaded),
                    _ => None,
                };
                core.advance_to(8).unwrap();
                assert_eq!(core.time(), 8);
                assert_eq!(core.entity_state(B), Some(arrival));
                assert_eq!(core.perspective(A).records().len(), 1);
                let receipt = core.delivery_receipt(InputId(1)).unwrap();
                assert_eq!(receipt.mode, mode);
                assert_eq!(receipt.delivery.due, 3);
                if let Some(reason) = rejection {
                    assert_eq!(receipt.state, DeliveryState::Rejected { at: 3, reason });
                    assert!(core.perspective(B).records().is_empty());
                    assert_eq!(core.trace(), &before);
                } else {
                    assert_eq!(receipt.state, DeliveryState::Delivered { at: 3 });
                    assert_eq!(
                        core.perspective(B).records(),
                        &[core.record(record).unwrap()]
                    );
                    assert_eq!(core.trace().len(), before.len() + 1);
                    assert_eq!(core.trace().back().unwrap().time, 3);
                    assert_eq!(
                        core.trace().back().unwrap().event,
                        Event::Delivered(receipt.delivery.clone())
                    );
                }
            }
        }
    }
}

#[test]
fn sender_need_not_be_registered_and_retirement_after_scheduling_does_not_cancel() {
    for registered_sender in [false, true] {
        let mut core = Core::with_lifecycle_capacity(16, 1, 2);
        core.register_entity(B, Residency::Loaded, Availability::Available)
            .unwrap();
        if registered_sender {
            core.register_entity(A, Residency::Loaded, Availability::Available)
                .unwrap();
        }
        let record = core.observe(A, "sender account".into());
        core.schedule_managed_identified(
            InputId(1),
            A,
            B,
            record,
            2,
            DeliveryMode::LoadedRecipient,
        )
        .unwrap();
        if registered_sender {
            core.set_entity_state(A, EntityState::Retired).unwrap();
        } else {
            assert_eq!(core.entity_state(A), None);
        }
        core.advance_to(2).unwrap();
        assert_eq!(
            core.delivery_receipt(InputId(1)).unwrap().state,
            DeliveryState::Delivered { at: 2 }
        );
        assert_eq!(
            core.perspective(B).records(),
            &[core.record(record).unwrap()]
        );
    }
}

#[test]
fn rejecting_an_entry_does_not_block_equal_time_legacy_or_managed_entries() {
    let mut core = Core::with_lifecycle_capacity(32, 2, 2);
    for entity in [B, C] {
        core.register_entity(entity, Residency::Loaded, Availability::Available)
            .unwrap();
    }
    let rejected = core.observe(A, "rejected".into());
    let legacy = core.observe(A, "legacy retired recipient".into());
    let managed = core.observe(A, "managed available recipient".into());
    let unknown = core.observe(A, "legacy unknown recipient".into());
    core.schedule_managed_identified(InputId(1), A, B, rejected, 5, DeliveryMode::LoadedRecipient)
        .unwrap();
    let before = core.trace().clone();
    assert!(
        core.schedule_managed_identified(
            InputId(1),
            A,
            B,
            rejected,
            5,
            DeliveryMode::LoadedRecipient
        )
        .unwrap()
        .duplicate
    );
    assert_eq!(core.trace(), &before);
    core.set_entity_state(B, EntityState::Retired).unwrap();
    core.schedule(A, B, legacy, 5).unwrap();
    core.schedule_managed_identified(InputId(2), A, C, managed, 5, DeliveryMode::AbstractReceipt)
        .unwrap();
    core.schedule(A, D, unknown, 5).unwrap();
    let scheduled: Vec<_> = core
        .trace()
        .iter()
        .filter_map(|entry| match &entry.event {
            Event::Scheduled(delivery) => Some(delivery.order),
            _ => None,
        })
        .collect();
    assert_eq!(scheduled, vec![0, 1, 2, 3]);
    core.advance_to(9).unwrap();
    let delivered: Vec<_> = core
        .trace()
        .iter()
        .filter_map(|entry| match &entry.event {
            Event::Delivered(delivery) => Some((entry.time, delivery.record, delivery.order)),
            _ => None,
        })
        .collect();
    assert_eq!(
        delivered,
        vec![(5, legacy, 1), (5, managed, 2), (5, unknown, 3)]
    );
    assert_eq!(
        core.delivery_receipt(InputId(1)).unwrap().state,
        DeliveryState::Rejected {
            at: 5,
            reason: DeliveryRejection::Retired,
        }
    );
    assert_eq!(
        core.delivery_receipt(InputId(2)).unwrap().state,
        DeliveryState::Delivered { at: 5 }
    );
    assert_eq!(
        core.perspective(B).records(),
        &[core.record(legacy).unwrap()]
    );
    assert_eq!(
        core.perspective(C).records(),
        &[core.record(managed).unwrap()]
    );
    assert_eq!(
        core.perspective(D).records(),
        &[core.record(unknown).unwrap()]
    );
    assert_eq!(core.entity_state(D), None);
}

#[test]
fn rejected_receipts_stay_terminal_after_recovery_and_a_new_id_can_resend() {
    let mut core = Core::with_lifecycle_capacity(16, 2, 1);
    core.register_entity(B, Residency::Loaded, Availability::Unavailable)
        .unwrap();
    let record = core.observe(A, "recoverable recipient".into());
    core.schedule_managed_identified(InputId(1), A, B, record, 3, DeliveryMode::AbstractReceipt)
        .unwrap();
    core.advance_to(8).unwrap();
    let rejected = core.delivery_receipt(InputId(1)).unwrap().clone();
    assert_eq!(
        rejected.state,
        DeliveryState::Rejected {
            at: 3,
            reason: DeliveryRejection::Unavailable,
        }
    );
    core.set_entity_state(B, present(Residency::Loaded, Availability::Available))
        .unwrap();
    let before = core.trace().clone();
    let dropped = core.dropped_trace();
    let retry = core
        .schedule_managed_identified(InputId(1), A, B, record, 3, DeliveryMode::AbstractReceipt)
        .unwrap();
    assert_eq!(retry.value, ());
    assert!(retry.duplicate);
    assert_eq!(core.time(), 8);
    core.advance_to(9).unwrap();
    assert_eq!(core.trace(), &before);
    assert_eq!(core.dropped_trace(), dropped);
    assert_eq!(core.delivery_receipt(InputId(1)), Some(&rejected));
    assert!(core.perspective(B).records().is_empty());
    assert!(
        !core
            .schedule_managed_identified(
                InputId(2),
                A,
                B,
                record,
                10,
                DeliveryMode::AbstractReceipt
            )
            .unwrap()
            .duplicate
    );
    assert_eq!(core.delivery_receipt(InputId(2)).unwrap().delivery.order, 1);
    core.advance_to(12).unwrap();
    assert_eq!(core.delivery_receipt(InputId(1)), Some(&rejected));
    assert_eq!(
        core.delivery_receipt(InputId(2)).unwrap().state,
        DeliveryState::Delivered { at: 10 }
    );
    assert_eq!(
        core.perspective(B).records(),
        &[core.record(record).unwrap()]
    );
}

#[test]
fn accepted_ids_conflict_on_every_field_mode_and_operation_before_other_validation() {
    let mut core = Core::with_lifecycle_capacity(32, 2, 1);
    core.register_entity(B, Residency::Loaded, Availability::Available)
        .unwrap();
    let record = core.observe(A, "original".into());
    let other = core.observe(A, "other".into());
    core.schedule_managed_identified(InputId(1), A, B, record, 3, DeliveryMode::LoadedRecipient)
        .unwrap();
    core.schedule_identified(InputId(2), A, B, record, 3)
        .unwrap();
    let action = core
        .evaluate(A, |_| {
            Some(Proposal {
                request: "host request".into(),
                causes: vec![record],
                rule: "known record".into(),
            })
        })
        .unwrap()
        .unwrap();
    core.advance_to(4).unwrap();
    core.set_entity_state(B, EntityState::Retired).unwrap();
    let before = core.trace().clone();
    let receipt = core.delivery_receipt(InputId(1)).unwrap().clone();
    for (from, to, content, due, mode) in [
        (C, B, record, 3, DeliveryMode::LoadedRecipient),
        (A, C, record, 3, DeliveryMode::LoadedRecipient),
        (A, B, other, 3, DeliveryMode::LoadedRecipient),
        (A, B, record, 2, DeliveryMode::LoadedRecipient),
        (A, B, record, 3, DeliveryMode::AbstractReceipt),
    ] {
        assert_eq!(
            core.schedule_managed_identified(InputId(1), from, to, content, due, mode),
            Err(DeliveryError::Input(InputError::Conflict))
        );
    }
    assert_eq!(
        core.schedule_identified(InputId(1), A, B, record, 3),
        Err(InputError::Conflict)
    );
    assert_eq!(
        core.observe_identified(InputId(1), A, "original".into()),
        Err(InputError::Conflict)
    );
    assert_eq!(
        core.account_identified(InputId(1), A, record, "account".into()),
        Err(InputError::Conflict)
    );
    assert_eq!(
        core.confirm_identified(InputId(1), action, "outcome".into()),
        Err(InputError::Conflict)
    );
    assert_eq!(
        core.schedule_managed_identified(
            InputId(2),
            A,
            B,
            record,
            3,
            DeliveryMode::LoadedRecipient
        ),
        Err(DeliveryError::Input(InputError::Conflict))
    );
    assert_eq!(core.delivery_receipt(InputId(2)), None);
    assert_eq!(core.delivery_receipt(InputId(1)), Some(&receipt));
    assert_eq!(core.trace(), &before);
    assert_eq!(core.time(), 4);
}

#[test]
fn identified_operations_share_capacity_and_terminal_receipts_keep_their_slots() {
    for rejected in [false, true] {
        let mut core = Core::with_lifecycle_capacity(32, 5, 1);
        core.register_entity(B, Residency::Loaded, Availability::Available)
            .unwrap();
        let basis = core
            .observe_identified(InputId(1), A, "basis".into())
            .unwrap()
            .value;
        let account = core
            .account_identified(InputId(2), A, basis, "account".into())
            .unwrap()
            .value;
        core.schedule_identified(InputId(3), A, C, account, 1)
            .unwrap();
        let action = core
            .evaluate(A, |_| {
                Some(Proposal {
                    request: "host request".into(),
                    causes: vec![basis],
                    rule: "known basis".into(),
                })
            })
            .unwrap()
            .unwrap();
        core.accept(action).unwrap();
        core.confirm_identified(InputId(4), action, "completed".into())
            .unwrap();
        core.schedule_managed_identified(
            InputId(5),
            A,
            B,
            account,
            1,
            DeliveryMode::LoadedRecipient,
        )
        .unwrap();
        if rejected {
            core.set_entity_state(B, EntityState::Retired).unwrap();
        }
        core.advance_to(2).unwrap();
        let terminal = core.delivery_receipt(InputId(5)).unwrap().clone();
        assert_eq!(
            terminal.state,
            if rejected {
                DeliveryState::Rejected {
                    at: 1,
                    reason: DeliveryRejection::Retired,
                }
            } else {
                DeliveryState::Delivered { at: 1 }
            }
        );
        for input in 1..=4 {
            assert_eq!(core.delivery_receipt(InputId(input)), None);
        }
        assert_eq!(core.delivery_receipt(InputId(99)), None);
        let before = core.trace().clone();
        assert_eq!(
            core.schedule_managed_identified(
                InputId(6),
                A,
                D,
                account,
                0,
                DeliveryMode::AbstractReceipt
            ),
            Err(DeliveryError::Input(InputError::CapacityReached))
        );
        assert_eq!(
            core.observe_identified(InputId(6), A, "overflow".into()),
            Err(InputError::CapacityReached)
        );
        assert!(
            core.schedule_managed_identified(
                InputId(5),
                A,
                B,
                account,
                1,
                DeliveryMode::LoadedRecipient
            )
            .unwrap()
            .duplicate
        );
        assert_eq!(core.delivery_receipt(InputId(5)), Some(&terminal));
        assert_eq!(core.trace(), &before);
    }
    let mut zero = Core::with_lifecycle_capacity(8, 0, 1);
    zero.register_entity(B, Residency::Loaded, Availability::Available)
        .unwrap();
    let record = zero.observe(A, "ordinary input".into());
    let before = zero.trace().clone();
    assert_eq!(
        zero.schedule_managed_identified(
            InputId(1),
            A,
            B,
            record,
            1,
            DeliveryMode::LoadedRecipient
        ),
        Err(DeliveryError::Input(InputError::CapacityReached))
    );
    assert_eq!(zero.delivery_receipt(InputId(1)), None);
    assert_eq!(zero.trace(), &before);
    zero.schedule(A, B, record, 1).unwrap();
    zero.advance_to(1).unwrap();
    assert_eq!(
        zero.perspective(B).records(),
        &[zero.record(record).unwrap()]
    );
}

#[test]
fn admission_failures_are_atomic_and_leave_the_id_and_sequence_available() {
    let mut core = Core::with_lifecycle_capacity(32, 4, 3);
    core.register_entity(B, Residency::Loaded, Availability::Available)
        .unwrap();
    core.register_entity(C, Residency::Loaded, Availability::Available)
        .unwrap();
    core.set_entity_state(C, EntityState::Retired).unwrap();
    core.advance_to(3).unwrap();
    let record = core.observe(A, "known only by sender".into());
    for (input, from, to, due, expected) in [
        (InputId(1), A, D, 5, DeliveryError::UnknownRecipient),
        (InputId(2), A, C, 5, DeliveryError::RecipientRetired),
        (InputId(3), D, B, 5, DeliveryError::Core(Error::NotKnown)),
        (InputId(4), A, B, 2, DeliveryError::Core(Error::PastTime)),
    ] {
        let before = core.trace().clone();
        let dropped = core.dropped_trace();
        assert_eq!(
            core.schedule_managed_identified(
                input,
                from,
                to,
                record,
                due,
                DeliveryMode::LoadedRecipient
            ),
            Err(expected)
        );
        assert_eq!(core.trace(), &before);
        assert_eq!(core.dropped_trace(), dropped);
        assert_eq!(core.time(), 3);
        assert_eq!(core.delivery_receipt(input), None);
        assert!(core.perspective(B).records().is_empty());
        assert!(core.perspective(C).records().is_empty());
        assert!(core.perspective(D).records().is_empty());
        let recipient = if input == InputId(1) {
            core.register_entity(D, Residency::Loaded, Availability::Available)
                .unwrap();
            D
        } else {
            B
        };
        let accepted = core
            .schedule_managed_identified(
                input,
                A,
                recipient,
                record,
                5,
                DeliveryMode::LoadedRecipient,
            )
            .unwrap();
        assert!(!accepted.duplicate);
        assert_eq!(
            core.delivery_receipt(input).unwrap().delivery.order,
            (input.0 - 1) as usize
        );
    }
    core.advance_to(8).unwrap();
    for input in 1..=4 {
        assert_eq!(
            core.delivery_receipt(InputId(input)).unwrap().state,
            DeliveryState::Delivered { at: 5 }
        );
    }
    assert_eq!(
        core.perspective(B).records(),
        &[core.record(record).unwrap()]
    );
    assert_eq!(
        core.perspective(D).records(),
        &[core.record(record).unwrap()]
    );
    assert!(core.perspective(C).records().is_empty());
}

#[test]
fn terminal_receipts_survive_disabled_and_evicted_trace_without_retry_side_effects() {
    for trace_capacity in [0, 1] {
        let mut core = Core::with_lifecycle_capacity(trace_capacity, 2, 2);
        core.register_entity(B, Residency::Loaded, Availability::Unavailable)
            .unwrap();
        core.register_entity(C, Residency::Loaded, Availability::Available)
            .unwrap();
        let record = core.observe(A, "retained receipt".into());
        for (input, to) in [(InputId(1), B), (InputId(2), C)] {
            core.schedule_managed_identified(
                input,
                A,
                to,
                record,
                1,
                DeliveryMode::LoadedRecipient,
            )
            .unwrap();
        }
        core.advance_to(4).unwrap();
        let rejected = core.delivery_receipt(InputId(1)).unwrap().clone();
        let delivered = core.delivery_receipt(InputId(2)).unwrap().clone();
        assert_eq!(
            rejected.state,
            DeliveryState::Rejected {
                at: 1,
                reason: DeliveryRejection::Unavailable
            }
        );
        assert_eq!(delivered.state, DeliveryState::Delivered { at: 1 });
        core.observe(A, "evict earlier events".into());
        let before = core.trace().clone();
        let dropped = core.dropped_trace();
        assert_eq!(before.len(), trace_capacity);
        assert!(dropped > 0);
        for (input, to) in [(InputId(1), B), (InputId(2), C)] {
            assert!(
                core.schedule_managed_identified(
                    input,
                    A,
                    to,
                    record,
                    1,
                    DeliveryMode::LoadedRecipient
                )
                .unwrap()
                .duplicate
            );
        }
        core.advance_to(5).unwrap();
        assert_eq!(core.trace(), &before);
        assert_eq!(core.dropped_trace(), dropped);
        assert_eq!(core.delivery_receipt(InputId(1)), Some(&rejected));
        assert_eq!(core.delivery_receipt(InputId(2)), Some(&delivered));
        assert!(core.perspective(B).records().is_empty());
        assert_eq!(
            core.perspective(C).records(),
            &[core.record(record).unwrap()]
        );
    }
}

#[test]
fn a_delivery_due_now_stays_pending_until_ordinary_advance_to_the_current_time() {
    let mut core = Core::with_lifecycle_capacity(16, 1, 1);
    core.register_entity(B, Residency::Loaded, Availability::Available)
        .unwrap();
    core.advance_to(7).unwrap();
    let record = core.observe(A, "same time".into());
    core.schedule_managed_identified(InputId(1), A, B, record, 7, DeliveryMode::LoadedRecipient)
        .unwrap();
    let pending = core.delivery_receipt(InputId(1)).unwrap().clone();
    let before = core.trace().clone();
    assert_eq!(pending.state, DeliveryState::Pending);
    assert!(core.perspective(B).records().is_empty());
    assert!(
        core.schedule_managed_identified(
            InputId(1),
            A,
            B,
            record,
            7,
            DeliveryMode::LoadedRecipient
        )
        .unwrap()
        .duplicate
    );
    assert_eq!(core.trace(), &before);
    assert_eq!(core.advance_to(6), Err(Error::PastTime));
    assert_eq!(core.delivery_receipt(InputId(1)), Some(&pending));
    assert_eq!(core.trace(), &before);
    core.advance_to(7).unwrap();
    assert_eq!(core.time(), 7);
    assert_eq!(
        core.delivery_receipt(InputId(1)).unwrap().state,
        DeliveryState::Delivered { at: 7 }
    );
    assert_eq!(
        core.perspective(B).records(),
        &[core.record(record).unwrap()]
    );
    let terminal = core.trace().clone();
    core.advance_to(7).unwrap();
    assert_eq!(core.trace(), &terminal);
}
