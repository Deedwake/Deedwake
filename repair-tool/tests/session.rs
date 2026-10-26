use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "repair-session-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn config(&self, capacity: usize) -> PathBuf {
        let path = self.0.join("repair.toml");
        fs::write(&path, format!("schema_version=1\npolicy_set=\"repair_cafe_v1\"\ntrace_capacity={capacity}\nreport_to_intake_ticks=1\nreport_to_technician_ticks=2\ntechnician_to_intake_ticks=1\ntechnician_to_collection_ticks=2\ncable_receipt_at_tick=4\n")).unwrap();
        path
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_repair-tool"))
        .args(args)
        .output()
        .unwrap()
}
fn result(args: &[&str], exit: i32) -> Value {
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
fn path(path: &std::path::Path) -> &str {
    path.to_str().unwrap()
}
fn session(
    source_flag: &str,
    source: &std::path::Path,
    actions: &[&str],
    save: Option<&std::path::Path>,
) -> Value {
    let mut args = vec!["session", source_flag, path(source), "--json"];
    for action in actions {
        args.extend(["--action", action]);
    }
    if let Some(save) = save {
        args.extend(["--save", path(save)]);
    }
    result(&args, 0)
}

#[test]
fn independent_processes_restore_then_continue_without_replay() {
    for capacity in [0, 1, 128] {
        let f = Fixture::new();
        let config = f.config(capacity);
        let saved = f.path("session.json");
        let unchanged = f.path("unchanged.json");
        let resumed = f.path("resumed.json");
        let direct = f.path("direct.json");
        // Each helper launches a separate process and waits for that process to exit.
        let first = session(
            "--config",
            &config,
            &["submit", "wait", "wait", "inspect", "repair"],
            Some(&saved),
        );
        assert!(
            first["actions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["status"] == "host_rejected")
        );
        let loaded = session("--load", &saved, &[], Some(&unchanged));
        assert_eq!(fs::read(&saved).unwrap(), fs::read(&unchanged).unwrap());
        assert_eq!(loaded["actions"], first["actions"]);
        assert_eq!(loaded["knowledge"], first["knowledge"]);
        assert!(loaded["steps"].as_array().unwrap().is_empty());
        let final_report = session(
            "--load",
            &saved,
            &["wait", "wait", "repair", "wait", "wait", "collect"],
            Some(&resumed),
        );
        assert!(
            final_report["actions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["action"] == "collect" && a["status"] == "confirmed")
        );
        session(
            "--config",
            &config,
            &[
                "submit", "wait", "wait", "inspect", "repair", "wait", "wait", "repair", "wait",
                "wait", "collect",
            ],
            Some(&direct),
        );
        assert_eq!(fs::read(&resumed).unwrap(), fs::read(&direct).unwrap());
        let snapshot: Value = serde_json::from_slice(&fs::read(&resumed).unwrap()).unwrap();
        assert_eq!(snapshot["host"]["repaired"], true);
        assert_eq!(snapshot["host"]["collected"], true);
        assert_eq!(
            snapshot["host"]["configuration"]["trace_capacity"],
            capacity
        );
    }
}

#[test]
fn strict_versions_unknown_duplicate_nested_fields_and_corruptions_are_rejected() {
    let f = Fixture::new();
    let config = f.config(128);
    let saved = f.path("session.json");
    session(
        "--config",
        &config,
        &["submit", "wait", "wait", "inspect", "repair"],
        Some(&saved),
    );
    let original = fs::read_to_string(&saved).unwrap();
    let original_value: Value = serde_json::from_str(&original).unwrap();
    let bad = f.path("bad.json");
    let protected = f.path("protected.json");
    fs::write(&protected, b"previous").unwrap();
    let mut corruptions = vec![];
    for field in ["format_version", "core_version", "host_version"] {
        let mut v = original_value.clone();
        v[field] = json!(2);
        corruptions.push(v.to_string());
    }
    let mut v = original_value.clone();
    v["content_version"] = json!("unknown");
    corruptions.push(v.to_string());
    let mut v = original_value.clone();
    v["core"]["version"] = json!(2);
    corruptions.push(v.to_string());
    for pointer in [
        "",
        "/core",
        "/host",
        "/host/configuration",
        "/host/action_history/0",
    ] {
        let mut v = original_value.clone();
        v.pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown".into(), json!(1));
        corruptions.push(v.to_string());
    }
    for (pointer, value) in [
        ("/host/report", json!(0)),
        ("/host/diagnosis", json!(0)),
        ("/host/repaired", json!(true)),
        ("/host/collected", json!(true)),
        ("/host/action_history/0/action_ref", json!(999)),
        ("/host/action_history/0/action", json!("collect")),
        ("/host/configuration/trace_capacity", json!(1)),
        ("/core/time", json!(1001)),
        ("/host/journal", json!(["unlinked"])),
        ("/host/dialogue", json!("x".repeat(4097))),
        ("/host/journal", json!(vec!["x"; 129])),
    ] {
        let mut v = original_value.clone();
        *v.pointer_mut(pointer).unwrap() = value;
        corruptions.push(v.to_string());
    }
    corruptions.push(original.replacen(
        "\"format_version\":1",
        "\"format_version\":1,\"format_version\":1",
        1,
    ));
    corruptions.push(original.replacen("\"host\":{", "\"host\":{\"repaired\":false,", 1));
    corruptions.push(original.replacen(
        "\"configuration\":{",
        "\"configuration\":{\"schema_version\":1,",
        1,
    ));
    corruptions.push(format!("{original} trailing"));
    corruptions.push(original[..original.len() / 2].to_owned());
    corruptions.push(format!("{}0{}", "[".repeat(65), "]".repeat(65)));
    for (index, source) in corruptions.iter().enumerate() {
        fs::write(&bad, source).unwrap();
        let report = result(
            &[
                "session",
                "--load",
                path(&bad),
                "--save",
                path(&protected),
                "--json",
            ],
            2,
        );
        assert_eq!(
            report["diagnostics"][0]["code"], "snapshot_load",
            "corruption {index}"
        );
        assert_eq!(fs::read(&protected).unwrap(), b"previous");
    }
}

#[test]
fn bounded_input_usage_and_real_disk_errors_are_structured() {
    let f = Fixture::new();
    let config = f.config(128);
    let saved = f.path("session.json");
    session("--config", &config, &[], Some(&saved));
    for args in [
        vec!["session", "--json"],
        vec![
            "session",
            "--config",
            path(&config),
            "--load",
            path(&saved),
            "--json",
        ],
        vec![
            "session",
            "--load",
            path(&saved),
            "--action",
            "script",
            "--json",
        ],
        vec!["session", "--load", path(&saved), "--json", "--json"],
    ] {
        assert_eq!(result(&args, 64)["diagnostics"][0]["code"], "usage");
    }
    let mut args = vec!["session", "--load", path(&saved), "--json"];
    for _ in 0..2129 {
        args.extend(["--action", "wait"]);
    }
    result(&args, 64);
    let large = f.path("large.json");
    fs::write(&large, vec![b' '; 8 * 1024 * 1024 + 1]).unwrap();
    assert!(
        result(&["session", "--load", path(&large), "--json"], 2)["diagnostics"][0]["message"]
            .as_str()
            .unwrap()
            .contains("8388608")
    );
    let invalid_utf8 = f.path("utf8.json");
    fs::write(&invalid_utf8, b"\xff").unwrap();
    result(&["session", "--load", path(&invalid_utf8), "--json"], 2);
    result(&["session", "--load", path(&f.0), "--json"], 2);
    result(
        &["session", "--load", path(&f.path("missing")), "--json"],
        2,
    );
    let destination_dir = f.path("existing-directory");
    fs::create_dir(&destination_dir).unwrap();
    assert_eq!(
        result(
            &[
                "session",
                "--load",
                path(&saved),
                "--save",
                path(&destination_dir),
                "--json"
            ],
            2
        )["diagnostics"][0]["code"],
        "snapshot_save"
    );
    assert!(destination_dir.is_dir());
    let missing_parent = f.path("missing-parent/session.json");
    result(
        &[
            "session",
            "--load",
            path(&saved),
            "--save",
            path(&missing_parent),
            "--json",
        ],
        2,
    );
    assert!(!fs::read_dir(&f.0).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")
    }));
}
