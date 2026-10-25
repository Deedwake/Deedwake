//! Host-only, compiled scenario runner. No scripts or expressions are accepted.
use deedwake_core::{ActionId, Event, RecordId};
use serde::Serialize;
use std::{
    collections::BTreeSet,
    env,
    ffi::OsString,
    fs::File,
    io::{self, Read, Write},
    path::PathBuf,
};
#[allow(dead_code)] // Shared scene also serves native/browser hosts.
#[path = "../../examples/repair/scene.rs"]
mod scene;
use scene::config;
#[allow(dead_code)] // Tool uses validation, while builds use code generation.
#[path = "../../examples/repair/config_build.rs"]
mod config_build;
use scene::{ActualStatus, Desk, DeveloperSnapshot, Fact, Repair, RepairAction};

const MAX_BYTES: usize = 65_536;
const MAX_STEPS: usize = 2128;
const CASES: [&str; 5] = [
    "delayed-recipients",
    "rejection-then-success",
    "equal-time",
    "repeated-actions",
    "reset",
];
const HELP: &str = "repair-tool check PATH [--json]\nrepair-tool test PATH --case NAME [--json]\nrepair-tool explain PATH --case NAME [--json]\nrepair-tool --help\n\nCases: delayed-recipients, rejection-then-success, equal-time, repeated-actions, reset.\nCases execute shared Rust using exactly PATH's validated configuration.\nEqual-time requires matching report delays and matching technician delivery delays.\nRejection-then-success requires the cable to arrive after the technician's report.\nDelayed-recipients requires different report recipient delays.\nExit codes: 0 success, 1 failed scenario expectation, 2 configuration/read error, 64 usage.\nJSON schema_version is 1; record_ref/action_ref are deterministic final-session references. Earlier reset-session step causes have null record_ref; session identifies each replay.\nLimits: 65536 input bytes, tick 1000, 2128 steps, 1024 retained trace entries.\nNo scripts, expressions, arbitrary policies, or executable fixture input.\n";

#[derive(Serialize)]
struct Diagnostic {
    code: String,
    field: Option<String>,
    filename: Option<String>,
    span: [usize; 2],
    line: usize,
    column: usize,
    message: String,
}
impl From<config_build::Diagnostic> for Diagnostic {
    fn from(d: config_build::Diagnostic) -> Self {
        Self {
            code: d.code.into(),
            field: d.field.map(|f| f.as_str().into()),
            filename: Some(d.filename),
            span: [d.span.start, d.span.end],
            line: d.line,
            column: d.column,
            message: d.message,
        }
    }
}
#[derive(Serialize)]
struct Evidence {
    #[serde(skip)]
    record: RecordId,
    record_ref: Option<usize>,
    fact: &'static str,
    source: &'static str,
    time: u64,
}
impl From<&scene::Evidence> for Evidence {
    fn from(e: &scene::Evidence) -> Self {
        Self {
            record: e.record,
            record_ref: None,
            fact: e.fact.as_str(),
            source: e.source.as_str(),
            time: e.time,
        }
    }
}
#[derive(Serialize)]
struct Decision {
    action: &'static str,
    disposition: &'static str,
    reason: &'static str,
    causes: Vec<Evidence>,
}
impl From<scene::DecisionExplanation> for Decision {
    fn from(d: scene::DecisionExplanation) -> Self {
        Self {
            action: d.action.as_str(),
            disposition: d.disposition.as_str(),
            reason: d.reason.as_str(),
            causes: d.causes.iter().map(Evidence::from).collect(),
        }
    }
}
#[derive(Serialize)]
struct Step {
    session: u32,
    index: usize,
    time: u32,
    action: &'static str,
    result: u32,
    decision: Option<Decision>,
}
#[derive(Serialize)]
struct Assertion {
    step: usize,
    time: u32,
    code: &'static str,
    expected: String,
    actual: String,
    status: &'static str,
    ok: Option<bool>,
}
#[derive(Serialize)]
struct Action {
    action_ref: usize,
    action: &'static str,
    status: &'static str,
    causes: Vec<Evidence>,
    confirmed: Option<Evidence>,
    rejection: Option<&'static str>,
}
#[derive(Serialize)]
struct Knowledge {
    desk: &'static str,
    facts: Vec<Evidence>,
}
#[derive(Serialize)]
struct Trace {
    time: u64,
    event: &'static str,
    record_ref: Option<usize>,
    action_ref: Option<usize>,
    from: Option<u64>,
    to: Option<u64>,
    due: Option<u64>,
    order: Option<usize>,
    causes: Vec<usize>,
    rule: String,
}
#[derive(Serialize)]
struct Report {
    schema_version: u32,
    session: u32,
    command: String,
    ok: bool,
    diagnostics: Vec<Diagnostic>,
    case: Option<String>,
    steps: Vec<Step>,
    assertions: Vec<Assertion>,
    actions: Vec<Action>,
    knowledge: Vec<Knowledge>,
    trace: Vec<Trace>,
    trace_truncated: bool,
    dropped_trace: u64,
}
impl Report {
    fn new(command: &str, case: Option<String>) -> Self {
        Self {
            schema_version: 1,
            session: 0,
            command: command.into(),
            ok: true,
            diagnostics: vec![],
            case,
            steps: vec![],
            assertions: vec![],
            actions: vec![],
            knowledge: vec![],
            trace: vec![],
            trace_truncated: false,
            dropped_trace: 0,
        }
    }
    fn snapshot(&mut self, snapshot: DeveloperSnapshot) {
        let mut record_ids = BTreeSet::new();
        let mut action_ids = BTreeSet::new();
        for (_, evidence) in &snapshot.known {
            for e in evidence {
                record_ids.insert(e.record);
            }
        }
        for a in &snapshot.actions {
            action_ids.insert(a.id);
            for e in &a.causes {
                record_ids.insert(e.record);
            }
            if let Some(e) = &a.confirmed {
                record_ids.insert(e.record);
            }
        }
        for e in &snapshot.trace {
            record_ids.extend(&e.causes);
            match &e.event {
                Event::Recorded(id) => {
                    record_ids.insert(*id);
                }
                Event::Scheduled(d) | Event::Delivered(d) => {
                    record_ids.insert(d.record);
                }
                Event::Proposed(id) | Event::Accepted(id) | Event::Rejected(id) => {
                    action_ids.insert(*id);
                }
                Event::Confirmed { action, record } => {
                    action_ids.insert(*action);
                    record_ids.insert(*record);
                }
            }
        }
        let records: Vec<RecordId> = record_ids.into_iter().collect();
        let actions: Vec<ActionId> = action_ids.into_iter().collect();
        let record_ref = |id: RecordId| {
            records
                .binary_search(&id)
                .expect("collected record reference")
        };
        let action_ref = |id: ActionId| {
            actions
                .binary_search(&id)
                .expect("collected action reference")
        };
        let evidence = |e: &scene::Evidence| {
            let mut value = Evidence::from(e);
            value.record_ref = Some(record_ref(e.record));
            value
        };
        for step in &mut self.steps {
            if step.session == self.session
                && let Some(decision) = &mut step.decision
            {
                for cause in &mut decision.causes {
                    cause.record_ref = records.binary_search(&cause.record).ok();
                }
            }
        }
        self.actions = snapshot
            .actions
            .iter()
            .map(|a| Action {
                action_ref: action_ref(a.id),
                action: a.action.as_str(),
                status: a.status.as_str(),
                causes: a.causes.iter().map(evidence).collect(),
                confirmed: a.confirmed.as_ref().map(evidence),
                rejection: a.rejection.map(|r| r.as_str()),
            })
            .collect();
        self.knowledge = snapshot
            .known
            .iter()
            .map(|(desk, facts)| Knowledge {
                desk: desk.as_str(),
                facts: facts.iter().map(evidence).collect(),
            })
            .collect();
        self.trace = snapshot
            .trace
            .into_iter()
            .map(|entry| {
                let mut t = Trace {
                    time: entry.time,
                    event: "",
                    record_ref: None,
                    action_ref: None,
                    from: None,
                    to: None,
                    due: None,
                    order: None,
                    causes: entry.causes.into_iter().map(record_ref).collect(),
                    rule: entry.rule,
                };
                let scheduled = matches!(&entry.event, Event::Scheduled(_));
                match entry.event {
                    Event::Recorded(id) => {
                        t.event = "recorded";
                        t.record_ref = Some(record_ref(id));
                    }
                    Event::Scheduled(d) | Event::Delivered(d) => {
                        t.event = if scheduled { "scheduled" } else { "delivered" };
                        t.record_ref = Some(record_ref(d.record));
                        t.from = Some(d.from.0);
                        t.to = Some(d.to.0);
                        t.due = Some(d.due);
                        t.order = Some(d.order);
                    }
                    Event::Proposed(id) => {
                        t.event = "proposed";
                        t.action_ref = Some(action_ref(id));
                    }
                    Event::Accepted(id) => {
                        t.event = "accepted";
                        t.action_ref = Some(action_ref(id));
                    }
                    Event::Rejected(id) => {
                        t.event = "host_rejected";
                        t.action_ref = Some(action_ref(id));
                    }
                    Event::Confirmed { action, record } => {
                        t.event = "confirmed";
                        t.action_ref = Some(action_ref(action));
                        t.record_ref = Some(record_ref(record));
                    }
                }
                t
            })
            .collect();
        self.dropped_trace = snapshot.dropped_trace;
        self.trace_truncated = snapshot.dropped_trace != 0;
    }
}

struct Args {
    command: String,
    path: PathBuf,
    case: Option<String>,
    json: bool,
}
fn args(raw: Vec<OsString>) -> Result<Option<Args>, String> {
    if raw.len() > 6 || raw.iter().any(|s| s.len() > 4096) {
        return Err("arguments exceed count or 4096-byte argument limit".into());
    }
    if raw.len() == 1 && raw[0] == "--help" {
        return Ok(None);
    }
    let command = raw
        .first()
        .and_then(|s| s.to_str())
        .ok_or("missing command")?;
    if !matches!(command, "check" | "test" | "explain") {
        return Err("expected check, test, explain, or --help".into());
    }
    let path = raw.get(1).ok_or("missing configuration path")?;
    if path.is_empty() || path.to_string_lossy().starts_with('-') {
        return Err("expected configuration path immediately after command".into());
    }
    let mut result = Args {
        command: command.into(),
        path: PathBuf::from(path),
        case: None,
        json: false,
    };
    let mut i = 2;
    while i < raw.len() {
        match raw[i].to_str() {
            Some("--json") if !result.json => result.json = true,
            Some("--case") if result.command != "check" && result.case.is_none() => {
                i += 1;
                let name = raw
                    .get(i)
                    .and_then(|s| s.to_str())
                    .ok_or("--case requires a built-in name")?;
                if !CASES.contains(&name) {
                    return Err("unknown built-in case".into());
                }
                result.case = Some(name.into());
            }
            _ => return Err("unrecognized, misplaced, or repeated argument".into()),
        }
        i += 1;
    }
    if result.command != "check" && result.case.is_none() {
        return Err("test and explain require --case NAME".into());
    }
    Ok(Some(result))
}
fn source_diagnostic(
    path: &str,
    code: &str,
    prefix: &str,
    start: usize,
    end: usize,
    message: String,
) -> Diagnostic {
    Diagnostic {
        code: code.into(),
        field: None,
        filename: Some(path.into()),
        span: [start, end],
        line: prefix.bytes().filter(|b| *b == b'\n').count() + 1,
        column: prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1,
        message,
    }
}
fn load(path: &PathBuf) -> Result<config::ValidatedConfig, Vec<Diagnostic>> {
    let filename = path.to_string_lossy();
    let io_error = |e: io::Error| {
        vec![source_diagnostic(
            &filename,
            "read_error",
            "",
            0,
            0,
            format!("cannot read configuration: {}", e.kind()),
        )]
    };
    let metadata = std::fs::metadata(path).map_err(io_error)?;
    if !metadata.is_file() {
        return Err(vec![source_diagnostic(
            &filename,
            "not_regular_file",
            "",
            0,
            0,
            "configuration must be a regular file".into(),
        )]);
    }
    let file = File::open(path).map_err(io_error)?;
    if !file.metadata().map_err(io_error)?.is_file() {
        return Err(vec![source_diagnostic(
            &filename,
            "not_regular_file",
            "",
            0,
            0,
            "configuration must be a regular file".into(),
        )]);
    }
    let mut file = file.take((MAX_BYTES + 1) as u64);
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(io_error)?;
    if bytes.len() > MAX_BYTES {
        return Err(vec![source_diagnostic(
            &filename,
            "input_too_large",
            "",
            0,
            0,
            format!("configuration exceeds {MAX_BYTES} bytes"),
        )]);
    }
    let source = std::str::from_utf8(&bytes).map_err(|e| {
        let start = e.valid_up_to();
        let prefix = std::str::from_utf8(&bytes[..start]).expect("valid UTF-8 prefix");
        vec![source_diagnostic(
            &filename,
            "invalid_utf8",
            prefix,
            start,
            start + e.error_len().unwrap_or(bytes.len() - start),
            "configuration must be valid UTF-8".into(),
        )]
    })?;
    config_build::validate_source(source, &filename)
        .map_err(|errors| errors.into_iter().map(Diagnostic::from).collect())
}

struct Runner {
    scene: Repair,
    report: Report,
}
impl Runner {
    fn assertion(
        &mut self,
        code: &'static str,
        expected: impl ToString,
        actual: impl ToString,
    ) -> bool {
        let expected = expected.to_string();
        let actual = actual.to_string();
        let ok = expected == actual;
        self.report.ok &= ok;
        self.report.assertions.push(Assertion {
            step: self.report.steps.len(),
            time: self.scene.time(),
            code,
            expected,
            actual,
            status: if ok { "passed" } else { "failed" },
            ok: Some(ok),
        });
        ok
    }
    fn act(&mut self, number: u32, expected: u32) {
        if self.report.steps.len() >= MAX_STEPS {
            self.assertion("step_budget", "at most 2128", "exceeded");
            return;
        }
        let action = [
            "reset",
            "submit",
            "inspect",
            "repair",
            "collect",
            "ask_intake",
            "wait",
        ][number as usize];
        let decision = match number {
            2 => Some(RepairAction::Inspect),
            3 => Some(RepairAction::Repair),
            4 => Some(RepairAction::Collect),
            _ => None,
        }
        .map(|a| self.scene.explain(a).into());
        let time = self.scene.time();
        let result = self.scene.act(number);
        if number == 0 && result == 0 {
            self.report.session += 1;
        }
        let time = if number == 0 { self.scene.time() } else { time };
        self.report.steps.push(Step {
            session: self.report.session,
            index: self.report.steps.len() + 1,
            time,
            action,
            result,
            decision,
        });
        if result != expected {
            self.assertion("action_result", expected, result);
        }
    }
    fn until(&mut self, target: u64) -> bool {
        if target > u64::from(config::MAX_TIME) {
            self.assertion(
                "fixture_clock_bound",
                "target tick <= 1000",
                format!("target tick {target}"),
            );
            return false;
        }
        while u64::from(self.scene.time()) < target {
            if self.report.steps.len() >= MAX_STEPS {
                self.assertion("step_budget", "at most 2128", "exceeded");
                return false;
            }
            self.act(6, 0);
        }
        true
    }
    fn knows(&self, desk: Desk, fact: Fact) -> bool {
        self.scene
            .developer_snapshot()
            .known
            .iter()
            .any(|(d, facts)| *d == desk && facts.iter().any(|e| e.fact == fact))
    }
    fn known(&mut self, desk: Desk, fact: Fact, expected: bool) {
        self.assertion(
            match fact {
                Fact::DonorReport => "recipient_report",
                Fact::Ready => "recipient_ready",
                _ => "recipient_fact",
            },
            format!("{}:{}={expected}", desk.as_str(), fact.as_str()),
            format!(
                "{}:{}={}",
                desk.as_str(),
                fact.as_str(),
                self.knows(desk, fact)
            ),
        );
    }
    fn finish(&mut self, inspect: bool) -> bool {
        let c = self.scene.configuration();
        if inspect {
            if !self.until(c.report_to_technician_ticks()) {
                return false;
            }
            self.act(2, 0);
        }
        if !self.until(c.cable_receipt_at_tick()) {
            return false;
        }
        self.act(3, 0);
        self.assertion(
            "repair_confirmed",
            true,
            self.scene.developer_snapshot().repaired,
        );
        let repaired_at = u64::from(self.scene.time());
        let mut checkpoints = vec![repaired_at];
        for delay in [
            c.technician_to_intake_ticks(),
            c.technician_to_collection_ticks(),
        ] {
            if delay > 0 {
                checkpoints.push(repaired_at + delay - 1);
            }
            checkpoints.push(repaired_at + delay);
        }
        checkpoints.sort_unstable();
        checkpoints.dedup();
        for tick in checkpoints {
            if !self.until(tick) {
                return false;
            }
            self.known(
                Desk::Intake,
                Fact::Ready,
                tick >= repaired_at + c.technician_to_intake_ticks(),
            );
            self.known(
                Desk::Collection,
                Fact::Ready,
                tick >= repaired_at + c.technician_to_collection_ticks(),
            );
        }
        self.act(4, 0);
        self.assertion(
            "collection_confirmed",
            true,
            self.scene.developer_snapshot().collected,
        );
        true
    }
    fn run(&mut self, case: &str) {
        let c = self.scene.configuration();
        match case {
            "delayed-recipients" => {
                if !self.assertion(
                    "distinct_report_delays",
                    true,
                    c.report_to_intake_ticks() != c.report_to_technician_ticks(),
                ) {
                    return;
                }
                self.act(1, 0);
                let mut ticks = vec![0];
                for delay in [c.report_to_intake_ticks(), c.report_to_technician_ticks()] {
                    if delay > 0 {
                        ticks.push(delay - 1);
                    }
                    ticks.push(delay);
                }
                ticks.sort_unstable();
                ticks.dedup();
                for tick in ticks {
                    self.until(tick);
                    self.known(
                        Desk::Intake,
                        Fact::DonorReport,
                        tick >= c.report_to_intake_ticks(),
                    );
                    self.known(
                        Desk::Technician,
                        Fact::DonorReport,
                        tick >= c.report_to_technician_ticks(),
                    );
                }
                self.finish(true);
            }
            "rejection-then-success" => {
                if !self.assertion(
                    "cable_arrives_after_report",
                    true,
                    c.cable_receipt_at_tick() > c.report_to_technician_ticks(),
                ) {
                    return;
                }
                self.act(1, 0);
                self.until(c.report_to_technician_ticks());
                self.act(2, 0);
                self.act(3, 0);
                let snapshot = self.scene.developer_snapshot();
                self.assertion(
                    "host_rejected",
                    "host_rejected",
                    snapshot
                        .actions
                        .last()
                        .map_or("missing", |a| a.status.as_str()),
                );
                self.assertion("rejection_did_not_repair", false, snapshot.repaired);
                self.finish(false);
                self.assertion(
                    "rejection_retained",
                    true,
                    self.scene
                        .developer_snapshot()
                        .actions
                        .iter()
                        .any(|a| a.status == ActualStatus::HostRejected),
                );
            }
            "equal-time" => {
                let reports = self.assertion(
                    "equal_report_delays",
                    c.report_to_intake_ticks(),
                    c.report_to_technician_ticks(),
                );
                let ready = self.assertion(
                    "equal_ready_delays",
                    c.technician_to_intake_ticks(),
                    c.technician_to_collection_ticks(),
                );
                if !reports || !ready {
                    return;
                }
                self.act(1, 0);
                self.until(c.report_to_intake_ticks());
                self.known(Desk::Intake, Fact::DonorReport, true);
                self.known(Desk::Technician, Fact::DonorReport, true);
                self.finish(true);
                let snapshot = self.scene.developer_snapshot();
                let deliveries: Vec<_> = snapshot
                    .trace
                    .iter()
                    .filter_map(|t| match &t.event {
                        Event::Delivered(d) => Some((t.time, d.order)),
                        _ => None,
                    })
                    .collect();
                if snapshot.dropped_trace > 0 {
                    self.report.assertions.push(Assertion {
                        step: self.report.steps.len(),
                        time: self.scene.time(),
                        code: "equal_time_schedule_order",
                        expected: "complete retained trace".into(),
                        actual: "trace truncated; delivery order not checked".into(),
                        status: "not_checked",
                        ok: None,
                    });
                } else {
                    self.assertion(
                        "equal_time_schedule_order",
                        true,
                        deliveries
                            .windows(2)
                            .all(|p| p[0].0 != p[1].0 || p[0].1 < p[1].1),
                    );
                }
            }
            "repeated-actions" => {
                self.act(1, 0);
                self.act(1, 1);
                self.until(c.report_to_technician_ticks());
                self.act(2, 0);
                self.act(2, 1);
                if !self.finish(false) {
                    return;
                }
                let before = self.scene.developer_snapshot();
                for _ in 0..3 {
                    for action in [1, 2, 3, 4] {
                        self.act(action, 1);
                    }
                }
                self.assertion(
                    "repeats_preserve_state",
                    true,
                    before == self.scene.developer_snapshot(),
                );
            }
            "reset" => {
                self.act(1, 0);
                if !self.finish(true) {
                    return;
                }
                let before = self.scene.developer_snapshot();
                self.act(0, 0);
                self.assertion(
                    "reset_preserves_configuration",
                    true,
                    self.scene.configuration() == c,
                );
                self.assertion(
                    "reset_restores_initial_state",
                    true,
                    self.scene.developer_snapshot() == Repair::with_config(c).developer_snapshot(),
                );
                self.act(1, 0);
                if !self.finish(true) {
                    return;
                }
                self.assertion(
                    "reset_replay_identical",
                    true,
                    before == self.scene.developer_snapshot(),
                );
            }
            _ => unreachable!("allowlisted case"),
        }
    }
}

fn emit(report: &Report, json: bool) -> io::Result<()> {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    if json {
        serde_json::to_writer(&mut out, report)?;
        writeln!(out)?;
    } else {
        writeln!(
            out,
            "{} {}{}",
            report.command,
            if report.ok { "ok" } else { "failed" },
            report
                .case
                .as_ref()
                .map(|c| format!(" case={c}"))
                .unwrap_or_default()
        )?;
        for d in &report.diagnostics {
            writeln!(
                out,
                "{}:{}:{} [{}] {} (bytes {}..{})",
                d.filename.as_deref().unwrap_or("<arguments>"),
                d.line,
                d.column,
                d.code,
                d.message,
                d.span[0],
                d.span[1]
            )?;
        }
        for a in &report.assertions {
            if a.ok == Some(false) {
                writeln!(
                    out,
                    "case={} step={} tick={} [{}] expected {}; actual {}",
                    report.case.as_deref().unwrap_or("none"),
                    a.step,
                    a.time,
                    a.code,
                    a.expected,
                    a.actual
                )?;
            } else if a.ok.is_none() {
                writeln!(
                    out,
                    "case={} step={} tick={} [{}] not_checked: {}",
                    report.case.as_deref().unwrap_or("none"),
                    a.step,
                    a.time,
                    a.code,
                    a.actual
                )?;
            }
        }
        if report.command == "explain" {
            for s in &report.steps {
                if let Some(d) = &s.decision {
                    writeln!(
                        out,
                        "step={} tick={} {}: {} ({})",
                        s.index, s.time, d.action, d.disposition, d.reason
                    )?;
                    for cause in &d.causes {
                        writeln!(
                            out,
                            "  cause {} from {} at tick {}",
                            cause.fact, cause.source, cause.time
                        )?;
                    }
                }
            }
            for a in &report.actions {
                writeln!(
                    out,
                    "action={} {} status={}",
                    a.action_ref, a.action, a.status
                )?;
            }
        }
        writeln!(
            out,
            "steps={} assertions={} retained_trace={} dropped_trace={} trace_truncated={}",
            report.steps.len(),
            report.assertions.len(),
            report.trace.len(),
            report.dropped_trace,
            report.trace_truncated
        )?;
    }
    Ok(())
}
fn main() {
    let raw: Vec<_> = env::args_os().skip(1).collect();
    let json_requested = raw.iter().any(|s| s == "--json");
    let (report, json, exit) = match args(raw) {
        Ok(None) => {
            print!("{HELP}");
            return;
        }
        Err(message) => {
            let mut r = Report::new("usage", None);
            r.ok = false;
            r.diagnostics.push(Diagnostic {
                code: "usage".into(),
                field: None,
                filename: None,
                span: [0, 0],
                line: 1,
                column: 1,
                message,
            });
            (r, json_requested, 64)
        }
        Ok(Some(a)) => {
            let mut report = Report::new(&a.command, a.case.clone());
            match load(&a.path) {
                Err(diagnostics) => {
                    report.ok = false;
                    report.diagnostics = diagnostics;
                    (report, a.json, 2)
                }
                Ok(configuration) => {
                    if let Some(case) = a.case {
                        let mut runner = Runner {
                            scene: Repair::with_config(configuration),
                            report,
                        };
                        runner.run(&case);
                        runner.report.snapshot(runner.scene.developer_snapshot());
                        report = runner.report;
                    }
                    let exit = if report.ok { 0 } else { 1 };
                    (report, a.json, exit)
                }
            }
        }
    };
    if emit(&report, json).is_err() {
        std::process::exit(2);
    }
    std::process::exit(exit);
}
