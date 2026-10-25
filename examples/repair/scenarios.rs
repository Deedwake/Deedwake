//! Five bounded, compiled scenarios shared by native and browser repair hosts.
use crate::scene::{self, ActualStatus, Desk, Fact, Repair, RepairAction, config};
use deedwake_core::Event;

pub const MAX_STEPS: usize = 2128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    DelayedRecipients,
    RejectionThenSuccess,
    EqualTime,
    RepeatedActions,
    Reset,
}
impl Case {
    pub const ALL: [Self; 5] = [
        Self::DelayedRecipients,
        Self::RejectionThenSuccess,
        Self::EqualTime,
        Self::RepeatedActions,
        Self::Reset,
    ];

    pub const fn id(self) -> u32 {
        match self {
            Self::DelayedRecipients => 1,
            Self::RejectionThenSuccess => 2,
            Self::EqualTime => 3,
            Self::RepeatedActions => 4,
            Self::Reset => 5,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DelayedRecipients => "delayed-recipients",
            Self::RejectionThenSuccess => "rejection-then-success",
            Self::EqualTime => "equal-time",
            Self::RepeatedActions => "repeated-actions",
            Self::Reset => "reset",
        }
    }

    pub fn from_id(id: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|case| case.id() == id)
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|case| case.as_str() == name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScenarioReport {
    pub session: u32,
    pub ok: bool,
    pub steps: Vec<ScenarioStep>,
    pub assertions: Vec<ScenarioAssertion>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScenarioStep {
    pub session: u32,
    pub index: usize,
    pub time: u32,
    pub action: &'static str,
    pub result: u32,
    pub decision: Option<scene::DecisionExplanation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScenarioAssertion {
    pub step: usize,
    pub time: u32,
    pub code: &'static str,
    pub expected: String,
    pub actual: String,
    pub status: &'static str,
    pub ok: Option<bool>,
}

pub fn run_case(configuration: config::ValidatedConfig, case: Case) -> CaseRun {
    let mut run = CaseRun {
        scene: Repair::with_config(configuration),
        report: ScenarioReport {
            session: 0,
            ok: true,
            steps: vec![],
            assertions: vec![],
        },
    };
    run.run(case);
    run
}

pub struct CaseRun {
    pub scene: Repair,
    pub report: ScenarioReport,
}
impl CaseRun {
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
        self.report.assertions.push(ScenarioAssertion {
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
        .map(|a| self.scene.explain(a));
        let time = self.scene.time();
        let result = self.scene.act(number);
        if number == 0 && result == 0 {
            self.report.session += 1;
        }
        let time = if number == 0 { self.scene.time() } else { time };
        self.report.steps.push(ScenarioStep {
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
    fn run(&mut self, case: Case) {
        let c = self.scene.configuration();
        match case {
            Case::DelayedRecipients => {
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
            Case::RejectionThenSuccess => {
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
            Case::EqualTime => {
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
                    self.report.assertions.push(ScenarioAssertion {
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
            Case::RepeatedActions => {
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
            Case::Reset => {
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
        }
    }
}
