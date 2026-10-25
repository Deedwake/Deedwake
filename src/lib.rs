//! Small host-driven records, delivery, and decisions. No wall clock or I/O.
//!
//! The host owns `Core`; policies receive only a borrowed `Perspective`. IDs are
//! scoped to one Core session. Entity IDs are stable identities supplied by the
//! host; do not reuse them for different entities within a session.

//! Minimal public-API use (a received account does not expose its hidden basis):
//!
//! ```
//! use deedwake_core::{Core, Entity};
//! let mut core = Core::new(16);
//! let observer = Entity(1);
//! let recipient = Entity(2);
//! let observation = core.observe(observer, "partial inspection".into());
//! let account = core.account(observer, observation, "inspection requested".into())?;
//! core.schedule(observer, recipient, account, 2)?;
//! assert!(core.perspective(recipient).records().is_empty());
//! core.advance_to(2)?;
//! assert_eq!(core.perspective(recipient).records().len(), 1);
//! # Ok::<(), deedwake_core::Error>(())
//! ```

use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Entity(pub u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RecordId(usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ActionId(usize);

/// A host-chosen input identity scoped to one Core session. The host must use a
/// new ID for each new input; IDs provide no authentication or cross-session guard.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct InputId(pub u64);

/// The original result, with `duplicate` set for an exact accepted retry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputReceipt<T> {
    pub value: T,
    pub duplicate: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputError {
    Core(Error),
    /// An accepted ID was reused with a different operation or arguments.
    Conflict,
    /// A new ID cannot be accepted because the input ledger is full.
    CapacityReached,
}

/// A correction is a new account, never an edit to an earlier record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Content {
    Observation(String),
    Account {
        summary: String,
        basis: RecordId,
    },
    ConfirmedOutcome {
        description: String,
        action: ActionId,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub id: RecordId,
    pub source: Entity,
    pub time: u64,
    pub content: Content,
}

/// These fields are visible only for records owned or received by this entity.
#[derive(Debug)]
pub struct Perspective<'a> {
    pub entity: Entity,
    pub time: u64,
    records: Vec<&'a Record>,
}
impl<'a> Perspective<'a> {
    /// Stable record-creation order, not delivery order; hidden records are absent.
    pub fn records(&self) -> &[&'a Record] {
        &self.records
    }
}

/// Domain-specific requests belong to the host. The core never interprets them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proposal {
    pub request: String,
    pub causes: Vec<RecordId>,
    pub rule: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Proposed,
    Accepted,
    Rejected(String),
    Confirmed(RecordId),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Action {
    pub actor: Entity,
    pub proposal: Proposal,
    pub status: Status,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Delivery {
    pub record: RecordId,
    pub from: Entity,
    pub to: Entity,
    pub due: u64,
    pub order: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Recorded(RecordId),
    Scheduled(Delivery),
    Delivered(Delivery),
    Proposed(ActionId),
    Accepted(ActionId),
    Rejected(ActionId),
    Confirmed { action: ActionId, record: RecordId },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceEntry {
    pub time: u64,
    pub event: Event,
    pub causes: Vec<RecordId>,
    pub rule: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    UnknownRecord,
    NotKnown,
    PastTime,
    UnknownAction,
    InvalidTransition,
    EmptyExplanation,
    SequenceExhausted,
}

#[derive(Debug)]
enum AppliedInput {
    Record(RecordId),
    Schedule {
        from: Entity,
        to: Entity,
        record: RecordId,
        due: u64,
    },
}

/// Host-only state. Trace is bounded; records and history are retained for this
/// small session. Hosts must bound session inputs before using this at scale.
#[derive(Debug)]
pub struct Core {
    time: u64,
    records: Vec<Record>,
    known: BTreeMap<Entity, BTreeSet<RecordId>>,
    pending: BTreeMap<(u64, usize), Delivery>,
    next_delivery: usize,
    actions: Vec<Action>,
    trace: VecDeque<TraceEntry>,
    trace_capacity: usize,
    dropped_trace: u64,
    inputs: BTreeMap<InputId, AppliedInput>,
    input_capacity: usize,
}
impl Core {
    /// Create a core with identified inputs disabled (capacity zero).
    pub fn new(trace_capacity: usize) -> Self {
        Self::with_input_capacity(trace_capacity, 0)
    }
    /// Retain at most `input_capacity` successful identified inputs for this
    /// session, independently of the trace. Accepted IDs are never evicted.
    /// Ordinary methods do not use this ledger. Exact retries return the original
    /// value without changing any state, even after delivery or confirmation.
    /// Failed calls leave their IDs available; full capacity rejects new IDs
    /// before validating or applying their operations.
    pub fn with_input_capacity(trace_capacity: usize, input_capacity: usize) -> Self {
        Self {
            time: 0,
            records: Vec::new(),
            known: BTreeMap::new(),
            pending: BTreeMap::new(),
            next_delivery: 0,
            actions: Vec::new(),
            trace: VecDeque::new(),
            trace_capacity,
            dropped_trace: 0,
            inputs: BTreeMap::new(),
            input_capacity,
        }
    }
    fn applied_input(&self, input: InputId) -> Result<Option<&AppliedInput>, InputError> {
        if let Some(applied) = self.inputs.get(&input) {
            return Ok(Some(applied));
        }
        if self.inputs.len() == self.input_capacity {
            return Err(InputError::CapacityReached);
        }
        Ok(None)
    }
    pub fn time(&self) -> u64 {
        self.time
    }
    /// Privileged host inspection; never give this accessor to a policy or UI.
    pub fn record(&self, id: RecordId) -> Option<&Record> {
        self.records.get(id.0)
    }
    pub fn action(&self, id: ActionId) -> Option<&Action> {
        self.actions.get(id.0)
    }
    pub fn trace(&self) -> &VecDeque<TraceEntry> {
        &self.trace
    }
    pub fn dropped_trace(&self) -> u64 {
        self.dropped_trace
    }
    pub fn perspective(&self, entity: Entity) -> Perspective<'_> {
        Perspective {
            entity,
            time: self.time,
            records: self
                .known
                .get(&entity)
                .into_iter()
                .flatten()
                .map(|id| &self.records[id.0])
                .collect(),
        }
    }
    fn knows(&self, entity: Entity, id: RecordId) -> bool {
        self.known.get(&entity).is_some_and(|ids| ids.contains(&id))
    }
    fn log(&mut self, event: Event, causes: Vec<RecordId>, rule: &str) {
        if self.trace_capacity == 0 {
            self.dropped_trace = self.dropped_trace.saturating_add(1);
            return;
        }
        if self.trace.len() == self.trace_capacity {
            self.trace.pop_front();
            self.dropped_trace = self.dropped_trace.saturating_add(1);
        }
        self.trace.push_back(TraceEntry {
            time: self.time,
            event,
            causes,
            rule: rule.into(),
        });
    }
    fn insert(&mut self, source: Entity, content: Content) -> RecordId {
        let id = RecordId(self.records.len());
        self.records.push(Record {
            id,
            source,
            time: self.time,
            content,
        });
        self.known.entry(source).or_default().insert(id);
        id
    }
    /// The host supplies what was observed; an observation is not confirmed truth.
    pub fn observe(&mut self, source: Entity, detail: String) -> RecordId {
        let id = self.insert(source, Content::Observation(detail));
        self.log(Event::Recorded(id), vec![], "Host supplied observation");
        id
    }
    /// Apply an observation once for `input`. An accepted retry must match the
    /// operation, source, and exact detail or it returns `InputError::Conflict`.
    pub fn observe_identified(
        &mut self,
        input: InputId,
        source: Entity,
        detail: String,
    ) -> Result<InputReceipt<RecordId>, InputError> {
        if let Some(applied) = self.applied_input(input)? {
            if let AppliedInput::Record(id) = applied {
                let record = &self.records[id.0];
                if record.source == source
                    && matches!(&record.content, Content::Observation(saved) if *saved == detail)
                {
                    return Ok(InputReceipt {
                        value: *id,
                        duplicate: true,
                    });
                }
            }
            return Err(InputError::Conflict);
        }
        let value = self.observe(source, detail);
        self.inputs.insert(input, AppliedInput::Record(value));
        Ok(InputReceipt {
            value,
            duplicate: false,
        })
    }
    /// Append an account based on a record known by its source. The basis ID
    /// does not grant recipients access to the underlying record.
    ///
    /// # Errors
    /// Returns `NotKnown` if the source lacks the basis (including unknown IDs).
    pub fn account(
        &mut self,
        source: Entity,
        basis: RecordId,
        summary: String,
    ) -> Result<RecordId, Error> {
        if !self.knows(source, basis) {
            return Err(Error::NotKnown);
        }
        let id = self.insert(source, Content::Account { summary, basis });
        self.log(
            Event::Recorded(id),
            vec![basis],
            "Account derived from known record",
        );
        Ok(id)
    }
    /// Apply an account once for `input`, matching source, basis, and exact
    /// summary on retry. Initial account errors are wrapped in `InputError::Core`.
    pub fn account_identified(
        &mut self,
        input: InputId,
        source: Entity,
        basis: RecordId,
        summary: String,
    ) -> Result<InputReceipt<RecordId>, InputError> {
        if let Some(applied) = self.applied_input(input)? {
            if let AppliedInput::Record(id) = applied {
                let record = &self.records[id.0];
                if record.source == source
                    && matches!(&record.content, Content::Account { summary: saved, basis: saved_basis }
                        if *saved == summary && *saved_basis == basis)
                {
                    return Ok(InputReceipt {
                        value: *id,
                        duplicate: true,
                    });
                }
            }
            return Err(InputError::Conflict);
        }
        let value = self
            .account(source, basis, summary)
            .map_err(InputError::Core)?;
        self.inputs.insert(input, AppliedInput::Record(value));
        Ok(InputReceipt {
            value,
            duplicate: false,
        })
    }
    /// Queue one delivery without changing recipient knowledge. Equal due times
    /// use scheduling order. Repeated schedules produce repeated delivery events;
    /// recipient knowledge retains one copy of each record.
    ///
    /// # Errors
    /// Returns `UnknownRecord`, `NotKnown`, `PastTime`, or `SequenceExhausted`
    /// without changing state. Same-time deliveries require `advance_to(time)`.
    pub fn schedule(
        &mut self,
        from: Entity,
        to: Entity,
        record: RecordId,
        due: u64,
    ) -> Result<(), Error> {
        if self.record(record).is_none() {
            return Err(Error::UnknownRecord);
        }
        if !self.knows(from, record) {
            return Err(Error::NotKnown);
        }
        if due < self.time {
            return Err(Error::PastTime);
        }
        let next = self
            .next_delivery
            .checked_add(1)
            .ok_or(Error::SequenceExhausted)?;
        let delivery = Delivery {
            record,
            from,
            to,
            due,
            order: self.next_delivery,
        };
        self.next_delivery = next;
        self.pending.insert((due, delivery.order), delivery.clone());
        self.log(
            Event::Scheduled(delivery),
            vec![record],
            "Delivery queued; recipient knowledge unchanged",
        );
        Ok(())
    }
    /// Queue a delivery once for `input`, matching every argument on retry.
    /// A retry is acknowledged even after arrival or when `due` is in the past.
    /// Initial schedule errors are wrapped in `InputError::Core`.
    pub fn schedule_identified(
        &mut self,
        input: InputId,
        from: Entity,
        to: Entity,
        record: RecordId,
        due: u64,
    ) -> Result<InputReceipt<()>, InputError> {
        if let Some(applied) = self.applied_input(input)? {
            if matches!(applied, AppliedInput::Schedule { from: saved_from, to: saved_to, record: saved_record, due: saved_due }
                if *saved_from == from && *saved_to == to && *saved_record == record && *saved_due == due)
            {
                return Ok(InputReceipt {
                    value: (),
                    duplicate: true,
                });
            }
            return Err(InputError::Conflict);
        }
        self.schedule(from, to, record, due)
            .map_err(InputError::Core)?;
        self.inputs.insert(
            input,
            AppliedInput::Schedule {
                from,
                to,
                record,
                due,
            },
        );
        Ok(InputReceipt {
            value: (),
            duplicate: false,
        })
    }
    /// Drains by (due time, insertion order). Even a jump records each delivery
    /// at its actual due time. Policies run only when the host evaluates them.
    pub fn advance_to(&mut self, time: u64) -> Result<(), Error> {
        if time < self.time {
            return Err(Error::PastTime);
        }
        while self
            .pending
            .first_key_value()
            .is_some_and(|((due, _), _)| *due <= time)
        {
            let (_, delivery) = self.pending.pop_first().expect("queue checked above");
            self.time = delivery.due;
            self.known
                .entry(delivery.to)
                .or_default()
                .insert(delivery.record);
            let record = delivery.record;
            self.log(
                Event::Delivered(delivery),
                vec![record],
                "Scheduled record received",
            );
        }
        self.time = time;
        Ok(())
    }
    /// Policies are ordinary functions over permitted knowledge, not services.
    /// Their output is only a proposal. A cause outside the perspective is rejected.
    pub fn evaluate(
        &mut self,
        entity: Entity,
        policy: impl FnOnce(Perspective<'_>) -> Option<Proposal>,
    ) -> Result<Option<ActionId>, Error> {
        let Some(proposal) = policy(self.perspective(entity)) else {
            return Ok(None);
        };
        if proposal.causes.iter().any(|id| !self.knows(entity, *id)) {
            return Err(Error::NotKnown);
        }
        if proposal.rule.trim().is_empty() || proposal.request.trim().is_empty() {
            return Err(Error::EmptyExplanation);
        }
        let id = ActionId(self.actions.len());
        let causes = proposal.causes.clone();
        let rule = proposal.rule.clone();
        self.actions.push(Action {
            actor: entity,
            proposal,
            status: Status::Proposed,
        });
        self.log(Event::Proposed(id), causes, &rule);
        Ok(Some(id))
    }
    pub fn accept(&mut self, id: ActionId) -> Result<(), Error> {
        let action = self.actions.get_mut(id.0).ok_or(Error::UnknownAction)?;
        if action.status != Status::Proposed {
            return Err(Error::InvalidTransition);
        }
        action.status = Status::Accepted;
        let causes = action.proposal.causes.clone();
        self.log(
            Event::Accepted(id),
            causes,
            "Host accepted; execution not yet confirmed",
        );
        Ok(())
    }
    /// The host may reject a proposal or cancel an accepted, unexecuted action.
    pub fn reject(&mut self, id: ActionId, reason: String) -> Result<(), Error> {
        if reason.trim().is_empty() {
            return Err(Error::EmptyExplanation);
        }
        let action = self.actions.get_mut(id.0).ok_or(Error::UnknownAction)?;
        if !matches!(action.status, Status::Proposed | Status::Accepted) {
            return Err(Error::InvalidTransition);
        }
        action.status = Status::Rejected(reason.clone());
        let causes = action.proposal.causes.clone();
        self.log(Event::Rejected(id), causes, &reason);
        Ok(())
    }
    /// Call only after the host executes the accepted action. Confirmation adds
    /// a record belonging to its actor; others learn it only through delivery.
    pub fn confirm(&mut self, id: ActionId, description: String) -> Result<RecordId, Error> {
        if description.trim().is_empty() {
            return Err(Error::EmptyExplanation);
        }
        let action = self.actions.get(id.0).ok_or(Error::UnknownAction)?;
        if action.status != Status::Accepted {
            return Err(Error::InvalidTransition);
        }
        let source = action.actor;
        let causes = action.proposal.causes.clone();
        let record = self.insert(
            source,
            Content::ConfirmedOutcome {
                description,
                action: id,
            },
        );
        self.actions[id.0].status = Status::Confirmed(record);
        self.log(
            Event::Confirmed { action: id, record },
            causes,
            "Host confirmed execution",
        );
        Ok(record)
    }
    /// Confirm an executed action once for `input`, matching action and exact
    /// description on retry. A fresh ID still requires an accepted action;
    /// initial confirmation errors are wrapped in `InputError::Core`.
    pub fn confirm_identified(
        &mut self,
        input: InputId,
        action: ActionId,
        description: String,
    ) -> Result<InputReceipt<RecordId>, InputError> {
        if let Some(applied) = self.applied_input(input)? {
            if let AppliedInput::Record(id) = applied {
                let record = &self.records[id.0];
                if matches!(&record.content, Content::ConfirmedOutcome { description: saved, action: saved_action }
                    if *saved == description && *saved_action == action)
                {
                    return Ok(InputReceipt {
                        value: *id,
                        duplicate: true,
                    });
                }
            }
            return Err(InputError::Conflict);
        }
        let value = self
            .confirm(action, description)
            .map_err(InputError::Core)?;
        self.inputs.insert(input, AppliedInput::Record(value));
        Ok(InputReceipt {
            value,
            duplicate: false,
        })
    }
}

#[cfg(test)]
mod identified_state_tests {
    use super::*;

    fn unchanged<T>(core: &mut Core, apply: impl FnOnce(&mut Core) -> T) -> T {
        let before = format!("{core:?}");
        let result = apply(core);
        assert_eq!(format!("{core:?}"), before);
        result
    }

    #[test]
    fn record_retries_preserve_all_state_after_trace_eviction() {
        let mut core = Core::with_input_capacity(1, 3);
        let source = Entity(1);
        let observed = core
            .observe_identified(InputId(1), source, "detail".into())
            .unwrap()
            .value;
        let account = core
            .account_identified(InputId(2), source, observed, "summary".into())
            .unwrap()
            .value;
        let action = core
            .evaluate(source, |_| {
                Some(Proposal {
                    request: "act".into(),
                    causes: vec![account],
                    rule: "rule".into(),
                })
            })
            .unwrap()
            .unwrap();
        core.accept(action).unwrap();
        let outcome = core
            .confirm_identified(InputId(3), action, "done".into())
            .unwrap()
            .value;
        core.advance_to(10).unwrap();
        assert!(core.dropped_trace > 0);
        assert_eq!(
            unchanged(&mut core, |core| core.observe_identified(
                InputId(1),
                source,
                "detail".into()
            )),
            Ok(InputReceipt {
                value: observed,
                duplicate: true
            })
        );
        assert_eq!(
            unchanged(&mut core, |core| core.account_identified(
                InputId(2),
                source,
                observed,
                "summary".into()
            )),
            Ok(InputReceipt {
                value: account,
                duplicate: true
            })
        );
        assert_eq!(
            unchanged(&mut core, |core| core.confirm_identified(
                InputId(3),
                action,
                "done".into()
            )),
            Ok(InputReceipt {
                value: outcome,
                duplicate: true
            })
        );
        assert_eq!(
            unchanged(&mut core, |core| core.observe_identified(
                InputId(2),
                source,
                "summary".into()
            )),
            Err(InputError::Conflict)
        );
        assert_eq!(
            unchanged(&mut core, |core| core.confirm_identified(
                InputId(3),
                action,
                "changed".into()
            )),
            Err(InputError::Conflict)
        );
    }

    #[test]
    fn schedule_retries_preserve_queue_and_sequence_before_and_after_arrival() {
        let mut core = Core::with_input_capacity(0, 1);
        let source = Entity(1);
        let target = Entity(2);
        let record = core.observe(source, "detail".into());
        core.schedule_identified(InputId(1), source, target, record, 5)
            .unwrap();
        assert_eq!(
            unchanged(&mut core, |core| core.schedule_identified(
                InputId(1),
                source,
                target,
                record,
                5
            )),
            Ok(InputReceipt {
                value: (),
                duplicate: true
            })
        );
        assert_eq!(
            unchanged(&mut core, |core| core.schedule_identified(
                InputId(1),
                source,
                target,
                record,
                6
            )),
            Err(InputError::Conflict)
        );
        assert_eq!(
            unchanged(&mut core, |core| core.schedule_identified(
                InputId(2),
                source,
                target,
                record,
                5
            )),
            Err(InputError::CapacityReached)
        );
        core.schedule(source, target, record, 5).unwrap();
        assert_eq!(core.next_delivery, 2);
        assert_eq!(
            core.pending.keys().copied().collect::<Vec<_>>(),
            vec![(5, 0), (5, 1)]
        );
        core.advance_to(10).unwrap();
        assert!(core.pending.is_empty());
        assert_eq!(
            unchanged(&mut core, |core| core.schedule_identified(
                InputId(1),
                source,
                target,
                record,
                5
            )),
            Ok(InputReceipt {
                value: (),
                duplicate: true
            })
        );
    }

    #[test]
    fn failed_initial_calls_preserve_state_and_leave_the_id_available() {
        let mut core = Core::with_input_capacity(0, 1);
        let source = Entity(1);
        let record = core.observe(source, "detail".into());
        assert_eq!(
            unchanged(&mut core, |core| core.account_identified(
                InputId(1),
                Entity(2),
                record,
                "summary".into()
            )),
            Err(InputError::Core(Error::NotKnown))
        );
        assert_eq!(
            unchanged(&mut core, |core| core.confirm_identified(
                InputId(1),
                ActionId(0),
                "done".into()
            )),
            Err(InputError::Core(Error::UnknownAction))
        );
        core.next_delivery = usize::MAX;
        assert_eq!(
            unchanged(&mut core, |core| core.schedule_identified(
                InputId(1),
                source,
                Entity(2),
                record,
                0
            )),
            Err(InputError::Core(Error::SequenceExhausted))
        );
        assert!(
            !core
                .observe_identified(InputId(1), source, "new".into())
                .unwrap()
                .duplicate
        );
        assert_eq!(
            unchanged(&mut core, |core| core.confirm_identified(
                InputId(2),
                ActionId(0),
                "done".into()
            )),
            Err(InputError::CapacityReached)
        );
    }
}
