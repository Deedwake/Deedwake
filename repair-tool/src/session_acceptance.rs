//! Local native measurements only. Freeze this profile in Git before measuring.
use super::*;
use serde_json::{Value, json};
use std::{
    hint::black_box,
    process::Command,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

const PROFILE_REVISION: u32 = 1;
const WARMUPS: usize = 3;
const SAMPLES: usize = 20;
const MAX_STEPS: usize = 2128;
// Profile revision 1: nearest-rank p95, integer nanoseconds, inclusive caps.
const CAPS_NS: [u64; 6] = [
    5_000_000,
    5_000_000,
    50_000_000,
    100_000_000,
    50_000_000,
    100_000_000,
];
const METRICS: [&str; 6] = [
    "inspection_player_output",
    "wait_due_player_output",
    "rejection_success_collection",
    "save_validate_json_fsync_rename",
    "read_decode_validate",
    "catchup_waits_tick2_to_1000",
];
const PREFIX: [u32; 5] = [1, 6, 6, 2, 3];
const SUFFIX: [u32; 8] = [5, 5, 6, 6, 3, 6, 6, 4];
const SEQUENCE: [u32; 13] = [1, 6, 6, 2, 3, 5, 5, 6, 6, 3, 6, 6, 4];

fn configurations() -> [(&'static str, config::ValidatedConfig); 3] {
    [
        ("default", config::DEFAULT_INPUT),
        (
            "equal_time",
            config::Input {
                report_to_intake_ticks: 2,
                technician_to_collection_ticks: 1,
                ..config::DEFAULT_INPUT
            },
        ),
        (
            "evicted",
            config::Input {
                trace_capacity: 1,
                ..config::DEFAULT_INPUT
            },
        ),
    ]
    .map(|(name, input)| {
        (
            name,
            config::validate(input).expect("compiled acceptance configuration"),
        )
    })
}

fn canonical(repair: &scene::Repair) -> Vec<u8> {
    serde_json::to_vec(&Envelope::capture(repair).unwrap()).unwrap()
}

fn player_output(repair: &scene::Repair) -> [String; 2] {
    [repair.output(0).unwrap(), repair.output(1).unwrap()]
}

fn actions(repair: &mut scene::Repair, sequence: &[u32]) {
    assert!(sequence.len() <= MAX_STEPS);
    for &action in sequence {
        assert_eq!(repair.act(action), 0, "acceptance action {action} failed");
        black_box(player_output(repair));
    }
}

fn pending(configuration: config::ValidatedConfig) -> scene::Repair {
    let mut repair = scene::Repair::with_config(configuration);
    actions(&mut repair, &PREFIX);
    repair
}

fn statistics(samples: &[u64], cap: u64) -> Value {
    assert!(!samples.is_empty());
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let rank = |percent: usize| sorted[(percent * sorted.len()).div_ceil(100) - 1];
    let p95 = rank(95);
    json!({"sample_count": samples.len(), "raw_ns": samples, "median_ns": rank(50),
        "p95_ns": p95, "max_ns": sorted[sorted.len()-1], "p95_cap_ns": cap, "passed": p95 <= cap,
        "percentile_method": "nearest_rank"})
}

struct WorkDirectory(PathBuf);
impl WorkDirectory {
    fn new(parent: &Path) -> Self {
        for _ in 0..64 {
            let path = parent.join(format!(
                ".deedwake-acceptance-{}-{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("cannot create acceptance work directory: {}", e.kind()),
            }
        }
        panic!("cannot create unique acceptance work directory");
    }
}
impl Drop for WorkDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn command(directory: &Path, program: &str, args: &[&str]) -> String {
    let output = Command::new(program)
        .args(args)
        .current_dir(directory)
        .output()
        .unwrap_or_else(|e| panic!("required provenance command {program} failed: {}", e.kind()));
    assert!(
        output.status.success(),
        "required provenance command {program} failed"
    );
    String::from_utf8(output.stdout)
        .expect("provenance must be UTF-8")
        .trim()
        .into()
}

fn clean_git(directory: &Path) -> String {
    let status = command(
        directory,
        "git",
        &["status", "--porcelain=v1", "--untracked-files=all"],
    );
    assert!(
        status.is_empty(),
        "acceptance requires a clean Git checkout including untracked files"
    );
    status
}

fn provenance(repository: &Path, work: &Path) -> Value {
    let rustc = command(repository, "rustc", &["-Vv"]);
    let pinned = include_str!("../../rust-toolchain.toml");
    assert!(
        rustc.lines().any(|line| line == "release: 1.99.1"),
        "acceptance requires pinned rustc 1.99.1"
    );
    let cpuinfo = fs::read_to_string("/proc/cpuinfo").expect("CPU provenance unavailable");
    let cpu = cpuinfo
        .lines()
        .find_map(|line| {
            line.strip_prefix("model name")
                .and_then(|s| s.split_once(':'))
                .map(|(_, value)| value.trim())
        })
        .expect("CPU model provenance unavailable");
    let meminfo = fs::read_to_string("/proc/meminfo").expect("memory provenance unavailable");
    let memory = meminfo
        .lines()
        .find(|line| line.starts_with("MemTotal:"))
        .expect("MemTotal provenance unavailable");
    let container_limit = [
        "/sys/fs/cgroup/memory.max",
        "/sys/fs/cgroup/memory/memory.limit_in_bytes",
    ]
    .iter()
    .find_map(|path| {
        fs::read_to_string(path)
            .ok()
            .map(|value| json!({"source": path, "value": value.trim()}))
    });
    let head = command(repository, "git", &["rev-parse", "HEAD"]);
    let tree = command(repository, "git", &["rev-parse", "HEAD^{tree}"]);
    assert_eq!(head.len(), 40, "Git HEAD unavailable");
    assert_eq!(tree.len(), 40, "Git tree unavailable");
    json!({
        "git": {"head": head, "tree": tree, "porcelain": clean_git(repository), "clean": true,
            "binary_source_identity": "not_embedded; build this exact clean checkout before invocation",
            "source_blobs": {"Cargo.lock": command(repository, "git", &["rev-parse", "HEAD:Cargo.lock"]),
                "rust-toolchain.toml": command(repository, "git", &["rev-parse", "HEAD:rust-toolchain.toml"]),
                "examples/repair/config.rs": command(repository, "git", &["rev-parse", "HEAD:examples/repair/config.rs"]),
                "repair-tool/src/session_acceptance.rs": command(repository, "git", &["rev-parse", "HEAD:repair-tool/src/session_acceptance.rs"]) }},
        "package_version": env!("CARGO_PKG_VERSION"),
        "versions": {"acceptance_schema": 1, "session_format": 1, "core_snapshot": deedwake_core::SNAPSHOT_VERSION,
            "host": 1, "content": "repair_cafe_v1", "configuration_schema": 1},
        "build": {"release_required": true, "release_artifact_directory_verified": true, "debug_assertions": cfg!(debug_assertions),
            "target_os": std::env::consts::OS, "target_arch": std::env::consts::ARCH,
            "rustc_vv": rustc, "cargo_version": command(repository, "cargo", &["-V"]),
            "pinned_toolchain_file": pinned, "caller_flags": "not_checked", "optimization_flags": "not_embedded"},
        "machine": {"os": command(repository, "uname", &["-sr"]), "arch": std::env::consts::ARCH,
            "cpu_model": cpu, "available_parallelism": std::thread::available_parallelism().expect("parallelism unavailable").get(),
            "mem_total": memory, "container_memory_limit": container_limit},
        "save_filesystem": command(work, "stat", &["-f", "-c", "%T", "."]),
        "run_started_unix_seconds_utc": SystemTime::now().duration_since(UNIX_EPOCH).expect("clock before Unix epoch").as_secs()
    })
}

#[derive(Default)]
struct HighWater {
    records: usize,
    actions: usize,
    pending: usize,
    trace: usize,
    trace_bytes: usize,
    snapshot_bytes: usize,
    dropped_trace: u64,
    steps: usize,
}
impl HighWater {
    fn observe(&mut self, repair: &scene::Repair) {
        let e = Envelope::capture(repair).unwrap();
        self.records = self.records.max(e.core.records.len());
        self.actions = self.actions.max(e.core.actions.len());
        self.pending = self.pending.max(e.core.pending.len());
        self.trace = self.trace.max(e.core.trace.len());
        self.trace_bytes = self
            .trace_bytes
            .max(serde_json::to_vec(&e.core.trace).unwrap().len());
        self.snapshot_bytes = self
            .snapshot_bytes
            .max(serde_json::to_vec(&e).unwrap().len());
        self.dropped_trace = self.dropped_trace.max(e.core.dropped_trace);
    }
    fn action(&mut self, repair: &mut scene::Repair, action: u32) {
        self.steps += 1;
        assert!(
            self.steps <= MAX_STEPS,
            "untimed verification exceeded 2128 steps"
        );
        actions(repair, &[action]);
        self.observe(repair);
    }
    fn report(&self, configuration: config::ValidatedConfig) -> Value {
        json!({"measurement": "separate_untimed_verification", "records": self.records, "actions": self.actions,
            "pending_deliveries": self.pending, "retained_trace_entries": self.trace,
            "serialized_trace_bytes": self.trace_bytes, "serialized_snapshot_bytes": self.snapshot_bytes,
            "dropped_trace_entries": self.dropped_trace, "observed_verification_steps": self.steps,
            "resource_caps": {"workload_steps": MAX_STEPS, "maximum_tick": config::MAX_TIME,
                "configured_trace_capacity": configuration.trace_capacity(), "snapshot_bytes": MAX_SNAPSHOT_BYTES,
                "snapshot_admission_limits": {"records": limits().max_records, "actions": limits().max_actions,
                    "pending_deliveries": limits().max_pending, "trace_capacity": limits().max_trace_capacity,
                    "inputs": limits().max_inputs, "entities": limits().max_entities, "known_owners": limits().max_known_owners,
                    "known_edges": limits().max_known_edges, "string_bytes": limits().max_string_bytes,
                    "total_string_bytes": limits().max_total_string_bytes},
                "host_entries": MAX_HOST_ENTRIES, "host_text_bytes": MAX_HOST_TEXT_BYTES,
                "host_total_text_bytes": MAX_HOST_TOTAL_TEXT_BYTES,
                "runtime_pending_queue_cap": "not_configured; snapshot admission limit is not a runtime queue cap"},
            "heap_bytes": "unmeasured", "rss_bytes": "unmeasured"})
    }
}

fn verify(configuration: config::ValidatedConfig, path: &Path, equal_time: bool) -> Value {
    let mut high = HighWater::default();
    let mut original = scene::Repair::with_config(configuration);
    high.observe(&original);
    for action in PREFIX {
        high.action(&mut original, action);
    }
    let before = canonical(&original);
    let envelope = Envelope::capture(&original).unwrap();
    assert_eq!(envelope.core.time, 2);
    assert!(!envelope.core.pending.is_empty());
    assert!(
        envelope.host.report.is_some()
            && envelope.host.diagnosis.is_some()
            && envelope.host.rejected_repair.is_some()
    );
    assert!(!envelope.host.repaired && !envelope.host.collected);
    save(&original, path).unwrap();
    let mut restored = read(path).unwrap();
    assert_eq!(
        canonical(&restored),
        before,
        "restore must preserve whole envelope without advancing or execution"
    );
    assert_eq!(player_output(&restored), player_output(&original));
    for action in SUFFIX {
        high.action(&mut original, action);
        actions(&mut restored, &[action]);
        assert_eq!(canonical(&original), canonical(&restored));
        assert_eq!(player_output(&original), player_output(&restored));
    }
    assert!(original.repaired && original.collected);
    high.action(&mut original, 5);
    let after_question = Envelope::capture(&original).unwrap();
    high.action(&mut original, 5);
    assert_eq!(
        canonical(&original),
        serde_json::to_vec(&after_question).unwrap(),
        "repeated intake is stable"
    );

    // Failure checks use the live restore boundary and compare future behavior too.
    let mut live = pending(configuration);
    let mut control = pending(configuration);
    let before_failed_restore = canonical(&live);
    let mut unsupported = Envelope::capture(&live).unwrap();
    unsupported.format_version += 1;
    fs::write(path, serde_json::to_vec(&unsupported).unwrap()).unwrap();
    assert!(restore_into(&mut live, path).is_err());
    assert_eq!(canonical(&live), before_failed_restore);
    for action in SUFFIX {
        actions(&mut live, &[action]);
        actions(&mut control, &[action]);
        assert_eq!(canonical(&live), canonical(&control));
        assert_eq!(player_output(&live), player_output(&control));
    }

    let mut catchup = pending(configuration);
    high.observe(&catchup);
    for _ in 2..config::MAX_TIME {
        high.action(&mut catchup, 6);
    }
    assert_eq!(catchup.time(), 1000);
    assert!(Envelope::capture(&catchup).unwrap().core.pending.is_empty());
    let final_state = canonical(&catchup);
    let final_output = player_output(&catchup);
    assert_eq!(catchup.act(6), 2);
    assert_eq!(canonical(&catchup), final_state);
    assert_eq!(player_output(&catchup), final_output);
    if configuration.trace_capacity() == 1 {
        let e = Envelope::capture(&original).unwrap();
        assert_eq!(e.core.trace.len(), 1);
        assert!(e.core.dropped_trace > 0);
        assert!(
            original
                .output(2)
                .unwrap()
                .contains(&format!("Dropped entries: {}", e.core.dropped_trace))
        );
    }
    let equal_time_assertions = if equal_time {
        let run = crate::scenarios::run_case(configuration, crate::scenarios::Case::EqualTime);
        assert!(run.report.ok && run.report.assertions.iter().all(|a| a.ok != Some(false)));
        assert!(!run.report.assertions.is_empty() && run.report.steps.len() <= MAX_STEPS);
        run.report.assertions.len()
    } else {
        0
    };
    json!({"passed": true, "canonical_full_state_restore_and_suffix": true,
        "pending_tick2_restore_inert": true, "repeated_intake": true,
        "unsupported_version_restore_atomic_and_future_equivalent": true,
        "tick1000_wait_returns2_unchanged": true, "trace_eviction_checked": configuration.trace_capacity() == 1,
        "equal_time_shared_case_assertion_count": equal_time_assertions,
        "separate_regression_coverage": ["accepted_known_unexecuted_restores_inert_without_policy_or_delivery", "each_precommit_disk_failure_preserves_old_file_and_cleans_temporary"],
        "high_water": high.report(configuration)})
}

fn metric_evidence(index: usize) -> Value {
    json!({"status": "incomplete", "phase": "not_started", "warmups_completed": 0,
        "sample_count": 0, "raw_ns": [], "p95_cap_ns": CAPS_NS[index], "passed": null})
}

fn append_sample(evidence: &mut Value, elapsed: u64) {
    let samples = evidence["raw_ns"].as_array_mut().unwrap();
    samples.push(json!(elapsed));
    evidence["sample_count"] = json!(samples.len());
}

fn complete_metric(evidence: &mut Value) {
    let samples: Vec<_> = evidence["raw_ns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    assert_eq!(samples.len(), SAMPLES);
    let summary = statistics(&samples, evidence["p95_cap_ns"].as_u64().unwrap());
    evidence
        .as_object_mut()
        .unwrap()
        .extend(summary.as_object().unwrap().clone());
    evidence["status"] = json!("complete");
    evidence["phase"] = json!("complete");
}

fn record_execution_error(report: &mut Value, error: &str) {
    report["status"] = json!("execution_error");
    report["execution_error"] = json!(
        error
            .lines()
            .next()
            .unwrap_or("panic")
            .chars()
            .filter(|c| !c.is_control())
            .take(512)
            .collect::<String>()
    );
    if let Some(index) = report["current_phase"]["configuration_index"].as_u64() {
        let metric = report["current_phase"]["metric"]
            .as_str()
            .map(str::to_owned);
        let configuration = &mut report["configurations"][index as usize];
        configuration["status"] = json!("execution_error");
        if let Some(metric) = metric {
            configuration["metrics"][metric]["status"] = json!("error");
        } else {
            configuration["verification"]["status"] = json!("error");
        }
    }
}

fn measure(
    configuration: config::ValidatedConfig,
    path: &Path,
    metric: usize,
    evidence: &mut Value,
) {
    for repetition in 0..WARMUPS + SAMPLES {
        evidence["phase"] = json!(if repetition < WARMUPS {
            "warmup_setup"
        } else {
            "sample_setup"
        });
        evidence["iteration"] = json!(repetition + 1);
        // Setups, snapshots, high-water counts and correctness introspection are untimed.
        // The full sequence alone includes initialization in its declared workload.
        let mut repair = match metric {
            0 => {
                let mut r = scene::Repair::with_config(configuration);
                actions(&mut r, &[1, 6, 6]);
                Some(r)
            }
            1 => {
                let mut r = scene::Repair::with_config(configuration);
                actions(&mut r, &[1, 6]);
                Some(r)
            }
            2 => None,
            3..=5 => Some(pending(configuration)),
            _ => unreachable!(),
        };
        if metric == 4 {
            save(repair.as_ref().unwrap(), path).unwrap();
        }
        evidence["phase"] = json!(if repetition < WARMUPS {
            "warmup"
        } else {
            "sample"
        });
        let mut restored = None;
        let started = Instant::now();
        let repair = repair.get_or_insert_with(|| scene::Repair::with_config(configuration));
        match metric {
            0 => actions(repair, &[2]),
            1 => actions(repair, &[6]),
            2 => actions(repair, &SEQUENCE),
            3 => save(repair, path).unwrap(),
            4 => {
                restored = Some(read(path).unwrap());
            }
            5 => {
                for _ in 2..config::MAX_TIME {
                    actions(repair, &[6]);
                }
            }
            _ => unreachable!(),
        }
        let elapsed = u64::try_from(started.elapsed().as_nanos()).expect("timing overflow");
        black_box(repair);
        black_box(restored);
        if repetition >= WARMUPS {
            append_sample(evidence, elapsed);
        } else {
            evidence["warmups_completed"] = json!(repetition + 1);
        }
    }
    complete_metric(evidence);
}

#[test]
#[allow(clippy::assertions_on_constants)] // Runtime preflight must fail only when explicitly invoked.
#[ignore = "explicit local native acceptance run; requires release, Linux, clean Git, and a new report path"]
fn native_repair_budgets() {
    assert!(
        !cfg!(debug_assertions),
        "acceptance requires cargo test --release"
    );
    assert!(
        cfg!(target_os = "linux"),
        "acceptance requires Linux atomic-save policy"
    );
    let executable = std::env::current_exe().expect("cannot inspect test artifact");
    assert_eq!(
        executable
            .parent()
            .and_then(Path::parent)
            .and_then(Path::file_name),
        Some(std::ffi::OsStr::new("release")),
        "acceptance requires the cargo test --release artifact"
    );
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    clean_git(repository);
    let report_path = PathBuf::from(
        std::env::var_os("DEEDWAKE_ACCEPTANCE_REPORT")
            .expect("DEEDWAKE_ACCEPTANCE_REPORT is required"),
    );
    assert!(report_path.is_absolute(), "report path must be absolute");
    let parent = report_path
        .parent()
        .expect("report path requires existing parent")
        .canonicalize()
        .expect("report parent must exist");
    assert!(
        !parent.starts_with(repository.canonicalize().unwrap()),
        "report and scratch directory must be outside clean checkout"
    );
    let mut report_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&report_path)
        .expect("report must be a NEW file in an existing parent");
    let mut report = json!({"schema_version": 1, "report_type": "native_repair_acceptance", "profile_revision": PROFILE_REVISION,
        "status": "running", "budget_kind": "provisional_engineering_baseline",
        "acceptance_scope": "this local native engineering baseline only; not owner-approved product acceptance criteria, a release gate, or complete MVP acceptance",
        "provenance": "not_checked", "warmups_per_metric": WARMUPS, "samples_per_metric": SAMPLES,
        "maximum_steps_per_workload": MAX_STEPS, "scope": "native Repair act and player output fields 0 and 1; synchronous local filesystem persistence",
        "excluded": ["process_startup", "build", "CLI_formatting", "browser_DOM", "heap_and_RSS"],
        "measurement_conditions": "warm filesystem cache after 3 warmups; OS scheduling and shared-machine contention are included, not controlled; native local measurements do not establish browser or universal latency",
        "configuration_order": ["default", "equal_time", "evicted"], "current_phase": {"operation": "work_directory"}, "configurations": []});
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let work = WorkDirectory::new(&parent);
        report["current_phase"] = json!({"operation": "provenance"});
        report["provenance"] = provenance(repository, &work.0);
        let path = work.0.join("session.json");
        let mut passed = true;
        for (configuration_index, (name, configuration)) in configurations().into_iter().enumerate()
        {
            report["configurations"]
                .as_array_mut()
                .unwrap()
                .push(json!({"name": name,
                "configuration": Configuration::from(configuration), "status": "incomplete",
                "verification": {"status": "incomplete"}, "metrics": {}}));
            report["current_phase"] = json!({"operation": "verification", "configuration_index": configuration_index, "configuration": name});
            report["configurations"][configuration_index]["verification"] =
                verify(configuration, &path, name == "equal_time");
            for (index, metric) in METRICS.iter().enumerate() {
                report["current_phase"] = json!({"operation": "measurement", "configuration_index": configuration_index, "configuration": name, "metric": metric});
                let evidence =
                    &mut report["configurations"][configuration_index]["metrics"][*metric];
                *evidence = metric_evidence(index);
                measure(configuration, &path, index, evidence);
                passed &= evidence["passed"].as_bool().unwrap();
            }
            report["configurations"][configuration_index]["status"] = json!("complete");
        }
        report["current_phase"] = json!({"operation": "final_git_check"});
        clean_git(repository);
        assert_eq!(
            command(repository, "git", &["rev-parse", "HEAD"]),
            report["provenance"]["git"]["head"].as_str().unwrap(),
            "Git HEAD changed during acceptance run"
        );
        passed
    }));
    report["status"] = json!(match &outcome {
        Ok(true) => "passed_budgets",
        Ok(false) => "failed_budgets",
        Err(_) => "execution_error",
    });
    if let Err(error) = &outcome {
        record_execution_error(
            &mut report,
            error
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| error.downcast_ref::<&str>().copied())
                .unwrap_or("non-string panic"),
        );
    } else {
        report["current_phase"] = json!({"operation": "complete"});
    }
    serde_json::to_writer_pretty(&mut report_file, &report)
        .expect("cannot write acceptance report");
    report_file.write_all(b"\n").unwrap();
    report_file
        .sync_all()
        .expect("cannot sync acceptance report");
    match outcome {
        Ok(passed) => assert!(
            passed,
            "native acceptance p95 budget failed; see JSON report"
        ),
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

#[test]
fn nearest_rank_and_inclusive_cap_use_injected_nanoseconds() {
    let samples: Vec<_> = (1..=20).rev().collect();
    let result = statistics(&samples, 19);
    assert_eq!(result["median_ns"], 10);
    assert_eq!(result["p95_ns"], 19);
    assert_eq!(result["max_ns"], 20);
    assert_eq!(result["raw_ns"], json!(samples));
    assert_eq!(result["passed"], true);
    assert_eq!(statistics(&samples, 18)["passed"], false);
    assert_eq!(statistics(&[7], 7)["p95_ns"], 7);
    for cap in CAPS_NS {
        assert_eq!(statistics(&[cap; SAMPLES], cap)["passed"], true);
        assert_eq!(statistics(&[cap + 1; SAMPLES], cap)["passed"], false);
    }
}

#[test]
fn compiled_profiles_and_workloads_are_bounded() {
    let profiles = configurations();
    assert_eq!(
        profiles[0].1,
        config::validate(config::DEFAULT_INPUT).unwrap()
    );
    assert_eq!(
        profiles[1].1.report_to_intake_ticks(),
        profiles[1].1.report_to_technician_ticks()
    );
    assert_eq!(
        profiles[1].1.technician_to_intake_ticks(),
        profiles[1].1.technician_to_collection_ticks()
    );
    assert_eq!(profiles[2].1.trace_capacity(), 1);
    assert!(SEQUENCE.len() <= MAX_STEPS);
    assert!(PREFIX.len() + (config::MAX_TIME as usize - 2) <= MAX_STEPS);
}

#[test]
fn untimed_verification_uses_real_sessions_for_all_profiles() {
    let work = WorkDirectory::new(&std::env::temp_dir());
    for (name, configuration) in configurations() {
        let result = verify(
            configuration,
            &work.0.join("session.json"),
            name == "equal_time",
        );
        assert_eq!(result["passed"], true);
    }
}

#[test]
fn injected_failures_preserve_failed_baseline_and_partial_evidence() {
    for measured_before_failure in [0, 3] {
        let mut earlier = metric_evidence(0);
        earlier["warmups_completed"] = json!(WARMUPS);
        for _ in 0..SAMPLES {
            append_sample(&mut earlier, CAPS_NS[0] + 1);
        }
        complete_metric(&mut earlier);
        let mut report = json!({"status": "running", "current_phase": {
            "operation": "measurement", "configuration_index": 0, "configuration": "default", "metric": METRICS[1]},
            "configurations": [{"status": "incomplete", "metrics": {METRICS[0]: earlier, METRICS[1]: metric_evidence(1)}}]});
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let evidence = &mut report["configurations"][0]["metrics"][METRICS[1]];
            evidence["warmups_completed"] = json!(if measured_before_failure == 0 {
                1
            } else {
                WARMUPS
            });
            evidence["phase"] = json!(if measured_before_failure == 0 {
                "warmup"
            } else {
                "sample"
            });
            for value in [11, 17, 23].into_iter().take(measured_before_failure) {
                append_sample(evidence, value);
            }
            panic!("injected workload failure\nprivate diagnostic excluded");
        }));
        let error = outcome.unwrap_err();
        record_execution_error(&mut report, error.downcast_ref::<&str>().copied().unwrap());
        let persisted: Value =
            serde_json::from_slice(&serde_json::to_vec(&report).unwrap()).unwrap();
        let metrics = &persisted["configurations"][0]["metrics"];
        assert_eq!(persisted["status"], "execution_error");
        assert_eq!(persisted["execution_error"], "injected workload failure");
        assert_eq!(persisted["current_phase"]["metric"], METRICS[1]);
        assert_eq!(metrics[METRICS[0]]["status"], "complete");
        assert_eq!(metrics[METRICS[0]]["passed"], false);
        assert_eq!(
            metrics[METRICS[0]]["raw_ns"],
            json!(vec![CAPS_NS[0] + 1; SAMPLES])
        );
        assert_eq!(metrics[METRICS[1]]["status"], "error");
        assert_eq!(metrics[METRICS[1]]["sample_count"], measured_before_failure);
        assert_eq!(
            metrics[METRICS[1]]["raw_ns"],
            json!(
                [11, 17, 23]
                    .into_iter()
                    .take(measured_before_failure)
                    .collect::<Vec<_>>()
            )
        );
        assert_eq!(
            metrics[METRICS[1]]["warmups_completed"],
            if measured_before_failure == 0 {
                1
            } else {
                WARMUPS
            }
        );
        assert!(metrics[METRICS[1]]["passed"].is_null());
        assert!(metrics[METRICS[1]].get("p95_ns").is_none());
    }
}
