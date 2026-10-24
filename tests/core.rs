use deedwake_core::{Content, Core, Entity, Error, Event, Proposal, Status};
const A: Entity = Entity(1);
const B: Entity = Entity(2);
fn proposal(cause: deedwake_core::RecordId) -> Proposal {
    Proposal {
        request: "change host state".into(),
        causes: vec![cause],
        rule: "Known evidence permits request".into(),
    }
}
#[test]
fn sent_accounts_are_immutable_and_unknown_bases_cannot_be_used() {
    let mut core = Core::new(32);
    let observed = core.observe(A, "partial view".into());
    let account = core
        .account(A, observed, "uncertain account".into())
        .unwrap();
    let snapshot = core.record(account).unwrap().clone();
    core.schedule(A, B, account, 3).unwrap();
    assert_eq!(
        core.account(B, observed, "unseen".into()),
        Err(Error::NotKnown)
    );
    assert_eq!(core.schedule(B, A, account, 1), Err(Error::NotKnown));
    core.account(A, observed, "later correction".into())
        .unwrap();
    assert_eq!(core.record(account), Some(&snapshot));
    core.advance_to(2).unwrap();
    assert!(core.perspective(B).records().is_empty());
    core.advance_to(3).unwrap();
    let view = core.perspective(B);
    assert_eq!(view.records(), &[&snapshot]);
    assert!(!view.records().iter().any(|r| r.id == observed));
}
#[test]
fn due_time_then_insertion_order_and_no_backwards_time() {
    let mut core = Core::new(32);
    let first = core.observe(A, "first".into());
    let second = core.observe(A, "second".into());
    core.schedule(A, B, second, 5).unwrap();
    core.schedule(A, B, first, 3).unwrap();
    core.schedule(A, B, second, 3).unwrap();
    core.advance_to(8).unwrap();
    let delivered: Vec<_> = core
        .trace()
        .iter()
        .filter_map(|entry| match &entry.event {
            Event::Delivered(d) => Some((entry.time, d.record, d.order)),
            _ => None,
        })
        .collect();
    assert_eq!(
        delivered,
        vec![(3, first, 1), (3, second, 2), (5, second, 0)]
    );
    assert_eq!(core.perspective(B).records().len(), 2);
    assert_eq!(core.advance_to(7), Err(Error::PastTime));
    assert_eq!(core.schedule(A, B, first, 7), Err(Error::PastTime));
    assert_eq!(core.time(), 8);
}
#[test]
fn policy_cannot_cite_hidden_records_and_host_controls_execution() {
    let mut core = Core::new(64);
    let cause = core.observe(A, "private observation".into());
    let before = core.trace().clone();
    assert_eq!(
        core.evaluate(B, |view| {
            assert!(view.records().is_empty());
            Some(proposal(cause))
        }),
        Err(Error::NotKnown)
    );
    assert_eq!(core.trace(), &before);
    let id = core
        .evaluate(A, |_| Some(proposal(cause)))
        .unwrap()
        .unwrap();
    assert_eq!(core.action(id).unwrap().status, Status::Proposed);
    assert_eq!(
        core.confirm(id, "not executed".into()),
        Err(Error::InvalidTransition)
    );
    core.accept(id).unwrap();
    assert_eq!(core.accept(id), Err(Error::InvalidTransition));
    assert!(
        !core
            .perspective(A)
            .records()
            .iter()
            .any(|r| matches!(r.content, Content::ConfirmedOutcome { .. }))
    );
    let outcome = core.confirm(id, "host changed state".into()).unwrap();
    assert!(
        matches!(core.record(outcome).unwrap().content, Content::ConfirmedOutcome { action, .. } if action == id)
    );
    assert_eq!(
        core.confirm(id, "duplicate".into()),
        Err(Error::InvalidTransition)
    );
    assert_eq!(
        core.reject(id, "too late".into()),
        Err(Error::InvalidTransition)
    );
    assert!(core.perspective(B).records().is_empty());
    core.schedule(A, B, outcome, 1).unwrap();
    core.advance_to(1).unwrap();
    assert_eq!(core.perspective(B).records().len(), 1);
}
#[test]
fn rejection_and_cancellation_never_create_confirmed_outcomes() {
    for accept in [false, true] {
        let mut core = Core::new(32);
        let cause = core.observe(A, "input".into());
        let id = core
            .evaluate(A, |_| Some(proposal(cause)))
            .unwrap()
            .unwrap();
        if accept {
            core.accept(id).unwrap();
        }
        assert_eq!(core.reject(id, "".into()), Err(Error::EmptyExplanation));
        core.reject(id, "Host declined".into()).unwrap();
        assert_eq!(core.accept(id), Err(Error::InvalidTransition));
        assert_eq!(
            core.confirm(id, "incorrect".into()),
            Err(Error::InvalidTransition)
        );
        assert_eq!(core.perspective(A).records().len(), 1);
        assert!(matches!(
            core.trace().back().unwrap().event,
            Event::Rejected(_)
        ));
    }
}
#[test]
fn trace_is_bounded_without_losing_records_or_causal_links() {
    let mut core = Core::new(2);
    let cause = core.observe(A, "input".into());
    let id = core
        .evaluate(A, |_| Some(proposal(cause)))
        .unwrap()
        .unwrap();
    core.accept(id).unwrap();
    let result = core.confirm(id, "actual result".into()).unwrap();
    assert_eq!(core.trace().len(), 2);
    assert_eq!(core.dropped_trace(), 2);
    assert_eq!(core.trace().back().unwrap().causes, vec![cause]);
    assert_eq!(core.perspective(A).records().len(), 2);
    assert!(core.record(result).is_some());
    let mut disabled = Core::new(0);
    disabled.observe(A, "input".into());
    assert!(disabled.trace().is_empty());
    assert_eq!(disabled.dropped_trace(), 1);
}
