//! Version 1 repair-only, read-only scalar projection for the developer browser.
use crate::{bridge, scene};
use scene::{ActualStatus, DecisionDisposition, DecisionReason, Desk, Fact, RepairAction};

const INVALID: u32 = u32::MAX;

fn action(row: u32) -> Option<RepairAction> {
    match row {
        0 => Some(RepairAction::Inspect),
        1 => Some(RepairAction::Repair),
        2 => Some(RepairAction::Collect),
        _ => None,
    }
}

fn action_code(action: RepairAction) -> u32 {
    match action {
        RepairAction::Inspect => 1,
        RepairAction::Repair => 2,
        RepairAction::Collect => 3,
    }
}

fn disposition_code(disposition: DecisionDisposition) -> u32 {
    match disposition {
        DecisionDisposition::NoProposal => 1,
        DecisionDisposition::WouldReject => 2,
        DecisionDisposition::ReadyToExecute => 3,
    }
}

fn reason_code(reason: DecisionReason) -> u32 {
    match reason {
        DecisionReason::ReportNotArrived => 1,
        DecisionReason::AlreadyDiagnosed => 2,
        DecisionReason::DiagnosisNotArrived => 3,
        DecisionReason::AlreadyRepaired => 4,
        DecisionReason::ReadyNotArrived => 5,
        DecisionReason::AlreadyCollected => 6,
        DecisionReason::CableReceiptNotArrived => 7,
        DecisionReason::PriorRejectionUnchanged => 8,
        DecisionReason::RepairNotExecuted => 9,
        DecisionReason::PrerequisitesMet => 10,
    }
}

fn fact_code(fact: Fact) -> u32 {
    match fact {
        Fact::DonorReport => 1,
        Fact::Diagnosis => 2,
        Fact::CableReceipt => 3,
        Fact::Ready => 4,
        Fact::Collected => 5,
    }
}

fn desk_code(desk: Desk) -> u32 {
    match desk {
        Desk::Player => 1,
        Desk::Donor => 2,
        Desk::Intake => 3,
        Desk::Technician => 4,
        Desk::Stock => 5,
        Desk::Collection => 6,
    }
}

fn status_code(status: ActualStatus) -> u32 {
    match status {
        ActualStatus::Proposed => 1,
        ActualStatus::AcceptedUnconfirmed => 2,
        ActualStatus::HostRejected => 3,
        ActualStatus::Confirmed => 4,
    }
}

fn inspect(repair: &scene::Repair, section: u32, row: u32, column: u32) -> u32 {
    match section {
        0 if column == 0 => match row {
            0 => 1,
            1 => repair.time(),
            2 => u32::try_from(repair.developer_snapshot().dropped_trace).unwrap_or(INVALID),
            3 => u32::try_from(repair.developer_snapshot().actions.len()).unwrap_or(INVALID),
            _ => INVALID,
        },
        1 if column == 0 => {
            let config = repair.configuration();
            match row {
                // ValidatedConfig admits only schema 1 and the repair_cafe_v1 policy set.
                0 | 1 => 1,
                2 => config.trace_capacity() as u32,
                3 => config.report_to_intake_ticks() as u32,
                4 => config.report_to_technician_ticks() as u32,
                5 => config.technician_to_intake_ticks() as u32,
                6 => config.technician_to_collection_ticks() as u32,
                7 => config.cable_receipt_at_tick() as u32,
                _ => INVALID,
            }
        }
        2 if column < 3 => {
            let Some(action) = action(row) else {
                return INVALID;
            };
            let explanation = repair.explain(action);
            match column {
                0 => disposition_code(explanation.disposition),
                1 => reason_code(explanation.reason),
                2 => u32::try_from(explanation.causes.len()).unwrap_or(INVALID),
                _ => INVALID,
            }
        }
        3 if column < 3 => {
            let Some(action) = action(row / 2) else {
                return INVALID;
            };
            let explanation = repair.explain(action);
            let Some(cause) = explanation.causes.get((row % 2) as usize) else {
                return INVALID;
            };
            match column {
                0 => fact_code(cause.fact),
                1 => desk_code(cause.source),
                2 => u32::try_from(cause.time).unwrap_or(INVALID),
                _ => INVALID,
            }
        }
        4 if row < 4 && column < 4 => {
            let snapshot = repair.developer_snapshot();
            let Some(actual) = snapshot.actions.get(row as usize) else {
                return INVALID;
            };
            match column {
                0 => action_code(actual.action),
                1 => status_code(actual.status),
                2 => actual.rejection.map_or(0, reason_code),
                3 => actual.confirmed.as_ref().map_or(0, |e| fact_code(e.fact)),
                _ => INVALID,
            }
        }
        _ => INVALID,
    }
}

/// Sections: 0 metadata, 1 compiled config, 2 decisions, 3 causes, 4 actual history.
/// Every absent or out-of-range row/column returns u32::MAX.
#[unsafe(no_mangle)]
pub extern "C" fn dw_repair_inspect(section: u32, row: u32, column: u32) -> u32 {
    bridge::inspect(|fixture| inspect(&fixture.0, section, row, column))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(repair: &scene::Repair, section: u32, row: u32, columns: u32) -> Vec<u32> {
        (0..columns)
            .map(|column| inspect(repair, section, row, column))
            .collect()
    }

    fn act(repair: &mut scene::Repair, actions: &[u32]) {
        for &action in actions {
            assert_eq!(repair.act(action), 0);
        }
    }

    #[test]
    fn compiled_configuration_and_projection_use_the_live_player_fixture_read_only() {
        bridge::dw_init();
        for action in [1, 6] {
            assert_eq!(bridge::dw_act(action), 0);
        }
        let before = bridge::inspect(|fixture| fixture.0.developer_snapshot());
        let config = before.configuration;
        let expected = [
            1,
            1,
            config.trace_capacity() as u32,
            config.report_to_intake_ticks() as u32,
            config.report_to_technician_ticks() as u32,
            config.technician_to_intake_ticks() as u32,
            config.technician_to_collection_ticks() as u32,
            config.cable_receipt_at_tick() as u32,
        ];
        for (row, value) in expected.into_iter().enumerate() {
            assert_eq!(dw_repair_inspect(1, row as u32, 0), value);
        }
        let player_outputs = || {
            (0..3)
                .map(|field| {
                    (0..bridge::dw_len(field))
                        .map(|index| bridge::dw_byte(field, index))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        };
        let output_before = player_outputs();
        for _ in 0..3 {
            for section in [0, 1, 2, 3, 4, 5, INVALID] {
                for row in [0, 1, 2, 3, 4, 5, 6, 7, 8, INVALID] {
                    for column in [0, 1, 2, 3, 4, INVALID] {
                        let expected =
                            bridge::inspect(|fixture| inspect(&fixture.0, section, row, column));
                        assert_eq!(dw_repair_inspect(section, row, column), expected);
                    }
                }
            }
        }
        assert_eq!(bridge::inspect(|f| f.0.developer_snapshot()), before);
        assert_eq!(player_outputs(), output_before);
        assert_eq!(dw_repair_inspect(0, 0, 0), 1);
        assert_eq!(dw_repair_inspect(0, 1, 0), bridge::dw_time());
        assert_eq!(dw_repair_inspect(0, 2, 0), before.dropped_trace as u32);
        assert_eq!(dw_repair_inspect(0, 3, 0), 0);
        assert_eq!(bridge::dw_act(6), 0);
        assert_eq!(dw_repair_inspect(0, 1, 0), 2);
        bridge::dw_init();
        assert_eq!(dw_repair_inspect(0, 1, 0), 0);
        assert_eq!(dw_repair_inspect(0, 3, 0), 0);
        assert_eq!(dw_repair_inspect(4, 0, 0), INVALID);
    }

    #[test]
    fn reasons_causes_rejection_reuse_and_actual_confirmations_match_typed_logic() {
        let mut repair = scene::Repair::new();
        assert_eq!(row(&repair, 2, 0, 3), [1, 1, 0]);
        assert_eq!(row(&repair, 2, 1, 3), [1, 3, 0]);
        assert_eq!(row(&repair, 2, 2, 3), [1, 5, 0]);
        act(&mut repair, &[1, 6]);
        // Intake knows the report at tick 1; the technician still cannot use it.
        assert!(
            repair
                .developer_snapshot()
                .known
                .iter()
                .any(|(desk, facts)| {
                    *desk == Desk::Intake && facts.iter().any(|fact| fact.fact == Fact::DonorReport)
                })
        );
        assert_eq!(row(&repair, 2, 0, 3), [1, 1, 0]);
        assert_eq!(row(&repair, 3, 0, 3), [INVALID; 3]);
        act(&mut repair, &[6]);
        let typed = repair.explain(RepairAction::Inspect);
        assert_eq!(typed.disposition, DecisionDisposition::ReadyToExecute);
        assert_eq!(typed.reason, DecisionReason::PrerequisitesMet);
        assert_eq!(typed.causes[0].fact, Fact::DonorReport);
        assert_eq!(row(&repair, 2, 0, 3), [3, 10, 1]);
        assert_eq!(row(&repair, 3, 0, 3), [1, 2, 0]);
        assert_eq!(row(&repair, 3, 1, 3), [INVALID; 3]);
        act(&mut repair, &[2]);
        assert_eq!(row(&repair, 2, 0, 3), [1, 2, 0]);
        assert_eq!(row(&repair, 2, 1, 3), [2, 7, 1]);
        assert_eq!(row(&repair, 3, 2, 3), [2, 4, 2]);
        assert_eq!(row(&repair, 4, 0, 4), [1, 4, 0, 2]);
        let typed = repair.developer_snapshot().actions.remove(0);
        assert_eq!(typed.status, ActualStatus::Confirmed);
        assert_eq!(typed.confirmed.unwrap().fact, Fact::Diagnosis);
        act(&mut repair, &[3]);
        let rejected = repair.developer_snapshot().actions.remove(1);
        assert_eq!(rejected.status, ActualStatus::HostRejected);
        assert_eq!(
            rejected.rejection,
            Some(DecisionReason::CableReceiptNotArrived)
        );
        assert!(rejected.confirmed.is_none());
        assert_eq!(row(&repair, 4, 1, 4), [2, 3, 7, 0]);
        assert_eq!(row(&repair, 2, 1, 3), [1, 8, 0]);
        let before = repair.developer_snapshot();
        act(&mut repair, &[3, 3]);
        assert_eq!(repair.developer_snapshot(), before);
        act(&mut repair, &[6, 6]);
        assert_eq!(row(&repair, 2, 1, 3), [3, 10, 2]);
        assert_eq!(row(&repair, 3, 2, 3), [2, 4, 2]);
        // Receipt creation at tick 0 is distinct from arrival at tick 4.
        assert_eq!(row(&repair, 3, 3, 3), [3, 5, 0]);
        act(&mut repair, &[3]);
        assert_eq!(row(&repair, 4, 2, 4), [2, 4, 0, 4]);
        assert_eq!(row(&repair, 2, 1, 3), [1, 4, 0]);
        assert_eq!(row(&repair, 2, 2, 3), [1, 5, 0]);
        act(&mut repair, &[6, 6]);
        assert_eq!(row(&repair, 2, 2, 3), [3, 10, 1]);
        assert_eq!(row(&repair, 3, 4, 3), [4, 4, 4]);
        act(&mut repair, &[4]);
        assert_eq!(row(&repair, 4, 3, 4), [3, 4, 0, 5]);
        assert_eq!(row(&repair, 2, 2, 3), [1, 6, 0]);
        assert_eq!(inspect(&repair, 0, 3, 0), 4);
        assert_eq!(repair.developer_snapshot().actions[1], rejected);
        assert_eq!(row(&repair, 4, 4, 4), [INVALID; 4]);
        act(&mut repair, &[0]);
        assert_eq!(inspect(&repair, 0, 3, 0), 0);
        assert_eq!(row(&repair, 4, 0, 4), [INVALID; 4]);
    }

    #[test]
    fn alternate_validated_configuration_trace_drops_and_all_bounds_are_read_only() {
        let config = scene::config::validate(scene::config::Input {
            trace_capacity: 0,
            report_to_intake_ticks: 7,
            report_to_technician_ticks: 0,
            technician_to_intake_ticks: 1000,
            technician_to_collection_ticks: 9,
            cable_receipt_at_tick: 0,
            ..scene::config::DEFAULT_INPUT
        })
        .unwrap();
        let mut repair = scene::Repair::with_config(config);
        act(&mut repair, &[1, 2, 3]);
        assert_eq!(inspect(&repair, 1, 2, 0), 0);
        assert_eq!(inspect(&repair, 1, 3, 0), 7);
        assert_eq!(inspect(&repair, 1, 4, 0), 0);
        assert_eq!(inspect(&repair, 1, 5, 0), 1000);
        assert_eq!(inspect(&repair, 1, 6, 0), 9);
        assert_eq!(inspect(&repair, 1, 7, 0), 0);
        let before = repair.developer_snapshot();
        assert!(before.dropped_trace > 0);
        assert!(before.trace.is_empty());
        assert_eq!(inspect(&repair, 0, 2, 0), before.dropped_trace as u32);
        for (section, rows, columns) in [(0, 4, 1), (1, 8, 1), (2, 3, 3), (3, 6, 3), (4, 4, 4)] {
            for row in [rows, INVALID] {
                for column in 0..columns {
                    assert_eq!(inspect(&repair, section, row, column), INVALID);
                }
            }
            for row in 0..rows {
                for column in [columns, INVALID] {
                    assert_eq!(inspect(&repair, section, row, column), INVALID);
                }
            }
        }
        for section in [5, INVALID] {
            assert_eq!(inspect(&repair, section, 0, 0), INVALID);
        }
        assert_eq!(repair.developer_snapshot(), before);
    }
}
