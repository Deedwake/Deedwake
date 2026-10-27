//! Versioned, privileged host snapshots. These DTOs expose all Core state,
//! including knowledge hidden from perspectives. Validation proves structural
//! consistency, not authenticity, provenance, or that execution really occurred.
//! Only DTOs support optional Serde; runtime `Core` is never deserialized.
use super::*;
use std::fmt;

/// Current portable snapshot format.
pub const SNAPSHOT_VERSION: u32 = 1;

/// Resource bounds applied before export clones data and before restore builds
/// a candidate. Hosts must separately bound encoded input bytes before decoding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotLimits {
    pub max_records: usize,
    pub max_pending: usize,
    pub max_inputs: usize,
    pub max_actions: usize,
    pub max_entities: usize,
    pub max_trace_capacity: usize,
    pub max_known_owners: usize,
    /// Aggregate record references in knowledge, proposal causes, and trace causes.
    pub max_known_edges: usize,
    pub max_string_bytes: usize,
    pub max_total_string_bytes: usize,
}
impl Default for SnapshotLimits {
    fn default() -> Self {
        Self {
            max_records: 16_384,
            max_pending: 16_384,
            max_inputs: 16_384,
            max_actions: 8_192,
            max_entities: 4_096,
            max_trace_capacity: 4_096,
            max_known_owners: 16_384,
            max_known_edges: 131_072,
            max_string_bytes: 64 * 1024,
            max_total_string_bytes: 4 * 1024 * 1024,
        }
    }
}

/// A rejected snapshot never changes an existing Core.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnapshotError {
    UnsupportedVersion(u32),
    LimitExceeded(&'static str),
    Invalid(&'static str),
    PositionOverflow,
}
impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion(v) => write!(f, "unsupported snapshot version {v}"),
            Self::LimitExceeded(field) => write!(f, "snapshot limit exceeded: {field}"),
            Self::Invalid(reason) => write!(f, "invalid snapshot: {reason}"),
            Self::PositionOverflow => f.write_str("snapshot position does not fit this platform"),
        }
    }
}
impl std::error::Error for SnapshotError {}

macro_rules! dto {
    ($($item:item)*) => {$ (
        #[doc = "Portable version-1 host snapshot data; references are zero-based u64 positions."]
        #[derive(Clone, Debug, PartialEq, Eq)]
        #[cfg_attr(feature = "snapshot-serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(feature = "snapshot-serde", serde(deny_unknown_fields))]
        $item
    )*};
}
dto! {
    pub struct CoreSnapshot {
        pub version: u32,
        pub time: u64,
        pub records: Vec<SnapshotRecord>,
        pub known: Vec<SnapshotKnowledge>,
        pub pending: Vec<SnapshotQueuedDelivery>,
        pub next_delivery: u64,
        pub actions: Vec<SnapshotAction>,
        pub trace: Vec<SnapshotTraceEntry>,
        pub trace_capacity: u64,
        pub dropped_trace: u64,
        pub inputs: Vec<SnapshotInput>,
        pub input_capacity: u64,
        pub entities: Vec<SnapshotEntity>,
        pub entity_capacity: u64,
    }
    pub struct SnapshotRecord { pub id: u64, pub source: u64, pub time: u64, pub content: SnapshotContent }
    pub enum SnapshotContent {
        Observation(String),
        Account { summary: String, basis: u64 },
        ConfirmedOutcome { description: String, action: u64 },
    }
    pub struct SnapshotKnowledge { pub entity: u64, pub records: Vec<u64> }
    pub struct SnapshotDelivery { pub record: u64, pub from: u64, pub to: u64, pub due: u64, pub order: u64 }
    pub struct SnapshotQueuedDelivery { pub delivery: SnapshotDelivery, pub managed_input: Option<u64> }
    pub struct SnapshotProposal { pub request: String, pub causes: Vec<u64>, pub rule: String }
    pub struct SnapshotAction { pub actor: u64, pub proposal: SnapshotProposal, pub status: SnapshotStatus }
    pub enum SnapshotStatus { Proposed, Accepted, Rejected(String), Confirmed(u64) }
    pub struct SnapshotTraceEntry { pub time: u64, pub event: SnapshotEvent, pub causes: Vec<u64>, pub rule: String }
    pub enum SnapshotEvent {
        Recorded(u64), Scheduled(SnapshotDelivery), Delivered(SnapshotDelivery),
        Proposed(u64), Accepted(u64), Rejected(u64), Confirmed { action: u64, record: u64 },
    }
    pub struct SnapshotInput { pub id: u64, pub applied: SnapshotAppliedInput }
    pub enum SnapshotAppliedInput {
        Record(u64),
        Schedule { from: u64, to: u64, record: u64, due: u64 },
        ManagedSchedule(SnapshotDeliveryReceipt),
    }
    pub struct SnapshotDeliveryReceipt { pub delivery: SnapshotDelivery, pub mode: SnapshotDeliveryMode, pub state: SnapshotDeliveryState }
    pub enum SnapshotDeliveryMode { LoadedRecipient, AbstractReceipt }
    pub enum SnapshotDeliveryState { Pending, Delivered { at: u64 }, Rejected { at: u64, reason: SnapshotDeliveryRejection } }
    pub enum SnapshotDeliveryRejection { Retired, Unavailable, Incapable, Unloaded }
    pub struct SnapshotEntity { pub entity: u64, pub state: SnapshotEntityState }
    pub enum SnapshotEntityState { Present { residency: SnapshotResidency, availability: SnapshotAvailability }, Retired }
    pub enum SnapshotResidency { Loaded, Unloaded }
    pub enum SnapshotAvailability { Available, Unavailable, Incapable }
}

fn invalid<T>(reason: &'static str) -> Result<T, SnapshotError> {
    Err(SnapshotError::Invalid(reason))
}
fn position(value: u64) -> Result<usize, SnapshotError> {
    usize::try_from(value).map_err(|_| SnapshotError::PositionOverflow)
}
fn wire(value: usize) -> Result<u64, SnapshotError> {
    u64::try_from(value).map_err(|_| SnapshotError::PositionOverflow)
}
fn bounded(value: usize, limit: usize, field: &'static str) -> Result<(), SnapshotError> {
    if value > limit {
        Err(SnapshotError::LimitExceeded(field))
    } else {
        Ok(())
    }
}
fn ordered<T: Ord>(values: impl IntoIterator<Item = T>) -> bool {
    let mut last = None;
    for value in values {
        if last.as_ref().is_some_and(|last| last >= &value) {
            return false;
        }
        last = Some(value);
    }
    true
}
struct Budget<'a> {
    limits: &'a SnapshotLimits,
    strings: usize,
    edges: usize,
}
impl<'a> Budget<'a> {
    fn new(limits: &'a SnapshotLimits) -> Self {
        Self {
            limits,
            strings: 0,
            edges: 0,
        }
    }
    fn string(&mut self, text: &str) -> Result<(), SnapshotError> {
        bounded(text.len(), self.limits.max_string_bytes, "string bytes")?;
        self.strings = self
            .strings
            .checked_add(text.len())
            .ok_or(SnapshotError::LimitExceeded("total string bytes"))?;
        bounded(
            self.strings,
            self.limits.max_total_string_bytes,
            "total string bytes",
        )
    }
    fn edges(&mut self, count: usize) -> Result<(), SnapshotError> {
        self.edges = self
            .edges
            .checked_add(count)
            .ok_or(SnapshotError::LimitExceeded("reference edges"))?;
        bounded(self.edges, self.limits.max_known_edges, "reference edges")
    }
}

impl Core {
    /// Resolve a snapshot position against this Core, without manufacturing an
    /// unchecked handle. Equal positions in different sessions remain aliases.
    pub fn record_id(&self, value: u64) -> Result<RecordId, SnapshotError> {
        let id = RecordId(position(value)?);
        self.record(id)
            .map(|_| id)
            .ok_or(SnapshotError::Invalid("unknown record position"))
    }
    /// Resolve an action position against this Core.
    pub fn action_id(&self, value: u64) -> Result<ActionId, SnapshotError> {
        let id = ActionId(position(value)?);
        self.action(id)
            .map(|_| id)
            .ok_or(SnapshotError::Invalid("unknown action position"))
    }
    /// Export an existing record handle as a portable position.
    pub fn record_position(&self, id: RecordId) -> Result<u64, SnapshotError> {
        self.record(id)
            .ok_or(SnapshotError::Invalid("unknown record handle"))?;
        wire(id.0)
    }
    /// Export an existing action handle as a portable position.
    pub fn action_position(&self, id: ActionId) -> Result<u64, SnapshotError> {
        self.action(id)
            .ok_or(SnapshotError::Invalid("unknown action handle"))?;
        wire(id.0)
    }
    fn snapshot_preflight(&self, limits: &SnapshotLimits) -> Result<(), SnapshotError> {
        bounded(self.records.len(), limits.max_records, "records")?;
        bounded(self.pending.len(), limits.max_pending, "pending")?;
        bounded(self.actions.len(), limits.max_actions, "actions")?;
        bounded(
            self.trace_capacity,
            limits.max_trace_capacity,
            "trace capacity",
        )?;
        bounded(self.trace.len(), limits.max_trace_capacity, "trace")?;
        bounded(self.input_capacity, limits.max_inputs, "input capacity")?;
        bounded(self.inputs.len(), limits.max_inputs, "inputs")?;
        bounded(self.entity_capacity, limits.max_entities, "entity capacity")?;
        bounded(self.entities.len(), limits.max_entities, "entities")?;
        bounded(self.known.len(), limits.max_known_owners, "known owners")?;
        let mut budget = Budget::new(limits);
        for ids in self.known.values() {
            budget.edges(ids.len())?;
        }
        for record in &self.records {
            match &record.content {
                Content::Observation(s) => budget.string(s)?,
                Content::Account { summary, .. } => budget.string(summary)?,
                Content::ConfirmedOutcome { description, .. } => budget.string(description)?,
            }
        }
        for action in &self.actions {
            budget.string(&action.proposal.request)?;
            budget.string(&action.proposal.rule)?;
            budget.edges(action.proposal.causes.len())?;
            if let Status::Rejected(s) = &action.status {
                budget.string(s)?;
            }
        }
        for entry in &self.trace {
            budget.string(&entry.rule)?;
            budget.edges(entry.causes.len())?;
        }
        Ok(())
    }
    /// Capture all privileged state. Bounds are checked before any DTO data is
    /// cloned. This is a host checkpoint, never a policy perspective or proof.
    pub fn snapshot(&self, limits: &SnapshotLimits) -> Result<CoreSnapshot, SnapshotError> {
        self.snapshot_preflight(limits)?;
        Ok(CoreSnapshot {
            version: SNAPSHOT_VERSION,
            time: self.time,
            records: self
                .records
                .iter()
                .map(|r| SnapshotRecord {
                    id: r.id.0 as u64,
                    source: r.source.0,
                    time: r.time,
                    content: match &r.content {
                        Content::Observation(s) => SnapshotContent::Observation(s.clone()),
                        Content::Account { summary, basis } => SnapshotContent::Account {
                            summary: summary.clone(),
                            basis: basis.0 as u64,
                        },
                        Content::ConfirmedOutcome {
                            description,
                            action,
                        } => SnapshotContent::ConfirmedOutcome {
                            description: description.clone(),
                            action: action.0 as u64,
                        },
                    },
                })
                .collect(),
            known: self
                .known
                .iter()
                .map(|(e, ids)| SnapshotKnowledge {
                    entity: e.0,
                    records: ids.iter().map(|id| id.0 as u64).collect(),
                })
                .collect(),
            pending: self
                .pending
                .values()
                .map(|q| SnapshotQueuedDelivery {
                    delivery: export_delivery(&q.delivery),
                    managed_input: q.managed_input.map(|i| i.0),
                })
                .collect(),
            next_delivery: wire(self.next_delivery)?,
            actions: self
                .actions
                .iter()
                .map(|a| SnapshotAction {
                    actor: a.actor.0,
                    proposal: SnapshotProposal {
                        request: a.proposal.request.clone(),
                        causes: a.proposal.causes.iter().map(|id| id.0 as u64).collect(),
                        rule: a.proposal.rule.clone(),
                    },
                    status: match &a.status {
                        Status::Proposed => SnapshotStatus::Proposed,
                        Status::Accepted => SnapshotStatus::Accepted,
                        Status::Rejected(s) => SnapshotStatus::Rejected(s.clone()),
                        Status::Confirmed(r) => SnapshotStatus::Confirmed(r.0 as u64),
                    },
                })
                .collect(),
            trace: self
                .trace
                .iter()
                .map(|t| SnapshotTraceEntry {
                    time: t.time,
                    causes: t.causes.iter().map(|r| r.0 as u64).collect(),
                    rule: t.rule.clone(),
                    event: match &t.event {
                        Event::Recorded(r) => SnapshotEvent::Recorded(r.0 as u64),
                        Event::Scheduled(d) => SnapshotEvent::Scheduled(export_delivery(d)),
                        Event::Delivered(d) => SnapshotEvent::Delivered(export_delivery(d)),
                        Event::Proposed(a) => SnapshotEvent::Proposed(a.0 as u64),
                        Event::Accepted(a) => SnapshotEvent::Accepted(a.0 as u64),
                        Event::Rejected(a) => SnapshotEvent::Rejected(a.0 as u64),
                        Event::Confirmed { action, record } => SnapshotEvent::Confirmed {
                            action: action.0 as u64,
                            record: record.0 as u64,
                        },
                    },
                })
                .collect(),
            trace_capacity: wire(self.trace_capacity)?,
            dropped_trace: self.dropped_trace,
            inputs: self
                .inputs
                .iter()
                .map(|(id, a)| SnapshotInput {
                    id: id.0,
                    applied: match a {
                        AppliedInput::Record(r) => SnapshotAppliedInput::Record(r.0 as u64),
                        AppliedInput::Schedule {
                            from,
                            to,
                            record,
                            due,
                        } => SnapshotAppliedInput::Schedule {
                            from: from.0,
                            to: to.0,
                            record: record.0 as u64,
                            due: *due,
                        },
                        AppliedInput::ManagedSchedule(r) => {
                            SnapshotAppliedInput::ManagedSchedule(SnapshotDeliveryReceipt {
                                delivery: export_delivery(&r.delivery),
                                mode: match r.mode {
                                    DeliveryMode::LoadedRecipient => {
                                        SnapshotDeliveryMode::LoadedRecipient
                                    }
                                    DeliveryMode::AbstractReceipt => {
                                        SnapshotDeliveryMode::AbstractReceipt
                                    }
                                },
                                state: match r.state {
                                    DeliveryState::Pending => SnapshotDeliveryState::Pending,
                                    DeliveryState::Delivered { at } => {
                                        SnapshotDeliveryState::Delivered { at }
                                    }
                                    DeliveryState::Rejected { at, reason } => {
                                        SnapshotDeliveryState::Rejected {
                                            at,
                                            reason: match reason {
                                                DeliveryRejection::Retired => {
                                                    SnapshotDeliveryRejection::Retired
                                                }
                                                DeliveryRejection::Unavailable => {
                                                    SnapshotDeliveryRejection::Unavailable
                                                }
                                                DeliveryRejection::Incapable => {
                                                    SnapshotDeliveryRejection::Incapable
                                                }
                                                DeliveryRejection::Unloaded => {
                                                    SnapshotDeliveryRejection::Unloaded
                                                }
                                            },
                                        }
                                    }
                                },
                            })
                        }
                    },
                })
                .collect(),
            input_capacity: wire(self.input_capacity)?,
            entities: self
                .entities
                .iter()
                .map(|(e, state)| SnapshotEntity {
                    entity: e.0,
                    state: match state {
                        EntityState::Retired => SnapshotEntityState::Retired,
                        EntityState::Present {
                            residency,
                            availability,
                        } => SnapshotEntityState::Present {
                            residency: match residency {
                                Residency::Loaded => SnapshotResidency::Loaded,
                                Residency::Unloaded => SnapshotResidency::Unloaded,
                            },
                            availability: match availability {
                                Availability::Available => SnapshotAvailability::Available,
                                Availability::Unavailable => SnapshotAvailability::Unavailable,
                                Availability::Incapable => SnapshotAvailability::Incapable,
                            },
                        },
                    },
                })
                .collect(),
            entity_capacity: wire(self.entity_capacity)?,
        })
    }
    /// Validate an owned DTO and build an isolated candidate. The caller may
    /// replace its live Core only after this succeeds (and host validation passes).
    pub fn from_snapshot(
        dto: CoreSnapshot,
        limits: &SnapshotLimits,
    ) -> Result<Self, SnapshotError> {
        dto.preflight(limits)?;
        if !ordered(dto.known.iter().map(|k| k.entity))
            || dto.known.iter().any(|k| !ordered(k.records.iter()))
        {
            return invalid("knowledge entries must be strictly ordered");
        }
        if !ordered(
            dto.pending
                .iter()
                .map(|q| (q.delivery.due, q.delivery.order)),
        ) {
            return invalid("queue entries must be strictly ordered");
        }
        if !ordered(dto.inputs.iter().map(|i| i.id))
            || !ordered(dto.entities.iter().map(|e| e.entity))
        {
            return invalid("ledger and lifecycle entries must be strictly ordered");
        }
        let mut candidate = Self {
            time: dto.time,
            records: dto
                .records
                .into_iter()
                .map(|r| {
                    Ok(Record {
                        id: RecordId(position(r.id)?),
                        source: Entity(r.source),
                        time: r.time,
                        content: match r.content {
                            SnapshotContent::Observation(s) => Content::Observation(s),
                            SnapshotContent::Account { summary, basis } => Content::Account {
                                summary,
                                basis: RecordId(position(basis)?),
                            },
                            SnapshotContent::ConfirmedOutcome {
                                description,
                                action,
                            } => Content::ConfirmedOutcome {
                                description,
                                action: ActionId(position(action)?),
                            },
                        },
                    })
                })
                .collect::<Result<_, SnapshotError>>()?,
            known: dto
                .known
                .into_iter()
                .map(|k| {
                    Ok((
                        Entity(k.entity),
                        k.records
                            .into_iter()
                            .map(|id| position(id).map(RecordId))
                            .collect::<Result<_, _>>()?,
                    ))
                })
                .collect::<Result<_, SnapshotError>>()?,
            pending: dto
                .pending
                .into_iter()
                .map(|q| {
                    let d = import_delivery(q.delivery)?;
                    Ok((
                        (d.due, d.order),
                        QueuedDelivery {
                            delivery: d,
                            managed_input: q.managed_input.map(InputId),
                        },
                    ))
                })
                .collect::<Result<_, SnapshotError>>()?,
            next_delivery: position(dto.next_delivery)?,
            actions: dto
                .actions
                .into_iter()
                .map(|a| {
                    Ok(Action {
                        actor: Entity(a.actor),
                        proposal: Proposal {
                            request: a.proposal.request,
                            causes: a
                                .proposal
                                .causes
                                .into_iter()
                                .map(|r| position(r).map(RecordId))
                                .collect::<Result<_, _>>()?,
                            rule: a.proposal.rule,
                        },
                        status: match a.status {
                            SnapshotStatus::Proposed => Status::Proposed,
                            SnapshotStatus::Accepted => Status::Accepted,
                            SnapshotStatus::Rejected(s) => Status::Rejected(s),
                            SnapshotStatus::Confirmed(r) => {
                                Status::Confirmed(RecordId(position(r)?))
                            }
                        },
                    })
                })
                .collect::<Result<_, SnapshotError>>()?,
            trace: dto
                .trace
                .into_iter()
                .map(|t| {
                    Ok(TraceEntry {
                        time: t.time,
                        rule: t.rule,
                        causes: t
                            .causes
                            .into_iter()
                            .map(|r| position(r).map(RecordId))
                            .collect::<Result<_, _>>()?,
                        event: match t.event {
                            SnapshotEvent::Recorded(r) => Event::Recorded(RecordId(position(r)?)),
                            SnapshotEvent::Scheduled(d) => Event::Scheduled(import_delivery(d)?),
                            SnapshotEvent::Delivered(d) => Event::Delivered(import_delivery(d)?),
                            SnapshotEvent::Proposed(a) => Event::Proposed(ActionId(position(a)?)),
                            SnapshotEvent::Accepted(a) => Event::Accepted(ActionId(position(a)?)),
                            SnapshotEvent::Rejected(a) => Event::Rejected(ActionId(position(a)?)),
                            SnapshotEvent::Confirmed { action, record } => Event::Confirmed {
                                action: ActionId(position(action)?),
                                record: RecordId(position(record)?),
                            },
                        },
                    })
                })
                .collect::<Result<_, SnapshotError>>()?,
            trace_capacity: position(dto.trace_capacity)?,
            dropped_trace: dto.dropped_trace,
            inputs: dto
                .inputs
                .into_iter()
                .map(|i| {
                    Ok((
                        InputId(i.id),
                        match i.applied {
                            SnapshotAppliedInput::Record(r) => {
                                AppliedInput::Record(RecordId(position(r)?))
                            }
                            SnapshotAppliedInput::Schedule {
                                from,
                                to,
                                record,
                                due,
                            } => AppliedInput::Schedule {
                                from: Entity(from),
                                to: Entity(to),
                                record: RecordId(position(record)?),
                                due,
                            },
                            SnapshotAppliedInput::ManagedSchedule(r) => {
                                AppliedInput::ManagedSchedule(DeliveryReceipt {
                                    delivery: import_delivery(r.delivery)?,
                                    mode: match r.mode {
                                        SnapshotDeliveryMode::LoadedRecipient => {
                                            DeliveryMode::LoadedRecipient
                                        }
                                        SnapshotDeliveryMode::AbstractReceipt => {
                                            DeliveryMode::AbstractReceipt
                                        }
                                    },
                                    state: match r.state {
                                        SnapshotDeliveryState::Pending => DeliveryState::Pending,
                                        SnapshotDeliveryState::Delivered { at } => {
                                            DeliveryState::Delivered { at }
                                        }
                                        SnapshotDeliveryState::Rejected { at, reason } => {
                                            DeliveryState::Rejected {
                                                at,
                                                reason: match reason {
                                                    SnapshotDeliveryRejection::Retired => {
                                                        DeliveryRejection::Retired
                                                    }
                                                    SnapshotDeliveryRejection::Unavailable => {
                                                        DeliveryRejection::Unavailable
                                                    }
                                                    SnapshotDeliveryRejection::Incapable => {
                                                        DeliveryRejection::Incapable
                                                    }
                                                    SnapshotDeliveryRejection::Unloaded => {
                                                        DeliveryRejection::Unloaded
                                                    }
                                                },
                                            }
                                        }
                                    },
                                })
                            }
                        },
                    ))
                })
                .collect::<Result<_, SnapshotError>>()?,
            input_capacity: position(dto.input_capacity)?,
            entities: dto
                .entities
                .into_iter()
                .map(|e| {
                    (
                        Entity(e.entity),
                        match e.state {
                            SnapshotEntityState::Retired => EntityState::Retired,
                            SnapshotEntityState::Present {
                                residency,
                                availability,
                            } => EntityState::Present {
                                residency: match residency {
                                    SnapshotResidency::Loaded => Residency::Loaded,
                                    SnapshotResidency::Unloaded => Residency::Unloaded,
                                },
                                availability: match availability {
                                    SnapshotAvailability::Available => Availability::Available,
                                    SnapshotAvailability::Unavailable => Availability::Unavailable,
                                    SnapshotAvailability::Incapable => Availability::Incapable,
                                },
                            },
                        },
                    )
                })
                .collect(),
            entity_capacity: position(dto.entity_capacity)?,
            text_bytes: 0,
            reference_edges: 0,
        };
        candidate.validate()?;
        // Derived accounting is rebuilt once on restore, never serialized.
        candidate.text_bytes = candidate
            .records
            .iter()
            .map(|r| match &r.content {
                Content::Observation(s) => s.len(),
                Content::Account { summary, .. } => summary.len(),
                Content::ConfirmedOutcome { description, .. } => description.len(),
            })
            .sum::<usize>()
            + candidate
                .actions
                .iter()
                .map(|a| {
                    a.proposal.request.len()
                        + a.proposal.rule.len()
                        + match &a.status {
                            Status::Rejected(s) => s.len(),
                            _ => 0,
                        }
                })
                .sum::<usize>()
            + candidate.trace.iter().map(|t| t.rule.len()).sum::<usize>();
        candidate.reference_edges = candidate.known.values().map(BTreeSet::len).sum::<usize>()
            + candidate
                .actions
                .iter()
                .map(|a| a.proposal.causes.len())
                .sum::<usize>()
            + candidate
                .trace
                .iter()
                .map(|t| t.causes.len())
                .sum::<usize>();
        Ok(candidate)
    }
}
fn export_delivery(d: &Delivery) -> SnapshotDelivery {
    SnapshotDelivery {
        record: d.record.0 as u64,
        from: d.from.0,
        to: d.to.0,
        due: d.due,
        order: d.order as u64,
    }
}
fn import_delivery(d: SnapshotDelivery) -> Result<Delivery, SnapshotError> {
    Ok(Delivery {
        record: RecordId(position(d.record)?),
        from: Entity(d.from),
        to: Entity(d.to),
        due: d.due,
        order: position(d.order)?,
    })
}

impl CoreSnapshot {
    fn preflight(&self, limits: &SnapshotLimits) -> Result<(), SnapshotError> {
        if self.version != SNAPSHOT_VERSION {
            return Err(SnapshotError::UnsupportedVersion(self.version));
        }
        bounded(self.records.len(), limits.max_records, "records")?;
        bounded(self.pending.len(), limits.max_pending, "pending")?;
        bounded(self.actions.len(), limits.max_actions, "actions")?;
        bounded(
            position(self.trace_capacity)?,
            limits.max_trace_capacity,
            "trace capacity",
        )?;
        bounded(self.trace.len(), limits.max_trace_capacity, "trace")?;
        bounded(
            position(self.input_capacity)?,
            limits.max_inputs,
            "input capacity",
        )?;
        bounded(self.inputs.len(), limits.max_inputs, "inputs")?;
        bounded(
            position(self.entity_capacity)?,
            limits.max_entities,
            "entity capacity",
        )?;
        bounded(self.entities.len(), limits.max_entities, "entities")?;
        bounded(self.known.len(), limits.max_known_owners, "known owners")?;
        let mut budget = Budget::new(limits);
        for k in &self.known {
            budget.edges(k.records.len())?;
        }
        for r in &self.records {
            match &r.content {
                SnapshotContent::Observation(s) => budget.string(s)?,
                SnapshotContent::Account { summary, .. } => budget.string(summary)?,
                SnapshotContent::ConfirmedOutcome { description, .. } => {
                    budget.string(description)?
                }
            }
        }
        for a in &self.actions {
            budget.string(&a.proposal.request)?;
            budget.string(&a.proposal.rule)?;
            budget.edges(a.proposal.causes.len())?;
            if let SnapshotStatus::Rejected(s) = &a.status {
                budget.string(s)?;
            }
        }
        for t in &self.trace {
            budget.string(&t.rule)?;
            budget.edges(t.causes.len())?;
        }
        Ok(())
    }
}

impl Core {
    fn valid_delivery(&self, d: &Delivery) -> Result<(), SnapshotError> {
        let record = self
            .record(d.record)
            .ok_or(SnapshotError::Invalid("delivery record"))?;
        if d.order >= self.next_delivery || d.due < record.time || !self.knows(d.from, d.record) {
            return invalid("delivery order, time, or sender knowledge");
        }
        Ok(())
    }
    fn validate(&self) -> Result<(), SnapshotError> {
        if self.inputs.len() > self.input_capacity
            || self.entities.len() > self.entity_capacity
            || self.trace.len() > self.trace_capacity
        {
            return invalid("saved capacity smaller than contents");
        }
        if self.dropped_trace > 0 && self.trace.len() != self.trace_capacity {
            return invalid("evicted trace must fill its capacity");
        }
        if !ordered(self.records.iter().map(|r| r.id)) {
            return invalid("record positions");
        }
        let mut prior_time = 0;
        for (index, r) in self.records.iter().enumerate() {
            if r.id.0 != index
                || r.time < prior_time
                || r.time > self.time
                || !self.knows(r.source, r.id)
            {
                return invalid("record position, time, or source knowledge");
            }
            prior_time = r.time;
            match &r.content {
                Content::Observation(_) => {}
                Content::Account { basis, .. } => {
                    if basis.0 >= index || !self.knows(r.source, *basis) {
                        return invalid("account basis");
                    }
                }
                Content::ConfirmedOutcome {
                    description,
                    action,
                } => {
                    let a = self
                        .action(*action)
                        .ok_or(SnapshotError::Invalid("outcome action"))?;
                    if description.trim().is_empty()
                        || a.actor != r.source
                        || a.status != Status::Confirmed(r.id)
                        || a.proposal.causes.iter().any(|c| c.0 >= index)
                    {
                        return invalid("outcome backlink, description, or cause");
                    }
                }
            }
        }
        for ids in self.known.values() {
            if ids.is_empty() || ids.iter().any(|r| self.record(*r).is_none()) {
                return invalid("knowledge record");
            }
        }
        for (index, a) in self.actions.iter().enumerate() {
            if a.proposal.request.trim().is_empty()
                || a.proposal.rule.trim().is_empty()
                || a.proposal.causes.iter().any(|r| !self.knows(a.actor, *r))
            {
                return invalid("action explanation or causes");
            }
            match &a.status {
                Status::Rejected(s) if s.trim().is_empty() => {
                    return invalid("rejection explanation");
                }
                Status::Confirmed(id) => {
                    let r = self
                        .record(*id)
                        .ok_or(SnapshotError::Invalid("confirmed record"))?;
                    if r.source != a.actor
                        || !matches!(r.content, Content::ConfirmedOutcome { action, .. } if action == ActionId(index))
                    {
                        return invalid("action outcome backlink");
                    }
                }
                _ => {}
            }
        }
        let mut pending_orders = BTreeMap::new();
        let mut required_schedules = BTreeMap::new();
        let mut available_schedules = BTreeMap::new();
        for (key, q) in &self.pending {
            let d = &q.delivery;
            self.valid_delivery(d)?;
            if *key != (d.due, d.order)
                || d.due < self.time
                || pending_orders.insert(d.order, q).is_some()
            {
                return invalid("pending queue time, key, or duplicate order");
            }
            if let Some(input) = q.managed_input {
                if !matches!(self.inputs.get(&input), Some(AppliedInput::ManagedSchedule(r)) if r.delivery == *d && r.state == DeliveryState::Pending)
                {
                    return invalid("managed queue receipt backlink");
                }
            } else {
                *available_schedules
                    .entry((d.from, d.to, d.record, d.due))
                    .or_insert(0usize) += 1;
            }
        }
        let mut managed_orders = BTreeMap::new();
        let mut identified_records = BTreeSet::new();
        let mut schedule_count = 0usize;
        for (input, applied) in &self.inputs {
            match applied {
                AppliedInput::Record(id) => {
                    if self.record(*id).is_none() || !identified_records.insert(*id) {
                        return invalid("identified record result");
                    }
                }
                AppliedInput::Schedule {
                    from,
                    to,
                    record,
                    due,
                } => {
                    let r = self
                        .record(*record)
                        .ok_or(SnapshotError::Invalid("identified schedule record"))?;
                    if !self.knows(*from, *record) || *due < r.time {
                        return invalid("identified schedule sender or time");
                    }
                    schedule_count += 1;
                    if *due > self.time || !self.knows(*to, *record) {
                        if *due < self.time {
                            return invalid("completed identified schedule knowledge");
                        }
                        *required_schedules
                            .entry((*from, *to, *record, *due))
                            .or_insert(0usize) += 1;
                    }
                }
                AppliedInput::ManagedSchedule(r) => {
                    schedule_count += 1;
                    self.valid_delivery(&r.delivery)?;
                    if !self.entities.contains_key(&r.delivery.to)
                        || managed_orders
                            .insert(r.delivery.order, (input, r))
                            .is_some()
                    {
                        return invalid("managed recipient or duplicate order");
                    }
                    let queued = pending_orders.get(&r.delivery.order);
                    match r.state {
                        DeliveryState::Pending => {
                            if !matches!(queued, Some(q) if q.managed_input == Some(*input) && q.delivery == r.delivery)
                            {
                                return invalid("pending receipt queue backlink");
                            }
                        }
                        DeliveryState::Delivered { at } => {
                            if at != r.delivery.due
                                || at > self.time
                                || queued.is_some()
                                || !self.knows(r.delivery.to, r.delivery.record)
                            {
                                return invalid("delivered receipt time, queue, or knowledge");
                            }
                        }
                        DeliveryState::Rejected { at, reason } => {
                            if at != r.delivery.due
                                || at > self.time
                                || queued.is_some()
                                || (reason == DeliveryRejection::Unloaded
                                    && r.mode != DeliveryMode::LoadedRecipient)
                                || (reason == DeliveryRejection::Retired
                                    && self.entity_state(r.delivery.to)
                                        != Some(EntityState::Retired))
                            {
                                return invalid("rejected receipt time, queue, or mode");
                            }
                        }
                    }
                }
            }
        }
        if schedule_count > self.next_delivery
            || required_schedules
                .iter()
                .any(|(key, count)| available_schedules.get(key).copied().unwrap_or(0) < *count)
        {
            return invalid("identified schedule queue or sequence");
        }
        // Every successful operation logs once; only rejection may have an
        // optional earlier acceptance that current state cannot establish.
        let rejected_deliveries = managed_orders
            .values()
            .filter(|(_, r)| matches!(r.state, DeliveryState::Rejected { .. }))
            .count();
        let delivered = self
            .next_delivery
            .checked_sub(self.pending.len())
            .and_then(|remaining| remaining.checked_sub(rejected_deliveries))
            .ok_or(SnapshotError::Invalid("delivery event count"))?;
        let accepted = self
            .actions
            .iter()
            .filter(|a| matches!(a.status, Status::Accepted | Status::Confirmed(_)))
            .count();
        let rejected = self
            .actions
            .iter()
            .filter(|a| matches!(a.status, Status::Rejected(_)))
            .count();
        let minimum = [
            self.records.len(),
            self.actions.len(),
            accepted,
            rejected,
            self.next_delivery,
            delivered,
        ]
        .into_iter()
        .try_fold(0u64, |count, n| {
            Ok::<_, SnapshotError>(count.saturating_add(wire(n)?))
        })?;
        let maximum = minimum.saturating_add(wire(rejected)?);
        let logged = self.dropped_trace.saturating_add(wire(self.trace.len())?);
        if logged < minimum || logged > maximum {
            return invalid("dropped trace counter");
        }
        self.validate_trace(&pending_orders, &managed_orders)
    }

    fn validate_trace(
        &self,
        pending: &BTreeMap<usize, &QueuedDelivery>,
        managed: &BTreeMap<usize, (&InputId, &DeliveryReceipt)>,
    ) -> Result<(), SnapshotError> {
        let mut last_time = 0;
        let mut last_schedule = None;
        let mut last_delivered = None;
        let mut last_creation = None;
        let mut last_proposal = None;
        let complete = self.dropped_trace == 0 && self.trace_capacity > 0;
        let mut deliveries: BTreeMap<usize, (&Delivery, bool)> = BTreeMap::new();
        let mut action_events = BTreeMap::new();
        let mut creation_events = BTreeSet::new();
        let mut replayed_known: BTreeMap<Entity, BTreeSet<RecordId>> = BTreeMap::new();
        for t in &self.trace {
            if t.time < last_time
                || t.time > self.time
                || t.causes
                    .iter()
                    .any(|r| self.record(*r).is_none_or(|r| r.time > t.time))
            {
                return invalid("trace time or cause");
            }
            if complete && t.causes.iter().any(|id| !creation_events.contains(id)) {
                return invalid("complete trace references a record before creation");
            }
            last_time = t.time;
            match &t.event {
                Event::Recorded(id) => {
                    let r = self
                        .record(*id)
                        .ok_or(SnapshotError::Invalid("trace record"))?;
                    let (causes, rule) = match r.content {
                        Content::Observation(_) => (vec![], "Host supplied observation"),
                        Content::Account { basis, .. } => {
                            (vec![basis], "Account derived from known record")
                        }
                        _ => return invalid("outcome recorded without confirmation"),
                    };
                    if t.time != r.time
                        || t.causes != causes
                        || t.rule != rule
                        || last_creation.is_some_and(|previous| *id <= previous)
                        || !creation_events.insert(*id)
                    {
                        return invalid("trace record evidence");
                    }
                    last_creation = Some(*id);
                    if complete {
                        if let Content::Account { basis, .. } = r.content
                            && !replayed_known
                                .get(&r.source)
                                .is_some_and(|ids| ids.contains(&basis))
                        {
                            return invalid("complete trace account basis was not known");
                        }
                        replayed_known.entry(r.source).or_default().insert(*id);
                    }
                }
                Event::Scheduled(d) | Event::Delivered(d) => {
                    self.valid_delivery(d)?;
                    if t.causes != [d.record] {
                        return invalid("trace delivery causes");
                    }
                    if let Some(q) = pending.get(&d.order)
                        && q.delivery != *d
                    {
                        return invalid("trace and pending delivery differ");
                    }
                    if let Some((_, r)) = managed.get(&d.order)
                        && r.delivery != *d
                    {
                        return invalid("trace and receipt delivery differ");
                    }
                    if let Event::Scheduled(_) = &t.event {
                        if complete
                            && !replayed_known
                                .get(&d.from)
                                .is_some_and(|ids| ids.contains(&d.record))
                        {
                            return invalid("complete trace sender did not know record");
                        }
                        if d.due < t.time
                            || self.records[d.record.0].time > t.time
                            || t.rule != "Delivery queued; recipient knowledge unchanged"
                            || last_schedule.is_some_and(|order| d.order <= order)
                            || deliveries.insert(d.order, (d, false)).is_some()
                        {
                            return invalid("trace schedule evidence");
                        }
                        last_schedule = Some(d.order);
                    } else {
                        if d.due != t.time
                            || t.rule != "Scheduled record received"
                            || !self.knows(d.to, d.record)
                            || pending.contains_key(&d.order)
                            || last_delivered.is_some_and(|key| (d.due, d.order) <= key)
                        {
                            return invalid("trace delivery evidence");
                        }
                        last_delivered = Some((d.due, d.order));
                        let scheduled = deliveries.insert(d.order, (d, true));
                        if complete && scheduled.is_none() {
                            return invalid("complete trace omits schedule");
                        }
                        if let Some((saved, delivered)) = scheduled
                            && (saved != d || delivered)
                        {
                            return invalid("duplicate or mismatched trace delivery");
                        }
                        if let Some((_, r)) = managed.get(&d.order)
                            && r.state != (DeliveryState::Delivered { at: d.due })
                        {
                            return invalid("trace contradicts managed receipt");
                        }
                        if complete {
                            replayed_known.entry(d.to).or_default().insert(d.record);
                        }
                    }
                }
                Event::Proposed(id)
                | Event::Accepted(id)
                | Event::Rejected(id)
                | Event::Confirmed { action: id, .. } => {
                    let a = self
                        .action(*id)
                        .ok_or(SnapshotError::Invalid("trace action"))?;
                    if t.causes != a.proposal.causes {
                        return invalid("trace action causes");
                    }
                    let stage = match &t.event {
                        Event::Proposed(_) => {
                            if t.rule != a.proposal.rule
                                || last_proposal.is_some_and(|previous| *id <= previous)
                            {
                                return invalid("trace proposal rule");
                            }
                            last_proposal = Some(*id);
                            if complete
                                && a.proposal.causes.iter().any(|id| {
                                    !replayed_known
                                        .get(&a.actor)
                                        .is_some_and(|ids| ids.contains(id))
                                })
                            {
                                return invalid("complete trace actor did not know cause");
                            }
                            0
                        }
                        Event::Accepted(_) => {
                            if a.status == Status::Proposed
                                || t.rule != "Host accepted; execution not yet confirmed"
                            {
                                return invalid("trace acceptance");
                            }
                            1
                        }
                        Event::Rejected(_) => {
                            if !matches!(&a.status, Status::Rejected(reason) if *reason == t.rule) {
                                return invalid("trace rejection");
                            }
                            2
                        }
                        Event::Confirmed { record, .. } => {
                            let r = self
                                .record(*record)
                                .ok_or(SnapshotError::Invalid("trace confirmation record"))?;
                            if a.status != Status::Confirmed(*record)
                                || r.time != t.time
                                || t.rule != "Host confirmed execution"
                                || last_creation.is_some_and(|previous| *record <= previous)
                                || !creation_events.insert(*record)
                            {
                                return invalid("trace confirmation");
                            }
                            last_creation = Some(*record);
                            if complete {
                                replayed_known.entry(r.source).or_default().insert(*record);
                            }
                            2
                        }
                        _ => unreachable!(),
                    };
                    let previous = action_events.insert(*id, stage);
                    if previous.is_some_and(|previous| previous >= stage) {
                        return invalid("trace action transition order");
                    }
                    if matches!(t.event, Event::Confirmed { .. }) && previous == Some(0) {
                        return invalid("trace omits acceptance after retained proposal");
                    }
                    if complete
                        && match &t.event {
                            Event::Proposed(_) => previous.is_some(),
                            Event::Accepted(_) | Event::Confirmed { .. } => {
                                previous != Some(stage - 1)
                            }
                            Event::Rejected(_) => !matches!(previous, Some(0 | 1)),
                            _ => unreachable!(),
                        }
                    {
                        return invalid("complete trace omits action transition");
                    }
                }
            }
        }
        for (id, stage) in &action_events {
            let expected = match self.actions[id.0].status {
                Status::Proposed => 0,
                Status::Accepted => 1,
                Status::Rejected(_) | Status::Confirmed(_) => 2,
            };
            if *stage != expected {
                return invalid("trace omits final transition after retained action event");
            }
        }
        for (order, (_, delivered)) in &deliveries {
            let rejected = managed
                .get(order)
                .is_some_and(|(_, r)| matches!(r.state, DeliveryState::Rejected { .. }));
            if *delivered == (pending.contains_key(order) || rejected) {
                return invalid("trace omits delivery after retained schedule");
            }
        }
        if complete
            && (creation_events.len() != self.records.len()
                || action_events.len() != self.actions.len()
                || deliveries.len() != self.next_delivery)
        {
            return invalid("complete trace omits creation or schedule");
        }
        if complete && replayed_known != self.known {
            return invalid("complete trace knowledge differs from saved knowledge");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_snapshot_round_trip_and_resume() {
        let limits = SnapshotLimits::default();
        let mut core = Core::with_lifecycle_capacity(2, 8, 2);
        let actor = Entity(1);
        let target = Entity(2);
        core.register_entity(target, Residency::Loaded, Availability::Available)
            .unwrap();
        let record = core
            .observe_identified(InputId(1), actor, "seen".into())
            .unwrap()
            .value;
        let account = core.account(actor, record, "reported".into()).unwrap();
        let action = core
            .evaluate(actor, |_| {
                Some(Proposal {
                    request: "act".into(),
                    causes: vec![record, record],
                    rule: "reason".into(),
                })
            })
            .unwrap()
            .unwrap();
        core.accept(action).unwrap();
        core.confirm_identified(InputId(2), action, "done".into())
            .unwrap();
        core.schedule_managed_identified(
            InputId(3),
            actor,
            target,
            account,
            0,
            DeliveryMode::LoadedRecipient,
        )
        .unwrap();
        let dto = core.snapshot(&limits).unwrap();
        let mut restored = Core::from_snapshot(dto.clone(), &limits).unwrap();
        assert_eq!(dto, restored.snapshot(&limits).unwrap());
        restored.advance_to(1).unwrap();
        core.advance_to(1).unwrap();
        assert_eq!(
            core.snapshot(&limits).unwrap(),
            restored.snapshot(&limits).unwrap()
        );
    }
    #[test]
    fn rejects_duplicate_entries_broken_links_and_limits() {
        let limits = SnapshotLimits::default();
        let mut core = Core::new(4);
        core.observe(Entity(1), "a".into());
        let good = core.snapshot(&limits).unwrap();
        let mut bad = good.clone();
        bad.known.push(bad.known[0].clone());
        assert!(Core::from_snapshot(bad, &limits).is_err());
        let mut bad = good.clone();
        bad.records[0].id = 1;
        assert!(Core::from_snapshot(bad, &limits).is_err());
        let mut bad = good;
        bad.known[0].records.clear();
        assert!(Core::from_snapshot(bad, &limits).is_err());
        let limits = SnapshotLimits {
            max_string_bytes: 0,
            ..limits
        };
        assert!(matches!(
            core.snapshot(&limits),
            Err(SnapshotError::LimitExceeded(_))
        ));
    }
    #[test]
    fn complete_trace_requires_creation_and_action_transitions() {
        let limits = SnapshotLimits::default();
        let mut core = Core::new(32);
        let actor = Entity(1);
        let record = core.observe(actor, "seen".into());
        let action = core
            .evaluate(actor, |_| {
                Some(Proposal {
                    request: "act".into(),
                    causes: vec![record],
                    rule: "reason".into(),
                })
            })
            .unwrap()
            .unwrap();
        core.accept(action).unwrap();
        core.confirm(action, "done".into()).unwrap();
        let good = core.snapshot(&limits).unwrap();
        assert!(Core::from_snapshot(good.clone(), &limits).is_ok());
        let mut bad = good.clone();
        bad.trace.clear();
        assert!(Core::from_snapshot(bad, &limits).is_err());
        let mut bad = good;
        bad.trace
            .retain(|t| !matches!(t.event, SnapshotEvent::Accepted(_)));
        assert!(Core::from_snapshot(bad.clone(), &limits).is_err());
        bad.dropped_trace = 1;
        bad.trace_capacity = bad.trace.len() as u64;
        assert!(Core::from_snapshot(bad, &limits).is_err());
    }
    #[test]
    fn lifecycle_changes_and_rejected_receipts_preserve_legacy_knowledge() {
        let limits = SnapshotLimits::default();
        let mut core = Core::with_lifecycle_capacity(0, 4, 1);
        let source = Entity(10);
        let target = Entity(20);
        core.register_entity(target, Residency::Unloaded, Availability::Available)
            .unwrap();
        let record = core.observe(source, String::new());
        core.schedule_managed_identified(
            InputId(1),
            source,
            target,
            record,
            1,
            DeliveryMode::LoadedRecipient,
        )
        .unwrap();
        core.schedule(source, target, record, 1).unwrap();
        core.advance_to(1).unwrap();
        core.set_entity_state(target, EntityState::Retired).unwrap();
        let dto = core.snapshot(&limits).unwrap();
        assert_eq!(
            Core::from_snapshot(dto.clone(), &limits)
                .unwrap()
                .snapshot(&limits)
                .unwrap(),
            dto
        );
    }
    #[test]
    fn trace_counter_accounts_for_events_even_when_history_is_off_or_evicted() {
        let limits = SnapshotLimits::default();
        for capacity in [0, 1] {
            let mut core = Core::new(capacity);
            for _ in 0..4 {
                core.observe(Entity(1), "seen".into());
            }
            let good = core.snapshot(&limits).unwrap();
            assert!(Core::from_snapshot(good.clone(), &limits).is_ok());
            for dropped in [0, 1, 100] {
                let mut bad = good.clone();
                bad.dropped_trace = dropped;
                assert!(Core::from_snapshot(bad, &limits).is_err());
            }
        }
    }
    #[test]
    fn complete_trace_rejects_unexplained_knowledge_and_retirement_is_permanent() {
        let limits = SnapshotLimits::default();
        let mut core = Core::with_lifecycle_capacity(32, 1, 1);
        let source = Entity(1);
        let target = Entity(2);
        core.register_entity(target, Residency::Loaded, Availability::Available)
            .unwrap();
        let record = core.observe(source, "seen".into());
        let mut bad = core.snapshot(&limits).unwrap();
        bad.known.push(SnapshotKnowledge {
            entity: target.0,
            records: vec![0],
        });
        assert!(Core::from_snapshot(bad, &limits).is_err());
        core.schedule_managed_identified(
            InputId(1),
            source,
            target,
            record,
            1,
            DeliveryMode::LoadedRecipient,
        )
        .unwrap();
        core.set_entity_state(target, EntityState::Retired).unwrap();
        core.advance_to(1).unwrap();
        let mut bad = core.snapshot(&limits).unwrap();
        bad.entities[0].state = SnapshotEntityState::Present {
            residency: SnapshotResidency::Loaded,
            availability: SnapshotAvailability::Available,
        };
        assert!(Core::from_snapshot(bad, &limits).is_err());
    }
}
