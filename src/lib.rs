//! Small host-driven records, delivery, and decisions. No wall clock or I/O.
//!
//! The host owns `Core`; policies receive only a borrowed `Perspective`. IDs are
//! scoped to one Core session. Entity IDs are stable identities supplied by the
//! host; do not reuse them for different entities within a session.
//! Lifecycle registration and guards are opt-in. Legacy APIs remain unmanaged,
//! including recording, delivery, evaluation, acceptance, and confirmation.

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

pub mod snapshot;
pub use snapshot::*;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliveryMode {
    LoadedRecipient,
    AbstractReceipt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliveryRejection {
    Retired,
    Unavailable,
    Incapable,
    Unloaded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliveryState {
    Pending,
    Delivered { at: u64 },
    Rejected { at: u64, reason: DeliveryRejection },
}

/// Privileged host receipt, retained independently of the bounded trace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliveryReceipt {
    pub delivery: Delivery,
    pub mode: DeliveryMode,
    pub state: DeliveryState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliveryError {
    Core(Error),
    Input(InputError),
    UnknownRecipient,
    RecipientRetired,
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

/// Host-owned residency, independent of availability and policy knowledge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Residency {
    Loaded,
    Unloaded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    Available,
    Unavailable,
    Incapable,
}

/// Privileged lifecycle state; retirement permanently reserves this identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityState {
    Present {
        residency: Residency,
        availability: Availability,
    },
    Retired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionMode {
    Direct,
    /// The host must validate abstract execution permissions and targets.
    DelegatedAbstract,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleError {
    Core(Error),
    UnknownEntity,
    AlreadyRegistered,
    CapacityReached,
    Retired,
    Unavailable,
    Incapable,
    Unloaded,
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
    ManagedSchedule(DeliveryReceipt),
}

#[derive(Debug)]
struct QueuedDelivery {
    delivery: Delivery,
    managed_input: Option<InputId>,
}

/// Host-only state. Trace is bounded; records and history are retained for this
/// small session. Hosts must bound session inputs before using this at scale.
#[derive(Debug)]
pub struct Core {
    time: u64,
    records: Vec<Record>,
    known: BTreeMap<Entity, BTreeSet<RecordId>>,
    pending: BTreeMap<(u64, usize), QueuedDelivery>,
    next_delivery: usize,
    actions: Vec<Action>,
    trace: VecDeque<TraceEntry>,
    trace_capacity: usize,
    dropped_trace: u64,
    inputs: BTreeMap<InputId, AppliedInput>,
    input_capacity: usize,
    entities: BTreeMap<Entity, EntityState>,
    entity_capacity: usize,
}
impl Core {
    /// Create a core with identified inputs and lifecycle registration disabled.
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
        Self::with_lifecycle_capacity(trace_capacity, input_capacity, 0)
    }
    /// Bound lifecycle registration independently of trace and identified inputs.
    /// Entries, including retired identities, are never evicted. Legacy methods
    /// remain unmanaged regardless of registration or lifecycle state.
    pub fn with_lifecycle_capacity(
        trace_capacity: usize,
        input_capacity: usize,
        entity_capacity: usize,
    ) -> Self {
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
            entities: BTreeMap::new(),
            entity_capacity,
        }
    }
    /// Explicitly register an identity; legacy activity never registers it.
    /// Duplicates, including retired identities, take priority over capacity.
    pub fn register_entity(
        &mut self,
        entity: Entity,
        residency: Residency,
        availability: Availability,
    ) -> Result<(), LifecycleError> {
        if self.entities.contains_key(&entity) {
            return Err(LifecycleError::AlreadyRegistered);
        }
        if self.entities.len() == self.entity_capacity {
            return Err(LifecycleError::CapacityReached);
        }
        self.entities.insert(
            entity,
            EntityState::Present {
                residency,
                availability,
            },
        );
        Ok(())
    }
    /// Update a registered identity without changing knowledge, history, queued
    /// deliveries, receipts, or action statuses. Retirement cannot be reversed;
    /// repeating it succeeds. The host must explicitly reconcile pending actions.
    pub fn set_entity_state(
        &mut self,
        entity: Entity,
        state: EntityState,
    ) -> Result<(), LifecycleError> {
        let current = self
            .entities
            .get_mut(&entity)
            .ok_or(LifecycleError::UnknownEntity)?;
        if *current == EntityState::Retired && state != EntityState::Retired {
            return Err(LifecycleError::Retired);
        }
        *current = state;
        Ok(())
    }
    /// Privileged host inspection; lifecycle state is absent from perspectives.
    pub fn entity_state(&self, entity: Entity) -> Option<EntityState> {
        self.entities.get(&entity).copied()
    }
    fn available_residency(&self, entity: Entity) -> Result<Residency, LifecycleError> {
        match self.entity_state(entity) {
            None => Err(LifecycleError::UnknownEntity),
            Some(EntityState::Retired) => Err(LifecycleError::Retired),
            Some(EntityState::Present {
                availability: Availability::Unavailable,
                ..
            }) => Err(LifecycleError::Unavailable),
            Some(EntityState::Present {
                availability: Availability::Incapable,
                ..
            }) => Err(LifecycleError::Incapable),
            Some(EntityState::Present {
                residency,
                availability: Availability::Available,
            }) => Ok(residency),
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
        self.pending.insert(
            (due, delivery.order),
            QueuedDelivery {
                delivery: delivery.clone(),
                managed_input: None,
            },
        );
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
    /// Queue one lifecycle-managed delivery using the shared input ledger.
    /// The recipient must be registered and not retired; its eligibility is
    /// checked again at the due time. Rejection is terminal and grants no knowledge.
    /// Exact retries preserve the original acknowledgement and receipt state,
    /// including after arrival, rejection, or lifecycle changes. Accepted IDs
    /// and their receipts are never evicted and consume the input capacity.
    pub fn schedule_managed_identified(
        &mut self,
        input: InputId,
        from: Entity,
        to: Entity,
        record: RecordId,
        due: u64,
        mode: DeliveryMode,
    ) -> Result<InputReceipt<()>, DeliveryError> {
        if let Some(applied) = self.applied_input(input).map_err(DeliveryError::Input)? {
            if let AppliedInput::ManagedSchedule(receipt) = applied {
                let delivery = &receipt.delivery;
                if delivery.from == from
                    && delivery.to == to
                    && delivery.record == record
                    && delivery.due == due
                    && receipt.mode == mode
                {
                    return Ok(InputReceipt {
                        value: (),
                        duplicate: true,
                    });
                }
            }
            return Err(DeliveryError::Input(InputError::Conflict));
        }
        match self.entity_state(to) {
            None => return Err(DeliveryError::UnknownRecipient),
            Some(EntityState::Retired) => return Err(DeliveryError::RecipientRetired),
            Some(EntityState::Present { .. }) => {}
        }
        let order = self.next_delivery;
        self.schedule(from, to, record, due)
            .map_err(DeliveryError::Core)?;
        let queued = self
            .pending
            .get_mut(&(due, order))
            .expect("successful schedule inserted the delivery");
        queued.managed_input = Some(input);
        self.inputs.insert(
            input,
            AppliedInput::ManagedSchedule(DeliveryReceipt {
                delivery: queued.delivery.clone(),
                mode,
                state: DeliveryState::Pending,
            }),
        );
        Ok(InputReceipt {
            value: (),
            duplicate: false,
        })
    }
    /// Privileged host inspection; receipts are absent from policy perspectives.
    /// Unknown IDs and IDs accepted for other operations have no delivery receipt.
    pub fn delivery_receipt(&self, input: InputId) -> Option<&DeliveryReceipt> {
        match self.inputs.get(&input) {
            Some(AppliedInput::ManagedSchedule(receipt)) => Some(receipt),
            _ => None,
        }
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
            let (_, queued) = self.pending.pop_first().expect("queue checked above");
            let delivery = queued.delivery;
            self.time = delivery.due;
            if let Some(input) = queued.managed_input {
                let Some(AppliedInput::ManagedSchedule(receipt)) = self.inputs.get_mut(&input)
                else {
                    unreachable!("managed queue entries retain their accepted input");
                };
                let rejection = match self
                    .entities
                    .get(&delivery.to)
                    .expect("managed recipients stay registered, including after retirement")
                {
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
                    } if receipt.mode == DeliveryMode::LoadedRecipient => {
                        Some(DeliveryRejection::Unloaded)
                    }
                    EntityState::Present { .. } => None,
                };
                receipt.state = match rejection {
                    Some(reason) => DeliveryState::Rejected {
                        at: delivery.due,
                        reason,
                    },
                    None => DeliveryState::Delivered { at: delivery.due },
                };
                if rejection.is_some() {
                    continue;
                }
            }
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
    /// This legacy method does not enforce lifecycle eligibility.
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
    /// Require a registered, available actor before invoking the policy, then
    /// apply normal proposal validation. Unloaded available actors may plan.
    pub fn evaluate_guarded(
        &mut self,
        entity: Entity,
        policy: impl FnOnce(Perspective<'_>) -> Option<Proposal>,
    ) -> Result<Option<ActionId>, LifecycleError> {
        self.available_residency(entity)?;
        self.evaluate(entity, policy).map_err(LifecycleError::Core)
    }
    /// Accept a proposal without lifecycle checks (legacy unmanaged API).
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
    /// Look up the action, require its actor to be registered and available,
    /// then apply the normal Proposed-to-Accepted transition.
    pub fn accept_guarded(&mut self, id: ActionId) -> Result<(), LifecycleError> {
        let actor = self
            .action(id)
            .ok_or(LifecycleError::Core(Error::UnknownAction))?
            .actor;
        self.available_residency(actor)?;
        self.accept(id).map_err(LifecycleError::Core)
    }
    /// Read-only check: require an existing Accepted action, then a registered,
    /// available actor. Direct execution additionally requires Loaded residency.
    /// DelegatedAbstract permits unloaded actors; the host validates permissions
    /// and all domain targets, which free-form requests cannot encode for Core.
    ///
    /// The host must serialize this check and domain validation with execution
    /// and confirmation. This check is no reservation or external transaction.
    pub fn check_execution(&self, id: ActionId, mode: ExecutionMode) -> Result<(), LifecycleError> {
        let action = self
            .action(id)
            .ok_or(LifecycleError::Core(Error::UnknownAction))?;
        if action.status != Status::Accepted {
            return Err(LifecycleError::Core(Error::InvalidTransition));
        }
        let residency = self.available_residency(action.actor)?;
        if mode == ExecutionMode::Direct && residency == Residency::Unloaded {
            return Err(LifecycleError::Unloaded);
        }
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
    /// This factual acknowledgement remains valid after lifecycle changes.
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
mod managed_delivery_state_tests {
    use super::*;

    fn unchanged<T>(core: &mut Core, apply: impl FnOnce(&mut Core) -> T) -> T {
        let before = format!("{core:?}");
        let result = apply(core);
        assert_eq!(format!("{core:?}"), before);
        result
    }

    #[test]
    fn failed_admission_preserves_all_state_and_does_not_reserve_input() {
        let mut core = Core::with_lifecycle_capacity(0, 1, 2);
        let source = Entity(1);
        let target = Entity(2);
        let retired = Entity(3);
        for entity in [target, retired] {
            core.register_entity(entity, Residency::Loaded, Availability::Available)
                .unwrap();
        }
        core.set_entity_state(retired, EntityState::Retired)
            .unwrap();
        let record = core.observe(source, "detail".into());
        core.schedule(source, target, record, 20).unwrap();
        core.advance_to(10).unwrap();
        core.next_delivery = usize::MAX;
        for (from, to, record, due, error) in [
            (
                source,
                Entity(4),
                record,
                10,
                DeliveryError::UnknownRecipient,
            ),
            (source, retired, record, 10, DeliveryError::RecipientRetired),
            (
                source,
                target,
                RecordId(99),
                10,
                DeliveryError::Core(Error::UnknownRecord),
            ),
            (
                Entity(4),
                target,
                record,
                10,
                DeliveryError::Core(Error::NotKnown),
            ),
            (
                source,
                target,
                record,
                9,
                DeliveryError::Core(Error::PastTime),
            ),
            (
                source,
                target,
                record,
                10,
                DeliveryError::Core(Error::SequenceExhausted),
            ),
        ] {
            assert_eq!(
                unchanged(&mut core, |core| core.schedule_managed_identified(
                    InputId(1),
                    from,
                    to,
                    record,
                    due,
                    DeliveryMode::LoadedRecipient
                )),
                Err(error)
            );
        }
        core.next_delivery = 1;
        assert_eq!(
            core.schedule_managed_identified(
                InputId(1),
                source,
                target,
                record,
                20,
                DeliveryMode::LoadedRecipient
            ),
            Ok(InputReceipt {
                value: (),
                duplicate: false
            })
        );
        assert_eq!(core.next_delivery, 2);
        assert_eq!(core.pending[&(20, 0)].managed_input, None);
        assert_eq!(core.pending[&(20, 1)].managed_input, Some(InputId(1)));
        assert_eq!(
            unchanged(&mut core, |core| core.schedule_managed_identified(
                InputId(2),
                Entity(4),
                retired,
                RecordId(99),
                0,
                DeliveryMode::AbstractReceipt
            )),
            Err(DeliveryError::Input(InputError::CapacityReached))
        );
    }

    #[test]
    fn exact_retries_preserve_all_state_before_and_after_terminal_receipts() {
        let mut core = Core::with_lifecycle_capacity(0, 2, 2);
        let source = Entity(1);
        let record = core.observe(source, "detail".into());
        let requests = [
            (InputId(1), Entity(2), DeliveryMode::LoadedRecipient),
            (InputId(2), Entity(3), DeliveryMode::AbstractReceipt),
        ];
        for (input, target, mode) in requests {
            core.register_entity(target, Residency::Unloaded, Availability::Available)
                .unwrap();
            core.schedule_managed_identified(input, source, target, record, 5, mode)
                .unwrap();
        }
        for terminal in [false, true] {
            if terminal {
                core.advance_to(10).unwrap();
                assert_eq!(
                    core.delivery_receipt(InputId(1)).unwrap().state,
                    DeliveryState::Rejected {
                        at: 5,
                        reason: DeliveryRejection::Unloaded
                    }
                );
                assert_eq!(
                    core.delivery_receipt(InputId(2)).unwrap().state,
                    DeliveryState::Delivered { at: 5 }
                );
                assert!(core.perspective(Entity(2)).records().is_empty());
                assert_eq!(core.perspective(Entity(3)).records()[0].id, record);
                assert!(core.pending.is_empty());
                assert_eq!(core.dropped_trace, 4);
                for (_, target, _) in requests {
                    core.set_entity_state(target, EntityState::Retired).unwrap();
                }
            }
            core.next_delivery = usize::MAX;
            for (input, target, mode) in requests {
                assert_eq!(
                    unchanged(&mut core, |core| core.schedule_managed_identified(
                        input, source, target, record, 5, mode
                    )),
                    Ok(InputReceipt {
                        value: (),
                        duplicate: true
                    })
                );
                assert_eq!(
                    unchanged(&mut core, |core| core.schedule_managed_identified(
                        input, source, target, record, 6, mode
                    )),
                    Err(DeliveryError::Input(InputError::Conflict))
                );
            }
        }
        assert_eq!(core.inputs.len(), 2);
        assert_eq!(core.next_delivery, usize::MAX);
        assert!(core.trace.is_empty());
    }

    #[test]
    fn managed_ids_conflict_with_every_other_identified_operation_without_effects() {
        let mut core = Core::with_lifecycle_capacity(0, 1, 1);
        let source = Entity(1);
        let target = Entity(2);
        let input = InputId(1);
        let record = core.observe(source, "detail".into());
        core.register_entity(target, Residency::Loaded, Availability::Available)
            .unwrap();
        core.schedule_managed_identified(
            input,
            source,
            target,
            record,
            0,
            DeliveryMode::LoadedRecipient,
        )
        .unwrap();
        assert_eq!(
            unchanged(&mut core, |core| core.observe_identified(
                input,
                source,
                "detail".into()
            )),
            Err(InputError::Conflict)
        );
        assert_eq!(
            unchanged(&mut core, |core| core.account_identified(
                input,
                source,
                record,
                "detail".into()
            )),
            Err(InputError::Conflict)
        );
        assert_eq!(
            unchanged(&mut core, |core| core
                .schedule_identified(input, source, target, record, 0)),
            Err(InputError::Conflict)
        );
        assert_eq!(
            unchanged(&mut core, |core| core.confirm_identified(
                input,
                ActionId(0),
                "detail".into()
            )),
            Err(InputError::Conflict)
        );
        assert_eq!(
            unchanged(&mut core, |core| core.schedule_managed_identified(
                input,
                source,
                target,
                record,
                0,
                DeliveryMode::AbstractReceipt
            )),
            Err(DeliveryError::Input(InputError::Conflict))
        );
    }
}

#[cfg(test)]
mod lifecycle_state_tests {
    use super::*;

    fn unchanged<T>(core: &mut Core, apply: impl FnOnce(&mut Core) -> T) -> T {
        let before = format!("{core:?}");
        let result = apply(core);
        assert_eq!(format!("{core:?}"), before);
        result
    }

    fn proposal() -> Proposal {
        Proposal {
            request: "act".into(),
            causes: vec![],
            rule: "rule".into(),
        }
    }

    #[test]
    fn registration_and_transition_failures_preserve_complete_state() {
        let actor = Entity(1);
        let mut core = Core::with_lifecycle_capacity(4, 1, 1);
        assert_eq!(
            unchanged(&mut core, |core| core
                .set_entity_state(actor, EntityState::Retired)),
            Err(LifecycleError::UnknownEntity)
        );
        assert_eq!(
            unchanged(&mut core, |core| core
                .evaluate_guarded(actor, |_| panic!("ineligible policy"))),
            Err(LifecycleError::UnknownEntity)
        );
        core.register_entity(actor, Residency::Loaded, Availability::Available)
            .unwrap();
        core.set_entity_state(actor, EntityState::Retired).unwrap();
        for entity in [actor, Entity(2)] {
            assert_eq!(
                unchanged(&mut core, |core| core.register_entity(
                    entity,
                    Residency::Loaded,
                    Availability::Available
                )),
                Err(if entity == actor {
                    LifecycleError::AlreadyRegistered
                } else {
                    LifecycleError::CapacityReached
                })
            );
        }
        assert_eq!(
            unchanged(&mut core, |core| core.set_entity_state(
                actor,
                EntityState::Present {
                    residency: Residency::Loaded,
                    availability: Availability::Available
                }
            )),
            Err(LifecycleError::Retired)
        );
        assert_eq!(
            unchanged(&mut core, |core| core
                .set_entity_state(actor, EntityState::Retired)),
            Ok(())
        );
    }

    #[test]
    fn guarded_failures_and_execution_checks_preserve_complete_state() {
        let actor = Entity(1);
        let mut core = Core::with_lifecycle_capacity(4, 0, 1);
        let action = core.evaluate(actor, |_| Some(proposal())).unwrap().unwrap();
        assert_eq!(
            unchanged(&mut core, |core| core.accept_guarded(ActionId(99))),
            Err(LifecycleError::Core(Error::UnknownAction))
        );
        assert_eq!(
            unchanged(&mut core, |core| core.accept_guarded(action)),
            Err(LifecycleError::UnknownEntity)
        );
        core.register_entity(actor, Residency::Loaded, Availability::Available)
            .unwrap();
        assert_eq!(
            unchanged(&mut core, |core| core.evaluate_guarded(actor, |_| {
                let mut invalid = proposal();
                invalid.causes.push(RecordId(99));
                Some(invalid)
            })),
            Err(LifecycleError::Core(Error::NotKnown))
        );
        assert_eq!(
            unchanged(&mut core, |core| core.evaluate_guarded(actor, |_| {
                let mut invalid = proposal();
                invalid.rule.clear();
                Some(invalid)
            })),
            Err(LifecycleError::Core(Error::EmptyExplanation))
        );
        assert_eq!(
            unchanged(&mut core, |core| core
                .check_execution(action, ExecutionMode::Direct)),
            Err(LifecycleError::Core(Error::InvalidTransition))
        );
        core.accept_guarded(action).unwrap();
        assert_eq!(
            unchanged(&mut core, |core| core.accept_guarded(action)),
            Err(LifecycleError::Core(Error::InvalidTransition))
        );
        for residency in [Residency::Loaded, Residency::Unloaded] {
            for availability in [
                Availability::Available,
                Availability::Unavailable,
                Availability::Incapable,
            ] {
                core.set_entity_state(
                    actor,
                    EntityState::Present {
                        residency,
                        availability,
                    },
                )
                .unwrap();
                let eligibility = match availability {
                    Availability::Available => Ok(()),
                    Availability::Unavailable => Err(LifecycleError::Unavailable),
                    Availability::Incapable => Err(LifecycleError::Incapable),
                };
                assert_eq!(
                    unchanged(&mut core, |core| core.evaluate_guarded(actor, |_| {
                        assert_eq!(availability, Availability::Available);
                        None
                    })),
                    eligibility.map(|()| None)
                );
                for mode in [ExecutionMode::Direct, ExecutionMode::DelegatedAbstract] {
                    let expected = eligibility.and_then(|()| {
                        if mode == ExecutionMode::Direct && residency == Residency::Unloaded {
                            Err(LifecycleError::Unloaded)
                        } else {
                            Ok(())
                        }
                    });
                    assert_eq!(
                        unchanged(&mut core, |core| core.check_execution(action, mode)),
                        expected
                    );
                }
                if availability != Availability::Available {
                    assert_eq!(
                        unchanged(&mut core, |core| core.accept_guarded(action)),
                        eligibility
                    );
                }
            }
        }
        core.set_entity_state(actor, EntityState::Retired).unwrap();
        assert_eq!(
            unchanged(&mut core, |core| core
                .evaluate_guarded(actor, |_| panic!("retired policy"))),
            Err(LifecycleError::Retired)
        );
        assert_eq!(
            unchanged(&mut core, |core| core.accept_guarded(action)),
            Err(LifecycleError::Retired)
        );
        assert_eq!(
            unchanged(&mut core, |core| core
                .check_execution(action, ExecutionMode::DelegatedAbstract)),
            Err(LifecycleError::Retired)
        );
        assert_eq!(
            unchanged(&mut core, |core| core
                .check_execution(ActionId(99), ExecutionMode::Direct)),
            Err(LifecycleError::Core(Error::UnknownAction))
        );
    }

    #[test]
    fn lifecycle_updates_preserve_history_queue_knowledge_and_receipts() {
        let actor = Entity(1);
        let mut core = Core::with_lifecycle_capacity(1, 3, 1);
        core.register_entity(actor, Residency::Loaded, Availability::Available)
            .unwrap();
        let observed = core
            .observe_identified(InputId(1), actor, "detail".into())
            .unwrap()
            .value;
        core.schedule_identified(InputId(2), actor, Entity(2), observed, 5)
            .unwrap();
        let completed = core
            .evaluate_guarded(actor, |_| Some(proposal()))
            .unwrap()
            .unwrap();
        core.accept_guarded(completed).unwrap();
        let receipt = core
            .confirm_identified(InputId(3), completed, "done".into())
            .unwrap();
        let proposed = core
            .evaluate_guarded(actor, |_| Some(proposal()))
            .unwrap()
            .unwrap();
        let accepted = core
            .evaluate_guarded(actor, |_| Some(proposal()))
            .unwrap()
            .unwrap();
        core.accept_guarded(accepted).unwrap();
        for state in [
            EntityState::Present {
                residency: Residency::Unloaded,
                availability: Availability::Available,
            },
            EntityState::Present {
                residency: Residency::Loaded,
                availability: Availability::Unavailable,
            },
            EntityState::Present {
                residency: Residency::Unloaded,
                availability: Availability::Incapable,
            },
            EntityState::Present {
                residency: Residency::Loaded,
                availability: Availability::Available,
            },
            EntityState::Retired,
        ] {
            // Construct the expected full state by changing only the registry entry.
            let previous = core.entities.insert(actor, state).unwrap();
            let expected = format!("{core:?}");
            core.entities.insert(actor, previous);
            core.set_entity_state(actor, state).unwrap();
            assert_eq!(format!("{core:?}"), expected);
            assert_eq!(core.action(proposed).unwrap().status, Status::Proposed);
            assert_eq!(core.action(accepted).unwrap().status, Status::Accepted);
            assert_eq!(
                unchanged(&mut core, |core| core.confirm_identified(
                    InputId(3),
                    completed,
                    "done".into()
                )),
                Ok(InputReceipt {
                    value: receipt.value,
                    duplicate: true
                })
            );
        }
        // Acknowledging work already completed remains factual after retirement.
        core.confirm(accepted, "already executed".into()).unwrap();
        core.advance_to(5).unwrap();
        assert_eq!(core.perspective(Entity(2)).records()[0].id, observed);
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
