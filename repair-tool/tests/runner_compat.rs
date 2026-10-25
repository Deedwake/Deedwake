#![allow(dead_code)] // Compile the same example-owned modules without CLI adapters.
#[path = "../../examples/repair/scenarios.rs"]
mod scenarios;
#[path = "../../examples/repair/scene.rs"]
mod scene;

use scenarios::{Case, MAX_STEPS, run_case};
use scene::config::{self, DEFAULT_INPUT, Input};

fn configuration(input: Input<'_>) -> config::ValidatedConfig {
    config::validate(input).unwrap()
}

#[test]
fn only_the_five_compiled_cases_have_stable_ids_and_names() {
    let names = [
        "delayed-recipients",
        "rejection-then-success",
        "equal-time",
        "repeated-actions",
        "reset",
    ];
    for (index, case) in Case::ALL.into_iter().enumerate() {
        assert_eq!(case.id(), index as u32 + 1);
        assert_eq!(case.as_str(), names[index]);
        assert_eq!(Case::from_id(case.id()), Some(case));
        assert_eq!(Case::from_name(case.as_str()), Some(case));
    }
    for id in [0, 6, u32::MAX] {
        assert_eq!(Case::from_id(id), None);
    }
    for name in ["", "Reset", "reset ", "custom"] {
        assert_eq!(Case::from_name(name), None);
    }
}

#[test]
fn cases_keep_the_supplied_configuration_and_precondition_failures() {
    let config = configuration(DEFAULT_INPUT);
    for case in Case::ALL {
        let run = run_case(config, case);
        assert_eq!(run.scene.configuration(), config);
        assert_eq!(run.report.ok, case != Case::EqualTime);
        assert!(run.report.steps.len() <= MAX_STEPS);
        if case == Case::EqualTime {
            assert!(run.report.steps.is_empty());
            assert_eq!(run.report.assertions.len(), 2);
            let failed = &run.report.assertions[0];
            assert_eq!(failed.code, "equal_report_delays");
            assert_eq!((&*failed.expected, &*failed.actual), ("1", "2"));
            assert_eq!((failed.step, failed.time), (0, 0));
            assert_eq!((failed.status, failed.ok), ("failed", Some(false)));
        }
    }
}

#[test]
fn reset_keeps_both_maximum_tick_sessions_with_global_step_indices() {
    let config = configuration(Input {
        report_to_intake_ticks: 1000,
        report_to_technician_ticks: 1000,
        technician_to_intake_ticks: 0,
        technician_to_collection_ticks: 0,
        ..DEFAULT_INPUT
    });
    let run = run_case(config, Case::Reset);
    assert!(run.report.ok);
    assert_eq!(run.report.session, 1);
    assert!(run.report.steps.len() > 2000);
    assert!(run.report.steps.len() <= MAX_STEPS);
    assert_eq!(run.scene.time(), config::MAX_TIME);
    let reset = run
        .report
        .steps
        .iter()
        .position(|s| s.action == "reset")
        .unwrap();
    for (index, step) in run.report.steps.iter().enumerate() {
        assert_eq!(step.index, index + 1);
        assert_eq!(step.session, u32::from(index >= reset));
        assert!(step.time <= config::MAX_TIME);
    }
    assert_eq!(run.report.steps[reset].time, 0);
    assert!(
        run.report
            .assertions
            .iter()
            .any(|a| { a.code == "reset_replay_identical" && a.ok == Some(true) })
    );
}

#[test]
fn zero_and_truncated_traces_leave_order_not_checked_without_failing_case() {
    for capacity in [0, 1, 1024] {
        let config = configuration(Input {
            trace_capacity: capacity,
            report_to_technician_ticks: 1,
            technician_to_collection_ticks: 1,
            ..DEFAULT_INPUT
        });
        let run = run_case(config, Case::EqualTime);
        assert!(run.report.ok);
        let order = run
            .report
            .assertions
            .iter()
            .find(|a| a.code == "equal_time_schedule_order")
            .unwrap();
        if capacity < 1024 {
            assert!(run.scene.developer_snapshot().dropped_trace > 0);
            assert_eq!((order.status, order.ok), ("not_checked", None));
        } else {
            assert_eq!((order.status, order.ok), ("passed", Some(true)));
        }
    }
}

#[test]
fn zero_delay_cases_complete_at_zero_and_horizon_errors_remain_explicit() {
    let zero = configuration(Input {
        report_to_intake_ticks: 0,
        report_to_technician_ticks: 0,
        technician_to_intake_ticks: 0,
        technician_to_collection_ticks: 0,
        cable_receipt_at_tick: 0,
        ..DEFAULT_INPUT
    });
    for case in [Case::EqualTime, Case::Reset] {
        let run = run_case(zero, case);
        assert!(run.report.ok);
        assert_eq!(run.scene.time(), 0);
        assert!(
            run.report
                .steps
                .iter()
                .all(|s| s.time == 0 && s.action != "wait")
        );
    }
    let late = configuration(Input {
        technician_to_collection_ticks: 1000,
        ..DEFAULT_INPUT
    });
    let run = run_case(late, Case::RepeatedActions);
    assert!(!run.report.ok);
    assert!(run.report.assertions.iter().any(|a| {
        a.code == "fixture_clock_bound" && a.actual == "target tick 1003" && a.ok == Some(false)
    }));
    assert!(run.report.steps.iter().all(|s| s.time <= config::MAX_TIME));
    assert!(run.report.steps.len() <= MAX_STEPS);
}
