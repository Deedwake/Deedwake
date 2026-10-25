use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const DEFAULT: &str = "schema_version = 1\npolicy_set = \"repair_cafe_v1\"\ntrace_capacity = 128\nreport_to_intake_ticks = 1\nreport_to_technician_ticks = 2\ntechnician_to_intake_ticks = 1\ntechnician_to_collection_ticks = 2\ncable_receipt_at_tick = 4\n";
struct Fixture(PathBuf);
impl Fixture {
    fn new(bytes: impl AsRef<[u8]>) -> Self {
        let path = std::env::temp_dir().join(format!(
            "repair-tool-{}-{}.toml",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, bytes).unwrap();
        Self(path)
    }
    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_repair-tool"))
        .args(args)
        .output()
        .unwrap()
}
fn json(args: &[&str], exit: i32) -> Value {
    let out = run(args);
    assert_eq!(
        out.status.code(),
        Some(exit),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stderr.is_empty());
    serde_json::from_slice(&out.stdout).unwrap()
}
fn case(fixture: &Fixture, command: &str, name: &str, exit: i32) -> Value {
    json(&[command, fixture.path(), "--case", name, "--json"], exit)
}

#[test]
fn help_and_usage_are_bounded_and_deterministic() {
    let help = run(&["--help"]);
    assert!(help.status.success());
    assert!(
        String::from_utf8(help.stdout)
            .unwrap()
            .contains("Exit codes: 0 success")
    );
    for args in [
        vec!["--json"],
        vec!["check", "--json"],
        vec!["check", "f", "--case", "reset", "--json"],
        vec!["test", "f", "--json"],
        vec!["test", "f", "--case", "unknown", "--json"],
        vec!["check", "f", "--json", "--json"],
        vec!["test", "f", "--case", "reset", "--json", "surprise"],
    ] {
        let first = json(&args, 64);
        let second = json(&args, 64);
        assert_eq!(first, second);
        assert_eq!(first["schema_version"], 1);
        assert_eq!(first["ok"], false);
        assert_eq!(first["diagnostics"][0]["code"], "usage");
        assert!(first["steps"].as_array().unwrap().is_empty());
    }
    let huge = "x".repeat(4097);
    let v = json(&["check", &huge, "--json"], 64);
    assert_eq!(v["diagnostics"][0]["code"], "usage");
}
#[test]
fn check_reports_shared_parser_semantics_and_source_spans() {
    let valid = Fixture::new(DEFAULT);
    let v = json(&["check", valid.path(), "--json"], 0);
    assert_eq!(v["command"], "check");
    assert_eq!(v["ok"], true);
    assert_eq!(v["case"], Value::Null);
    assert_eq!(v["trace_truncated"], false);
    assert_eq!(v["dropped_trace"], 0);
    let bad = Fixture::new(DEFAULT.replace("trace_capacity = 128", "trace_capacity = 1025"));
    let v = json(&["check", bad.path(), "--json"], 2);
    let d = &v["diagnostics"][0];
    assert_eq!(d["code"], "out_of_range");
    assert_eq!(d["field"], "trace_capacity");
    assert_eq!(d["filename"], bad.path());
    assert_eq!(d["line"], 3);
    assert_eq!(d["column"], 18);
    let start = d["span"][0].as_u64().unwrap() as usize;
    let end = d["span"][1].as_u64().unwrap() as usize;
    let source = fs::read_to_string(&bad.0).unwrap();
    assert_eq!(&source[start..end], "1025");
    for source in [
        DEFAULT.replace("schema_version = 1", "schema_version = 2"),
        DEFAULT.replace(
            "policy_set = \"repair_cafe_v1\"",
            "policy_set = \"arbitrary_code\"",
        ),
        format!("{DEFAULT}script = \"run things\"\n"),
        "[broken".into(),
    ] {
        let f = Fixture::new(source);
        let v = json(&["check", f.path(), "--json"], 2);
        assert_eq!(v["ok"], false);
        assert!(!v["diagnostics"].as_array().unwrap().is_empty());
    }
}
#[test]
fn byte_limit_utf8_and_file_errors_have_structured_diagnostics() {
    let invalid = Fixture::new(b"# \xc3\xa9\n\xff");
    let v = json(&["check", invalid.path(), "--json"], 2);
    let d = &v["diagnostics"][0];
    assert_eq!(d["code"], "invalid_utf8");
    assert_eq!(d["line"], 2);
    assert_eq!(d["column"], 1);
    assert_eq!(d["span"], serde_json::json!([5, 6]));
    let exact = Fixture::new(format!(
        "{DEFAULT}#{}",
        " ".repeat(65536 - DEFAULT.len() - 1)
    ));
    json(&["check", exact.path(), "--json"], 0);
    let large = Fixture::new(vec![b' '; 65537]);
    let v = json(&["check", large.path(), "--json"], 2);
    assert_eq!(v["diagnostics"][0]["code"], "input_too_large");
    let missing = Fixture::new(DEFAULT);
    fs::remove_file(&missing.0).unwrap();
    let v = json(&["check", missing.path(), "--json"], 2);
    assert_eq!(v["diagnostics"][0]["code"], "read_error");
    let dir = std::env::temp_dir();
    let v = json(&["check", dir.to_str().unwrap(), "--json"], 2);
    assert_eq!(v["diagnostics"][0]["code"], "not_regular_file");
}
#[test]
fn all_compiled_cases_run_shared_scene_and_json_is_deterministic() {
    let default = Fixture::new(DEFAULT);
    let equal = Fixture::new(
        DEFAULT
            .replace(
                "report_to_technician_ticks = 2",
                "report_to_technician_ticks = 1",
            )
            .replace(
                "technician_to_collection_ticks = 2",
                "technician_to_collection_ticks = 1",
            ),
    );
    for name in [
        "delayed-recipients",
        "rejection-then-success",
        "equal-time",
        "repeated-actions",
        "reset",
    ] {
        let fixture = if name == "equal-time" {
            &equal
        } else {
            &default
        };
        let v = case(fixture, "test", name, 0);
        let second = case(fixture, "test", name, 0);
        assert_eq!(v, second);
        for field in [
            "schema_version",
            "command",
            "ok",
            "diagnostics",
            "case",
            "steps",
            "assertions",
            "actions",
            "knowledge",
            "trace",
            "trace_truncated",
            "dropped_trace",
        ] {
            assert!(v.get(field).is_some(), "missing {field}");
        }
        assert_eq!(v["case"], name);
        assert!(v["steps"].as_array().unwrap().len() <= 2128);
        assert!(
            v["assertions"]
                .as_array()
                .unwrap()
                .iter()
                .all(|a| a["ok"] == true)
        );
        assert!(
            v["actions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["status"] == "confirmed")
        );
        if name == "rejection-then-success" {
            assert!(
                v["actions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|a| a["status"] == "host_rejected"
                        && a["rejection"] == "cable_receipt_not_arrived")
            );
        }
    }
}
#[test]
fn incompatible_config_reports_case_step_expected_and_actual_without_substitution() {
    let f = Fixture::new(DEFAULT);
    let v = case(&f, "test", "equal-time", 1);
    assert_eq!(v["ok"], false);
    assert_eq!(v["case"], "equal-time");
    assert!(v["steps"].as_array().unwrap().is_empty());
    let failed = &v["assertions"][0];
    assert_eq!(failed["code"], "equal_report_delays");
    assert_eq!(failed["step"], 0);
    assert_eq!(failed["expected"], "1");
    assert_eq!(failed["actual"], "2");
    let text = run(&["test", f.path(), "--case", "equal-time"]);
    assert_eq!(text.status.code(), Some(1));
    let text = String::from_utf8(text.stdout).unwrap();
    assert!(text.contains("case=equal-time step=0 tick=0"));
    assert!(text.contains("expected 1; actual 2"));
    let late = Fixture::new(DEFAULT.replace(
        "technician_to_collection_ticks = 2",
        "technician_to_collection_ticks = 1000",
    ));
    let v = case(&late, "test", "repeated-actions", 1);
    assert!(
        v["assertions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["code"] == "fixture_clock_bound" && a["actual"] == "target tick 1003")
    );
}
#[test]
fn explain_uses_typed_decisions_and_statuses_even_with_zero_trace() {
    let f = Fixture::new(DEFAULT.replace("trace_capacity = 128", "trace_capacity = 0"));
    let v = case(&f, "explain", "rejection-then-success", 0);
    assert_eq!(v["trace"], serde_json::json!([]));
    assert_eq!(v["trace_truncated"], true);
    assert!(v["dropped_trace"].as_u64().unwrap() > 0);
    let steps = v["steps"].as_array().unwrap();
    assert!(
        steps
            .iter()
            .any(|s| s["decision"]["disposition"] == "would_reject"
                && s["decision"]["reason"] == "cable_receipt_not_arrived")
    );
    assert!(steps.iter().any(|s| {
        s["decision"]["action"] == "repair"
            && s["decision"]["disposition"] == "ready_to_execute"
            && s["decision"]["causes"]
                .as_array()
                .is_some_and(|c| c.iter().any(|e| e["fact"] == "cable_receipt"))
    }));
    assert!(
        v["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["status"] == "host_rejected")
    );
    let text = run(&["explain", f.path(), "--case", "rejection-then-success"]);
    assert!(text.status.success());
    let text = String::from_utf8(text.stdout).unwrap();
    assert!(text.contains("would_reject (cable_receipt_not_arrived)"));
    assert!(text.contains("status=confirmed"));
}
#[test]
fn reset_supports_two_maximum_length_sessions_and_zero_delays() {
    let late = Fixture::new(
        DEFAULT
            .replace(
                "report_to_intake_ticks = 1",
                "report_to_intake_ticks = 1000",
            )
            .replace(
                "report_to_technician_ticks = 2",
                "report_to_technician_ticks = 1000",
            )
            .replace(
                "technician_to_intake_ticks = 1",
                "technician_to_intake_ticks = 0",
            )
            .replace(
                "technician_to_collection_ticks = 2",
                "technician_to_collection_ticks = 0",
            ),
    );
    let v = case(&late, "test", "reset", 0);
    assert!(v["steps"].as_array().unwrap().len() > 2000);
    let zero = Fixture::new(
        DEFAULT
            .replace("report_to_intake_ticks = 1", "report_to_intake_ticks = 0")
            .replace(
                "report_to_technician_ticks = 2",
                "report_to_technician_ticks = 0",
            )
            .replace(
                "technician_to_intake_ticks = 1",
                "technician_to_intake_ticks = 0",
            )
            .replace(
                "technician_to_collection_ticks = 2",
                "technician_to_collection_ticks = 0",
            )
            .replace("cable_receipt_at_tick = 4", "cable_receipt_at_tick = 0"),
    );
    case(&zero, "test", "equal-time", 0);
    case(&zero, "test", "reset", 0);
}
#[cfg(unix)]
#[test]
fn non_utf8_path_is_accepted_without_treating_it_as_cli_syntax() {
    use std::os::unix::ffi::OsStringExt;
    let mut path = std::env::temp_dir().into_os_string().into_vec();
    path.extend(format!("/repair-tool-nonutf8-{}-", std::process::id()).bytes());
    path.push(0xff);
    let path = PathBuf::from(std::ffi::OsString::from_vec(path));
    fs::write(&path, DEFAULT).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_repair-tool"))
        .arg("check")
        .arg(&path)
        .arg("--json")
        .output()
        .unwrap();
    fs::remove_file(path).unwrap();
    assert!(out.status.success());
}

#[test]
fn evidence_references_join_final_trace_and_reset_sessions_are_explicit() {
    let f = Fixture::new(DEFAULT);
    let v = case(&f, "explain", "reset", 0);
    assert_eq!(v["session"], 1);
    let actions = v["actions"].as_array().unwrap();
    for action in actions {
        for cause in action["causes"].as_array().unwrap() {
            let reference = &cause["record_ref"];
            assert!(reference.is_u64());
            assert!(
                v["trace"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|entry| &entry["record_ref"] == reference)
            );
        }
    }
    for step in v["steps"].as_array().unwrap() {
        if let Some(causes) = step["decision"]["causes"].as_array() {
            for cause in causes {
                assert_eq!(cause["record_ref"].is_null(), step["session"] == 0);
            }
        }
    }
}
#[test]
fn equal_time_zero_trace_marks_order_check_unavailable() {
    let source = DEFAULT
        .replace("trace_capacity = 128", "trace_capacity = 0")
        .replace(
            "report_to_technician_ticks = 2",
            "report_to_technician_ticks = 1",
        )
        .replace(
            "technician_to_collection_ticks = 2",
            "technician_to_collection_ticks = 1",
        );
    let f = Fixture::new(source);
    let v = case(&f, "test", "equal-time", 0);
    let assertion = v["assertions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["code"] == "equal_time_schedule_order")
        .unwrap();
    assert_eq!(assertion["status"], "not_checked");
    assert!(assertion["ok"].is_null());
    let text = run(&["test", f.path(), "--case", "equal-time"]);
    assert!(
        String::from_utf8(text.stdout)
            .unwrap()
            .contains("not_checked: trace truncated")
    );
}
