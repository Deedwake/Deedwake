use std::collections::BTreeMap;

use deedwake_core::{
    ActionId, Availability, Content, Core, Entity, EntityState, Error, Event, ExecutionMode,
    InputError, InputId, LifecycleError, Proposal, Record, RecordId, Residency, Status,
};

const A: Entity = Entity(1);
const B: Entity = Entity(2);
const C: Entity = Entity(3);

fn present(residency: Residency, availability: Availability) -> EntityState {
    EntityState::Present {
        residency,
        availability,
    }
}

fn proposal(cause: RecordId) -> Proposal {
    Proposal {
        request: "increment the specified host target".into(),
        causes: vec![cause],
        rule: "Known inspection permits a request".into(),
    }
}

fn propose(core: &mut Core, actor: Entity, cause: RecordId) -> ActionId {
    core.evaluate_guarded(actor, |_| Some(proposal(cause)))
        .unwrap()
        .unwrap()
}

fn records(core: &Core, entity: Entity) -> Vec<Record> {
    core.perspective(entity)
        .records()
        .iter()
        .map(|record| (*record).clone())
        .collect()
}

fn assert_no_outcomes(core: &Core, entity: Entity) {
    assert!(
        core.perspective(entity)
            .records()
            .iter()
            .all(|record| { !matches!(record.content, Content::ConfirmedOutcome { .. }) })
    );
}

#[derive(Debug, PartialEq, Eq)]
enum HostError {
    Lifecycle(LifecycleError),
    PermissionDenied,
    MissingTarget,
}

// The serialized host call owns domain permission, exact target lookup, and mutation.
fn execute(
    core: &Core,
    action: ActionId,
    mode: ExecutionMode,
    permitted: bool,
    target: Entity,
    world: &mut BTreeMap<Entity, u32>,
) -> Result<(), HostError> {
    core.check_execution(action, mode)
        .map_err(HostError::Lifecycle)?;
    if !permitted {
        return Err(HostError::PermissionDenied);
    }
    let counter = world.get_mut(&target).ok_or(HostError::MissingTarget)?;
    *counter += 1;
    Ok(())
}

#[test]
fn direct_host_flow_rechecks_availability_and_residency_before_mutation() {
    let mut core = Core::with_lifecycle_capacity(64, 1, 1);
    core.register_entity(A, Residency::Loaded, Availability::Available)
        .unwrap();
    let cause = core.observe(A, "inspection".into());
    let action = propose(&mut core, A, cause);
    let mut world = BTreeMap::from([(B, 0)]);

    core.set_entity_state(A, present(Residency::Loaded, Availability::Unavailable))
        .unwrap();
    let before = core.trace().clone();
    assert_eq!(
        core.evaluate_guarded(A, |_| panic!("unavailable policy must not run")),
        Err(LifecycleError::Unavailable)
    );
    assert_eq!(
        core.accept_guarded(action),
        Err(LifecycleError::Unavailable)
    );
    assert_eq!(core.action(action).unwrap().status, Status::Proposed);
    assert_eq!(core.trace(), &before);
    assert_eq!(world[&B], 0);
    assert_no_outcomes(&core, A);

    core.set_entity_state(A, present(Residency::Loaded, Availability::Available))
        .unwrap();
    core.accept_guarded(action).unwrap();
    core.set_entity_state(A, present(Residency::Unloaded, Availability::Available))
        .unwrap();
    let before = core.trace().clone();
    assert_eq!(
        execute(&core, action, ExecutionMode::Direct, true, B, &mut world),
        Err(HostError::Lifecycle(LifecycleError::Unloaded))
    );
    assert_eq!(core.trace(), &before);
    assert_eq!(core.action(action).unwrap().status, Status::Accepted);
    assert_eq!(world[&B], 0);
    assert_no_outcomes(&core, A);

    core.set_entity_state(A, present(Residency::Loaded, Availability::Available))
        .unwrap();
    execute(&core, action, ExecutionMode::Direct, true, B, &mut world).unwrap();
    let outcome = core
        .confirm_identified(InputId(1), action, "target incremented".into())
        .unwrap();
    assert!(!outcome.duplicate);
    assert_eq!(world[&B], 1);
    assert_eq!(
        core.action(action).unwrap().status,
        Status::Confirmed(outcome.value)
    );
    assert_eq!(
        execute(&core, action, ExecutionMode::Direct, true, B, &mut world),
        Err(HostError::Lifecycle(LifecycleError::Core(
            Error::InvalidTransition
        )))
    );
    let before = core.trace().clone();
    let retry = core
        .confirm_identified(InputId(1), action, "target incremented".into())
        .unwrap();
    assert!(retry.duplicate);
    assert_eq!(retry.value, outcome.value);
    assert_eq!(core.trace(), &before);
    assert_eq!(world[&B], 1);
    // Retrying the receipt acknowledges data; the host does not execute again.
}

#[test]
fn delegated_unloaded_execution_still_needs_host_permission_and_exact_target() {
    let mut core = Core::with_lifecycle_capacity(32, 0, 1);
    core.register_entity(A, Residency::Unloaded, Availability::Available)
        .unwrap();
    let cause = core.observe(A, "remote inspection".into());
    let action = propose(&mut core, A, cause);
    core.accept_guarded(action).unwrap();
    let mut world = BTreeMap::from([(B, 0)]);
    let before = core.trace().clone();
    assert_eq!(
        core.check_execution(action, ExecutionMode::DelegatedAbstract),
        Ok(())
    );
    assert_eq!(
        execute(
            &core,
            action,
            ExecutionMode::DelegatedAbstract,
            false,
            B,
            &mut world
        ),
        Err(HostError::PermissionDenied)
    );
    assert_eq!(
        execute(
            &core,
            action,
            ExecutionMode::DelegatedAbstract,
            true,
            C,
            &mut world
        ),
        Err(HostError::MissingTarget)
    );
    assert_eq!(world, BTreeMap::from([(B, 0)]));
    assert_eq!(core.trace(), &before);
    assert_eq!(core.action(action).unwrap().status, Status::Accepted);
    assert_no_outcomes(&core, A);

    execute(
        &core,
        action,
        ExecutionMode::DelegatedAbstract,
        true,
        B,
        &mut world,
    )
    .unwrap();
    core.confirm(action, "delegated target incremented".into())
        .unwrap();
    assert_eq!(world, BTreeMap::from([(B, 1)]));
}

#[test]
fn unavailable_and_incapable_actors_cannot_plan_accept_or_execute_in_either_mode() {
    let mut core = Core::with_lifecycle_capacity(64, 0, 1);
    core.register_entity(A, Residency::Loaded, Availability::Available)
        .unwrap();
    let cause = core.observe(A, "inspection".into());
    let proposed = propose(&mut core, A, cause);
    let accepted = propose(&mut core, A, cause);
    core.accept_guarded(accepted).unwrap();
    let mut world = BTreeMap::from([(B, 0)]);
    for (availability, error) in [
        (Availability::Unavailable, LifecycleError::Unavailable),
        (Availability::Incapable, LifecycleError::Incapable),
    ] {
        core.set_entity_state(A, present(Residency::Loaded, availability))
            .unwrap();
        let before = core.trace().clone();
        assert_eq!(
            core.evaluate_guarded(A, |_| panic!("ineligible policy must not run")),
            Err(error)
        );
        assert_eq!(core.accept_guarded(proposed), Err(error));
        for mode in [ExecutionMode::Direct, ExecutionMode::DelegatedAbstract] {
            assert_eq!(
                execute(&core, accepted, mode, true, B, &mut world),
                Err(HostError::Lifecycle(error))
            );
        }
        assert_eq!(core.trace(), &before);
        assert_eq!(core.action(proposed).unwrap().status, Status::Proposed);
        assert_eq!(core.action(accepted).unwrap().status, Status::Accepted);
        assert_eq!(world[&B], 0);
        assert_no_outcomes(&core, A);
    }
}

#[test]
fn unknown_entities_do_not_run_policies_and_execution_requires_accepted_actions_first() {
    let mut core = Core::with_lifecycle_capacity(32, 0, 1);
    assert_eq!(
        core.evaluate_guarded(A, |_| panic!("unknown policy must not run")),
        Err(LifecycleError::UnknownEntity)
    );
    assert_eq!(
        core.set_entity_state(A, EntityState::Retired),
        Err(LifecycleError::UnknownEntity)
    );
    assert_eq!(core.entity_state(A), None);
    let cause = core.observe(A, "unmanaged input".into());
    let action = core
        .evaluate(A, |_| Some(proposal(cause)))
        .unwrap()
        .unwrap();
    assert_eq!(
        core.accept_guarded(action),
        Err(LifecycleError::UnknownEntity)
    );
    assert_eq!(
        core.check_execution(action, ExecutionMode::Direct),
        Err(LifecycleError::Core(Error::InvalidTransition))
    );
    core.accept(action).unwrap();
    assert_eq!(
        core.check_execution(action, ExecutionMode::DelegatedAbstract),
        Err(LifecycleError::UnknownEntity)
    );
    let empty = Core::with_lifecycle_capacity(32, 0, 1);
    assert_eq!(
        empty.check_execution(action, ExecutionMode::Direct),
        Err(LifecycleError::Core(Error::UnknownAction))
    );
    assert_no_outcomes(&core, A);
}

#[test]
fn retirement_is_a_terminal_session_tombstone_and_registration_failure_is_atomic() {
    let mut core = Core::with_lifecycle_capacity(32, 0, 1);
    core.register_entity(A, Residency::Loaded, Availability::Available)
        .unwrap();
    let cause = core.observe(A, "retained identity history".into());
    let before = core.trace().clone();
    assert_eq!(
        core.register_entity(B, Residency::Loaded, Availability::Available),
        Err(LifecycleError::CapacityReached)
    );
    assert_eq!(core.entity_state(B), None);
    assert_eq!(
        core.entity_state(A),
        Some(present(Residency::Loaded, Availability::Available))
    );
    core.set_entity_state(A, present(Residency::Unloaded, Availability::Incapable))
        .unwrap();
    core.set_entity_state(A, EntityState::Retired).unwrap();
    core.set_entity_state(A, EntityState::Retired).unwrap();
    assert_eq!(
        core.set_entity_state(A, present(Residency::Loaded, Availability::Available)),
        Err(LifecycleError::Retired)
    );
    assert_eq!(
        core.register_entity(A, Residency::Loaded, Availability::Available),
        Err(LifecycleError::AlreadyRegistered)
    );
    assert_eq!(
        core.register_entity(B, Residency::Loaded, Availability::Available),
        Err(LifecycleError::CapacityReached)
    );
    assert_eq!(core.entity_state(A), Some(EntityState::Retired));
    assert_eq!(core.entity_state(B), None);
    assert_eq!(core.record(cause).unwrap().source, A);
    assert_eq!(core.trace(), &before);
    assert_eq!(
        core.evaluate_guarded(A, |_| panic!("retired policy must not run")),
        Err(LifecycleError::Retired)
    );

    let mut fresh = Core::with_lifecycle_capacity(32, 0, 1);
    fresh
        .register_entity(A, Residency::Loaded, Availability::Available)
        .unwrap();
    assert!(fresh.perspective(A).records().is_empty());
}

#[test]
fn lifecycle_changes_leave_pending_actions_for_explicit_host_rejection() {
    let mut core = Core::with_lifecycle_capacity(64, 0, 1);
    core.register_entity(A, Residency::Loaded, Availability::Available)
        .unwrap();
    let cause = core.observe(A, "inspection".into());
    let proposed = propose(&mut core, A, cause);
    let accepted = propose(&mut core, A, cause);
    core.accept_guarded(accepted).unwrap();
    for state in [
        present(Residency::Unloaded, Availability::Unavailable),
        present(Residency::Loaded, Availability::Incapable),
        EntityState::Retired,
    ] {
        core.set_entity_state(A, state).unwrap();
        assert_eq!(core.action(proposed).unwrap().status, Status::Proposed);
        assert_eq!(core.action(accepted).unwrap().status, Status::Accepted);
        assert_no_outcomes(&core, A);
    }
    assert_eq!(core.accept_guarded(proposed), Err(LifecycleError::Retired));
    assert_eq!(
        core.check_execution(accepted, ExecutionMode::Direct),
        Err(LifecycleError::Retired)
    );
    core.reject(proposed, "host discarded pending proposal".into())
        .unwrap();
    core.reject(accepted, "host cancelled unexecuted action".into())
        .unwrap();
    assert!(matches!(
        core.action(proposed).unwrap().status,
        Status::Rejected(_)
    ));
    assert!(matches!(
        core.action(accepted).unwrap().status,
        Status::Rejected(_)
    ));
    assert_no_outcomes(&core, A);
}

#[test]
fn lifecycle_updates_do_not_create_observations_or_disclose_host_state() {
    let mut core = Core::with_lifecycle_capacity(64, 0, 2);
    core.register_entity(A, Residency::Loaded, Availability::Available)
        .unwrap();
    core.register_entity(B, Residency::Loaded, Availability::Available)
        .unwrap();
    let private = core.observe(A, "private inspection".into());
    core.observe(B, "different private inspection".into());
    let before: Vec<_> = [A, B, C].map(|entity| records(&core, entity)).into();
    let trace = core.trace().clone();
    for state in [
        present(Residency::Unloaded, Availability::Available),
        present(Residency::Loaded, Availability::Unavailable),
        present(Residency::Loaded, Availability::Incapable),
        EntityState::Retired,
    ] {
        core.set_entity_state(A, state).unwrap();
        assert_eq!(core.entity_state(A), Some(state));
        for (entity, expected) in [A, B, C].into_iter().zip(&before) {
            assert_eq!(records(&core, entity), *expected);
        }
    }
    assert_eq!(core.trace(), &trace);
    assert!(
        core.perspective(B)
            .records()
            .iter()
            .all(|record| record.id != private)
    );
    assert!(core.perspective(C).records().is_empty());
}

#[test]
fn unload_reload_preserves_records_pending_delivery_action_history_and_input_receipts() {
    let mut core = Core::with_lifecycle_capacity(64, 3, 2);
    core.register_entity(A, Residency::Loaded, Availability::Available)
        .unwrap();
    core.register_entity(B, Residency::Loaded, Availability::Available)
        .unwrap();
    let observed = core
        .observe_identified(InputId(1), A, "inspection".into())
        .unwrap()
        .value;
    let account = core
        .account_identified(InputId(2), A, observed, "account".into())
        .unwrap()
        .value;
    core.schedule_identified(InputId(3), A, B, account, 5)
        .unwrap();
    let action = propose(&mut core, A, account);
    core.accept_guarded(action).unwrap();
    let saved_records = records(&core, A);
    let saved_action = core.action(action).unwrap().clone();
    let saved_trace = core.trace().clone();
    core.set_entity_state(A, present(Residency::Unloaded, Availability::Available))
        .unwrap();
    core.set_entity_state(B, present(Residency::Unloaded, Availability::Available))
        .unwrap();
    assert_eq!(core.trace(), &saved_trace);
    assert_eq!(records(&core, A), saved_records);
    assert!(core.perspective(B).records().is_empty());
    // Existing queue policy delivers on time even while the recipient is unloaded.
    core.advance_to(5).unwrap();
    assert_eq!(
        core.perspective(B).records(),
        &[core.record(account).unwrap()]
    );
    assert!(
        core.perspective(B)
            .records()
            .iter()
            .all(|record| record.id != observed)
    );
    core.set_entity_state(A, present(Residency::Loaded, Availability::Available))
        .unwrap();
    core.set_entity_state(B, present(Residency::Loaded, Availability::Available))
        .unwrap();
    assert_eq!(records(&core, A), saved_records);
    assert_eq!(core.action(action), Some(&saved_action));
    assert_eq!(core.check_execution(action, ExecutionMode::Direct), Ok(()));
    let before = core.trace().clone();
    let observation_retry = core
        .observe_identified(InputId(1), A, "inspection".into())
        .unwrap();
    assert_eq!(observation_retry.value, observed);
    assert!(observation_retry.duplicate);
    let account_retry = core
        .account_identified(InputId(2), A, observed, "account".into())
        .unwrap();
    assert_eq!(account_retry.value, account);
    assert!(account_retry.duplicate);
    assert!(
        core.schedule_identified(InputId(3), A, B, account, 5)
            .unwrap()
            .duplicate
    );
    core.advance_to(6).unwrap();
    assert_eq!(core.trace(), &before);
    assert_eq!(
        core.trace()
            .iter()
            .filter(|entry| matches!(entry.event, Event::Delivered(_)))
            .count(),
        1
    );
}

#[test]
fn completed_work_can_be_confirmed_after_retirement_and_receipt_retry_ignores_full_capacities() {
    let mut core = Core::with_lifecycle_capacity(32, 1, 1);
    core.register_entity(A, Residency::Loaded, Availability::Available)
        .unwrap();
    let cause = core.observe(A, "inspection".into());
    let action = propose(&mut core, A, cause);
    core.accept_guarded(action).unwrap();
    let mut world = BTreeMap::from([(B, 0)]);
    execute(&core, action, ExecutionMode::Direct, true, B, &mut world).unwrap();
    core.set_entity_state(A, EntityState::Retired).unwrap();
    let outcome = core
        .confirm_identified(InputId(1), action, "completed before retirement".into())
        .unwrap();
    assert!(!outcome.duplicate);
    assert_eq!(
        core.action(action).unwrap().status,
        Status::Confirmed(outcome.value)
    );
    let before = core.trace().clone();
    assert_eq!(
        core.register_entity(B, Residency::Loaded, Availability::Available),
        Err(LifecycleError::CapacityReached)
    );
    let retry = core
        .confirm_identified(InputId(1), action, "completed before retirement".into())
        .unwrap();
    assert!(retry.duplicate);
    assert_eq!(retry.value, outcome.value);
    assert_eq!(
        core.confirm_identified(InputId(2), action, "completed before retirement".into()),
        Err(InputError::CapacityReached)
    );
    assert_eq!(core.trace(), &before);
    assert_eq!(world[&B], 1);
    assert_eq!(
        core.perspective(A)
            .records()
            .iter()
            .filter(|record| matches!(record.content, Content::ConfirmedOutcome { .. }))
            .count(),
        1
    );
}

#[test]
fn legacy_constructors_keep_raw_host_operations_unmanaged_and_registration_disabled() {
    for mut core in [Core::new(32), Core::with_input_capacity(32, 1)] {
        assert_eq!(
            core.register_entity(A, Residency::Loaded, Availability::Available),
            Err(LifecycleError::CapacityReached)
        );
        let cause = core.observe(A, "legacy input".into());
        let action = core
            .evaluate(A, |_| Some(proposal(cause)))
            .unwrap()
            .unwrap();
        core.accept(action).unwrap();
        let outcome = core.confirm(action, "legacy host executed".into()).unwrap();
        assert_eq!(
            core.action(action).unwrap().status,
            Status::Confirmed(outcome)
        );
        assert_eq!(core.entity_state(A), None);
        assert_eq!(core.perspective(A).records().len(), 2);
    }
}
