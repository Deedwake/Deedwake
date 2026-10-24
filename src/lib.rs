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
}
impl Core {
    pub fn new(trace_capacity: usize) -> Self {
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
        }
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
}
