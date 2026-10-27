use serde_json::Value;
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "convoy-native-session-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
    fn config(&self, confidence: &str, reaction: &str) -> PathBuf {
        let p = self.path("convoy.toml");
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let s = fs::read_to_string(root.join("example-web/convoy.toml"))
            .unwrap()
            .replace(
                "sela_confidence = \"trusted\"",
                &format!("sela_confidence = \"{confidence}\""),
            )
            .replace(
                "reaction = \"restrict_until_correction\"",
                &format!("reaction = \"{reaction}\""),
            );
        fs::write(&p, s).unwrap();
        p
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn raw(args: &[OsString]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_convoy-tool"))
        .args(args)
        .output()
        .unwrap()
}
fn run(
    source: &Path,
    load: bool,
    ops: &[(&str, &str)],
    save: Option<&Path>,
    expected: i32,
) -> Value {
    let mut args = vec![
        OsString::from(if load { "--load" } else { "--config" }),
        source.into(),
        "--json".into(),
        "--trace".into(),
    ];
    for (flag, value) in ops {
        args.push((*flag).into());
        args.push((*value).into());
    }
    if let Some(path) = save {
        args.push("--save".into());
        args.push(path.into());
    }
    let out = raw(&args);
    assert_eq!(
        out.status.code(),
        Some(expected),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stderr.is_empty());
    serde_json::from_slice(&out.stdout).unwrap()
}
fn same_state(a: &Value, b: &Value) {
    for key in [
        "time", "dialogue", "journal", "choices", "provider", "receipts", "trace",
    ] {
        assert_eq!(a[key], b[key], "{key}");
    }
}

#[test]
fn separate_process_pending_and_offscreen_receipt_restore_is_inert_and_continuation_matches() {
    for confidence in ["trusted", "unverified"] {
        for reaction in ["restrict_until_correction", "warn_and_continue"] {
            let f = Scratch::new();
            let cfg = f.config(confidence, reaction);
            let pending = f.path("pending.json");
            let identical = f.path("identical.json");
            let resumed_file = f.path("resumed.json");
            let full_file = f.path("full.json");
            let prefix = [
                ("--action", "hold"),
                ("--action", "ask"),
                ("--action", "wait"),
                ("--provider", "unloaded"),
                ("--action", "wait"),
                ("--action", "wait"),
            ];
            let saved = run(&cfg, false, &prefix, Some(&pending), 0);
            assert_eq!(saved["provider"]["residency"], "unloaded");
            assert_eq!(saved["receipts"][0]["state"], "delivered");
            assert_eq!(saved["receipts"][0]["at"], 3);
            let loaded = run(&pending, true, &[], Some(&identical), 0);
            same_state(&saved, &loaded);
            assert_eq!(fs::read(&pending).unwrap(), fs::read(&identical).unwrap());
            let suffix = [
                ("--provider", "loaded"),
                ("--action", "visit"),
                ("--action", "release"),
                ("--action", "handoff"),
                ("--action", "wait"),
                ("--action", "visit"),
                ("--action", "wait"),
                ("--action", "wait"),
                ("--action", "visit"),
            ];
            let resumed = run(&pending, true, &suffix, Some(&resumed_file), 0);
            let all: Vec<_> = prefix.into_iter().chain(suffix).collect();
            let full = run(&cfg, false, &all, Some(&full_file), 0);
            same_state(&resumed, &full);
            assert_eq!(
                fs::read(resumed_file).unwrap(),
                fs::read(full_file).unwrap()
            );
        }
    }
}

#[test]
fn pending_correction_and_terminal_receipts_survive_new_processes_without_resend() {
    let f = Scratch::new();
    let cfg = f.config("trusted", "restrict_until_correction");
    let saved = f.path("pending.json");
    let prefix = [
        ("--action", "hold"),
        ("--action", "wait"),
        ("--action", "wait"),
        ("--action", "wait"),
        ("--action", "visit"),
        ("--action", "release"),
        ("--action", "handoff"),
        ("--provider", "unavailable"),
    ];
    let first = run(&cfg, false, &prefix, Some(&saved), 0);
    assert_eq!(first["receipts"][1]["state"], "pending");
    assert_eq!(first["receipts"][2]["state"], "pending");
    let receipt = run(&saved, true, &[("--action", "wait")], Some(&saved), 0);
    assert_eq!(receipt["receipts"][2]["state"], "rejected");
    assert_eq!(receipt["receipts"][2]["reason"], "unavailable");
    let restored = run(
        &saved,
        true,
        &[
            ("--provider", "loaded"),
            ("--action", "visit"),
            ("--action", "wait"),
            ("--action", "wait"),
            ("--action", "visit"),
        ],
        None,
        0,
    );
    assert_eq!(restored["receipts"][2]["state"], "rejected");
    assert!(
        restored["dialogue"]
            .as_str()
            .unwrap()
            .contains("At hour 6, I restored")
    );
    let retired = run(
        &cfg,
        false,
        &[
            ("--provider", "retired"),
            ("--action", "hold"),
            ("--action", "release"),
            ("--action", "handoff"),
        ],
        Some(&saved),
        0,
    );
    assert!(
        retired["receipts"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["receipt"].is_null())
    );
    let loaded = run(&saved, true, &[], None, 0);
    same_state(&retired, &loaded);
}

#[test]
fn invalid_files_and_failed_save_leave_existing_destination_untouched() {
    let f = Scratch::new();
    let cfg = f.config("trusted", "restrict_until_correction");
    let saved = f.path("valid.json");
    run(&cfg, false, &[("--action", "hold")], Some(&saved), 0);
    let good = fs::read(&saved).unwrap();
    for (name, bytes) in [
        ("bad", b"{".to_vec()),
        ("duplicate", b"{\"format\":\"x\",\"format\":\"x\"}".to_vec()),
        (
            "deep",
            format!("{}0{}", "[".repeat(65), "]".repeat(65)).into_bytes(),
        ),
        ("large", vec![b' '; 8 * 1024 * 1024 + 1]),
    ] {
        let path = f.path(name);
        fs::write(&path, bytes).unwrap();
        let report = run(&path, true, &[], Some(&saved), 2);
        assert_eq!(report["diagnostics"][0]["code"], "snapshot_load");
        assert_eq!(fs::read(&saved).unwrap(), good);
    }
    let missing_parent = f.path("missing/target.json");
    let report = run(&saved, true, &[], Some(&missing_parent), 2);
    assert_eq!(report["diagnostics"][0]["code"], "snapshot_save");
    assert_eq!(fs::read(&saved).unwrap(), good);
}

#[test]
fn tool_usage_and_provider_rejection_are_structured_and_operations_remain_ordered() {
    let help = raw(&["--help".into()]);
    assert!(help.status.success());
    assert!(
        String::from_utf8(help.stdout)
            .unwrap()
            .contains("privileged host controls")
    );
    let no_source = raw(&["--json".into()]);
    assert_eq!(no_source.status.code(), Some(64));
    let f = Scratch::new();
    let cfg = f.config("trusted", "restrict_until_correction");
    let result = run(
        &cfg,
        false,
        &[
            ("--provider", "unloaded"),
            ("--action", "visit"),
            ("--provider", "loaded"),
            ("--action", "visit"),
        ],
        None,
        1,
    );
    assert_eq!(result["steps"][1]["result"], 1);
    assert_eq!(result["steps"][3]["result"], 0);
    assert_eq!(result["time"], 0);
}
