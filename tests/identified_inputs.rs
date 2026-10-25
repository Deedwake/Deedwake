use deedwake_core::{
    ActionId, Content, Core, Entity, Error, Event, InputError, InputId, InputReceipt, Proposal,
    RecordId, Status,
};

const A: Entity = Entity(1);
const B: Entity = Entity(2);
const C: Entity = Entity(3);

fn receipt<T: std::fmt::Debug + PartialEq>(actual: InputReceipt<T>, value: T, duplicate: bool) {
    assert_eq!(actual.value, value);
    assert_eq!(actual.duplicate, duplicate);
}

fn propose(core: &mut Core, actor: Entity, cause: RecordId) -> ActionId {
    core.evaluate(actor, |_| {
        Some(Proposal {
            request: "repair inspection finding".into(),
            causes: vec![cause],
            rule: "Received inspection permits a repair request".into(),
        })
    })
    .unwrap()
    .unwrap()
}

#[test]
fn host_retries_keep_original_receipts_through_delivery_and_confirmation() {
    let mut core = Core::with_input_capacity(32, 4);
    let observed = core
        .observe_identified(InputId(1), A, "inspection finding".into())
        .unwrap();
    assert!(!observed.duplicate);
    let observed = observed.value;
    core.advance_to(2).unwrap();
    let account = core
        .account_identified(InputId(2), A, observed, "repair needed".into())
        .unwrap();
    assert!(!account.duplicate);
    let account = account.value;
    receipt(
        core.schedule_identified(InputId(3), A, B, account, 5)
            .unwrap(),
        (),
        false,
    );
    assert!(core.perspective(B).records().is_empty());
    core.advance_to(8).unwrap();
    let action = core
        .evaluate(B, |view| {
            assert_eq!(view.records().len(), 1);
            assert_eq!(view.records()[0].id, account);
            assert!(!view.records().iter().any(|record| record.id == observed));
            Some(Proposal {
                request: "repair inspection finding".into(),
                causes: vec![account],
                rule: "Received account permits a repair request".into(),
            })
        })
        .unwrap()
        .unwrap();
    assert_eq!(core.action(action).unwrap().status, Status::Proposed);
    core.accept(action).unwrap();
    let outcome = core
        .confirm_identified(InputId(4), action, "repair completed".into())
        .unwrap();
    assert!(!outcome.duplicate);
    let outcome = outcome.value;
    assert_eq!(
        core.action(action).unwrap().status,
        Status::Confirmed(outcome)
    );
    assert_eq!(
        core.record(account).unwrap().content,
        Content::Account {
            summary: "repair needed".into(),
            basis: observed,
        }
    );
    assert_eq!(
        core.record(outcome).unwrap().content,
        Content::ConfirmedOutcome {
            description: "repair completed".into(),
            action,
        }
    );
    core.advance_to(10).unwrap();
    let before = core.trace().clone();
    let dropped = core.dropped_trace();
    receipt(
        core.observe_identified(InputId(1), A, "inspection finding".into())
            .unwrap(),
        observed,
        true,
    );
    receipt(
        core.account_identified(InputId(2), A, observed, "repair needed".into())
            .unwrap(),
        account,
        true,
    );
    receipt(
        core.schedule_identified(InputId(3), A, B, account, 5)
            .unwrap(),
        (),
        true,
    );
    receipt(
        core.confirm_identified(InputId(4), action, "repair completed".into())
            .unwrap(),
        outcome,
        true,
    );
    assert_eq!(
        core.confirm_identified(InputId(5), action, "repair completed".into()),
        Err(InputError::CapacityReached)
    );
    assert_eq!(core.trace(), &before);
    assert_eq!(core.dropped_trace(), dropped);
    assert_eq!(core.time(), 10);
    assert_eq!(core.record(observed).unwrap().time, 0);
    assert_eq!(core.record(account).unwrap().time, 2);
    assert_eq!(core.record(outcome).unwrap().time, 8);
    assert_eq!(core.perspective(A).records().len(), 2);
    assert_eq!(core.perspective(B).records().len(), 2);
    core.advance_to(11).unwrap();
    assert_eq!(core.trace(), &before);
}

#[test]
fn accepted_ids_conflict_on_operation_kind_and_every_payload_field() {
    let mut core = Core::with_input_capacity(32, 8);
    let observed = core
        .observe_identified(InputId(1), A, "finding".into())
        .unwrap()
        .value;
    let other = core.observe(A, "other finding".into());
    let account = core
        .account_identified(InputId(2), A, observed, "report".into())
        .unwrap()
        .value;
    core.schedule_identified(InputId(3), A, B, account, 5)
        .unwrap();
    let action = propose(&mut core, A, account);
    let other_action = propose(&mut core, A, account);
    core.accept(action).unwrap();
    let outcome = core
        .confirm_identified(InputId(4), action, "done".into())
        .unwrap()
        .value;
    let before = core.trace().clone();

    assert_eq!(
        core.observe_identified(InputId(1), B, "finding".into()),
        Err(InputError::Conflict)
    );
    assert_eq!(
        core.observe_identified(InputId(1), A, "changed".into()),
        Err(InputError::Conflict)
    );
    assert_eq!(
        core.account_identified(InputId(2), B, observed, "report".into()),
        Err(InputError::Conflict)
    );
    assert_eq!(
        core.account_identified(InputId(2), A, other, "report".into()),
        Err(InputError::Conflict)
    );
    assert_eq!(
        core.account_identified(InputId(2), A, observed, "changed".into()),
        Err(InputError::Conflict)
    );
    assert_eq!(
        core.schedule_identified(InputId(3), B, B, account, 5),
        Err(InputError::Conflict)
    );
    assert_eq!(
        core.schedule_identified(InputId(3), A, C, account, 5),
        Err(InputError::Conflict)
    );
    assert_eq!(
        core.schedule_identified(InputId(3), A, B, observed, 5),
        Err(InputError::Conflict)
    );
    assert_eq!(
        core.schedule_identified(InputId(3), A, B, account, 6),
        Err(InputError::Conflict)
    );
    assert_eq!(
        core.confirm_identified(InputId(4), other_action, "done".into()),
        Err(InputError::Conflict)
    );
    assert_eq!(
        core.confirm_identified(InputId(4), action, "changed".into()),
        Err(InputError::Conflict)
    );

    assert_eq!(
        core.account_identified(InputId(1), A, observed, "report".into()),
        Err(InputError::Conflict)
    );
    assert_eq!(
        core.schedule_identified(InputId(2), A, B, account, 5),
        Err(InputError::Conflict)
    );
    assert_eq!(
        core.confirm_identified(InputId(3), action, "done".into()),
        Err(InputError::Conflict)
    );
    assert_eq!(
        core.observe_identified(InputId(4), A, "finding".into()),
        Err(InputError::Conflict)
    );
    assert_eq!(core.trace(), &before);
    assert_eq!(core.perspective(A).records().len(), 4);
    assert!(core.perspective(B).records().is_empty());
    assert_eq!(
        core.action(action).unwrap().status,
        Status::Confirmed(outcome)
    );
    assert_eq!(core.action(other_action).unwrap().status, Status::Proposed);
}

#[test]
fn scheduling_retries_preserve_equal_time_order_without_consuming_sequence() {
    let mut core = Core::with_input_capacity(32, 3);
    let first = core.observe(A, "first".into());
    let second = core.observe(A, "second".into());
    let third = core.observe(A, "third".into());
    core.schedule_identified(InputId(1), A, B, first, 5)
        .unwrap();
    receipt(
        core.schedule_identified(InputId(1), A, B, first, 5)
            .unwrap(),
        (),
        true,
    );
    core.schedule_identified(InputId(2), A, B, second, 5)
        .unwrap();
    core.schedule_identified(InputId(3), A, B, third, 5)
        .unwrap();
    core.advance_to(9).unwrap();
    let deliveries: Vec<_> = core
        .trace()
        .iter()
        .filter_map(|entry| match &entry.event {
            Event::Delivered(delivery) => Some((entry.time, delivery.record, delivery.order)),
            _ => None,
        })
        .collect();
    assert_eq!(
        deliveries,
        vec![(5, first, 0), (5, second, 1), (5, third, 2)]
    );
    let before = core.trace().clone();
    receipt(
        core.schedule_identified(InputId(1), A, B, first, 5)
            .unwrap(),
        (),
        true,
    );
    core.advance_to(10).unwrap();
    assert_eq!(core.trace(), &before);
    assert_eq!(core.perspective(B).records().len(), 3);
}

#[test]
fn receipts_survive_disabled_or_evicted_trace() {
    for trace_capacity in [0, 1] {
        let mut core = Core::with_input_capacity(trace_capacity, 4);
        let observed = core
            .observe_identified(InputId(1), A, "finding".into())
            .unwrap()
            .value;
        let account = core
            .account_identified(InputId(2), A, observed, "report".into())
            .unwrap()
            .value;
        core.schedule_identified(InputId(3), A, B, account, 1)
            .unwrap();
        core.advance_to(2).unwrap();
        let action = propose(&mut core, B, account);
        core.accept(action).unwrap();
        let outcome = core
            .confirm_identified(InputId(4), action, "done".into())
            .unwrap()
            .value;
        let before = core.trace().clone();
        let dropped = core.dropped_trace();
        assert_eq!(before.len(), trace_capacity);
        assert!(dropped > 0);
        receipt(
            core.observe_identified(InputId(1), A, "finding".into())
                .unwrap(),
            observed,
            true,
        );
        receipt(
            core.account_identified(InputId(2), A, observed, "report".into())
                .unwrap(),
            account,
            true,
        );
        receipt(
            core.schedule_identified(InputId(3), A, B, account, 1)
                .unwrap(),
            (),
            true,
        );
        receipt(
            core.confirm_identified(InputId(4), action, "done".into())
                .unwrap(),
            outcome,
            true,
        );
        assert_eq!(core.trace(), &before);
        assert_eq!(core.dropped_trace(), dropped);
        assert_eq!(core.perspective(B).records().len(), 2);
    }
}

#[test]
fn full_capacity_still_acknowledges_retries_and_rejects_conflicts_before_effects() {
    let mut core = Core::with_input_capacity(16, 1);
    let observed = core
        .observe_identified(InputId(1), A, "finding".into())
        .unwrap()
        .value;
    let before = core.trace().clone();
    receipt(
        core.observe_identified(InputId(1), A, "finding".into())
            .unwrap(),
        observed,
        true,
    );
    assert_eq!(
        core.observe_identified(InputId(1), A, "changed".into()),
        Err(InputError::Conflict)
    );
    assert_eq!(
        core.account_identified(InputId(1), B, observed, "report".into()),
        Err(InputError::Conflict)
    );
    assert_eq!(
        core.observe_identified(InputId(2), A, "another".into()),
        Err(InputError::CapacityReached)
    );
    assert_eq!(
        core.account_identified(InputId(2), B, observed, "unseen".into()),
        Err(InputError::CapacityReached)
    );
    assert_eq!(core.trace(), &before);
    assert_eq!(core.perspective(A).records().len(), 1);
    assert!(core.perspective(B).records().is_empty());
    let ordinary = core.observe(A, "ordinary input remains available".into());
    assert_ne!(ordinary, observed);
}

#[test]
fn zero_input_capacity_and_legacy_constructor_keep_ordinary_workflow_available() {
    for mut core in [Core::new(32), Core::with_input_capacity(32, 0)] {
        let observed = core.observe(A, "finding".into());
        let account = core.account(A, observed, "report".into()).unwrap();
        core.schedule(A, B, account, 1).unwrap();
        core.advance_to(1).unwrap();
        let action = propose(&mut core, B, account);
        core.accept(action).unwrap();
        let before = core.trace().clone();
        assert_eq!(
            core.observe_identified(InputId(1), A, "finding".into()),
            Err(InputError::CapacityReached)
        );
        assert_eq!(
            core.account_identified(InputId(1), A, observed, "report".into()),
            Err(InputError::CapacityReached)
        );
        assert_eq!(
            core.schedule_identified(InputId(1), A, B, account, 2),
            Err(InputError::CapacityReached)
        );
        assert_eq!(
            core.confirm_identified(InputId(1), action, "done".into()),
            Err(InputError::CapacityReached)
        );
        assert_eq!(core.trace(), &before);
        assert_eq!(core.action(action).unwrap().status, Status::Accepted);
        let outcome = core.confirm(action, "done".into()).unwrap();
        assert_eq!(
            core.action(action).unwrap().status,
            Status::Confirmed(outcome)
        );
    }
}

#[test]
fn failed_inputs_do_not_reserve_ids_or_capacity_and_can_be_corrected() {
    let mut core = Core::with_input_capacity(32, 4);
    let observed = core.observe(A, "finding".into());
    assert_eq!(
        core.account_identified(InputId(1), B, observed, "report".into()),
        Err(InputError::Core(Error::NotKnown))
    );
    core.schedule(A, B, observed, 1).unwrap();
    core.advance_to(3).unwrap();
    let account = core
        .account_identified(InputId(1), B, observed, "report".into())
        .unwrap();
    assert!(!account.duplicate);
    let account = account.value;
    assert_eq!(
        core.schedule_identified(InputId(2), B, C, account, 2),
        Err(InputError::Core(Error::PastTime))
    );
    receipt(
        core.schedule_identified(InputId(2), B, C, account, 4)
            .unwrap(),
        (),
        false,
    );
    let action = propose(&mut core, B, account);
    assert_eq!(
        core.confirm_identified(InputId(3), action, "done".into()),
        Err(InputError::Core(Error::InvalidTransition))
    );
    core.accept(action).unwrap();
    let outcome = core
        .confirm_identified(InputId(3), action, "done".into())
        .unwrap();
    assert!(!outcome.duplicate);
    assert_eq!(
        core.confirm_identified(InputId(4), action, "done".into()),
        Err(InputError::Core(Error::InvalidTransition))
    );
    let other_action = propose(&mut core, B, account);
    core.accept(other_action).unwrap();
    assert_eq!(
        core.confirm_identified(InputId(4), other_action, " \n ".into()),
        Err(InputError::Core(Error::EmptyExplanation))
    );
    let other_outcome = core
        .confirm_identified(InputId(4), other_action, "other repair done".into())
        .unwrap();
    assert!(!other_outcome.duplicate);
    assert_ne!(outcome.value, other_outcome.value);
    assert_eq!(
        core.observe_identified(InputId(5), A, "full".into()),
        Err(InputError::CapacityReached)
    );
    core.advance_to(4).unwrap();
    assert_eq!(core.perspective(C).records()[0].id, account);
}

#[test]
fn fresh_ids_preserve_equal_text_inputs_and_numeric_ids_are_local_to_each_session() {
    let mut core = Core::with_input_capacity(32, 6);
    let first = core
        .observe_identified(InputId(1), A, "same finding".into())
        .unwrap();
    let second = core
        .observe_identified(InputId(2), A, "same finding".into())
        .unwrap();
    assert!(!first.duplicate);
    assert!(!second.duplicate);
    assert_ne!(first.value, second.value);
    let first_account = core
        .account_identified(InputId(3), A, first.value, "same report".into())
        .unwrap();
    let second_account = core
        .account_identified(InputId(4), A, first.value, "same report".into())
        .unwrap();
    assert!(!first_account.duplicate);
    assert!(!second_account.duplicate);
    assert_ne!(first_account.value, second_account.value);
    receipt(
        core.schedule_identified(InputId(5), A, B, first_account.value, 1)
            .unwrap(),
        (),
        false,
    );
    receipt(
        core.schedule_identified(InputId(6), A, B, first_account.value, 1)
            .unwrap(),
        (),
        false,
    );
    core.advance_to(1).unwrap();
    assert_eq!(
        core.trace()
            .iter()
            .filter(|entry| matches!(entry.event, Event::Delivered(_)))
            .count(),
        2
    );
    assert_eq!(core.perspective(B).records().len(), 1);
    let mut next_session = Core::with_input_capacity(4, 1);
    let next = next_session
        .observe_identified(InputId(1), B, "different session finding".into())
        .unwrap();
    assert!(!next.duplicate);
    assert_eq!(next_session.record(next.value).unwrap().source, B);
    assert_eq!(
        next_session.record(next.value).unwrap().content,
        Content::Observation("different session finding".into())
    );
}
