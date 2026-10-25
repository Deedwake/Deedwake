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
use scene::DeveloperSnapshot;
#[allow(dead_code)] // Case IDs also serve the browser host.
#[path = "../../examples/repair/scenarios.rs"]
mod scenarios;
use scenarios::Case;

const MAX_BYTES: usize = 65_536;
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
impl From<scenarios::ScenarioStep> for Step {
    fn from(step: scenarios::ScenarioStep) -> Self {
        Self {
            session: step.session,
            index: step.index,
            time: step.time,
            action: step.action,
            result: step.result,
            decision: step.decision.map(Decision::from),
        }
    }
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
impl From<scenarios::ScenarioAssertion> for Assertion {
    fn from(assertion: scenarios::ScenarioAssertion) -> Self {
        Self {
            step: assertion.step,
            time: assertion.time,
            code: assertion.code,
            expected: assertion.expected,
            actual: assertion.actual,
            status: assertion.status,
            ok: assertion.ok,
        }
    }
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
                if Case::from_name(name).is_none() {
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
                        let run = scenarios::run_case(
                            configuration,
                            Case::from_name(&case).expect("allowlisted case"),
                        );
                        report.session = run.report.session;
                        report.ok = run.report.ok;
                        report.steps = run.report.steps.into_iter().map(Step::from).collect();
                        report.assertions = run
                            .report
                            .assertions
                            .into_iter()
                            .map(Assertion::from)
                            .collect();
                        report.snapshot(run.scene.developer_snapshot());
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
