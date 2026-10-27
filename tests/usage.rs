use deedwake_core::*;

fn recomputed(core: &Core) -> CoreUsage {
    let snapshot = core.snapshot(&SnapshotLimits::default()).unwrap();
    let text_bytes = snapshot
        .records
        .iter()
        .map(|r| match &r.content {
            SnapshotContent::Observation(s) => s.len(),
            SnapshotContent::Account { summary, .. } => summary.len(),
            SnapshotContent::ConfirmedOutcome { description, .. } => description.len(),
        })
        .sum::<usize>()
        + snapshot
            .actions
            .iter()
            .map(|a| {
                a.proposal.request.len()
                    + a.proposal.rule.len()
                    + match &a.status {
                        SnapshotStatus::Rejected(s) => s.len(),
                        _ => 0,
                    }
            })
            .sum::<usize>()
        + snapshot.trace.iter().map(|t| t.rule.len()).sum::<usize>();
    CoreUsage {
        records: snapshot.records.len(),
        actions: snapshot.actions.len(),
        pending: snapshot.pending.len(),
        inputs: snapshot.inputs.len(),
        entities: snapshot.entities.len(),
        known_owners: snapshot.known.len(),
        trace_entries: snapshot.trace.len(),
        next_delivery: snapshot.next_delivery as usize,
        text_bytes,
        reference_edges: snapshot
            .known
            .iter()
            .map(|k| k.records.len())
            .sum::<usize>()
            + snapshot
                .actions
                .iter()
                .map(|a| a.proposal.causes.len())
                .sum::<usize>()
            + snapshot.trace.iter().map(|t| t.causes.len()).sum::<usize>(),
    }
}
fn check(core: &Core) {
    assert_eq!(core.usage(), recomputed(core));
}

#[test]
fn usage_matches_independent_snapshot_totals_through_all_mutations_and_restore() {
    for capacity in [0, 1, 4, 128] {
        let mut core = Core::with_lifecycle_capacity(capacity, 8, 2);
        let source = Entity(1);
        let target = Entity(2);
        check(&core);
        core.register_entity(source, Residency::Loaded, Availability::Available)
            .unwrap();
        core.register_entity(target, Residency::Loaded, Availability::Available)
            .unwrap();
        check(&core);
        let first = core
            .observe_identified(InputId(1), source, "snow ☃".into())
            .unwrap();
        check(&core);
        let usage = core.usage();
        assert!(
            core.observe_identified(InputId(1), source, "snow ☃".into())
                .unwrap()
                .duplicate
        );
        assert_eq!(
            core.observe_identified(InputId(1), source, "different".into()),
            Err(InputError::Conflict)
        );
        assert_eq!(core.usage(), usage);
        let account = core
            .account_identified(InputId(2), source, first.value, "heard".into())
            .unwrap()
            .value;
        check(&core);
        let usage = core.usage();
        assert!(
            core.account_identified(InputId(2), source, first.value, "heard".into())
                .unwrap()
                .duplicate
        );
        assert_eq!(
            core.account_identified(InputId(2), source, first.value, "changed".into()),
            Err(InputError::Conflict)
        );
        assert_eq!(core.usage(), usage);
        assert_eq!(
            core.account(target, account, "not known".into()),
            Err(Error::NotKnown)
        );
        check(&core);
        core.schedule_identified(InputId(3), source, target, account, 1)
            .unwrap();
        core.schedule(source, target, account, 1).unwrap();
        check(&core);
        core.advance_to(1).unwrap();
        check(&core); // repeated delivery adds trace causes, never duplicate knowledge.
        let usage = core.usage();
        assert!(
            core.schedule_identified(InputId(3), source, target, account, 1)
                .unwrap()
                .duplicate
        );
        assert_eq!(core.advance_to(0), Err(Error::PastTime));
        assert_eq!(core.usage(), usage);
        assert_eq!(
            core.evaluate(target, |_| Some(Proposal {
                request: "".into(),
                rule: "why".into(),
                causes: vec![account]
            })),
            Err(Error::EmptyExplanation)
        );
        assert_eq!(core.usage(), usage);
        for reject in [false, true] {
            let id = core
                .evaluate_guarded(target, |_| {
                    Some(Proposal {
                        request: "respond".into(),
                        rule: "two cited causes".into(),
                        causes: vec![account, account],
                    })
                })
                .unwrap()
                .unwrap();
            check(&core);
            core.accept_guarded(id).unwrap();
            check(&core);
            if reject {
                core.reject(id, "cancelled".into()).unwrap();
            } else {
                core.confirm_identified(InputId(4), id, "done".into())
                    .unwrap();
                let usage = core.usage();
                assert!(
                    core.confirm_identified(InputId(4), id, "done".into())
                        .unwrap()
                        .duplicate
                );
                assert_eq!(
                    core.confirm_identified(InputId(4), id, "changed".into()),
                    Err(InputError::Conflict)
                );
                assert_eq!(core.usage(), usage);
            }
            check(&core);
            let usage = core.usage();
            assert_eq!(
                core.reject(id, "again".into()),
                Err(Error::InvalidTransition)
            );
            assert_eq!(core.usage(), usage);
        }
        core.schedule_managed_identified(
            InputId(5),
            source,
            target,
            account,
            2,
            DeliveryMode::LoadedRecipient,
        )
        .unwrap();
        let usage = core.usage();
        assert!(
            core.schedule_managed_identified(
                InputId(5),
                source,
                target,
                account,
                2,
                DeliveryMode::LoadedRecipient
            )
            .unwrap()
            .duplicate
        );
        assert_eq!(core.usage(), usage);
        core.set_entity_state(target, EntityState::Retired).unwrap();
        check(&core);
        let usage = core.usage();
        assert_eq!(
            core.schedule_managed_identified(
                InputId(6),
                source,
                target,
                account,
                2,
                DeliveryMode::LoadedRecipient
            ),
            Err(DeliveryError::RecipientRetired)
        );
        assert_eq!(
            core.evaluate_guarded(target, |_| panic!("retired actor cannot evaluate")),
            Err(LifecycleError::Retired)
        );
        assert_eq!(core.usage(), usage);
        core.advance_to(2).unwrap();
        check(&core);
        let usage = core.usage();
        assert!(
            core.schedule_managed_identified(
                InputId(5),
                source,
                target,
                account,
                2,
                DeliveryMode::LoadedRecipient
            )
            .unwrap()
            .duplicate
        );
        assert_eq!(core.usage(), usage);
        for input in 6..=8 {
            core.observe_identified(InputId(input), source, "fill ledger".into())
                .unwrap();
            check(&core);
        }
        let usage = core.usage();
        assert_eq!(
            core.observe_identified(InputId(9), source, "full".into()),
            Err(InputError::CapacityReached)
        );
        assert_eq!(core.usage(), usage);
        let dto = core.snapshot(&SnapshotLimits::default()).unwrap();
        let mut restored = Core::from_snapshot(dto, &SnapshotLimits::default()).unwrap();
        assert_eq!(core.usage(), restored.usage());
        check(&restored);
        for c in [&mut core, &mut restored] {
            c.observe(source, "later".into());
            check(c);
        }
        assert_eq!(core.usage(), restored.usage());
    }
}
