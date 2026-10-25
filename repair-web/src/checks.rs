//! Version 1 bounded developer checks, isolated from the interactive fixture.
use crate::{bridge, scenarios};
use scenarios::{Case, CaseRun};
use std::cell::RefCell;

const INVALID: u32 = u32::MAX;
const INVALID_BYTE: u32 = 256;
const MAX_ASSERTIONS: usize = 128;
const MAX_TEXT_BYTES: usize = 1024;

struct StoredRun {
    case: Case,
    run: CaseRun,
}

thread_local! { static LATEST: RefCell<Option<StoredRun>> = const { RefCell::new(None) }; }

fn value(stored: Option<&StoredRun>, section: u32, row: u32, column: u32) -> u32 {
    if section == 0 && column == 0 {
        match row {
            0 => return 1,
            1 => return u32::from(stored.is_some()),
            _ => {}
        }
    }
    let Some(stored) = stored else {
        return INVALID;
    };
    let report = &stored.run.report;
    match section {
        0 if column == 0 => match row {
            2 => stored.case.id(),
            3 => u32::from(report.ok),
            4 => report.session,
            5 if report.steps.len() <= scenarios::MAX_STEPS => {
                u32::try_from(report.steps.len()).unwrap_or(INVALID)
            }
            6 if report.assertions.len() <= MAX_ASSERTIONS => {
                u32::try_from(report.assertions.len()).unwrap_or(INVALID)
            }
            7 => u32::try_from(
                report
                    .assertions
                    .iter()
                    .filter(|assertion| assertion.status == "not_checked")
                    .count(),
            )
            .unwrap_or(INVALID),
            8 => stored.run.scene.time(),
            9 => u32::try_from(stored.run.scene.developer_snapshot().dropped_trace)
                .unwrap_or(INVALID),
            10 => {
                u32::try_from(stored.run.scene.configuration().trace_capacity()).unwrap_or(INVALID)
            }
            _ => INVALID,
        },
        1 if column < 3 => {
            let Some(assertion) = usize::try_from(row)
                .ok()
                .filter(|&row| row < MAX_ASSERTIONS)
                .and_then(|row| report.assertions.get(row))
            else {
                return INVALID;
            };
            match column {
                0 => u32::try_from(assertion.step).unwrap_or(INVALID),
                1 => assertion.time,
                2 => match assertion.status {
                    "passed" => 1,
                    "failed" => 2,
                    "not_checked" => 3,
                    _ => INVALID,
                },
                _ => INVALID,
            }
        }
        2 if column < 5 => {
            let Some(step) = usize::try_from(row)
                .ok()
                .filter(|&row| row < scenarios::MAX_STEPS)
                .and_then(|row| report.steps.get(row))
            else {
                return INVALID;
            };
            match column {
                0 => step.session,
                1 => u32::try_from(step.index).unwrap_or(INVALID),
                2 => step.time,
                3 => match step.action {
                    "reset" => 0,
                    "submit" => 1,
                    "inspect" => 2,
                    "repair" => 3,
                    "collect" => 4,
                    "ask_intake" => 5,
                    "wait" => 6,
                    _ => INVALID,
                },
                4 if step.result <= 2 => step.result,
                _ => INVALID,
            }
        }
        _ => INVALID,
    }
}

fn text(stored: Option<&StoredRun>, field: u32, row: u32) -> Option<&[u8]> {
    let assertion = stored?.run.report.assertions.get(
        usize::try_from(row)
            .ok()
            .filter(|&row| row < MAX_ASSERTIONS)?,
    )?;
    let bytes = match field {
        0 => assertion.code.as_bytes(),
        1 => assertion.expected.as_bytes(),
        2 => assertion.actual.as_bytes(),
        _ => return None,
    };
    (bytes.len() <= MAX_TEXT_BYTES).then_some(bytes)
}

/// Execute a compiled case in a fresh fixture; invalid IDs preserve the last run.
#[unsafe(no_mangle)]
pub extern "C" fn dw_repair_check_run(case_id: u32) -> u32 {
    let Some(case) = Case::from_id(case_id) else {
        return 1;
    };
    let configuration = bridge::inspect(|fixture| fixture.0.configuration());
    let run = scenarios::run_case(configuration, case);
    LATEST.with(|latest| *latest.borrow_mut() = Some(StoredRun { case, run }));
    0
}

/// Sections: 0 metadata, 1 assertions, 2 steps. Invalid coordinates return u32::MAX.
#[unsafe(no_mangle)]
pub extern "C" fn dw_repair_check_value(section: u32, row: u32, column: u32) -> u32 {
    LATEST.with(|latest| value(latest.borrow().as_ref(), section, row, column))
}

/// Assertion fields: 0 code, 1 expected, 2 actual. Invalid fields return u32::MAX.
#[unsafe(no_mangle)]
pub extern "C" fn dw_repair_check_len(field: u32, row: u32) -> u32 {
    LATEST.with(|latest| {
        text(latest.borrow().as_ref(), field, row)
            .and_then(|bytes| u32::try_from(bytes.len()).ok())
            .unwrap_or(INVALID)
    })
}

/// Return one UTF-8 byte, or 256 for an invalid field, row, or byte index.
#[unsafe(no_mangle)]
pub extern "C" fn dw_repair_check_byte(field: u32, row: u32, index: u32) -> u32 {
    LATEST.with(|latest| {
        usize::try_from(index)
            .ok()
            .and_then(|index| {
                text(latest.borrow().as_ref(), field, row)?
                    .get(index)
                    .copied()
            })
            .map_or(INVALID_BYTE, u32::from)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{self, config};

    fn clear() {
        LATEST.with(|latest| *latest.borrow_mut() = None);
    }

    fn projection() -> Vec<u32> {
        let mut output = (0..=10)
            .map(|row| dw_repair_check_value(0, row, 0))
            .collect::<Vec<_>>();
        for (section, rows, columns) in [
            (1, dw_repair_check_value(0, 6, 0), 3),
            (2, dw_repair_check_value(0, 5, 0), 5),
        ] {
            for row in 0..rows {
                for column in 0..columns {
                    output.push(dw_repair_check_value(section, row, column));
                }
                if section == 1 {
                    for field in 0..3 {
                        let len = dw_repair_check_len(field, row);
                        output.push(len);
                        for index in 0..len {
                            output.push(dw_repair_check_byte(field, row, index));
                        }
                    }
                }
            }
        }
        output
    }

    fn player_projection() -> Vec<u32> {
        let mut output = vec![bridge::dw_version(), bridge::dw_time()];
        output.extend((0..=7).map(|action| bridge::dw_enabled(action)));
        for field in 0..3 {
            let len = bridge::dw_len(field);
            output.push(len);
            output.extend((0..len).map(|index| bridge::dw_byte(field, index)));
        }
        output
    }

    #[test]
    fn coordinates_are_bounded_before_and_after_a_report_and_invalid_ids_preserve_it() {
        clear();
        for case_id in [0, 6, INVALID] {
            assert_eq!(dw_repair_check_run(case_id), 1);
        }
        assert_eq!(dw_repair_check_value(0, 0, 0), 1);
        assert_eq!(dw_repair_check_value(0, 1, 0), 0);
        for section in [0, 1, 2, 3, INVALID] {
            for row in [0, 1, 2, 10, 11, INVALID] {
                for column in [0, 1, 2, 3, 4, 5, INVALID] {
                    if section == 0 && row < 2 && column == 0 {
                        continue;
                    }
                    assert_eq!(dw_repair_check_value(section, row, column), INVALID);
                }
            }
        }
        for field in [0, 1, 2, 3, INVALID] {
            assert_eq!(dw_repair_check_len(field, 0), INVALID);
            assert_eq!(dw_repair_check_byte(field, 0, 0), INVALID_BYTE);
        }
        assert_eq!(dw_repair_check_run(1), 0);
        assert_eq!(dw_repair_check_value(0, 1, 0), 1);
        let before = projection();
        for case_id in [0, 6, INVALID] {
            assert_eq!(dw_repair_check_run(case_id), 1);
            assert_eq!(projection(), before);
        }
        for (section, rows, columns) in [
            (0, 11, 1),
            (1, dw_repair_check_value(0, 6, 0), 3),
            (2, dw_repair_check_value(0, 5, 0), 5),
        ] {
            for row in [rows, INVALID] {
                for column in 0..columns {
                    assert_eq!(dw_repair_check_value(section, row, column), INVALID);
                }
            }
            for row in 0..rows {
                for column in [columns, INVALID] {
                    assert_eq!(dw_repair_check_value(section, row, column), INVALID);
                }
            }
        }
        for section in [3, INVALID] {
            assert_eq!(dw_repair_check_value(section, 0, 0), INVALID);
        }
        for row in [dw_repair_check_value(0, 6, 0), INVALID] {
            for field in [0, 1, 2, 3, INVALID] {
                assert_eq!(dw_repair_check_len(field, row), INVALID);
                assert_eq!(dw_repair_check_byte(field, row, 0), INVALID_BYTE);
            }
        }
        for field in [3, INVALID] {
            assert_eq!(dw_repair_check_len(field, 0), INVALID);
            assert_eq!(dw_repair_check_byte(field, 0, 0), INVALID_BYTE);
        }
        for field in 0..3 {
            assert_eq!(
                dw_repair_check_byte(field, 0, dw_repair_check_len(field, 0)),
                INVALID_BYTE
            );
            assert_eq!(dw_repair_check_byte(field, 0, INVALID), INVALID_BYTE);
        }
        assert_eq!(projection(), before);
    }

    #[test]
    fn compiled_configuration_default_success_failure_and_fresh_runs_are_preserved() {
        bridge::dw_init();
        let configuration = bridge::inspect(|fixture| fixture.0.configuration());
        for case in Case::ALL {
            assert_eq!(dw_repair_check_run(case.id()), 0);
            let direct = scenarios::run_case(configuration, case);
            LATEST.with(|latest| {
                let latest = latest.borrow();
                let stored = latest.as_ref().unwrap();
                assert_eq!(stored.case, case);
                assert_eq!(stored.run.scene.configuration(), configuration);
                assert_eq!(stored.run.report, direct.report);
                assert_eq!(
                    stored.run.scene.developer_snapshot(),
                    direct.scene.developer_snapshot()
                );
            });
            assert_eq!(dw_repair_check_value(0, 2, 0), case.id());
            assert_eq!(
                dw_repair_check_value(0, 3, 0),
                u32::from(case != Case::EqualTime)
            );
            let before = projection();
            assert_eq!(dw_repair_check_run(case.id()), 0);
            assert_eq!(projection(), before);
        }
        assert_eq!(dw_repair_check_value(0, 4, 0), 1);
        let reset = projection();
        assert_eq!(dw_repair_check_run(1), 0);
        assert_eq!(dw_repair_check_value(0, 4, 0), 0);
        assert_eq!(dw_repair_check_run(5), 0);
        assert_eq!(projection(), reset);
        assert_eq!(dw_repair_check_run(3), 0);
        assert_eq!(dw_repair_check_value(0, 3, 0), 0);
        assert_eq!(dw_repair_check_value(1, 0, 2), 2);
    }

    #[test]
    fn valid_invalid_and_read_operations_preserve_the_complete_interactive_fixture() {
        bridge::dw_init();
        let mut control = crate::configured_repair();
        for action in [1, 6, 6, 2, 3, 5] {
            assert_eq!(bridge::dw_act(action), control.act(action));
        }
        let snapshot = bridge::inspect(|fixture| fixture.0.developer_snapshot());
        let player = player_projection();
        for case_id in [1, 2, 3, 4, 5, 0, 6, INVALID] {
            assert_eq!(
                dw_repair_check_run(case_id),
                u32::from(!(1..=5).contains(&case_id))
            );
            let _ = projection();
            assert_eq!(
                bridge::inspect(|fixture| fixture.0.developer_snapshot()),
                snapshot
            );
            assert_eq!(player_projection(), player);
        }
        for action in [3, 6, 6, 3, 6, 6, 4, 0, 1, 6] {
            assert_eq!(bridge::dw_act(action), control.act(action));
            assert_eq!(
                bridge::inspect(|fixture| fixture.0.developer_snapshot()),
                control.developer_snapshot()
            );
            for field in 0..3 {
                let expected = control.output(field).unwrap().into_bytes();
                let actual = (0..bridge::dw_len(field))
                    .map(|index| u8::try_from(bridge::dw_byte(field, index)).unwrap())
                    .collect::<Vec<_>>();
                assert_eq!(actual, expected);
            }
        }
    }

    #[test]
    fn zero_and_truncated_trace_metadata_text_and_scalar_fields_match_the_shared_run() {
        for capacity in [0, 8, 128] {
            let configuration = config::validate(config::Input {
                trace_capacity: capacity,
                report_to_intake_ticks: 2,
                technician_to_intake_ticks: 2,
                ..config::DEFAULT_INPUT
            })
            .unwrap();
            let stored = StoredRun {
                case: Case::EqualTime,
                run: scenarios::run_case(configuration, Case::EqualTime),
            };
            let report = &stored.run.report;
            let snapshot = stored.run.scene.developer_snapshot();
            assert!(report.ok);
            assert!(report.steps.len() <= scenarios::MAX_STEPS);
            assert!(report.assertions.len() <= MAX_ASSERTIONS);
            assert_eq!(
                value(Some(&stored), 0, 5, 0),
                u32::try_from(report.steps.len()).unwrap()
            );
            assert_eq!(
                value(Some(&stored), 0, 6, 0),
                u32::try_from(report.assertions.len()).unwrap()
            );
            assert_eq!(value(Some(&stored), 0, 7, 0), u32::from(capacity < 128));
            assert_eq!(value(Some(&stored), 0, 8, 0), stored.run.scene.time());
            assert_eq!(
                value(Some(&stored), 0, 9, 0),
                u32::try_from(snapshot.dropped_trace).unwrap()
            );
            assert_eq!(
                value(Some(&stored), 0, 10, 0),
                u32::try_from(capacity).unwrap()
            );
            assert_eq!(snapshot.dropped_trace > 0, capacity < 128);
            if capacity == 0 {
                assert!(snapshot.trace.is_empty());
            }
            for (row, assertion) in report.assertions.iter().enumerate() {
                let row = u32::try_from(row).unwrap();
                assert_eq!(
                    value(Some(&stored), 1, row, 0),
                    u32::try_from(assertion.step).unwrap()
                );
                assert_eq!(value(Some(&stored), 1, row, 1), assertion.time);
                assert_eq!(
                    value(Some(&stored), 1, row, 2),
                    if assertion.ok.is_none() { 3 } else { 1 }
                );
                for (field, expected) in [assertion.code, &assertion.expected, &assertion.actual]
                    .into_iter()
                    .enumerate()
                {
                    assert_eq!(
                        text(Some(&stored), u32::try_from(field).unwrap(), row),
                        Some(expected.as_bytes())
                    );
                }
            }
            for (row, step) in report.steps.iter().enumerate() {
                let row = u32::try_from(row).unwrap();
                assert_eq!(value(Some(&stored), 2, row, 0), step.session);
                assert_eq!(
                    value(Some(&stored), 2, row, 1),
                    u32::try_from(step.index).unwrap()
                );
                assert_eq!(value(Some(&stored), 2, row, 2), step.time);
                assert!(value(Some(&stored), 2, row, 3) <= 6);
                assert_eq!(value(Some(&stored), 2, row, 4), step.result);
            }
        }
    }

    #[test]
    fn assertion_text_is_exact_utf8_with_a_fixed_byte_limit() {
        let mut stored = StoredRun {
            case: Case::DelayedRecipients,
            run: scenarios::run_case(
                scene::Repair::new().configuration(),
                Case::DelayedRecipients,
            ),
        };
        stored.run.report.assertions[0].actual = "é".repeat(MAX_TEXT_BYTES / 2);
        LATEST.with(|latest| *latest.borrow_mut() = Some(stored));
        assert_eq!(dw_repair_check_len(2, 0), 1024);
        assert_eq!(dw_repair_check_byte(2, 0, 0), 0xc3);
        assert_eq!(dw_repair_check_byte(2, 0, 1), 0xa9);
        assert_eq!(dw_repair_check_byte(2, 0, 1023), 0xa9);
        assert_eq!(dw_repair_check_byte(2, 0, 1024), INVALID_BYTE);
        LATEST.with(|latest| {
            latest.borrow_mut().as_mut().unwrap().run.report.assertions[0]
                .actual
                .push('x')
        });
        assert_eq!(dw_repair_check_len(2, 0), INVALID);
        assert_eq!(dw_repair_check_byte(2, 0, 0), INVALID_BYTE);
    }
}
