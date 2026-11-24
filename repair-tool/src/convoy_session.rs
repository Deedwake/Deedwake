//! Native configured-Convoy checkpoints. Validation establishes consistency,
//! never authenticity or proof that the host actually executed an action.
use crate::{scene, session_io};
use deedwake_core::*;
use scene::config::{self, Confidence, Observer, Reaction, Retention, ValidatedConfig};
use serde::{Deserialize, Serialize};
pub use session_io::SaveError;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

fn limits() -> SnapshotLimits {
    // Native admission ceilings, not runtime caps or performance promises.
    SnapshotLimits {
        max_records: 4096,
        max_actions: 4,
        max_pending: 3,
        max_inputs: 3,
        max_entities: 4,
        max_trace_capacity: 1024,
        max_known_owners: 4,
        max_known_edges: 32768,
        max_string_bytes: 4096,
        max_total_string_bytes: 1024 * 1024,
    }
}
fn invalid() -> String {
    "saved Convoy references, routes, knowledge, or flags disagree".into()
}
fn snapshot_error(e: impl std::fmt::Display) -> String {
    format!("invalid Core snapshot: {e}")
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    schema_version: u64,
    profile_id: String,
    observer: String,
    retention: String,
    reaction: String,
    sela_confidence: String,
    trace_capacity: u64,
    initial_delivery_ticks: u64,
    routine_delivery_ticks: u64,
    intermediary_delivery_ticks: u64,
}
impl Configuration {
    fn capture(c: &ValidatedConfig) -> Self {
        Self {
            schema_version: 2,
            profile_id: c.profile_id().into(),
            observer: c.observer().as_str().into(),
            retention: c.retention().as_str().into(),
            reaction: c.reaction().as_str().into(),
            sela_confidence: c.confidence().as_str().into(),
            trace_capacity: c.trace_capacity() as u64,
            initial_delivery_ticks: c.initial_delivery_ticks().into(),
            routine_delivery_ticks: c.routine_delivery_ticks().into(),
            intermediary_delivery_ticks: c.intermediary_delivery_ticks().into(),
        }
    }
    fn validate(&self) -> Result<ValidatedConfig, String> {
        if self.schema_version != 2 {
            return Err("saved Convoy configuration must use schema 2".into());
        }
        config::validate(config::Input {
            schema_version: self.schema_version,
            profile_id: &self.profile_id,
            observer: &self.observer,
            retention: &self.retention,
            reaction: &self.reaction,
            sela_confidence: &self.sela_confidence,
            trace_capacity: self.trace_capacity,
            initial_delivery_ticks: self.initial_delivery_ticks,
            routine_delivery_ticks: self.routine_delivery_ticks,
            intermediary_delivery_ticks: self.intermediary_delivery_ticks,
        })
        .map_err(|e| format!("invalid saved Convoy configuration: {e:?}"))
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SavedObserver {
    Mara,
    Sela,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SavedRetention {
    Precise,
    Incomplete,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Operation {
    Restrict,
    Restore,
    Warn,
    ResolveWarning,
}
impl Operation {
    fn runtime(self) -> scene::ServiceOperation {
        match self {
            Self::Restrict => scene::ServiceOperation::Restrict,
            Self::Restore => scene::ServiceOperation::Restore,
            Self::Warn => scene::ServiceOperation::Warn,
            Self::ResolveWarning => scene::ServiceOperation::ResolveWarning,
        }
    }
    fn capture(value: scene::ServiceOperation) -> Self {
        match value {
            scene::ServiceOperation::Restrict => Self::Restrict,
            scene::ServiceOperation::Restore => Self::Restore,
            scene::ServiceOperation::Warn => Self::Warn,
            scene::ServiceOperation::ResolveWarning => Self::ResolveWarning,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Meaning {
    Report {
        observer: SavedObserver,
        retention: SavedRetention,
    },
    Release {},
    Verification {},
    Service {
        operation: Operation,
        action: u64,
    },
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fact {
    record: u64,
    meaning: Meaning,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Host {
    configuration: Configuration,
    facts: Vec<Fact>,
    verification: Option<u64>,
    held: bool,
    access: bool,
    report: Option<u64>,
    hold: Option<u64>,
    release: Option<u64>,
    service_decision: Option<u64>,
    refused: bool,
    journal: Vec<String>,
    dialogue: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    format: String,
    format_version: u32,
    core_version: u32,
    host_version: u32,
    content_version: String,
    core: CoreSnapshot,
    host: Host,
}
impl Envelope {
    fn capture(scene: &scene::Convoy) -> Result<Self, String> {
        if scene.condition != scene::Condition::Current || scene.study.is_some() {
            return Err("native Convoy sessions require configured mode".into());
        }
        let configuration = scene
            .config
            .as_ref()
            .ok_or("native Convoy sessions require configured mode")?;
        validate_text(&scene.journal, &scene.dialogue)?;
        if scene.facts.len() > 5 {
            return Err(invalid());
        }
        let core = &scene.core;
        let record = |id| core.record_position(id).map_err(snapshot_error);
        let action = |id| core.action_position(id).map_err(snapshot_error);
        let facts = scene
            .facts
            .iter()
            .map(|(&id, meaning)| {
                Ok(Fact {
                    record: record(id)?,
                    meaning: match *meaning {
                        scene::Fact::Report {
                            observer,
                            retention,
                        } => Meaning::Report {
                            observer: match observer {
                                scene::WITNESS => SavedObserver::Mara,
                                scene::INTERMEDIARY => SavedObserver::Sela,
                                _ => return Err(invalid()),
                            },
                            retention: match retention {
                                Retention::Precise => SavedRetention::Precise,
                                Retention::Incomplete => SavedRetention::Incomplete,
                            },
                        },
                        scene::Fact::Release => Meaning::Release {},
                        scene::Fact::Verification => Meaning::Verification {},
                        scene::Fact::Service {
                            operation,
                            action: id,
                        } => Meaning::Service {
                            operation: Operation::capture(operation),
                            action: action(id)?,
                        },
                    },
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(Self {
            format: "deedwake.convoy.session".into(),
            format_version: 1,
            core_version: 1,
            host_version: 1,
            content_version: "configured_convoy_v1".into(),
            core: core.snapshot(&limits()).map_err(snapshot_error)?,
            host: Host {
                configuration: Configuration::capture(configuration),
                facts,
                verification: scene.verification.map(record).transpose()?,
                held: scene.held,
                access: scene.access,
                report: scene.report.map(record).transpose()?,
                hold: scene.hold.map(record).transpose()?,
                release: scene.release.map(record).transpose()?,
                service_decision: scene.service_decision.map(action).transpose()?,
                refused: scene.refused,
                journal: scene.journal.clone(),
                dialogue: scene.dialogue.clone(),
            },
        })
    }
    fn restore(self) -> Result<scene::Convoy, String> {
        if self.format != "deedwake.convoy.session"
            || self.format_version != 1
            || self.core_version != 1
            || self.host_version != 1
            || self.content_version != "configured_convoy_v1"
        {
            return Err(
                "unsupported Convoy snapshot format, Core, host, or content version".into(),
            );
        }
        let configuration = self.host.configuration.validate()?;
        let core = Core::from_snapshot(self.core.clone(), &limits()).map_err(snapshot_error)?;
        validate_host(&self.core, &self.host, &configuration)?;
        let record = |raw| core.record_id(raw).map_err(snapshot_error);
        let action = |raw| core.action_id(raw).map_err(snapshot_error);
        let facts = self
            .host
            .facts
            .into_iter()
            .map(|f| {
                Ok((
                    record(f.record)?,
                    match f.meaning {
                        Meaning::Report {
                            observer,
                            retention,
                        } => scene::Fact::Report {
                            observer: match observer {
                                SavedObserver::Mara => scene::WITNESS,
                                SavedObserver::Sela => scene::INTERMEDIARY,
                            },
                            retention: match retention {
                                SavedRetention::Precise => Retention::Precise,
                                SavedRetention::Incomplete => Retention::Incomplete,
                            },
                        },
                        Meaning::Release {} => scene::Fact::Release,
                        Meaning::Verification {} => scene::Fact::Verification,
                        Meaning::Service {
                            operation,
                            action: raw,
                        } => scene::Fact::Service {
                            operation: operation.runtime(),
                            action: action(raw)?,
                        },
                    },
                ))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        // Construct an isolated candidate without advancing, delivering, or evaluating policy.
        Ok(scene::Convoy {
            condition: scene::Condition::Current,
            study: None,
            config: Some(configuration),
            facts,
            verification: self.host.verification.map(record).transpose()?,
            held: self.host.held,
            access: self.host.access,
            report: self.host.report.map(record).transpose()?,
            hold: self.host.hold.map(record).transpose()?,
            release: self.host.release.map(record).transpose()?,
            service_decision: self.host.service_decision.map(action).transpose()?,
            refused: self.host.refused,
            journal: self.host.journal,
            dialogue: self.host.dialogue,
            core,
        })
    }
}

fn validate_text(journal: &[String], dialogue: &str) -> Result<(), String> {
    if journal.len() > 128
        || dialogue.len() > 4096
        || journal.iter().any(|s| s.len() > 4096)
        || dialogue.len() + journal.iter().map(String::len).sum::<usize>() > 128 * 1024
    {
        return Err("saved Convoy host exceeds count or text limits".into());
    }
    Ok(())
}

fn validate_host(dto: &CoreSnapshot, host: &Host, config: &ValidatedConfig) -> Result<(), String> {
    use scene::ServiceOperation as Op;
    validate_text(&host.journal, &host.dialogue)?;
    if dto.time > 1000
        || dto.next_delivery > 4096
        || dto.trace_capacity != config.trace_capacity() as u64
        || dto.input_capacity != 3
        || dto.entity_capacity != 4
        || dto.entities.len() != 4
        || host.facts.len() > 5
    {
        return Err(invalid());
    }
    for (index, entity) in dto.entities.iter().enumerate() {
        if entity.entity != index as u64 + 1
            || (entity.entity != scene::PROVIDER.0
                && entity.state
                    != (SnapshotEntityState::Present {
                        residency: SnapshotResidency::Loaded,
                        availability: SnapshotAvailability::Available,
                    }))
        {
            return Err(invalid());
        }
    }
    let retired = dto.entities[2].state == SnapshotEntityState::Retired;
    let observer = match config.observer() {
        Observer::Mara => scene::WITNESS.0,
        Observer::Sela => scene::INTERMEDIARY.0,
    };
    let record = |raw: u64| dto.records.get(raw as usize).ok_or_else(invalid);
    let mut facts = BTreeMap::new();
    let mut report = None;
    let mut release = None;
    let mut verification = None;
    let mut services = vec![];
    for fact in &host.facts {
        let r = record(fact.record)?;
        if facts.insert(fact.record, &fact.meaning).is_some() {
            return Err(invalid());
        }
        match &fact.meaning {
            Meaning::Report {
                observer: saved_observer,
                retention,
            } => {
                let saved_observer = match saved_observer {
                    SavedObserver::Mara => scene::WITNESS.0,
                    SavedObserver::Sela => scene::INTERMEDIARY.0,
                };
                let retention = match retention {
                    SavedRetention::Precise => Retention::Precise,
                    SavedRetention::Incomplete => Retention::Incomplete,
                };
                if report.replace(fact.record).is_some()
                    || saved_observer != observer
                    || r.source != observer
                    || retention != config.retention()
                {
                    return Err(invalid());
                }
            }
            Meaning::Release {} => {
                if release.replace(fact.record).is_some() {
                    return Err(invalid());
                }
            }
            Meaning::Verification {} => {
                if verification.replace(fact.record).is_some() {
                    return Err(invalid());
                }
            }
            Meaning::Service { operation, action } => {
                services.push((*action, fact.record, operation.runtime()))
            }
        }
    }
    if host.report != report
        || host.release != release
        || host.verification != verification
        || host.report.is_some() != host.hold.is_some()
        || host.held != (host.hold.is_some() && host.release.is_none())
        || (release.is_some() && host.hold.is_none())
        || (verification.is_some() && release.is_none())
        || services.len() > 2
    {
        return Err(invalid());
    }
    let mut assigned_records = BTreeSet::new();
    let mut assigned_actions = BTreeSet::new();
    let mut confirmed = |raw: u64,
                         actor: u64,
                         request: &str,
                         description: &str,
                         causes: &[u64]|
     -> Result<(), String> {
        let r = record(raw)?;
        let SnapshotContent::ConfirmedOutcome {
            description: text,
            action,
        } = &r.content
        else {
            return Err(invalid());
        };
        let a = dto.actions.get(*action as usize).ok_or_else(invalid)?;
        if r.source != actor
            || text != description
            || a.actor != actor
            || a.proposal.request != request
            || a.proposal.causes != causes
            || a.status != SnapshotStatus::Confirmed(raw)
            || !assigned_actions.insert(*action)
        {
            return Err(invalid());
        }
        Ok(())
    };
    if let Some(hold) = host.hold {
        let r = record(hold)?;
        let SnapshotContent::ConfirmedOutcome { action, .. } = &r.content else {
            return Err(invalid());
        };
        let a = dto.actions.get(*action as usize).ok_or_else(invalid)?;
        let [strap] = a.proposal.causes.as_slice() else {
            return Err(invalid());
        };
        let strap_record = record(*strap)?;
        if strap_record.source != scene::PLAYER.0
            || strap_record.time != r.time
            || !matches!(&strap_record.content, SnapshotContent::Observation(s) if s == "Loose load strap")
        {
            return Err(invalid());
        }
        confirmed(
            hold,
            scene::PLAYER.0,
            "hold convoy",
            "Convoy held",
            &[*strap],
        )?;
        assigned_records.extend([hold, *strap]);
        let report = record(report.ok_or_else(invalid)?)?;
        let SnapshotContent::Account { summary, basis } = &report.content else {
            return Err(invalid());
        };
        let seen = record(*basis)?;
        let expected_summary = match config.retention() {
            Retention::Precise => scene::PRECISE_REPORT,
            Retention::Incomplete => scene::INCOMPLETE_REPORT,
        };
        if summary != expected_summary
            || seen.source != observer
            || seen.time != r.time
            || report.time != r.time
            || !matches!(&seen.content, SnapshotContent::Observation(s) if s == scene::STUDY_OBSERVATION)
            || *basis <= hold
            || report.id <= *basis
        {
            return Err(invalid());
        }
        assigned_records.extend([report.id, *basis]);
    }
    if let Some(release) = release {
        confirmed(
            release,
            scene::PLAYER.0,
            "release convoy",
            scene::RELEASE,
            &[host.hold.ok_or_else(invalid)?],
        )?;
        assigned_records.insert(release);
    }
    if let Some(verification) = verification {
        let r = record(verification)?;
        if r.source != scene::INTERMEDIARY.0
            || !matches!(&r.content, SnapshotContent::Account { summary, basis } if summary == scene::VERIFIED && Some(*basis) == release)
        {
            return Err(invalid());
        }
        assigned_records.insert(verification);
    }
    services.sort_by_key(|&(action, _, _)| action);
    let mut previous = None;
    for &(action, outcome, operation) in &services {
        let a = dto.actions.get(action as usize).ok_or_else(invalid)?;
        let [cause] = a.proposal.causes.as_slice() else {
            return Err(invalid());
        };
        let correct_cause = match (previous, operation, facts.get(cause)) {
            (
                None,
                Op::Restrict,
                Some(Meaning::Report {
                    retention: SavedRetention::Incomplete,
                    ..
                }),
            ) => config.reaction() == Reaction::RestrictUntilCorrection,
            (
                None,
                Op::Warn,
                Some(Meaning::Report {
                    retention: SavedRetention::Incomplete,
                    ..
                }),
            ) => config.reaction() == Reaction::WarnAndContinue,
            (Some(Op::Restrict), Op::Restore, Some(Meaning::Release {}))
            | (Some(Op::Warn), Op::ResolveWarning, Some(Meaning::Release {})) => true,
            (Some(Op::Restrict), Op::Restore, Some(Meaning::Verification {}))
            | (Some(Op::Warn), Op::ResolveWarning, Some(Meaning::Verification {})) => {
                config.confidence() == Confidence::Trusted
            }
            _ => false,
        };
        if !correct_cause {
            return Err(invalid());
        }
        confirmed(
            outcome,
            scene::PROVIDER.0,
            operation.request(),
            operation.description(),
            &[*cause],
        )?;
        if !matches!(&record(outcome)?.content, SnapshotContent::ConfirmedOutcome { action: producer, .. } if *producer == action)
        {
            return Err(invalid());
        }
        assigned_records.insert(outcome);
        previous = Some(operation);
    }
    if assigned_actions.len() != dto.actions.len()
        || host.service_decision != services.last().map(|s| s.0)
        || host.access != (previous != Some(Op::Restrict))
        || (host.refused && !services.iter().any(|s| s.2 == Op::Restrict))
    {
        return Err(invalid());
    }
    let outcomes = dto
        .actions
        .iter()
        .map(|a| match a.status {
            SnapshotStatus::Confirmed(id) => Some(id),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(invalid)?;
    if outcomes.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(invalid());
    }
    // Remaining records are bounded player-facing visit observations, not domain facts.
    if dto.records.iter().any(|r| {
        !assigned_records.contains(&r.id)
            && !(r.source == scene::PLAYER.0
                && matches!(r.content, SnapshotContent::Observation(_)))
    }) {
        return Err(invalid());
    }

    let routes = [
        (
            1,
            report,
            observer,
            u64::from(config.initial_delivery_ticks()),
        ),
        (
            2,
            release,
            scene::PLAYER.0,
            u64::from(config.routine_delivery_ticks()),
        ),
        (
            3,
            verification,
            scene::INTERMEDIARY.0,
            u64::from(config.intermediary_delivery_ticks()),
        ),
    ];
    let mut delivered = BTreeMap::new();
    for (input, fact, from, delay) in routes {
        let receipt = dto.inputs.iter().find(|i| i.id == input);
        match (fact, receipt) {
            (None, None) => {}
            (Some(_), None) if retired => {}
            (Some(fact), Some(receipt)) => {
                let SnapshotAppliedInput::ManagedSchedule(receipt) = &receipt.applied else {
                    return Err(invalid());
                };
                let d = &receipt.delivery;
                if d.record != fact
                    || d.from != from
                    || d.to != scene::PROVIDER.0
                    || d.due != record(fact)?.time.checked_add(delay).ok_or_else(invalid)?
                    || d.due > 2000
                    || receipt.mode != SnapshotDeliveryMode::AbstractReceipt
                    || matches!(
                        receipt.state,
                        SnapshotDeliveryState::Rejected {
                            reason: SnapshotDeliveryRejection::Unloaded,
                            ..
                        }
                    )
                {
                    return Err(invalid());
                }
                if let SnapshotDeliveryState::Delivered { at } = receipt.state {
                    delivered.insert(fact, at);
                }
            }
            _ => return Err(invalid()),
        }
    }
    if dto.inputs.iter().any(|i| !(1..=3).contains(&i.id))
        || dto
            .pending
            .iter()
            .any(|q| !matches!(q.managed_input, Some(1..=3)))
    {
        return Err(invalid());
    }
    for &(_, outcome, operation) in &services {
        let SnapshotContent::ConfirmedOutcome { action, .. } = &record(outcome)?.content else {
            return Err(invalid());
        };
        let cause = dto.actions[*action as usize].proposal.causes[0];
        let outcome_time = record(outcome)?.time;
        if !delivered.get(&cause).is_some_and(|at| *at <= outcome_time) {
            return Err(invalid());
        }
        let correction = delivered
            .iter()
            .filter(|(id, at)| **id < outcome && **at <= outcome_time)
            .map(|(&id, _)| id)
            .find(|id| {
                matches!(facts.get(id), Some(Meaning::Release {}))
                    || (config.confidence() == Confidence::Trusted
                        && matches!(facts.get(id), Some(Meaning::Verification {})))
            });
        // A correction created later in the same hour cannot undo the earlier decision.
        match operation {
            Op::Restrict | Op::Warn if correction.is_some() => return Err(invalid()),
            Op::Restore | Op::ResolveWarning if correction != Some(cause) => return Err(invalid()),
            _ => {}
        }
    }
    let mut expected_known: BTreeSet<_> = dto.records.iter().map(|r| (r.source, r.id)).collect();
    for &fact in delivered.keys() {
        expected_known.insert((scene::PROVIDER.0, fact));
    }
    if let Some(release) = release.filter(|_| verification.is_some()) {
        expected_known.insert((scene::INTERMEDIARY.0, release));
    }
    let actual_known: BTreeSet<_> = dto
        .known
        .iter()
        .flat_map(|k| k.records.iter().map(move |&r| (k.entity, r)))
        .collect();
    // Explicit questions may hand the report to the player; no clock inference is used.
    if let Some(report) = report.filter(|r| actual_known.contains(&(scene::PLAYER.0, *r))) {
        expected_known.insert((scene::PLAYER.0, report));
    }
    if dto
        .known
        .iter()
        .any(|k| !(1..=4).contains(&k.entity) || k.records.is_empty())
        || actual_known != expected_known
    {
        return Err(invalid());
    }
    // Retained unmanaged traffic can only be the two explicit synchronous handoffs.
    for entry in &dto.trace {
        let d = match &entry.event {
            SnapshotEvent::Scheduled(d) | SnapshotEvent::Delivered(d) => d,
            _ => continue,
        };
        if dto.inputs.iter().any(
            |i| matches!(&i.applied, SnapshotAppliedInput::ManagedSchedule(r) if r.delivery == *d),
        ) {
            continue;
        }
        let question = Some(d.record) == report && d.from == observer && d.to == scene::PLAYER.0;
        let handoff = verification.is_some()
            && Some(d.record) == release
            && d.from == scene::PLAYER.0
            && d.to == scene::INTERMEDIARY.0;
        if !(question || handoff) || d.due != entry.time {
            return Err(invalid());
        }
    }
    Ok(())
}

pub fn read(path: &Path) -> Result<scene::Convoy, String> {
    let bytes = session_io::read_bytes(path)?;
    let envelope: Envelope =
        serde_json::from_slice(&bytes).map_err(|e| format!("invalid Convoy snapshot JSON: {e}"))?;
    envelope.restore()
}
pub fn save(scene: &scene::Convoy, path: &Path) -> Result<(), SaveError> {
    let envelope = Envelope::capture(scene)?;
    envelope.clone().restore()?;
    let bytes = serde_json::to_vec(&envelope)
        .map_err(|e| format!("cannot serialize Convoy snapshot: {e}"))?;
    if bytes.len() > session_io::MAX_SNAPSHOT_BYTES {
        return Err("Convoy snapshot exceeds file limit".into());
    }
    session_io::replace(path, &bytes)
}
#[allow(dead_code)] // The one-shot CLI uses read; embedders can replace a live host atomically.
pub fn restore_into(scene: &mut scene::Convoy, path: &Path) -> Result<(), String> {
    let candidate = read(path)?;
    *scene = candidate;
    Ok(())
}

#[cfg(test)]
#[path = "convoy_acceptance.rs"]
mod acceptance;

// Historical Core-admitted state; not reachable through fresh configured commands.
#[cfg(test)]
fn historical_near_text() -> Envelope {
    let mut c = scene::Convoy::with_config(
        config::validate(config::Input {
            trace_capacity: 1024,
            ..config::DEFAULT_INPUT
        })
        .unwrap(),
    );
    for action in [1, 6, 6, 6, 5, 3, 4, 6, 5] {
        assert_eq!(c.act(action), 0);
    }
    let mut old = Envelope::capture(&c).unwrap();
    old.core.actions[0].proposal.rule = "a".repeat(4096);
    for entry in &mut old.core.trace {
        if entry.event == SnapshotEvent::Proposed(0) {
            entry.rule = "a".repeat(4096);
        }
    }
    let mut c = old.restore().unwrap();
    let prior = c.core.usage().text_bytes;
    c.core.observe(scene::PLAYER, "x".into());
    let overhead = c.core.usage().text_bytes - prior - 1;
    let target = 1024 * 1024 - 1;
    while c.core.usage().text_bytes + 2000 + overhead <= target {
        c.core.observe(scene::PLAYER, "x".repeat(2000));
    }
    let remaining = target - c.core.usage().text_bytes;
    let mut old = Envelope::capture(&c).unwrap();
    let SnapshotContent::Observation(text) = &mut old.core.records.last_mut().unwrap().content
    else {
        panic!("observation")
    };
    text.push_str(&"x".repeat(remaining));
    assert!(text.len() <= 4096);
    old
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    fn temporary() -> PathBuf {
        std::env::temp_dir().join(format!(
            "convoy-codec-{}-{}.json",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }
    fn configured(input: config::Input<'_>) -> scene::Convoy {
        scene::Convoy::with_config(config::validate(input).unwrap())
    }
    fn fresh() -> scene::Convoy {
        configured(config::Input {
            trace_capacity: 0,
            ..config::DEFAULT_INPUT
        })
    }
    fn acts(c: &mut scene::Convoy, actions: &[u32]) {
        for &action in actions {
            assert_eq!(c.act(action), 0);
        }
    }
    fn bytes(c: &scene::Convoy) -> Vec<u8> {
        let saved = Envelope::capture(c).unwrap();
        saved.clone().restore().unwrap();
        serde_json::to_vec(&saved).unwrap()
    }
    fn roundtrip(c: &scene::Convoy) -> scene::Convoy {
        let encoded = bytes(c);
        let restored = serde_json::from_slice::<Envelope>(&encoded)
            .unwrap()
            .restore()
            .unwrap();
        assert_eq!(bytes(&restored), encoded);
        restored
    }
    fn full() -> scene::Convoy {
        let mut c = fresh();
        acts(&mut c, &[1, 2, 6, 6, 6, 5, 3, 4, 6, 5, 6, 6, 5]);
        assert!(c.access && c.refused);
        assert_eq!(c.facts.len(), 5);
        c
    }
    fn state(residency: Residency, availability: Availability) -> EntityState {
        EntityState::Present {
            residency,
            availability,
        }
    }

    #[test]
    fn full_core_and_host_roundtrip_is_byte_equivalent_and_continues() {
        let mut uninterrupted = full();
        let mut restored = roundtrip(&uninterrupted);
        let path = temporary();
        save(&uninterrupted, &path).unwrap();
        assert_eq!(fs::read(&path).unwrap(), bytes(&uninterrupted));
        assert_eq!(bytes(&read(&path).unwrap()), bytes(&uninterrupted));
        for action in [2, 5, 6, 5, 6, 2] {
            assert_eq!(restored.act(action), uninterrupted.act(action));
            assert_eq!(bytes(&restored), bytes(&uninterrupted));
        }
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn valid_old_native_text_near_ceiling_loads_inert_and_remains_saveable() {
        let old = historical_near_text();
        let path = temporary();
        fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
        let mut loaded = read(&path).unwrap();
        assert_eq!(loaded.core.usage().text_bytes, 1024 * 1024 - 1);
        let before = bytes(&loaded);
        assert_eq!(
            loaded.act_checked(5),
            Err(scene::CommandError::Capacity(
                scene::limits::LimitResource::TextBytes
            ))
        );
        assert_eq!(bytes(&loaded), before);
        save(&loaded, &path).unwrap();
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(loaded.act_checked(0), Ok(()));
        assert_eq!(loaded.core.usage().text_bytes, 0);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn admission_ceiling_preserves_late_pending_routes_and_rejects_excess_without_truncation() {
        let mut c = configured(config::Input {
            trace_capacity: 0,
            initial_delivery_ticks: 1000,
            routine_delivery_ticks: 1000,
            intermediary_delivery_ticks: 1000,
            ..config::DEFAULT_INPUT
        });
        for _ in 0..1000 {
            assert_eq!(c.act(6), 0);
        }
        acts(&mut c, &[1, 3, 4]);
        let mut c = roundtrip(&c);
        assert_eq!(c.time(), 1000);
        assert_eq!(c.act(6), 2);
        assert_eq!(c.core.snapshot(&limits()).unwrap().pending.len(), 3);
        for kind in [
            scene::ProviderDelivery::Report,
            scene::ProviderDelivery::Routine,
            scene::ProviderDelivery::Intermediary,
        ] {
            let receipt = c.provider_delivery_receipt(kind).unwrap();
            assert_eq!(receipt.delivery.due, 2000);
            assert_eq!(receipt.state, DeliveryState::Pending);
        }
        // Historical saves may predate configured admission reserves. Build one
        // through the explicitly unmanaged Core API instead of bypassing new act limits.
        let report = c.report.unwrap();
        let source = c.core.record(report).unwrap().source;
        while c.core.usage().next_delivery < 4096 {
            c.core
                .schedule(source, scene::PLAYER, report, c.core.time())
                .unwrap();
            c.core.advance_to(c.core.time()).unwrap();
        }
        let mut c = roundtrip(&c);
        c.core
            .schedule(source, scene::PLAYER, report, c.core.time())
            .unwrap();
        c.core.advance_to(c.core.time()).unwrap();
        assert!(Envelope::capture(&c).unwrap().restore().is_err());

        let mut c = fresh();
        for _ in 0..4096 {
            c.core
                .observe(scene::PLAYER, "bounded visit observation".into());
        }
        let mut c = roundtrip(&c);
        c.core.observe(scene::PLAYER, "one too many".into());
        assert!(Envelope::capture(&c).is_err());
        let mut c = fresh();
        c.core.observe(scene::PLAYER, "x".repeat(4097));
        assert!(Envelope::capture(&c).is_err());
        let mut e = Envelope::capture(&fresh()).unwrap();
        e.host.journal = vec!["bounded".into(); 129];
        assert!(e.restore().is_err());
        let mut e = Envelope::capture(&fresh()).unwrap();
        e.host.journal = vec!["x".repeat(1024); 128];
        assert!(e.restore().is_err());
    }

    #[test]
    fn configured_variants_roundtrip_at_every_quiescent_boundary() {
        for observer in ["mara", "sela"] {
            for retention in ["precise", "incomplete"] {
                for reaction in ["restrict_until_correction", "warn_and_continue"] {
                    for confidence in ["trusted", "unverified"] {
                        for trace in [0, 1, 128] {
                            for delays in [
                                [0, 0, 0],
                                [3, 3, 1],
                                [3, 3, 3],
                                [0, 1, 0],
                                [1000, 1000, 1000],
                            ] {
                                let mut c = configured(config::Input {
                                    observer,
                                    retention,
                                    reaction,
                                    sela_confidence: confidence,
                                    trace_capacity: trace,
                                    initial_delivery_ticks: delays[0],
                                    routine_delivery_ticks: delays[1],
                                    intermediary_delivery_ticks: delays[2],
                                    ..config::DEFAULT_INPUT
                                });
                                for action in [5, 1, 2, 6, 6, 6, 5, 3, 4, 6, 6, 6, 5] {
                                    let mut restored = roundtrip(&c);
                                    assert_eq!(restored.act(action), c.act(action));
                                    assert_eq!(bytes(&restored), bytes(&c));
                                }
                                roundtrip(&c);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn pending_due_now_and_unloaded_delivered_receipt_restore_inertly() {
        let mut c = configured(config::Input {
            initial_delivery_ticks: 0,
            trace_capacity: 0,
            ..config::DEFAULT_INPUT
        });
        // A Core-consistent queue can be captured at its due clock; no delivery is inferred.
        c.core
            .set_entity_state(
                scene::PROVIDER,
                state(Residency::Unloaded, Availability::Available),
            )
            .unwrap();
        acts(&mut c, &[1]);
        // Host action settled this immediate receipt but execution remains gated by residency.
        assert!(matches!(
            c.provider_delivery_receipt(scene::ProviderDelivery::Report)
                .unwrap()
                .state,
            DeliveryState::Delivered { at: 0 }
        ));
        assert!(c.service_decision.is_none());
        let mut c = roundtrip(&c);
        c.set_provider_state(state(Residency::Loaded, Availability::Available))
            .unwrap();
        assert!(c.service_decision.is_none());
        let mut c = roundtrip(&c);
        assert!(c.service_decision.is_none());
        acts(&mut c, &[2]);
        assert!(!c.access && c.service_decision.is_some());

        let mut pending = fresh();
        acts(&mut pending, &[1]);
        let mut e = Envelope::capture(&pending).unwrap();
        e.core.time = 3;
        // Tracing is off, so clock advancement itself adds no trace event.
        let pending = e.restore().unwrap();
        assert!(matches!(
            pending
                .provider_delivery_receipt(scene::ProviderDelivery::Report)
                .unwrap()
                .state,
            DeliveryState::Pending
        ));
        let mut pending = roundtrip(&pending);
        assert!(pending.access);
        acts(&mut pending, &[2]);
        assert!(!pending.access);
    }

    #[test]
    fn pending_unloaded_receipt_keeps_its_outcome_and_delays_execution() {
        let mut c = fresh();
        acts(&mut c, &[1]);
        c.set_provider_state(state(Residency::Unloaded, Availability::Available))
            .unwrap();
        let mut c = roundtrip(&c);
        acts(&mut c, &[6, 6, 6]);
        assert!(c.access && c.service_decision.is_none());
        assert!(matches!(
            c.provider_delivery_receipt(scene::ProviderDelivery::Report)
                .unwrap()
                .state,
            DeliveryState::Delivered { at: 3 }
        ));
        let mut c = roundtrip(&c);
        assert_eq!(c.act(5), 1);
        c.set_provider_state(state(Residency::Loaded, Availability::Available))
            .unwrap();
        let mut c = roundtrip(&c);
        assert!(c.access);
        acts(&mut c, &[5]);
        assert!(!c.access && c.refused);
    }

    #[test]
    fn terminal_rejections_and_retirement_before_admission_remain_distinct() {
        for availability in [Availability::Unavailable, Availability::Incapable] {
            let mut c = fresh();
            acts(&mut c, &[1]);
            c.set_provider_state(state(Residency::Loaded, availability))
                .unwrap();
            acts(&mut c, &[6, 6, 6]);
            assert!(matches!(
                c.provider_delivery_receipt(scene::ProviderDelivery::Report)
                    .unwrap()
                    .state,
                DeliveryState::Rejected { .. }
            ));
            c.set_provider_state(state(Residency::Loaded, Availability::Available))
                .unwrap();
            let mut restored = roundtrip(&c);
            acts(&mut restored, &[2, 5, 6]);
            assert!(restored.access && restored.service_decision.is_none());
        }
        for before_admission in [false, true] {
            let mut c = fresh();
            if before_admission {
                c.set_provider_state(EntityState::Retired).unwrap();
            }
            acts(&mut c, &[1]);
            if !before_admission {
                c.set_provider_state(EntityState::Retired).unwrap();
            }
            acts(&mut c, &[6, 6, 6, 3, 4]);
            assert_eq!(
                c.provider_delivery_receipt(scene::ProviderDelivery::Report)
                    .is_none(),
                before_admission
            );
            if !before_admission {
                assert!(matches!(
                    c.provider_delivery_receipt(scene::ProviderDelivery::Report)
                        .unwrap()
                        .state,
                    DeliveryState::Rejected {
                        reason: DeliveryRejection::Retired,
                        ..
                    }
                ));
            }
            assert!(
                c.provider_delivery_receipt(scene::ProviderDelivery::Routine)
                    .is_none()
            );
            assert!(
                c.provider_delivery_receipt(scene::ProviderDelivery::Intermediary)
                    .is_none()
            );
            roundtrip(&c);
        }
    }

    #[test]
    fn restored_identified_routes_retry_exactly_at_full_capacity() {
        let c = full();
        let mut c = roundtrip(&c);
        let receipts: Vec<_> = (1..=3)
            .map(|id| {
                (
                    InputId(id),
                    c.core.delivery_receipt(InputId(id)).unwrap().clone(),
                )
            })
            .collect();
        let before = bytes(&c);
        for (id, r) in receipts {
            let result = c
                .core
                .schedule_managed_identified(
                    id,
                    r.delivery.from,
                    r.delivery.to,
                    r.delivery.record,
                    r.delivery.due,
                    r.mode,
                )
                .unwrap();
            assert!(result.duplicate);
        }
        assert_eq!(before, bytes(&c));
    }

    fn forge_restriction(c: &mut scene::Convoy) {
        let action = c
            .core
            .evaluate(scene::PROVIDER, |_| {
                Some(Proposal {
                    request: "restrict loading".into(),
                    causes: vec![c.report.unwrap()],
                    rule: "test inconsistent host decision".into(),
                })
            })
            .unwrap()
            .unwrap();
        c.core.accept(action).unwrap();
        let outcome = c.core.confirm(action, "Loading restricted".into()).unwrap();
        c.facts.insert(
            outcome,
            scene::Fact::Service {
                operation: scene::ServiceOperation::Restrict,
                action,
            },
        );
        c.service_decision = Some(action);
        c.access = false;
    }

    #[test]
    fn service_decisions_require_arrival_and_respect_already_arrived_corrections() {
        let mut c = fresh();
        acts(&mut c, &[1]);
        // A general Core can contain an unmanaged delivery that this host never issued.
        c.core
            .schedule(scene::WITNESS, scene::PROVIDER, c.report.unwrap(), 0)
            .unwrap();
        c.core.advance_to(0).unwrap();
        forge_restriction(&mut c);
        c.core.advance_to(3).unwrap();
        let e = Envelope::capture(&c).unwrap();
        Core::from_snapshot(e.core.clone(), &limits()).unwrap();
        assert!(e.restore().is_err());

        for correction_delay in [0, 1] {
            let mut c = configured(config::Input {
                trace_capacity: 0,
                initial_delivery_ticks: 0,
                routine_delivery_ticks: correction_delay,
                intermediary_delivery_ticks: correction_delay,
                ..config::DEFAULT_INPUT
            });
            c.set_provider_state(state(Residency::Unloaded, Availability::Available))
                .unwrap();
            acts(&mut c, &[1, 3, 4, 6]);
            c.set_provider_state(state(Residency::Loaded, Availability::Available))
                .unwrap();
            forge_restriction(&mut c);
            let e = Envelope::capture(&c).unwrap();
            Core::from_snapshot(e.core.clone(), &limits()).unwrap();
            assert!(e.restore().is_err());
        }
        // A real zero-delay hold and subsequent release can create both outcomes at hour zero.
        let mut c = configured(config::Input {
            trace_capacity: 0,
            initial_delivery_ticks: 0,
            routine_delivery_ticks: 0,
            intermediary_delivery_ticks: 0,
            ..config::DEFAULT_INPUT
        });
        acts(&mut c, &[1, 3, 4]);
        assert_eq!(c.time(), 0);
        assert!(c.access && c.service_decision.is_some());
        roundtrip(&c);
    }

    #[test]
    fn rejects_corrupt_domain_state_without_changing_live_or_future_state() {
        type Corrupt = fn(&mut Envelope);
        let mutations: &[Corrupt] = &[
            |e| e.format = "other".into(),
            |e| e.format_version = 2,
            |e| e.core_version = 2,
            |e| e.host_version = 2,
            |e| e.content_version = "other".into(),
            |e| e.core.version = 2,
            |e| e.host.configuration.schema_version = 1,
            |e| e.host.configuration.trace_capacity = 1025,
            |e| e.core.time = 1001,
            |e| e.core.next_delivery = 4097,
            |e| e.core.input_capacity = 2,
            |e| e.core.entity_capacity = 3,
            |e| e.core.entities[0].state = SnapshotEntityState::Retired,
            |e| e.host.facts.push(e.host.facts[0].clone()),
            |e| e.host.facts[0].record = u64::MAX,
            |e| {
                if let Meaning::Report { observer, .. } = &mut e.host.facts[0].meaning {
                    *observer = SavedObserver::Sela;
                }
            },
            |e| {
                if let Meaning::Report { retention, .. } = &mut e.host.facts[0].meaning {
                    *retention = SavedRetention::Precise;
                }
            },
            |e| e.host.report = e.host.release,
            |e| e.host.hold = e.host.release,
            |e| e.host.release = None,
            |e| e.host.verification = None,
            |e| e.host.held = true,
            |e| e.host.access = false,
            |e| e.host.service_decision = Some(0),
            |e| e.host.configuration.reaction = "warn_and_continue".into(),
            |e| e.host.configuration.sela_confidence = "unverified".into(),
            |e| e.core.actions[0].actor = scene::PROVIDER.0,
            |e| e.core.actions[0].status = SnapshotStatus::Accepted,
            |e| e.core.actions[0].status = SnapshotStatus::Proposed,
            |e| e.core.actions[0].status = SnapshotStatus::Rejected("no".into()),
            |e| e.core.actions[0].proposal.causes.clear(),
            |e| e.core.actions[2].proposal.causes[0] = e.host.report.unwrap(),
            |e| {
                let v = e.host.verification.unwrap() as usize;
                if let SnapshotContent::Account { basis, .. } = &mut e.core.records[v].content {
                    *basis = e.host.report.unwrap();
                }
            },
            |e| e.core.inputs[0].id = 99,
            |e| {
                e.core.inputs.remove(0);
            },
            |e| {
                if let SnapshotAppliedInput::ManagedSchedule(r) = &mut e.core.inputs[0].applied {
                    r.mode = SnapshotDeliveryMode::LoadedRecipient;
                }
            },
            |e| {
                if let SnapshotAppliedInput::ManagedSchedule(r) = &mut e.core.inputs[0].applied {
                    r.delivery.due += 1;
                }
            },
            |e| {
                if let SnapshotAppliedInput::ManagedSchedule(r) = &mut e.core.inputs[0].applied {
                    r.delivery.record = e.host.release.unwrap();
                }
            },
            |e| {
                if let SnapshotAppliedInput::ManagedSchedule(r) = &mut e.core.inputs[0].applied {
                    r.state = SnapshotDeliveryState::Rejected {
                        at: r.delivery.due,
                        reason: SnapshotDeliveryRejection::Unloaded,
                    };
                }
            },
            |e| {
                let k = e
                    .core
                    .known
                    .iter_mut()
                    .find(|k| k.entity == scene::PROVIDER.0)
                    .unwrap();
                k.records.insert(0, 0);
            },
            |e| e.core.known[0].entity = 99,
            |e| e.host.journal.push("x".repeat(4097)),
            |e| e.host.dialogue = "x".repeat(4097),
            |e| {
                let f = e
                    .host
                    .facts
                    .iter_mut()
                    .find(|f| matches!(f.meaning, Meaning::Service { .. }))
                    .unwrap();
                if let Meaning::Service { operation, .. } = &mut f.meaning {
                    *operation = Operation::Restore;
                }
            },
        ];
        let original = Envelope::capture(&full()).unwrap();
        let path = temporary();
        for (index, corrupt) in mutations.iter().enumerate() {
            let mut bad = original.clone();
            corrupt(&mut bad);
            fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
            let mut live = fresh();
            acts(&mut live, &[1, 6]);
            let before = bytes(&live);
            let mut expected = roundtrip(&live);
            assert!(
                restore_into(&mut live, &path).is_err(),
                "accepted mutation {index}"
            );
            assert_eq!(bytes(&live), before, "mutation {index}");
            acts(&mut live, &[6, 6, 5, 3, 4, 6, 5]);
            acts(&mut expected, &[6, 6, 5, 3, 4, 6, 5]);
            assert_eq!(
                bytes(&live),
                bytes(&expected),
                "future after mutation {index}"
            );
        }
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_unmanaged_queue_leaks_missing_receipts_and_unsupported_modes() {
        let mut c = fresh();
        acts(&mut c, &[1]);
        let original = Envelope::capture(&c).unwrap();
        for change in 0..5 {
            let mut e = original.clone();
            match change {
                0 => e.core.pending[0].managed_input = None,
                1 => e.core.pending[0].managed_input = Some(99),
                2 => e.core.inputs.clear(),
                3 => e.core.known.push(SnapshotKnowledge {
                    entity: 3,
                    records: vec![e.host.report.unwrap()],
                }),
                _ => {
                    e.core.pending[0].delivery.record = 0;
                }
            }
            assert!(e.restore().is_err(), "accepted queue mutation {change}");
        }
        let mut no_restriction = Envelope::capture(&fresh()).unwrap();
        no_restriction.host.refused = true;
        assert!(no_restriction.restore().is_err());
        assert!(Envelope::capture(&scene::Convoy::new()).is_err());
        assert!(
            Envelope::capture(&scene::Convoy::with_condition(scene::Condition::Control)).is_err()
        );
        assert!(
            Envelope::capture(&scene::Convoy::with_study(scene::Study::PreciseImmediate)).is_err()
        );
    }

    #[test]
    fn json_rejects_unknown_duplicate_fields_and_enum_values() {
        let good = String::from_utf8(bytes(&full())).unwrap();
        let cases = [
            good.replacen(
                "\"format_version\":1",
                "\"format_version\":1,\"format_version\":1",
                1,
            ),
            good.replacen("\"host_version\":1", "\"host_version\":1,\"unknown\":0", 1),
            good.replacen("\"held\":false", "\"held\":false,\"held\":false", 1),
            good.replacen("\"kind\":\"report\"", "\"kind\":\"report\",\"extra\":0", 1),
            good.replacen(
                "\"kind\":\"release\"",
                "\"kind\":\"release\",\"extra\":0",
                1,
            ),
            good.replacen(
                "\"kind\":\"verification\"",
                "\"kind\":\"verification\",\"extra\":0",
                1,
            ),
            good.replacen(
                "\"kind\":\"report\"",
                "\"kind\":\"report\",\"kind\":\"report\"",
                1,
            ),
            good.replacen("\"kind\":\"report\"", "\"kind\":\"foreign\"", 1),
            good.replacen("\"operation\":\"restrict\"", "\"operation\":\"unknown\"", 1),
            good.replacen(
                "\"observer\":\"mara\",\"retention\":\"incomplete\"}",
                "\"observer\":\"mara\",\"observer\":\"mara\",\"retention\":\"incomplete\"}",
                1,
            ),
        ];
        for (index, text) in cases.into_iter().enumerate() {
            assert_ne!(text, good, "ineffective mutation {index}");
            assert!(
                serde_json::from_str::<Envelope>(&text).is_err(),
                "accepted JSON mutation {index}"
            );
        }
    }

    #[test]
    fn save_rejects_invalid_candidate_before_replacing_the_destination() {
        let path = temporary();
        fs::write(&path, b"keep").unwrap();
        let mut c = full();
        c.access = false;
        assert!(matches!(
            save(&c, &path),
            Err(SaveError::BeforeReplacement(_))
        ));
        assert_eq!(fs::read(&path).unwrap(), b"keep");
        fs::remove_file(path).unwrap();
    }
}
