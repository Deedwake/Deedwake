//! Native-only durable sessions for the synchronous synthetic Repair host.
use crate::{Diagnostic, Report, Step, config, load, scene};
use deedwake_core::{Content, Core, CoreSnapshot, SnapshotLimits, Status};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub const MAX_SNAPSHOT_BYTES: usize = 8 * 1024 * 1024;
const MAX_HOST_ENTRIES: usize = 128;
const MAX_HOST_TEXT_BYTES: usize = 4096;
const MAX_HOST_TOTAL_TEXT_BYTES: usize = 128 * 1024;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

fn limits() -> SnapshotLimits {
    SnapshotLimits {
        max_records: 256,
        max_actions: MAX_HOST_ENTRIES,
        max_pending: 256,
        max_inputs: 0,
        max_entities: 0,
        max_trace_capacity: 1024,
        max_known_owners: 6,
        max_known_edges: 1536,
        max_string_bytes: MAX_HOST_TEXT_BYTES,
        max_total_string_bytes: 1024 * 1024,
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    schema_version: u64,
    policy_set: String,
    trace_capacity: u64,
    report_to_intake_ticks: u64,
    report_to_technician_ticks: u64,
    technician_to_intake_ticks: u64,
    technician_to_collection_ticks: u64,
    cable_receipt_at_tick: u64,
}
impl From<config::ValidatedConfig> for Configuration {
    fn from(c: config::ValidatedConfig) -> Self {
        Self {
            schema_version: 1,
            policy_set: "repair_cafe_v1".into(),
            trace_capacity: c.trace_capacity() as u64,
            report_to_intake_ticks: c.report_to_intake_ticks(),
            report_to_technician_ticks: c.report_to_technician_ticks(),
            technician_to_intake_ticks: c.technician_to_intake_ticks(),
            technician_to_collection_ticks: c.technician_to_collection_ticks(),
            cable_receipt_at_tick: c.cable_receipt_at_tick(),
        }
    }
}
impl Configuration {
    fn validate(&self) -> Result<config::ValidatedConfig, String> {
        config::validate(config::Input {
            schema_version: self.schema_version,
            policy_set: &self.policy_set,
            trace_capacity: self.trace_capacity,
            report_to_intake_ticks: self.report_to_intake_ticks,
            report_to_technician_ticks: self.report_to_technician_ticks,
            technician_to_intake_ticks: self.technician_to_intake_ticks,
            technician_to_collection_ticks: self.technician_to_collection_ticks,
            cable_receipt_at_tick: self.cable_receipt_at_tick,
        })
        .map_err(|errors| format!("invalid saved configuration: {errors:?}"))
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Inspect,
    Repair,
    Collect,
}
impl Kind {
    fn action(self) -> scene::RepairAction {
        match self {
            Self::Inspect => scene::RepairAction::Inspect,
            Self::Repair => scene::RepairAction::Repair,
            Self::Collect => scene::RepairAction::Collect,
        }
    }
}
impl From<scene::RepairAction> for Kind {
    fn from(action: scene::RepairAction) -> Self {
        match action {
            scene::RepairAction::Inspect => Self::Inspect,
            scene::RepairAction::Repair => Self::Repair,
            scene::RepairAction::Collect => Self::Collect,
        }
    }
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Rejection {
    CableReceiptNotArrived,
    RepairNotExecuted,
}
impl Rejection {
    fn reason(self) -> scene::DecisionReason {
        match self {
            Self::CableReceiptNotArrived => scene::DecisionReason::CableReceiptNotArrived,
            Self::RepairNotExecuted => scene::DecisionReason::RepairNotExecuted,
        }
    }
    fn capture(reason: scene::DecisionReason) -> Result<Self, String> {
        match reason {
            scene::DecisionReason::CableReceiptNotArrived => Ok(Self::CableReceiptNotArrived),
            scene::DecisionReason::RepairNotExecuted => Ok(Self::RepairNotExecuted),
            _ => Err("unsupported host rejection reason".into()),
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct History {
    action: Kind,
    action_ref: u64,
    rejection: Option<Rejection>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Host {
    configuration: Configuration,
    report: Option<u64>,
    diagnosis: Option<u64>,
    rejected_repair: Option<u64>,
    repaired: bool,
    collected: bool,
    action_history: Vec<History>,
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
fn snapshot_error(e: impl std::fmt::Debug) -> String {
    format!("invalid Core snapshot: {e:?}")
}
impl Envelope {
    fn capture(repair: &scene::Repair) -> Result<Self, String> {
        validate_text(
            &repair.journal,
            &repair.dialogue,
            repair.action_history.len(),
        )?;
        let core = &repair.core;
        Ok(Self {
            format: "deedwake.repair.session".into(),
            format_version: 1,
            core_version: 1,
            host_version: 1,
            content_version: "repair_cafe_v1".into(),
            core: core.snapshot(&limits()).map_err(snapshot_error)?,
            host: Host {
                configuration: repair.configuration.into(),
                report: repair
                    .report
                    .map(|id| core.record_position(id))
                    .transpose()
                    .map_err(snapshot_error)?,
                diagnosis: repair
                    .diagnosis
                    .map(|id| core.record_position(id))
                    .transpose()
                    .map_err(snapshot_error)?,
                rejected_repair: repair
                    .rejected_repair
                    .map(|id| core.action_position(id))
                    .transpose()
                    .map_err(snapshot_error)?,
                repaired: repair.repaired,
                collected: repair.collected,
                action_history: repair
                    .action_history
                    .iter()
                    .map(|(kind, id, reason)| {
                        Ok(History {
                            action: (*kind).into(),
                            action_ref: core.action_position(*id).map_err(snapshot_error)?,
                            rejection: reason.map(Rejection::capture).transpose()?,
                        })
                    })
                    .collect::<Result<_, String>>()?,
                journal: repair.journal.clone(),
                dialogue: repair.dialogue.clone(),
            },
        })
    }

    fn restore(self) -> Result<scene::Repair, String> {
        if self.format != "deedwake.repair.session"
            || self.format_version != 1
            || self.core_version != 1
            || self.host_version != 1
            || self.content_version != "repair_cafe_v1"
        {
            return Err("unsupported snapshot format, Core, host, or content version".into());
        }
        let configuration = self.host.configuration.validate()?;
        if self.core.time > u64::from(config::MAX_TIME)
            || self.core.trace_capacity != configuration.trace_capacity() as u64
            || self.core.next_delivery > 256
            || self.core.input_capacity != 0
            || self.core.entity_capacity != 0
            || !self.core.inputs.is_empty()
            || !self.core.entities.is_empty()
        {
            return Err("saved clock, trace capacity, or host mode does not match Repair".into());
        }
        let record_count = self.core.records.len();
        let action_count = self.core.actions.len();
        let core = Core::from_snapshot(self.core, &limits()).map_err(snapshot_error)?;
        validate_host(&core, &self.host, record_count, action_count)?;
        let record = |raw| core.record_id(raw).map_err(snapshot_error);
        let action = |raw| core.action_id(raw).map_err(snapshot_error);
        let report = self.host.report.map(record).transpose()?;
        let diagnosis = self.host.diagnosis.map(record).transpose()?;
        let rejected_repair = self.host.rejected_repair.map(action).transpose()?;
        let action_history = self
            .host
            .action_history
            .into_iter()
            .map(|h| {
                Ok((
                    h.action.action(),
                    action(h.action_ref)?,
                    h.rejection.map(Rejection::reason),
                ))
            })
            .collect::<Result<_, String>>()?;
        // Construction does not advance time, evaluate policies, or execute actions.
        Ok(scene::Repair {
            core,
            configuration,
            report,
            diagnosis,
            rejected_repair,
            repaired: self.host.repaired,
            collected: self.host.collected,
            action_history,
            journal: self.host.journal,
            dialogue: self.host.dialogue,
        })
    }
}

fn validate_text(journal: &[String], dialogue: &str, history_len: usize) -> Result<(), String> {
    if history_len > MAX_HOST_ENTRIES
        || journal.len() > MAX_HOST_ENTRIES
        || dialogue.len() > MAX_HOST_TEXT_BYTES
        || journal.iter().any(|s| s.len() > MAX_HOST_TEXT_BYTES)
        || journal.iter().map(String::len).sum::<usize>() + dialogue.len()
            > MAX_HOST_TOTAL_TEXT_BYTES
    {
        return Err("saved Repair host exceeds count or text limits".into());
    }
    Ok(())
}

fn validate_host(
    core: &Core,
    host: &Host,
    record_count: usize,
    action_count: usize,
) -> Result<(), String> {
    use scene::{
        COLLECTED_CODE, COLLECTION, DIAGNOSIS_CODE, DONOR, PLAYER, READY_CODE, REPORT_CODE, STOCK,
        TECHNICIAN,
    };
    let invalid = || "saved Repair domain references, flags, or history disagree".to_owned();
    validate_text(&host.journal, &host.dialogue, host.action_history.len())?;
    if host.action_history.len() != action_count {
        return Err(invalid());
    }
    let journal: BTreeSet<&str> = host.journal.iter().map(String::as_str).collect();
    if journal.len() != host.journal.len()
        || !(journal.contains(host.dialogue.as_str())
            || host.dialogue == scene::WAIT_DIALOGUE
            || (host.journal.is_empty() && host.dialogue == scene::INITIAL_DIALOGUE))
    {
        return Err(invalid());
    }
    let mut reports = vec![];
    let mut diagnoses = vec![];
    let mut ready = vec![];
    let mut collected = vec![];
    let mut original = vec![];
    let mut cable = vec![];
    let mut player = BTreeSet::new();
    let mut player_order = vec![];
    for position in 0..record_count {
        let id = core.record_id(position as u64).map_err(snapshot_error)?;
        let r = core.record(id).ok_or_else(invalid)?;
        match (r.source, &r.content) {
            (PLAYER, Content::Observation(text)) if journal.contains(text.as_str()) => {
                if !player.insert(text.as_str()) {
                    return Err(invalid());
                }
                player_order.push(text.as_str());
            }
            (STOCK, Content::Observation(text)) if text == scene::CABLE_CODE && r.time == 0 => {
                cable.push(id)
            }
            (DONOR, Content::Observation(text))
                if text == "Fictional lamp does not light; cause not inspected" =>
            {
                original.push(id)
            }
            (DONOR, Content::Account { summary, basis }) if summary == REPORT_CODE => {
                let b = core.record(*basis).ok_or_else(invalid)?;
                if b.source != DONOR
                    || b.time != r.time
                    || !matches!(&b.content, Content::Observation(t) if t == "Fictional lamp does not light; cause not inspected")
                {
                    return Err(invalid());
                }
                reports.push(id);
            }
            (TECHNICIAN, Content::ConfirmedOutcome { description, .. })
                if description == DIAGNOSIS_CODE =>
            {
                diagnoses.push(id)
            }
            (TECHNICIAN, Content::ConfirmedOutcome { description, .. })
                if description == READY_CODE =>
            {
                ready.push(id)
            }
            (COLLECTION, Content::ConfirmedOutcome { description, .. })
                if description == COLLECTED_CODE =>
            {
                collected.push(id)
            }
            _ => return Err(invalid()),
        }
    }
    for (message, present) in [
        (scene::REPORT, !reports.is_empty()),
        (scene::DIAGNOSIS, !diagnoses.is_empty()),
        (scene::READY, !ready.is_empty()),
        (scene::COLLECTED, !collected.is_empty()),
        (scene::CABLE, !cable.is_empty()),
    ] {
        if journal.contains(message) && !present {
            return Err(invalid());
        }
        if message != scene::CABLE && present && !journal.contains(message) {
            return Err(invalid());
        }
    }
    let report = host
        .report
        .map(|v| core.record_id(v))
        .transpose()
        .map_err(snapshot_error)?;
    let diagnosis = host
        .diagnosis
        .map(|v| core.record_id(v))
        .transpose()
        .map_err(snapshot_error)?;
    if cable.len() != 1
        || original.len() != reports.len()
        || reports.len() > 1
        || diagnoses.len() > 1
        || ready.len() > 1
        || collected.len() > 1
        || report != reports.first().copied()
        || diagnosis != diagnoses.first().copied()
        || host.repaired != !ready.is_empty()
        || host.collected != !collected.is_empty()
        || (!diagnoses.is_empty() && reports.is_empty())
        || (!ready.is_empty() && diagnoses.is_empty())
        || (host.collected && !host.repaired)
        || player != journal
        || player_order != host.journal.iter().map(String::as_str).collect::<Vec<_>>()
    {
        return Err(invalid());
    }
    let mut rejected_repair = None;
    for (position, history) in host.action_history.iter().enumerate() {
        if history.action_ref != position as u64 {
            return Err(invalid());
        }
        let action_id = core.action_id(history.action_ref).map_err(snapshot_error)?;
        let action = core.action(action_id).ok_or_else(invalid)?;
        let kind = history.action.action();
        let (request, required, outcome) = match kind {
            scene::RepairAction::Inspect => ("inspect donated lamp", report, DIAGNOSIS_CODE),
            scene::RepairAction::Repair => ("replace cable and mark ready", diagnosis, READY_CODE),
            scene::RepairAction::Collect => (
                "hand over repaired lamp",
                ready.first().copied(),
                COLLECTED_CODE,
            ),
        };
        let required = required.ok_or_else(invalid)?;
        let causes = &action.proposal.causes;
        if action.actor != kind.actor()
            || action.proposal.request != request
            || causes.first() != Some(&required)
            || causes
                .iter()
                .any(|id| core.record(*id).and_then(scene::evidence).is_none())
            || (kind != scene::RepairAction::Repair && causes.len() != 1)
            || (kind == scene::RepairAction::Repair
                && (causes.len() > 2 || (causes.len() == 2 && causes[1] != cable[0])))
        {
            return Err(invalid());
        }
        match &action.status {
            Status::Proposed if history.rejection.is_none() => {}
            Status::Accepted
                if history.rejection.is_none()
                    && (kind != scene::RepairAction::Repair || causes.len() == 2) => {}
            Status::Confirmed(id) if history.rejection.is_none() => {
                let record = core.record(*id).ok_or_else(invalid)?;
                if !matches!(&record.content, Content::ConfirmedOutcome { description, action: producer } if description == outcome && *producer == action_id)
                    || (kind == scene::RepairAction::Repair && causes.len() != 2)
                {
                    return Err(invalid());
                }
            }
            Status::Rejected(reason) => {
                let rejection = history.rejection.map(Rejection::reason);
                if kind == scene::RepairAction::Repair
                    && rejection == Some(scene::DecisionReason::CableReceiptNotArrived)
                    && reason == scene::REJECTION
                    && causes.len() == 1
                    && rejected_repair.is_none()
                {
                    rejected_repair = Some(action_id);
                } else {
                    return Err(invalid());
                }
            }
            _ => return Err(invalid()),
        }
    }
    if host
        .rejected_repair
        .map(|raw| core.action_id(raw))
        .transpose()
        .map_err(snapshot_error)?
        != rejected_repair
    {
        return Err(invalid());
    }
    // The Repair host owns only these six desks, and has no lifecycle/input ledger.
    let dto = core.snapshot(&limits()).map_err(snapshot_error)?;
    let configuration = host.configuration.validate()?;
    let mut expected_known = BTreeSet::new();
    let mut expected_pending = BTreeSet::new();
    for position in 0..record_count {
        let id = core.record_id(position as u64).map_err(snapshot_error)?;
        let record = core.record(id).ok_or_else(invalid)?;
        expected_known.insert((record.source.0, position as u64));
        let fact = scene::evidence(record).map(|e| e.fact);
        let routes = match fact {
            Some(scene::Fact::CableReceipt) => {
                vec![(scene::TECHNICIAN.0, configuration.cable_receipt_at_tick())]
            }
            Some(scene::Fact::DonorReport) => vec![
                (
                    scene::INTAKE.0,
                    record.time + configuration.report_to_intake_ticks(),
                ),
                (
                    scene::TECHNICIAN.0,
                    record.time + configuration.report_to_technician_ticks(),
                ),
            ],
            Some(scene::Fact::Diagnosis | scene::Fact::Ready) => vec![
                (
                    scene::INTAKE.0,
                    record.time + configuration.technician_to_intake_ticks(),
                ),
                (
                    scene::COLLECTION.0,
                    record.time + configuration.technician_to_collection_ticks(),
                ),
            ],
            _ => vec![],
        };
        for (to, due) in routes {
            if due <= core.time() {
                expected_known.insert((to, position as u64));
            } else {
                expected_pending.insert((position as u64, record.source.0, to, due));
            }
        }
        let message = match fact {
            Some(scene::Fact::CableReceipt) => Some(scene::CABLE),
            Some(scene::Fact::DonorReport) => Some(scene::REPORT),
            Some(scene::Fact::Diagnosis) => Some(scene::DIAGNOSIS),
            Some(scene::Fact::Ready) => Some(scene::READY),
            Some(scene::Fact::Collected) => Some(scene::COLLECTED),
            None => None,
        };
        if message.is_some_and(|message| journal.contains(message)) {
            expected_known.insert((scene::PLAYER.0, position as u64));
        }
    }
    let actual_known: BTreeSet<_> = dto
        .known
        .iter()
        .flat_map(|known| known.records.iter().map(move |id| (known.entity, *id)))
        .collect();
    let actual_pending: BTreeSet<_> = dto
        .pending
        .iter()
        .map(|queued| {
            let delivery = &queued.delivery;
            (delivery.record, delivery.from, delivery.to, delivery.due)
        })
        .collect();
    if dto
        .known
        .iter()
        .any(|known| !(1..=6).contains(&known.entity))
        || actual_known != expected_known
        || actual_pending != expected_pending
        || actual_pending.len() != dto.pending.len()
        || dto
            .pending
            .iter()
            .any(|queued| queued.managed_input.is_some())
    {
        return Err(invalid());
    }
    Ok(())
}

fn check_nesting(bytes: &[u8]) -> Result<(), String> {
    let (mut depth, mut in_string, mut escaped) = (0usize, false, false);
    for &byte in bytes {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
        } else {
            match byte {
                b'"' => in_string = true,
                b'[' | b'{' => {
                    depth += 1;
                    if depth > 64 {
                        return Err("snapshot nesting exceeds 64 levels".into());
                    }
                }
                b']' | b'}' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    Ok(())
}

#[allow(dead_code)] // Native hosts can replace a live session as well as start one.
pub fn restore_into(repair: &mut scene::Repair, path: &Path) -> Result<(), String> {
    let candidate = read(path)?;
    *repair = candidate;
    Ok(())
}

pub fn read(path: &Path) -> Result<scene::Repair, String> {
    if !fs::metadata(path)
        .map_err(|e| format!("cannot inspect snapshot: {}", e.kind()))?
        .is_file()
    {
        return Err("snapshot must be a regular file".into());
    }
    let file = File::open(path).map_err(|e| format!("cannot open snapshot: {}", e.kind()))?;
    if !file
        .metadata()
        .map_err(|e| format!("cannot inspect snapshot: {}", e.kind()))?
        .is_file()
    {
        return Err("snapshot must be a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take((MAX_SNAPSHOT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("cannot read snapshot: {}", e.kind()))?;
    if bytes.len() > MAX_SNAPSHOT_BYTES {
        return Err("snapshot exceeds 8388608 bytes".into());
    }
    check_nesting(&bytes)?;
    let envelope: Envelope =
        serde_json::from_slice(&bytes).map_err(|e| format!("invalid snapshot JSON: {e}"))?;
    envelope.restore()
}

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[derive(Debug)]
pub enum SaveError {
    BeforeReplacement(String),
    DurabilityUncertain(String),
}
impl From<String> for SaveError {
    fn from(message: String) -> Self {
        Self::BeforeReplacement(message)
    }
}
impl From<&str> for SaveError {
    fn from(message: &str) -> Self {
        Self::BeforeReplacement(message.into())
    }
}
impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BeforeReplacement(message) | Self::DurabilityUncertain(message) => {
                f.write_str(message)
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SaveStage {
    Write,
    FileSync,
    Rename,
    DirectorySync,
}

fn replace(path: &Path, bytes: &[u8]) -> Result<(), SaveError> {
    replace_with(path, bytes, |_| Ok(()))
}

// The stage callback tests disk failures at the actual commit boundary.
fn replace_with(
    path: &Path,
    bytes: &[u8],
    stage: impl Fn(SaveStage) -> std::io::Result<()>,
) -> Result<(), SaveError> {
    if !cfg!(target_os = "linux") {
        return Err("atomic session replacement is supported only on Linux".into());
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let directory =
        File::open(parent).map_err(|e| format!("cannot open snapshot directory: {}", e.kind()))?;
    let mut created = None;
    for _ in 0..64 {
        let temp = parent.join(format!(
            ".repair-session-{}-{}.tmp",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&temp) {
            Ok(file) => {
                created = Some((Temporary(temp), file));
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(format!("cannot create snapshot temporary file: {}", e.kind()).into());
            }
        }
    }
    let (temp, mut file) = created.ok_or("cannot create unique snapshot temporary file")?;
    stage(SaveStage::Write)
        .and_then(|_| file.write_all(bytes))
        .and_then(|_| stage(SaveStage::FileSync))
        .and_then(|_| file.sync_all())
        .map_err(|e| {
            format!(
                "cannot write and sync snapshot temporary file: {}",
                e.kind()
            )
        })?;
    stage(SaveStage::Rename)
        .and_then(|_| fs::rename(&temp.0, path))
        .map_err(|e| format!("cannot atomically replace snapshot: {}", e.kind()))?;
    stage(SaveStage::DirectorySync).and_then(|_| directory.sync_all()).map_err(|e| SaveError::DurabilityUncertain(format!("snapshot replacement committed; durability uncertain because directory sync failed: {}", e.kind())))?;
    Ok(())
}
pub fn save(repair: &scene::Repair, path: &Path) -> Result<(), SaveError> {
    let envelope = Envelope::capture(repair)?;
    envelope.clone().restore()?;
    let bytes =
        serde_json::to_vec(&envelope).map_err(|e| format!("cannot serialize snapshot: {e}"))?;
    if bytes.len() > MAX_SNAPSHOT_BYTES {
        return Err("serialized snapshot exceeds 8388608 bytes".into());
    }
    replace(path, &bytes)
}

struct Args {
    source: PathBuf,
    from_snapshot: bool,
    save: Option<PathBuf>,
    actions: Vec<(&'static str, u32)>,
    json: bool,
}
fn arguments(raw: &[OsString]) -> Result<Args, String> {
    if raw.len() > 4264 || raw.iter().any(|s| s.len() > 4096) {
        return Err("session arguments exceed count or 4096-byte argument limit".into());
    }
    let mut result = Args {
        source: PathBuf::new(),
        from_snapshot: false,
        save: None,
        actions: vec![],
        json: false,
    };
    let mut source = false;
    let mut index = 1;
    while index < raw.len() {
        let flag = raw[index].to_str().ok_or("expected session option")?;
        match flag {
            "--json" if !result.json => result.json = true,
            "--config" | "--load" if !source => {
                index += 1;
                let path = raw.get(index).ok_or("session source requires a path")?;
                if path.is_empty() || path.to_string_lossy().starts_with('-') {
                    return Err("expected session source path".into());
                }
                result.source = path.into();
                result.from_snapshot = flag == "--load";
                source = true;
            }
            "--save" if result.save.is_none() => {
                index += 1;
                let path = raw.get(index).ok_or("--save requires a path")?;
                if path.is_empty() || path.to_string_lossy().starts_with('-') {
                    return Err("expected snapshot destination path".into());
                }
                result.save = Some(path.into());
            }
            "--action" => {
                index += 1;
                let action = match raw.get(index).and_then(|s| s.to_str()) {
                    Some("reset") => ("reset", 0), Some("submit") => ("submit", 1),
                    Some("inspect") => ("inspect", 2), Some("repair") => ("repair", 3),
                    Some("collect") => ("collect", 4), Some("intake") => ("intake", 5),
                    Some("wait") => ("wait", 6), _ => return Err("--action requires reset, submit, inspect, repair, collect, intake, or wait".into()),
                };
                if result.actions.len() == crate::scenarios::MAX_STEPS {
                    return Err("session exceeds 2128 actions".into());
                }
                result.actions.push(action);
            }
            _ => return Err("unrecognized, misplaced, or repeated session option".into()),
        }
        index += 1;
    }
    if !source {
        return Err("session requires exactly one --config PATH or --load SNAPSHOT".into());
    }
    Ok(result)
}

fn diagnostic(path: Option<&Path>, code: &str, message: String) -> Diagnostic {
    Diagnostic {
        code: code.into(),
        field: None,
        filename: path.map(|p| p.to_string_lossy().into_owned()),
        span: [0, 0],
        line: 1,
        column: 1,
        message,
    }
}
pub fn run(raw: Vec<OsString>) -> (Report, bool, i32) {
    let json_requested = raw.iter().any(|a| a == "--json");
    let mut report = Report::new("session", None);
    let args = match arguments(&raw) {
        Ok(args) => args,
        Err(e) => {
            report.ok = false;
            report.diagnostics.push(diagnostic(None, "usage", e));
            return (report, json_requested, 64);
        }
    };
    let candidate = if args.from_snapshot {
        read(&args.source).map_err(|e| vec![diagnostic(Some(&args.source), "snapshot_load", e)])
    } else {
        load(&args.source).map(scene::Repair::with_config)
    };
    let mut repair = match candidate {
        Ok(repair) => repair,
        Err(diagnostics) => {
            report.ok = false;
            report.diagnostics = diagnostics;
            return (report, args.json, 2);
        }
    };
    for (index, (name, action)) in args.actions.into_iter().enumerate() {
        if action == 0 {
            report.session += 1;
        }
        let result = repair.act(action);
        report.steps.push(Step {
            session: report.session,
            index: index + 1,
            time: repair.time(),
            action: name,
            result,
            decision: None,
        });
    }
    if let Some(path) = args.save
        && let Err(e) = save(&repair, &path)
    {
        let code = if matches!(&e, SaveError::DurabilityUncertain(_)) {
            "snapshot_save_uncertain"
        } else {
            "snapshot_save"
        };
        report.ok = false;
        report
            .diagnostics
            .push(diagnostic(Some(&path), code, e.to_string()));
    }
    report.snapshot(repair.developer_snapshot());
    let exit = if report.ok { 0 } else { 2 };
    (report, args.json, exit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use deedwake_core::{Proposal, SnapshotContent, SnapshotStatus};
    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "repair-session-unit-{}-{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn act(repair: &mut scene::Repair, actions: &[u32]) {
        for action in actions {
            assert_eq!(repair.act(*action), 0);
        }
    }
    fn bytes(repair: &scene::Repair) -> Vec<u8> {
        serde_json::to_vec(&Envelope::capture(repair).unwrap()).unwrap()
    }

    #[test]
    fn failed_live_restore_preserves_whole_bundle_and_future_behavior() {
        let scratch = Scratch::new();
        let path = scratch.0.join("snapshot.json");
        let mut live = scene::Repair::new();
        act(&mut live, &[1, 6, 6, 2, 3]);
        let before = bytes(&live);
        let mut control = Envelope::capture(&live).unwrap().restore().unwrap();
        let mut corrupt = Envelope::capture(&live).unwrap();
        corrupt.host.repaired = true;
        fs::write(&path, serde_json::to_vec(&corrupt).unwrap()).unwrap();
        assert!(restore_into(&mut live, &path).is_err());
        assert_eq!(bytes(&live), before);
        act(&mut live, &[6, 6, 3, 6, 6, 4]);
        act(&mut control, &[6, 6, 3, 6, 6, 4]);
        assert_eq!(bytes(&live), bytes(&control));
        save(&control, &path).unwrap();
        let mut fresh = scene::Repair::new();
        restore_into(&mut fresh, &path).unwrap();
        assert_eq!(bytes(&fresh), bytes(&control));
    }

    #[test]
    fn accepted_known_unexecuted_restores_inert_without_policy_or_delivery() {
        let mut repair = scene::Repair::new();
        act(&mut repair, &[1, 6, 6, 2, 6, 6]);
        let decision = repair.explain(scene::RepairAction::Repair);
        let id = repair
            .core
            .evaluate(scene::TECHNICIAN, |_| {
                Some(Proposal {
                    request: "replace cable and mark ready".into(),
                    causes: decision.causes.iter().map(|e| e.record).collect(),
                    rule: "synthetic accepted but not executed".into(),
                })
            })
            .unwrap()
            .unwrap();
        repair.core.accept(id).unwrap();
        repair
            .action_history
            .push((scene::RepairAction::Repair, id, None));
        let original = bytes(&repair);
        let restored = Envelope::capture(&repair).unwrap().restore().unwrap();
        assert_eq!(bytes(&restored), original);
        assert!(!restored.repaired && !restored.collected);
        assert!(matches!(
            restored.core.action(id).unwrap().status,
            Status::Accepted
        ));
        assert!(
            restored
                .developer_snapshot()
                .actions
                .iter()
                .any(|a| a.status == scene::ActualStatus::AcceptedUnconfirmed)
        );
    }

    #[test]
    fn repair_domain_validation_rejects_structurally_valid_core_corruptions() {
        let mut repair = scene::Repair::new();
        act(&mut repair, &[1, 6, 6, 2, 3]);
        let original = Envelope::capture(&repair).unwrap();
        for mutation in 0..16 {
            let mut e = original.clone();
            e.core.trace_capacity = 0;
            e.host.configuration.trace_capacity = 0;
            e.core.dropped_trace += e.core.trace.len() as u64;
            e.core.trace.clear();
            match mutation {
                0 => e.host.report = Some(0),
                1 => e.host.diagnosis = Some(0),
                2 => e.host.rejected_repair = None,
                3 => e.host.action_history[0].action = Kind::Collect,
                4 => e.host.action_history[0].action_ref = 1,
                5 => e.host.action_history[1].rejection = None,
                6 => {
                    // A Core-valid local cause is not necessarily Repair evidence.
                    e.core.records[e.host.report.unwrap() as usize].content =
                        SnapshotContent::Observation("generic local observation".into());
                }
                7 => e.core.pending[0].delivery.to = 99,
                8 => e.core.pending[0].delivery.due += 1,
                9 => e.core.known.push(deedwake_core::SnapshotKnowledge {
                    entity: 99,
                    records: vec![0],
                }),
                10 => {
                    e.core.trace_capacity = 0;
                    e.host.configuration.trace_capacity = 0;
                    e.core.dropped_trace += e.core.trace.len() as u64;
                    e.core.trace.clear();
                    e.core.next_delivery = u64::MAX;
                    e.core.dropped_trace = u64::MAX;
                }
                11 => e.host.journal.push("unlinked player knowledge".into()),
                12 => {
                    e.core.records[0].content =
                        SnapshotContent::Observation("counterfeit cable".into())
                }
                13 => e.host.journal.swap(0, 1),
                14 => {
                    e.host.journal.push(scene::READY.into());
                    e.core.dropped_trace += 1;
                    e.core.records.push(deedwake_core::SnapshotRecord {
                        id: e.core.records.len() as u64,
                        source: scene::PLAYER.0,
                        time: e.core.time,
                        content: SnapshotContent::Observation(scene::READY.into()),
                    });
                    let id = e.core.records.len() as u64 - 1;
                    e.core
                        .known
                        .iter_mut()
                        .find(|k| k.entity == scene::PLAYER.0)
                        .unwrap()
                        .records
                        .push(id);
                }
                15 => {
                    e.host.journal.remove(0);
                }
                _ => unreachable!(),
            }
            e.core
                .pending
                .sort_by_key(|p| (p.delivery.due, p.delivery.order));
            if [6, 7, 8, 10, 14].contains(&mutation) {
                Core::from_snapshot(e.core.clone(), &limits()).unwrap();
            }
            assert!(e.restore().is_err(), "corruption {mutation} loaded");
        }
        // Generic Core still supports an exhausted sequence; Repair rejects it before its unwrapping host calls.
        let mut exhausted = original.core;
        exhausted.next_delivery = u64::MAX;
        exhausted.dropped_trace = u64::MAX;
        exhausted.trace.clear();
        exhausted.trace_capacity = 0;
        Core::from_snapshot(exhausted, &limits()).unwrap();
    }

    #[test]
    fn limits_precede_host_export_clones_and_failed_save_preserves_previous_file() {
        let scratch = Scratch::new();
        let path = scratch.0.join("snapshot.json");
        let mut repair = scene::Repair::new();
        save(&repair, &path).unwrap();
        let previous = fs::read(&path).unwrap();
        repair.journal.push("x".repeat(MAX_HOST_TEXT_BYTES + 1));
        assert!(save(&repair, &path).is_err());
        assert_eq!(fs::read(&path).unwrap(), previous);
        assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 1);
    }

    #[test]
    fn each_precommit_disk_failure_preserves_old_file_and_cleans_temporary() {
        let scratch = Scratch::new();
        let path = scratch.0.join("snapshot.json");
        for failed_stage in [SaveStage::Write, SaveStage::FileSync, SaveStage::Rename] {
            fs::write(&path, b"previous").unwrap();
            let error = replace_with(&path, b"replacement", |stage| {
                if stage == failed_stage {
                    Err(std::io::Error::other("injected failure"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert!(!matches!(error, SaveError::DurabilityUncertain(_)));
            assert_eq!(fs::read(&path).unwrap(), b"previous");
            assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 1);
        }
        let error = replace_with(&path, b"replacement", |stage| {
            if stage == SaveStage::DirectorySync {
                Err(std::io::Error::other("injected failure"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert!(matches!(error, SaveError::DurabilityUncertain(_)));
        assert_eq!(fs::read(&path).unwrap(), b"replacement");
        assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 1);
    }

    #[test]
    fn host_zero_trace_rejection_equal_time_and_reset_roundtrip() {
        for capacity in [0, 1, 128] {
            let configuration = config::validate(config::Input {
                trace_capacity: capacity,
                report_to_technician_ticks: 1,
                technician_to_collection_ticks: 1,
                ..config::DEFAULT_INPUT
            })
            .unwrap();
            let mut repair = scene::Repair::with_config(configuration);
            act(&mut repair, &[1, 6, 2, 3]);
            let before = bytes(&repair);
            let mut restored = Envelope::capture(&repair).unwrap().restore().unwrap();
            assert_eq!(bytes(&restored), before);
            act(&mut repair, &[6, 6, 6, 3, 6, 4]);
            act(&mut restored, &[6, 6, 6, 3, 6, 4]);
            assert_eq!(bytes(&restored), bytes(&repair));
            act(&mut restored, &[0]);
            assert_eq!(
                bytes(&Envelope::capture(&restored).unwrap().restore().unwrap()),
                bytes(&scene::Repair::with_config(configuration))
            );
        }
    }

    #[test]
    fn malformed_accepted_repair_without_cable_is_rejected() {
        let mut repair = scene::Repair::new();
        act(&mut repair, &[1, 6, 6, 2, 3]);
        let mut e = Envelope::capture(&repair).unwrap();
        e.core.actions[1].status = SnapshotStatus::Accepted;
        e.core.trace.clear();
        e.host.rejected_repair = None;
        e.host.action_history[1].rejection = None;
        assert!(e.restore().is_err());
    }
}
