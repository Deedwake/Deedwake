//! Test-only qualification fixtures and one real live-host replacement worker.
use super::*;
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    hint::black_box,
    io::Write,
    path::PathBuf,
    time::Instant,
};

const KINDS: [scene::ProviderDelivery; 3] = [
    scene::ProviderDelivery::Report,
    scene::ProviderDelivery::Routine,
    scene::ProviderDelivery::Intermediary,
];
const FILES: [&str; 14] = [
    "default.toml",
    "precise.toml",
    "warn.toml",
    "equal.toml",
    "trace1.toml",
    "trace1024.toml",
    "held.json",
    "pending.json",
    "equal.json",
    "pressure.json",
    "trace1-pressure.json",
    "trace1024-pressure.json",
    "oldneartext.json",
    "fixtures.json",
];

fn configurations() -> [(&'static str, ValidatedConfig); 6] {
    [
        ("default", config::DEFAULT_INPUT),
        (
            "precise",
            config::Input {
                retention: "precise",
                initial_delivery_ticks: 0,
                routine_delivery_ticks: 0,
                intermediary_delivery_ticks: 0,
                ..config::DEFAULT_INPUT
            },
        ),
        (
            "warn",
            config::Input {
                reaction: "warn_and_continue",
                sela_confidence: "unverified",
                initial_delivery_ticks: 0,
                ..config::DEFAULT_INPUT
            },
        ),
        (
            "equal",
            config::Input {
                initial_delivery_ticks: 1,
                routine_delivery_ticks: 1,
                intermediary_delivery_ticks: 1,
                ..config::DEFAULT_INPUT
            },
        ),
        (
            "trace1",
            config::Input {
                trace_capacity: 1,
                ..config::DEFAULT_INPUT
            },
        ),
        (
            "trace1024",
            config::Input {
                trace_capacity: 1024,
                ..config::DEFAULT_INPUT
            },
        ),
    ]
    .map(|(name, input)| (name, config::validate(input).unwrap()))
}

fn canonical(c: &scene::Convoy) -> Vec<u8> {
    serde_json::to_vec(&Envelope::capture(c).unwrap()).unwrap()
}

fn player_output(c: &scene::Convoy) -> [String; 2] {
    [c.output(0).unwrap(), c.output(1).unwrap()]
}

fn usage(c: &scene::Convoy) -> Value {
    let u = c.core.usage();
    let e = Envelope::capture(c).unwrap();
    json!({"records": u.records, "actions": u.actions, "pending_deliveries": u.pending,
        "inputs": u.inputs, "entities": u.entities, "known_owners": u.known_owners,
        "reference_edges": u.reference_edges, "next_delivery": u.next_delivery,
        "core_text_bytes": u.text_bytes, "retained_trace_entries": u.trace_entries,
        "serialized_trace_bytes": serde_json::to_vec(&e.core.trace).unwrap().len(),
        "dropped_trace_entries": e.core.dropped_trace,
        "serialized_snapshot_bytes": serde_json::to_vec(&e).unwrap().len(),
        "host_text_bytes": c.dialogue.len() + c.journal.iter().map(String::len).sum::<usize>()})
}

#[derive(Default)]
struct HighWater(BTreeMap<String, u64>);
impl HighWater {
    fn observe(&mut self, c: &scene::Convoy) {
        for (key, value) in usage(c).as_object().unwrap() {
            let high = self.0.entry(key.clone()).or_default();
            *high = (*high).max(value.as_u64().unwrap());
        }
        assert!(c.core.usage().trace_entries <= c.config().unwrap().trace_capacity());
    }

    fn actions(&mut self, c: &mut scene::Convoy, actions: &[u32]) {
        self.observe(c);
        for &action in actions {
            assert_eq!(
                c.act_checked(action),
                Ok(()),
                "qualification action {action}"
            );
            black_box(player_output(c));
            self.observe(c);
        }
    }
}

fn operations(c: &scene::Convoy) -> Vec<(u64, scene::ServiceOperation)> {
    c.facts
        .iter()
        .filter_map(|(id, fact)| match fact {
            scene::Fact::Service { operation, .. } => {
                Some((c.core.record(*id).unwrap().time, *operation))
            }
            _ => None,
        })
        .collect()
}

fn snapshot(
    c: &mut scene::Convoy,
    directory: &Path,
    filename: &str,
    continuation: &[u32],
) -> Value {
    let path = directory.join(filename);
    let before = canonical(c);
    let outputs = player_output(c);
    save(c, &path).unwrap();
    assert_eq!(fs::read(&path).unwrap(), before);
    let mut restored = read(&path).unwrap();
    for _ in 0..3 {
        assert_eq!(canonical(&read(&path).unwrap()), before);
        assert_eq!(canonical(&restored), before);
        assert_eq!(player_output(&restored), outputs);
    }
    for &action in continuation {
        assert_eq!(restored.act_checked(action), c.act_checked(action));
        assert_eq!(canonical(&restored), canonical(c));
        assert_eq!(player_output(&restored), player_output(c));
    }
    // Keep the fixture boundary for the caller after comparing its real uninterrupted host.
    *c = read(&path).unwrap();
    let e = Envelope::capture(c).unwrap();
    json!({"file": filename, "bytes": before.len(), "state_usage": usage(c),
        "expected": {"time": e.core.time, "held": e.host.held, "access": e.host.access,
            "refused": e.host.refused, "configuration": e.host.configuration,
            "facts": e.host.facts.len(), "service_outcomes": operations(c).len()},
        "correctness": {"save_canonical_byte_equal": true, "read_inert": true,
            "repeated_read_inert": true, "continuation_canonical_byte_equal": true,
            "continuation": continuation}})
}

fn pressure(configuration: ValidatedConfig, directory: &Path, name: &str) -> Value {
    let mut c = scene::Convoy::with_config(configuration);
    let mut high = HighWater::default();
    high.actions(&mut c, &[1]);
    let mut accepted_questions = 0;
    let rejected = loop {
        assert!(
            accepted_questions < 4096,
            "optional questions must be bounded"
        );
        let before = canonical(&c);
        match c.act_checked(2) {
            Ok(()) => {
                accepted_questions += 1;
                high.observe(&c);
            }
            Err(scene::CommandError::Capacity(resource)) => {
                assert_eq!(
                    canonical(&c),
                    before,
                    "capacity refusal changed whole state"
                );
                break resource;
            }
            result => panic!("expected typed capacity refusal, got {result:?}"),
        }
    };
    assert_eq!(rejected, scene::limits::LimitResource::DeliverySequence);
    let mut evidence = snapshot(&mut c, directory, name, &[6, 6, 6, 3, 4, 6, 5]);
    high.actions(&mut c, &[6, 6, 6, 3, 4, 6, 5]);
    assert!(c.access && !c.held);
    assert_eq!(
        c.core.usage().next_delivery,
        scene::limits::MAX_DELIVERY_SEQUENCE
    );
    // Completion and reset must still be saveable after optional traffic exhausts its reserve.
    let completed = Envelope::capture(&c).unwrap();
    completed.restore().unwrap();
    save(&c, &directory.join(format!("{name}.completed"))).unwrap();
    fs::remove_file(directory.join(format!("{name}.completed"))).unwrap();
    high.actions(&mut c, &[0]);
    assert_eq!(c.core.usage().records, 0);
    save(&c, &directory.join(format!("{name}.reset"))).unwrap();
    fs::remove_file(directory.join(format!("{name}.reset"))).unwrap();
    evidence["accepted_questions"] = json!(accepted_questions);
    evidence["capacity_resource"] = json!(rejected.as_str());
    evidence["capacity_refusal_whole_state_unchanged"] = json!(true);
    evidence["reserved_completion_save_reset"] = json!(true);
    evidence["high_water"] = json!(high.0);
    evidence
}

fn present(residency: Residency, availability: Availability) -> EntityState {
    EntityState::Present {
        residency,
        availability,
    }
}

fn verify_lifecycle(configuration: ValidatedConfig) -> Value {
    let mut high = HighWater::default();
    let mut c = scene::Convoy::with_config(configuration.clone());
    c.set_provider_state(present(Residency::Unloaded, Availability::Available))
        .unwrap();
    high.actions(&mut c, &[1, 3, 4, 6, 6, 6]);
    for kind in KINDS {
        assert!(matches!(
            c.provider_delivery_receipt(kind).unwrap().state,
            DeliveryState::Delivered { .. }
        ));
    }
    assert!(operations(&c).is_empty());
    let before = canonical(&c);
    assert_eq!(c.act_checked(5), Err(scene::CommandError::InvalidAction));
    assert_eq!(canonical(&c), before);
    let mut before = Envelope::capture(&c).unwrap();
    c.set_provider_state(present(Residency::Loaded, Availability::Available))
        .unwrap();
    let after = Envelope::capture(&c).unwrap();
    before.core.entities = after.core.entities.clone();
    assert_eq!(
        serde_json::to_vec(&before).unwrap(),
        serde_json::to_vec(&after).unwrap()
    );
    high.actions(&mut c, &[5]);
    // All three routes already arrived, so the explicit visit observes the correction.
    assert!(c.access && operations(&c).is_empty());

    // A report alone received while unloaded must execute on the next explicit visit.
    let mut c = scene::Convoy::with_config(configuration.clone());
    c.set_provider_state(present(Residency::Unloaded, Availability::Available))
        .unwrap();
    high.actions(&mut c, &[1, 6, 6, 6]);
    assert!(operations(&c).is_empty());
    c.set_provider_state(present(Residency::Loaded, Availability::Available))
        .unwrap();
    assert!(operations(&c).is_empty());
    high.actions(&mut c, &[5]);
    assert_eq!(operations(&c), vec![(3, scene::ServiceOperation::Restrict)]);
    assert!(c.refused && !c.access);

    let mut c = scene::Convoy::with_config(configuration);
    high.actions(&mut c, &[1, 3, 4]);
    c.set_provider_state(present(Residency::Loaded, Availability::Unavailable))
        .unwrap();
    high.actions(&mut c, &[6, 6, 6]);
    let receipts: Vec<_> = KINDS
        .map(|kind| c.provider_delivery_receipt(kind).unwrap().clone())
        .into();
    for receipt in &receipts {
        assert!(matches!(
            receipt.state,
            DeliveryState::Rejected {
                reason: DeliveryRejection::Unavailable,
                ..
            }
        ));
    }
    let next_delivery = c.core.usage().next_delivery;
    c.set_provider_state(present(Residency::Loaded, Availability::Available))
        .unwrap();
    high.actions(&mut c, &[6, 5]);
    assert!(c.access && operations(&c).is_empty());
    assert_eq!(c.core.usage().next_delivery, next_delivery);
    for (kind, receipt) in KINDS.into_iter().zip(receipts) {
        assert_eq!(c.provider_delivery_receipt(kind), Some(&receipt));
    }
    json!({"passed": true, "unloaded_receives_three_routes": true,
        "setter_is_inert": true, "explicit_visit_executes_received_report": true,
        "unavailable_terminal_rejections_survive_recovery_without_resend": true,
        "high_water": high.0})
}

fn verify_record_reserve(configuration: ValidatedConfig, directory: &Path) -> Value {
    let mut c = scene::Convoy::with_config(configuration);
    let mut count = 0;
    loop {
        let before =
            (c.core.usage().records >= scene::limits::MAX_RECORDS - 16).then(|| canonical(&c));
        match c.act_checked(5) {
            Ok(()) => count += 1,
            Err(scene::CommandError::Capacity(resource)) => {
                assert_eq!(resource, scene::limits::LimitResource::Records);
                assert_eq!(canonical(&c), before.expect("near record ceiling"));
                break;
            }
            error => panic!("unexpected fresh visit result: {error:?}"),
        }
        assert!(count < 4096);
    }
    let mut high = HighWater::default();
    high.actions(&mut c, &[1, 6, 6, 6, 3, 4, 6, 6, 6, 5]);
    assert!(c.access && !c.held);
    assert_eq!(c.core.usage().records, scene::limits::MAX_RECORDS);
    let path = directory.join("reserve-check.json");
    save(&c, &path).unwrap();
    high.actions(&mut c, &[0]);
    assert_eq!(c.core.usage().records, 0);
    save(&c, &path).unwrap();
    fs::remove_file(path).unwrap();
    json!({"passed": true, "accepted_optional_visits": count,
        "capacity_resource": "records", "refusal_atomic": true,
        "completion_save_reset": true, "high_water": high.0})
}

fn prepare(directory: &Path) -> Value {
    for filename in FILES {
        assert!(
            !directory.join(filename).exists(),
            "fixture already exists: {filename}"
        );
    }
    let configurations = configurations();
    for (name, configuration) in &configurations {
        let value = serde_json::to_value(Configuration::capture(configuration)).unwrap();
        let text = value
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| format!("{key} = {value}\n"))
            .collect::<String>();
        assert_eq!(
            crate::config_build::validate_source(&text, name).unwrap(),
            *configuration
        );
        fs::write(directory.join(format!("{name}.toml")), text).unwrap();
    }
    let mut fixtures = Vec::new();
    let mut high = HighWater::default();
    let mut c = scene::Convoy::with_config(configurations[0].1.clone());
    high.actions(&mut c, &[1, 6, 6, 6]);
    assert!(c.held && !c.access && !c.refused);
    fixtures.push(snapshot(
        &mut c,
        directory,
        "held.json",
        &[5, 3, 4, 6, 5, 6, 6, 5],
    ));
    high.actions(&mut c, &[5, 3, 4]);
    assert_eq!(c.time(), 3);
    assert!(c.refused && !c.access && c.core.usage().pending == 2);
    fixtures.push(snapshot(
        &mut c,
        directory,
        "pending.json",
        &[6, 5, 6, 6, 5, 2, 5],
    ));
    high.actions(&mut c, &[6, 5, 6, 6, 5, 2, 5]);
    assert_eq!(
        operations(&c),
        vec![
            (3, scene::ServiceOperation::Restrict),
            (4, scene::ServiceOperation::Restore)
        ]
    );
    assert!(c.access && c.refused);
    let outcomes = operations(&c);
    let output = player_output(&c);
    high.actions(&mut c, &[2, 5, 2, 5]);
    assert_eq!(operations(&c), outcomes);
    assert_eq!(player_output(&c), output);

    let mut precise = scene::Convoy::with_config(configurations[1].1.clone());
    high.actions(&mut precise, &[1, 2, 5, 3, 4, 5]);
    assert!(precise.access && !precise.refused && operations(&precise).is_empty());

    let mut warn = scene::Convoy::with_config(configurations[2].1.clone());
    high.actions(&mut warn, &[1, 5, 3, 4, 6, 5]);
    assert_eq!(operations(&warn), vec![(0, scene::ServiceOperation::Warn)]);
    assert!(warn.access && !warn.refused);
    assert!(warn.dialogue.contains("unverified for me"));
    high.actions(&mut warn, &[6, 6, 5]);
    assert_eq!(
        operations(&warn),
        vec![
            (0, scene::ServiceOperation::Warn),
            (3, scene::ServiceOperation::ResolveWarning)
        ]
    );

    let mut equal = scene::Convoy::with_config(configurations[3].1.clone());
    high.actions(&mut equal, &[1, 3, 4]);
    assert_eq!(equal.time(), 0);
    assert_eq!(equal.core.usage().pending, 3);
    for kind in KINDS {
        assert_eq!(
            equal.provider_delivery_receipt(kind).unwrap().delivery.due,
            1
        );
    }
    fixtures.push(snapshot(&mut equal, directory, "equal.json", &[6, 5, 2, 5]));
    high.actions(&mut equal, &[6, 5]);
    let e = Envelope::capture(&equal).unwrap();
    let delivered: Vec<_> = e
        .core
        .trace
        .iter()
        .filter_map(|entry| match &entry.event {
            SnapshotEvent::Delivered(d) if d.to == scene::PROVIDER.0 => Some(d.record),
            _ => None,
        })
        .collect();
    assert_eq!(
        delivered,
        vec![
            e.host.report.unwrap(),
            e.host.release.unwrap(),
            e.host.verification.unwrap()
        ]
    );
    assert!(equal.access && !equal.refused && operations(&equal).is_empty());

    for (index, name) in [
        (0, "pressure.json"),
        (4, "trace1-pressure.json"),
        (5, "trace1024-pressure.json"),
    ] {
        let evidence = pressure(configurations[index].1.clone(), directory, name);
        assert!(
            evidence["high_water"]["dropped_trace_entries"]
                .as_u64()
                .unwrap()
                > 0
        );
        fixtures.push(evidence);
    }
    let mut old = historical_near_text().restore().unwrap();
    let mut old_evidence = snapshot(&mut old, directory, "oldneartext.json", &[5]);
    assert_eq!(old.core.usage().text_bytes, 1024 * 1024 - 1);
    let mut loaded = read(&directory.join("oldneartext.json")).unwrap();
    let before = canonical(&loaded);
    assert_eq!(
        loaded.act_checked(5),
        Err(scene::CommandError::Capacity(
            scene::limits::LimitResource::TextBytes
        ))
    );
    assert_eq!(canonical(&loaded), before);
    save(&loaded, &directory.join("oldneartext.json")).unwrap();
    old_evidence["origin"] = json!("historical_core_admitted_not_fresh_player_reachable");
    old_evidence["visit_refusal_atomic_and_saveable"] = json!(true);
    fixtures.push(old_evidence);
    let report = json!({"schema_version": 1, "status": "passed", "files": FILES,
        "configurations": configurations.iter().map(|(name, c)| json!({"name": name,
            "file": format!("{name}.toml"), "configuration": Configuration::capture(c)})).collect::<Vec<_>>(),
        "fixtures": fixtures, "semantic_high_water": high.0,
        "correctness": {"default_restriction_and_correction_keep_history": true,
            "repeated_actions_do_not_duplicate_service_outcomes": true,
            "precise_immediate_does_not_fabricate_restriction": true,
            "warn_unverified_pouch_waits_for_gate_receipt": true,
            "equal_due_routes_preserve_dispatch_order": true, "trace_counts_bounded": true,
            "lifecycle": verify_lifecycle(configurations[0].1.clone()),
            "fresh_record_reserve": verify_record_reserve(configurations[0].1.clone(), directory)}});
    fs::write(
        directory.join("fixtures.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    report
}

fn restore_worker(directory: &Path, report: &mut Value) {
    let mut live = read(&directory.join("pressure.json")).unwrap();
    report["result"] = json!({"before_usage": usage(&live)});
    let started = Instant::now();
    let restored = restore_into(&mut live, &directory.join("oldneartext.json"));
    if restored.is_ok() {
        black_box(player_output(&live));
    }
    let inner_ns = u64::try_from(started.elapsed().as_nanos()).unwrap();
    report["result"]["inner_ns"] = json!(inner_ns);
    report["result"]["restore_succeeded"] = json!(restored.is_ok());
    restored.unwrap();
    let canonical_byte_equal =
        canonical(&live) == fs::read(directory.join("oldneartext.json")).unwrap();
    report["result"]["candidate_usage"] = usage(&live);
    report["result"]["canonical_byte_equal"] = json!(canonical_byte_equal);
    report["result"]["player_output_generated"] = json!(true);
    report["result"]["old_live_retained_during_candidate_validation"] = json!(true);
    assert!(canonical_byte_equal);
    assert_eq!(live.core.usage().text_bytes, 1024 * 1024 - 1);
}

fn outside_repository(path: &Path) -> PathBuf {
    assert!(path.is_absolute(), "qualification paths must be absolute");
    let resolved = path
        .canonicalize()
        .expect("qualification directory must exist");
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .canonicalize()
        .unwrap();
    assert!(
        !resolved.starts_with(repository),
        "qualification files must be outside Git checkout"
    );
    resolved
}

#[test]
#[allow(clippy::assertions_on_constants)] // Only the explicitly invoked worker enforces release.
#[ignore = "explicit release qualification worker; lead must freeze the profile before measurement"]
fn qualification_worker() {
    let result = PathBuf::from(
        std::env::var_os("DEEDWAKE_CONVOY_QUAL_RESULT")
            .expect("DEEDWAKE_CONVOY_QUAL_RESULT required"),
    );
    assert!(result.is_absolute());
    outside_repository(result.parent().unwrap());
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&result)
        .expect("qualification result must be a new file");
    let mut report = json!({"schema_version": 1, "status": "execution_error"});
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert!(
            !cfg!(debug_assertions),
            "worker requires release test binary"
        );
        let mode =
            std::env::var("DEEDWAKE_CONVOY_QUAL_MODE").expect("DEEDWAKE_CONVOY_QUAL_MODE required");
        report["mode"] = json!(mode);
        let directory = PathBuf::from(
            std::env::var_os("DEEDWAKE_CONVOY_QUAL_DIR")
                .expect("DEEDWAKE_CONVOY_QUAL_DIR required"),
        );
        let directory = outside_repository(&directory);
        assert!(directory.is_dir());
        match mode.as_str() {
            "prepare" => report["result"] = prepare(&directory),
            "restore_into" => restore_worker(&directory, &mut report),
            _ => panic!("unsupported qualification mode"),
        }
    }));
    if let Err(error) = &outcome {
        report["error"] = json!(
            error
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| error.downcast_ref::<&str>().copied())
                .unwrap_or("non-string panic")
        );
    } else {
        report["status"] = json!("passed");
    }
    serde_json::to_writer_pretty(&mut file, &report).unwrap();
    file.write_all(b"\n").unwrap();
    file.sync_all().unwrap();
    if let Err(error) = outcome {
        std::panic::resume_unwind(error);
    }
}

#[test]
fn untimed_qualification_fixtures_use_real_core_and_codec() {
    let directory = std::env::temp_dir().join(format!(
        "convoy-qualification-fixtures-{}",
        std::process::id()
    ));
    fs::create_dir(&directory).unwrap();
    let result = std::panic::catch_unwind(|| prepare(&directory));
    fs::remove_dir_all(&directory).unwrap();
    assert_eq!(result.unwrap()["status"], "passed");
}
