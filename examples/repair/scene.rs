//! Bounded, synthetic repair café host shared by native and WebAssembly callers.
//! No real appliance control, external messages, purchases, or safety advice.
use deedwake_core::{
    ActionId, Content, Core, Entity, Perspective, Proposal, Record, RecordId, Status, TraceEntry,
};

#[path = "config.rs"]
pub mod config;
use config::{MAX_TIME, ValidatedConfig};

const PLAYER: Entity = Entity(1);
const DONOR: Entity = Entity(2);
const INTAKE: Entity = Entity(3);
const TECHNICIAN: Entity = Entity(4);
const STOCK: Entity = Entity(5);
const COLLECTION: Entity = Entity(6);
const REPORT_CODE: &str = "repair.report.broken.v1";
const DIAGNOSIS_CODE: &str = "repair.diagnosis.cable.v1";
const CABLE_CODE: &str = "repair.stock.cable.v1";
const READY_CODE: &str = "repair.ready.v1";
const COLLECTED_CODE: &str = "repair.collected.v1";

const REPORT: &str =
    "Donor report: the donated fictional lamp does not light; the cause is unknown.";
const DIAGNOSIS: &str = "Inspection confirmed: cable-only fault in the fictional lamp.";
const CABLE: &str = "Synthetic stock receipt: a replacement cable is available.";
const READY: &str = "Repair confirmed: fictional cable replaced; lamp ready for collection.";
const COLLECTED: &str = "Collection confirmed: the fictional lamp was handed over.";
const REJECTION: &str =
    "Repair rejected: no cable availability receipt has reached the technician.";

fn account(view: &Perspective<'_>, source: Entity, summary: &str) -> Option<RecordId> {
    view.records().iter().find_map(|record| {
        (record.source == source
            && matches!(&record.content, Content::Account { summary: text, .. } if text == summary))
        .then_some(record.id)
    })
}

fn outcome(view: &Perspective<'_>, source: Entity, description: &str) -> Option<RecordId> {
    view.records().iter().find_map(|record| {
        (record.source == source
            && matches!(&record.content, Content::ConfirmedOutcome { description: text, .. } if text == description))
        .then_some(record.id)
    })
}

fn cable(view: &Perspective<'_>) -> Option<RecordId> {
    view.records().iter().find_map(|record| {
        (record.source == STOCK
            && matches!(&record.content, Content::Observation(text) if text == CABLE_CODE))
        .then_some(record.id)
    })
}

/// Compiled repair actions available to host-side developer inspection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepairAction {
    Inspect,
    Repair,
    Collect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Desk {
    Player,
    Donor,
    Intake,
    Technician,
    Stock,
    Collection,
}

impl Desk {
    fn entity(self) -> Entity {
        match self {
            Self::Player => PLAYER,
            Self::Donor => DONOR,
            Self::Intake => INTAKE,
            Self::Technician => TECHNICIAN,
            Self::Stock => STOCK,
            Self::Collection => COLLECTION,
        }
    }
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Player => "player",
            Self::Donor => "donor",
            Self::Intake => "intake",
            Self::Technician => "technician",
            Self::Stock => "stock",
            Self::Collection => "collection",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fact {
    DonorReport,
    Diagnosis,
    CableReceipt,
    Ready,
    Collected,
}

impl Fact {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DonorReport => "donor_report",
            Self::Diagnosis => "diagnosis",
            Self::CableReceipt => "cable_receipt",
            Self::Ready => "ready",
            Self::Collected => "collected",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Evidence {
    pub fact: Fact,
    pub record: RecordId,
    pub source: Desk,
    pub time: u64,
}

fn evidence(record: &Record) -> Option<Evidence> {
    let (fact, source) = match (record.source, &record.content) {
        (DONOR, Content::Account { summary, .. }) if summary == REPORT_CODE => {
            (Fact::DonorReport, Desk::Donor)
        }
        (TECHNICIAN, Content::ConfirmedOutcome { description, .. })
            if description == DIAGNOSIS_CODE =>
        {
            (Fact::Diagnosis, Desk::Technician)
        }
        (STOCK, Content::Observation(text)) if text == CABLE_CODE => {
            (Fact::CableReceipt, Desk::Stock)
        }
        (TECHNICIAN, Content::ConfirmedOutcome { description, .. })
            if description == READY_CODE =>
        {
            (Fact::Ready, Desk::Technician)
        }
        (COLLECTION, Content::ConfirmedOutcome { description, .. })
            if description == COLLECTED_CODE =>
        {
            (Fact::Collected, Desk::Collection)
        }
        _ => return None,
    };
    Some(Evidence {
        fact,
        record: record.id,
        source,
        time: record.time,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionReason {
    ReportNotArrived,
    AlreadyDiagnosed,
    DiagnosisNotArrived,
    AlreadyRepaired,
    ReadyNotArrived,
    AlreadyCollected,
    CableReceiptNotArrived,
    PriorRejectionUnchanged,
    RepairNotExecuted,
    PrerequisitesMet,
}

impl DecisionReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReportNotArrived => "report_not_arrived",
            Self::AlreadyDiagnosed => "already_diagnosed",
            Self::DiagnosisNotArrived => "diagnosis_not_arrived",
            Self::AlreadyRepaired => "already_repaired",
            Self::ReadyNotArrived => "ready_not_arrived",
            Self::AlreadyCollected => "already_collected",
            Self::CableReceiptNotArrived => "cable_receipt_not_arrived",
            Self::PriorRejectionUnchanged => "prior_rejection_unchanged",
            Self::RepairNotExecuted => "repair_not_executed",
            Self::PrerequisitesMet => "prerequisites_met",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionDisposition {
    NoProposal,
    WouldReject,
    ReadyToExecute,
}

impl DecisionDisposition {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoProposal => "no_proposal",
            Self::WouldReject => "would_reject",
            Self::ReadyToExecute => "ready_to_execute",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionExplanation {
    pub action: RepairAction,
    pub disposition: DecisionDisposition,
    pub reason: DecisionReason,
    pub causes: Vec<Evidence>,
}

/// Actual lifecycle state, never inferred from the handled-action ABI return value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActualStatus {
    Proposed,
    AcceptedUnconfirmed,
    HostRejected,
    Confirmed,
}

impl ActualStatus {
    pub fn from_core(status: &Status) -> Self {
        match status {
            Status::Proposed => Self::Proposed,
            Status::Accepted => Self::AcceptedUnconfirmed,
            Status::Rejected(_) => Self::HostRejected,
            Status::Confirmed(_) => Self::Confirmed,
        }
    }
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::AcceptedUnconfirmed => "accepted_unconfirmed",
            Self::HostRejected => "host_rejected",
            Self::Confirmed => "confirmed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeveloperAction {
    pub action: RepairAction,
    pub id: ActionId,
    pub status: ActualStatus,
    pub causes: Vec<Evidence>,
    pub confirmed: Option<Evidence>,
    pub rejection: Option<DecisionReason>,
}

/// Privileged host state. This is not a player journal or policy perspective.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeveloperSnapshot {
    pub time: u64,
    pub configuration: ValidatedConfig,
    pub known: Vec<(Desk, Vec<Evidence>)>,
    pub actions: Vec<DeveloperAction>,
    pub repaired: bool,
    pub collected: bool,
    pub trace: Vec<TraceEntry>,
    pub dropped_trace: u64,
}

impl RepairAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Inspect => "inspect",
            Self::Repair => "repair",
            Self::Collect => "collect",
        }
    }
    fn actor(self) -> Entity {
        match self {
            Self::Inspect | Self::Repair => TECHNICIAN,
            Self::Collect => COLLECTION,
        }
    }
}

struct PolicyDecision {
    reason: DecisionReason,
    causes: Vec<RecordId>,
}

impl PolicyDecision {
    fn proposal(self, action: RepairAction) -> Option<Proposal> {
        if self.reason != DecisionReason::PrerequisitesMet {
            return None;
        }
        let (request, rule) = match action {
            RepairAction::Inspect => (
                "inspect donated lamp",
                "An arrived donor report permits a synthetic inspection, not a diagnosis by itself",
            ),
            RepairAction::Repair => (
                "replace cable and mark ready",
                "A cable-only diagnosis proposes repair; the host must check the cable receipt and execute it",
            ),
            RepairAction::Collect => (
                "hand over repaired lamp",
                "Only an arrived confirmed readiness record permits a collection proposal",
            ),
        };
        Some(Proposal {
            request: request.into(),
            rule: rule.into(),
            causes: self.causes,
        })
    }
}

fn policy_decision(view: &Perspective<'_>, action: RepairAction) -> PolicyDecision {
    let (completed, already, prerequisite, missing) = match action {
        RepairAction::Inspect => (
            outcome(view, TECHNICIAN, DIAGNOSIS_CODE),
            DecisionReason::AlreadyDiagnosed,
            account(view, DONOR, REPORT_CODE),
            DecisionReason::ReportNotArrived,
        ),
        RepairAction::Repair => (
            outcome(view, TECHNICIAN, READY_CODE),
            DecisionReason::AlreadyRepaired,
            outcome(view, TECHNICIAN, DIAGNOSIS_CODE),
            DecisionReason::DiagnosisNotArrived,
        ),
        RepairAction::Collect => (
            outcome(view, COLLECTION, COLLECTED_CODE),
            DecisionReason::AlreadyCollected,
            outcome(view, TECHNICIAN, READY_CODE),
            DecisionReason::ReadyNotArrived,
        ),
    };
    if completed.is_some() {
        return PolicyDecision {
            reason: already,
            causes: vec![],
        };
    }
    let Some(prerequisite) = prerequisite else {
        return PolicyDecision {
            reason: missing,
            causes: vec![],
        };
    };
    let mut causes = vec![prerequisite];
    if action == RepairAction::Repair {
        causes.extend(cable(view));
    }
    PolicyDecision {
        reason: DecisionReason::PrerequisitesMet,
        causes,
    }
}

fn host_rejection(
    view: &Perspective<'_>,
    action: RepairAction,
    repaired: bool,
) -> Option<DecisionReason> {
    match action {
        RepairAction::Repair if cable(view).is_none() => {
            Some(DecisionReason::CableReceiptNotArrived)
        }
        RepairAction::Collect if !repaired => Some(DecisionReason::RepairNotExecuted),
        _ => None,
    }
}

fn inspection_policy(view: Perspective<'_>) -> Option<Proposal> {
    policy_decision(&view, RepairAction::Inspect).proposal(RepairAction::Inspect)
}

fn repair_policy(view: Perspective<'_>) -> Option<Proposal> {
    policy_decision(&view, RepairAction::Repair).proposal(RepairAction::Repair)
}

fn collection_policy(view: Perspective<'_>) -> Option<Proposal> {
    policy_decision(&view, RepairAction::Collect).proposal(RepairAction::Collect)
}

pub struct Repair {
    core: Core,
    configuration: ValidatedConfig,
    report: Option<RecordId>,
    diagnosis: Option<RecordId>,
    rejected_repair: Option<ActionId>,
    repaired: bool,
    collected: bool,
    action_history: Vec<(RepairAction, ActionId, Option<DecisionReason>)>,
    journal: Vec<String>,
    dialogue: String,
}

impl Repair {
    pub fn new() -> Self {
        Self::with_config(config::validate(config::DEFAULT_INPUT).expect("valid repair defaults"))
    }

    pub fn with_config(configuration: ValidatedConfig) -> Self {
        let mut core = Core::new(configuration.trace_capacity());
        // Created is not received: only the stock source knows this until its scheduled arrival.
        let receipt = core.observe(STOCK, CABLE_CODE.into());
        core.schedule(
            STOCK,
            TECHNICIAN,
            receipt,
            configuration.cable_receipt_at_tick(),
        )
        .unwrap();
        core.advance_to(0).unwrap();
        Self {
            core,
            configuration,
            report: None,
            diagnosis: None,
            rejected_repair: None,
            repaired: false,
            collected: false,
            action_history: vec![],
            journal: vec![],
            dialogue: "A synthetic repair café: submit a donated fictional lamp, visit the technician, and follow the delayed reports to intake and collection. Everything here is a bounded simulation.".into(),
        }
    }

    pub fn configuration(&self) -> ValidatedConfig {
        self.configuration
    }

    fn after(&self, delay: u64) -> u64 {
        self.core
            .time()
            .checked_add(delay)
            .expect("validated delay fits the bounded repair clock")
    }

    fn learn(&mut self, message: &str) {
        // Fixed encounter messages are recorded once, bounding records on repeat visits.
        if !self.journal.iter().any(|entry| entry == message) {
            self.core.observe(PLAYER, message.into());
            self.journal.push(message.into());
        }
        self.dialogue = message.into();
    }

    fn receive(&mut self, source: Entity, record: RecordId) {
        if !self
            .core
            .perspective(PLAYER)
            .records()
            .iter()
            .any(|known| known.id == record)
        {
            self.core
                .schedule(source, PLAYER, record, self.core.time())
                .unwrap();
            self.core.advance_to(self.core.time()).unwrap();
        }
        let view = self.core.perspective(PLAYER);
        let known = view
            .records()
            .iter()
            .find(|known| known.id == record)
            .expect("local encounter delivered its record");
        let message = match (known.source, &known.content) {
            (STOCK, Content::Observation(text)) if text == CABLE_CODE => CABLE,
            (DONOR, Content::Account { summary, .. }) if summary == REPORT_CODE => REPORT,
            (TECHNICIAN, Content::ConfirmedOutcome { description, .. })
                if description == DIAGNOSIS_CODE =>
            {
                DIAGNOSIS
            }
            (TECHNICIAN, Content::ConfirmedOutcome { description, .. })
                if description == READY_CODE =>
            {
                READY
            }
            (COLLECTION, Content::ConfirmedOutcome { description, .. })
                if description == COLLECTED_CODE =>
            {
                COLLECTED
            }
            (_, Content::Observation(text)) => text,
            (_, Content::Account { summary, .. }) => summary,
            (_, Content::ConfirmedOutcome { description, .. }) => description,
        }
        .to_owned();
        self.learn(&message);
    }

    fn submit(&mut self) {
        let observed = self.core.observe(
            DONOR,
            "Fictional lamp does not light; cause not inspected".into(),
        );
        let report = self
            .core
            .account(DONOR, observed, REPORT_CODE.into())
            .unwrap();
        self.core
            .schedule(
                DONOR,
                INTAKE,
                report,
                self.after(self.configuration.report_to_intake_ticks()),
            )
            .unwrap();
        self.core
            .schedule(
                DONOR,
                TECHNICIAN,
                report,
                self.after(self.configuration.report_to_technician_ticks()),
            )
            .unwrap();
        self.report = Some(report);
        self.receive(DONOR, report);
        self.learn(&format!("You submit the fictional lamp. The fixed, incomplete donor report reaches intake after {} fictional hour(s) and the technician after {} fictional hour(s).", self.configuration.report_to_intake_ticks(), self.configuration.report_to_technician_ticks()));
    }

    fn inspect(&mut self) {
        let Some(action) = self.core.evaluate(TECHNICIAN, inspection_policy).unwrap() else {
            self.learn("Technician: no donor report has reached me, so I cannot propose an inspection yet.");
            return;
        };
        self.action_history
            .push((RepairAction::Inspect, action, None));
        self.core.accept(action).unwrap();
        // The synthetic host performs the inspection before confirming its finding.
        let finding = self.core.confirm(action, DIAGNOSIS_CODE.into()).unwrap();
        self.diagnosis = Some(finding);
        self.core
            .schedule(
                TECHNICIAN,
                INTAKE,
                finding,
                self.after(self.configuration.technician_to_intake_ticks()),
            )
            .unwrap();
        self.core
            .schedule(
                TECHNICIAN,
                COLLECTION,
                finding,
                self.after(self.configuration.technician_to_collection_ticks()),
            )
            .unwrap();
        self.receive(TECHNICIAN, finding);
    }

    fn repair(&mut self) {
        let rejection = host_rejection(
            &self.core.perspective(TECHNICIAN),
            RepairAction::Repair,
            self.repaired,
        );
        let has_cable = rejection.is_none();
        if self.reuses_rejection(RepairAction::Repair, rejection) {
            self.learn(REJECTION);
            return;
        }
        let Some(action) = self.core.evaluate(TECHNICIAN, repair_policy).unwrap() else {
            self.learn("Technician: I have no confirmed inspection diagnosis on which to request a repair.");
            return;
        };
        self.action_history
            .push((RepairAction::Repair, action, rejection));
        if !has_cable {
            self.core.reject(action, REJECTION.into()).unwrap();
            self.rejected_repair = Some(action);
            self.learn(REJECTION);
            return;
        }
        self.core.accept(action).unwrap();
        self.repaired = true;
        let ready = self.core.confirm(action, READY_CODE.into()).unwrap();
        self.core
            .schedule(
                TECHNICIAN,
                INTAKE,
                ready,
                self.after(self.configuration.technician_to_intake_ticks()),
            )
            .unwrap();
        self.core
            .schedule(
                TECHNICIAN,
                COLLECTION,
                ready,
                self.after(self.configuration.technician_to_collection_ticks()),
            )
            .unwrap();
        self.receive(TECHNICIAN, ready);
    }

    fn collect(&mut self) {
        let Some(action) = self.core.evaluate(COLLECTION, collection_policy).unwrap() else {
            self.learn("Collection: no confirmed readiness record has reached this desk. I cannot hand over the lamp.");
            return;
        };
        let rejection = host_rejection(
            &self.core.perspective(COLLECTION),
            RepairAction::Collect,
            self.repaired,
        );
        self.action_history
            .push((RepairAction::Collect, action, rejection));
        if rejection.is_some() {
            self.core
                .reject(action, "Synthetic host has not executed the repair".into())
                .unwrap();
            self.learn("Collection: the synthetic host cannot confirm a completed repair.");
            return;
        }
        self.core.accept(action).unwrap();
        self.collected = true;
        let receipt = self.core.confirm(action, COLLECTED_CODE.into()).unwrap();
        self.receive(COLLECTION, receipt);
    }

    fn ask_intake(&mut self) {
        let view = self.core.perspective(INTAKE);
        let known = outcome(&view, TECHNICIAN, READY_CODE)
            .or_else(|| outcome(&view, TECHNICIAN, DIAGNOSIS_CODE))
            .or_else(|| account(&view, DONOR, REPORT_CODE));
        let message = if outcome(&view, TECHNICIAN, READY_CODE).is_some() {
            "Intake: a confirmed repair receipt has arrived. Our desk knows the lamp is ready; collection has its own delivery delay."
        } else if outcome(&view, TECHNICIAN, DIAGNOSIS_CODE).is_some() {
            "Intake: the inspection finding has arrived, but no confirmed repair receipt has reached me."
        } else if known.is_some() {
            "Intake: I have only the donor's incomplete broken report, with no inspection finding or confirmed repair receipt."
        } else {
            "Intake: no donor report has reached me yet."
        };
        if let Some(record) = known {
            self.receive(INTAKE, record);
        }
        self.learn(message);
    }

    fn reuses_rejection(&self, action: RepairAction, rejection: Option<DecisionReason>) -> bool {
        action == RepairAction::Repair
            && rejection == Some(DecisionReason::CableReceiptNotArrived)
            && self.rejected_repair.is_some()
    }

    /// Read-only prediction using the same local policy and host check as execution.
    pub fn explain(&self, action: RepairAction) -> DecisionExplanation {
        let view = self.core.perspective(action.actor());
        let rejection = host_rejection(&view, action, self.repaired);
        if self.reuses_rejection(action, rejection) {
            return DecisionExplanation {
                action,
                disposition: DecisionDisposition::NoProposal,
                reason: DecisionReason::PriorRejectionUnchanged,
                causes: vec![],
            };
        }
        let decision = policy_decision(&view, action);
        let causes = decision
            .causes
            .iter()
            .map(|id| {
                evidence(
                    view.records()
                        .iter()
                        .find(|record| record.id == *id)
                        .unwrap(),
                )
                .unwrap()
            })
            .collect();
        let (disposition, reason) = if decision.reason != DecisionReason::PrerequisitesMet {
            (DecisionDisposition::NoProposal, decision.reason)
        } else if let Some(reason) = rejection {
            (DecisionDisposition::WouldReject, reason)
        } else {
            (
                DecisionDisposition::ReadyToExecute,
                DecisionReason::PrerequisitesMet,
            )
        };
        DecisionExplanation {
            action,
            disposition,
            reason,
            causes,
        }
    }

    pub fn developer_snapshot(&self) -> DeveloperSnapshot {
        let known = [
            Desk::Player,
            Desk::Donor,
            Desk::Intake,
            Desk::Technician,
            Desk::Stock,
            Desk::Collection,
        ]
        .into_iter()
        .map(|desk| {
            (
                desk,
                self.core
                    .perspective(desk.entity())
                    .records()
                    .iter()
                    .filter_map(|record| evidence(record))
                    .collect(),
            )
        })
        .collect();
        let actions = self
            .action_history
            .iter()
            .map(|(kind, id, rejection)| {
                let action = self.core.action(*id).unwrap();
                let confirmed = match action.status {
                    Status::Confirmed(record) => evidence(self.core.record(record).unwrap()),
                    _ => None,
                };
                DeveloperAction {
                    action: *kind,
                    id: *id,
                    status: ActualStatus::from_core(&action.status),
                    causes: action
                        .proposal
                        .causes
                        .iter()
                        .map(|id| evidence(self.core.record(*id).unwrap()).unwrap())
                        .collect(),
                    confirmed,
                    rejection: *rejection,
                }
            })
            .collect();
        DeveloperSnapshot {
            time: self.core.time(),
            configuration: self.configuration,
            known,
            actions,
            repaired: self.repaired,
            collected: self.collected,
            trace: self.core.trace().iter().cloned().collect(),
            dropped_trace: self.core.dropped_trace(),
        }
    }

    pub fn time(&self) -> u32 {
        self.core.time() as u32
    }

    pub fn enabled(&self, action: u32) -> bool {
        // These track completed player actions, never unseen delivery arrivals.
        match action {
            0 | 5 => true,
            1 => self.report.is_none(),
            2 => self.diagnosis.is_none(),
            3 => !self.repaired,
            4 => !self.collected,
            6 => self.time() < MAX_TIME,
            _ => false,
        }
    }

    pub fn act(&mut self, action: u32) -> u32 {
        if action == 6 && self.time() == MAX_TIME {
            return 2;
        }
        if !self.enabled(action) {
            return 1;
        }
        match action {
            0 => *self = Self::with_config(self.configuration),
            1 => self.submit(),
            2 => self.inspect(),
            3 => self.repair(),
            4 => self.collect(),
            5 => self.ask_intake(),
            6 => {
                self.core.advance_to(self.after(1)).unwrap();
                self.dialogue =
                    "One fictional hour passes. Visit a desk to learn what has reached it.".into();
            }
            _ => unreachable!(),
        }
        0
    }

    pub fn output(&self, field: u32) -> Option<String> {
        match field {
            0 => Some(self.dialogue.clone()),
            1 => Some(self.journal.join("\n")),
            2 => Some(format!(
                "Dropped entries: {}\n{:#?}",
                self.core.dropped_trace(),
                self.core.trace()
            )),
            _ => None,
        }
    }
}

impl Default for Repair {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deedwake_core::{Event, Status};

    fn act(scene: &mut Repair, actions: &[u32]) {
        for &action in actions {
            assert_eq!(
                scene.act(action),
                0,
                "action {action} at hour {}",
                scene.time()
            );
        }
    }

    #[test]
    fn developer_explanation_is_read_only_and_keeps_local_causes() {
        let mut scene = Repair::new();
        act(&mut scene, &[1, 6]);
        let before = scene.developer_snapshot();
        let outputs = (scene.output(0), scene.output(1), scene.output(2));
        for _ in 0..3 {
            let inspection = scene.explain(RepairAction::Inspect);
            assert_eq!(inspection.disposition, DecisionDisposition::NoProposal);
            assert_eq!(inspection.reason, DecisionReason::ReportNotArrived);
            assert!(inspection.causes.is_empty());
            assert_eq!(
                scene.explain(RepairAction::Repair).reason,
                DecisionReason::DiagnosisNotArrived
            );
            assert_eq!(
                scene.explain(RepairAction::Collect).reason,
                DecisionReason::ReadyNotArrived
            );
            assert_eq!(scene.developer_snapshot(), before);
        }
        assert_eq!((scene.output(0), scene.output(1), scene.output(2)), outputs);
        // Intake knows the report at tick 1; explaining the technician must not borrow it.
        assert!(
            before
                .known
                .iter()
                .find(|(desk, _)| *desk == Desk::Intake)
                .unwrap()
                .1
                .iter()
                .any(|fact| fact.fact == Fact::DonorReport)
        );
        assert_eq!(scene.act(2), 0); // Handled does not mean a proposal or confirmation.
        assert!(scene.developer_snapshot().actions.is_empty());
        assert!(scene.diagnosis.is_none());
    }

    #[test]
    fn developer_reasons_match_proposals_host_rejection_and_confirmation() {
        let mut scene = Repair::new();
        act(&mut scene, &[1, 6, 6]);
        let inspect = scene.explain(RepairAction::Inspect);
        assert_eq!(inspect.disposition, DecisionDisposition::ReadyToExecute);
        assert_eq!(inspect.reason, DecisionReason::PrerequisitesMet);
        assert_eq!(
            inspect.causes.iter().map(|e| e.fact).collect::<Vec<_>>(),
            vec![Fact::DonorReport]
        );
        act(&mut scene, &[2]);
        let inspection = scene.developer_snapshot().actions.pop().unwrap();
        assert_eq!(inspection.status, ActualStatus::Confirmed);
        assert_eq!(inspection.causes, inspect.causes);
        assert_eq!(inspection.confirmed.unwrap().fact, Fact::Diagnosis);
        assert_eq!(
            scene.explain(RepairAction::Inspect).reason,
            DecisionReason::AlreadyDiagnosed
        );

        let repair = scene.explain(RepairAction::Repair);
        assert_eq!(repair.disposition, DecisionDisposition::WouldReject);
        assert_eq!(repair.reason, DecisionReason::CableReceiptNotArrived);
        assert_eq!(scene.developer_snapshot().actions.len(), 1);
        act(&mut scene, &[3]);
        let rejected = scene.developer_snapshot().actions.pop().unwrap();
        assert_eq!(rejected.action, RepairAction::Repair);
        assert_eq!(rejected.status, ActualStatus::HostRejected);
        assert_eq!(rejected.rejection, Some(repair.reason));
        assert_eq!(rejected.causes, repair.causes);
        assert_eq!(rejected.confirmed, None);
        let before = scene.developer_snapshot();
        let outputs = (scene.output(0), scene.output(1), scene.output(2));
        for _ in 0..3 {
            let repeated = scene.explain(RepairAction::Repair);
            assert_eq!(repeated.disposition, DecisionDisposition::NoProposal);
            assert_eq!(repeated.reason, DecisionReason::PriorRejectionUnchanged);
            assert!(repeated.causes.is_empty());
            assert_eq!(scene.developer_snapshot(), before);
        }
        assert_eq!((scene.output(0), scene.output(1), scene.output(2)), outputs);
        act(&mut scene, &[3]);
        assert_eq!(scene.developer_snapshot(), before);

        act(&mut scene, &[6, 6]);
        let repair = scene.explain(RepairAction::Repair);
        assert_eq!(repair.disposition, DecisionDisposition::ReadyToExecute);
        assert_eq!(
            repair.causes.iter().map(|e| e.fact).collect::<Vec<_>>(),
            vec![Fact::Diagnosis, Fact::CableReceipt]
        );
        act(&mut scene, &[3]);
        let repaired = scene.developer_snapshot().actions.pop().unwrap();
        assert_eq!(repaired.status, ActualStatus::Confirmed);
        assert_eq!(repaired.causes, repair.causes);
        assert_eq!(repaired.confirmed.unwrap().fact, Fact::Ready);
        assert_eq!(
            scene.explain(RepairAction::Repair).reason,
            DecisionReason::AlreadyRepaired
        );
        assert_eq!(
            scene.explain(RepairAction::Collect).reason,
            DecisionReason::ReadyNotArrived
        );
        let journal = scene.output(1);
        act(&mut scene, &[6, 6]);
        assert_eq!(scene.output(1), journal);
        let collection = scene.explain(RepairAction::Collect);
        assert_eq!(collection.disposition, DecisionDisposition::ReadyToExecute);
        assert_eq!(collection.causes[0].fact, Fact::Ready);
        act(&mut scene, &[4]);
        let collected = scene.developer_snapshot().actions.pop().unwrap();
        assert_eq!(collected.status, ActualStatus::Confirmed);
        assert_eq!(collected.causes, collection.causes);
        assert_eq!(collected.confirmed.unwrap().fact, Fact::Collected);
        assert_eq!(
            scene.explain(RepairAction::Collect).reason,
            DecisionReason::AlreadyCollected
        );
        assert_eq!(scene.developer_snapshot().actions[1], rejected);
        act(&mut scene, &[0]);
        assert!(scene.developer_snapshot().actions.is_empty());
    }

    #[test]
    fn actual_status_projection_uses_real_core_lifecycle() {
        let mut core = Core::new(0);
        let action = core
            .evaluate(TECHNICIAN, |_| {
                Some(Proposal {
                    request: "synthetic host test".into(),
                    causes: vec![],
                    rule: "execute then confirm".into(),
                })
            })
            .unwrap()
            .unwrap();
        assert_eq!(
            ActualStatus::from_core(&core.action(action).unwrap().status),
            ActualStatus::Proposed
        );
        core.accept(action).unwrap();
        assert_eq!(
            ActualStatus::from_core(&core.action(action).unwrap().status),
            ActualStatus::AcceptedUnconfirmed
        );
        assert!(core.perspective(TECHNICIAN).records().is_empty());
        core.confirm(action, DIAGNOSIS_CODE.into()).unwrap();
        assert_eq!(
            ActualStatus::from_core(&core.action(action).unwrap().status),
            ActualStatus::Confirmed
        );
        assert_eq!(core.perspective(TECHNICIAN).records().len(), 1);
    }

    #[test]
    fn developer_state_survives_trace_truncation_including_zero() {
        let mut expected = None;
        for capacity in [0, 1, 8, 128] {
            let configuration = config::validate(config::Input {
                trace_capacity: capacity,
                ..config::DEFAULT_INPUT
            })
            .unwrap();
            let mut scene = Repair::with_config(configuration);
            act(&mut scene, &[1, 6, 6, 2, 3, 6, 6, 3, 6, 6, 4]);
            let snapshot = scene.developer_snapshot();
            assert!(snapshot.trace.len() <= capacity as usize);
            if capacity == 0 {
                assert!(snapshot.trace.is_empty());
            }
            if capacity <= 8 {
                assert!(snapshot.dropped_trace > 0);
            }
            assert_eq!(snapshot.actions.len(), 4);
            assert_eq!(snapshot.actions[1].status, ActualStatus::HostRejected);
            assert_eq!(snapshot.actions[3].status, ActualStatus::Confirmed);
            let projection = (
                snapshot.known,
                snapshot.actions,
                snapshot.repaired,
                snapshot.collected,
                scene.output(0),
                scene.output(1),
            );
            if let Some(expected) = &expected {
                assert_eq!(&projection, expected);
            } else {
                expected = Some(projection);
            }
        }
    }

    #[test]
    fn configured_delays_and_trace_limits_preserve_history_knowledge_and_reset() {
        let mut expected_projection = None;
        for trace_capacity in [0, 8] {
            let configuration = config::validate(config::Input {
                trace_capacity,
                report_to_intake_ticks: 3,
                report_to_technician_ticks: 1,
                technician_to_intake_ticks: 2,
                technician_to_collection_ticks: 4,
                cable_receipt_at_tick: 3,
                ..config::DEFAULT_INPUT
            })
            .unwrap();
            let mut scene = Repair::with_config(configuration);
            let replay = |scene: &mut Repair| {
                act(scene, &[1]);
                let original = scene.core.record(scene.report.unwrap()).unwrap().clone();
                act(scene, &[6, 2, 3]);
                let rejected = scene.rejected_repair.unwrap();
                assert!(account(&scene.core.perspective(INTAKE), DONOR, REPORT_CODE).is_none());
                assert!(cable(&scene.core.perspective(TECHNICIAN)).is_none());
                act(scene, &[6]);
                let journal = scene.journal.clone();
                act(scene, &[6]);
                assert_eq!(scene.journal, journal);
                assert!(account(&scene.core.perspective(INTAKE), DONOR, REPORT_CODE).is_some());
                assert!(
                    outcome(&scene.core.perspective(INTAKE), TECHNICIAN, DIAGNOSIS_CODE).is_some()
                );
                assert!(cable(&scene.core.perspective(TECHNICIAN)).is_some());
                act(scene, &[3, 6, 5]);
                assert!(scene.dialogue.contains("no confirmed repair receipt"));
                act(scene, &[6, 5, 4]);
                assert!(scene.dialogue.contains("no confirmed readiness"));
                let ready =
                    outcome(&scene.core.perspective(INTAKE), TECHNICIAN, READY_CODE).unwrap();
                assert!(
                    scene
                        .core
                        .perspective(COLLECTION)
                        .records()
                        .iter()
                        .all(|r| r.id != ready)
                );
                act(scene, &[6, 6, 4]);
                assert!(scene.collected);
                assert_eq!(scene.core.record(original.id), Some(&original));
                assert!(
                    matches!(&scene.core.action(rejected).unwrap().status, Status::Rejected(reason) if reason == REJECTION)
                );
                assert!(scene.journal.iter().any(|entry| entry == REJECTION));
                (scene.output(0), scene.output(1), scene.output(2))
            };
            let expected = replay(&mut scene);
            assert!(scene.core.trace().len() <= trace_capacity as usize);
            assert!(scene.core.dropped_trace() > 0);
            assert_eq!(scene.act(0), 0);
            assert_eq!(scene.configuration(), configuration);
            assert_eq!(scene.time(), 0);
            assert!(scene.report.is_none() && scene.diagnosis.is_none());
            assert!(scene.rejected_repair.is_none() && scene.journal.is_empty());
            assert!(!scene.repaired && !scene.collected);
            assert_eq!(replay(&mut scene), expected);
            let projection = (scene.output(0), scene.output(1));
            if let Some(expected_projection) = &expected_projection {
                assert_eq!(&projection, expected_projection);
            } else {
                expected_projection = Some(projection);
            }
        }
    }

    #[test]
    fn zero_delays_deliver_at_the_current_tick_and_clock_stays_bounded() {
        let configuration = config::validate(config::Input {
            report_to_intake_ticks: 0,
            report_to_technician_ticks: 0,
            technician_to_intake_ticks: 0,
            technician_to_collection_ticks: 0,
            cable_receipt_at_tick: 0,
            ..config::DEFAULT_INPUT
        })
        .unwrap();
        let mut scene = Repair::with_config(configuration);
        assert!(cable(&scene.core.perspective(TECHNICIAN)).is_some());
        act(&mut scene, &[1, 2, 3, 4]);
        assert!(scene.collected);
        assert_eq!(scene.time(), 0);
        for _ in 0..MAX_TIME {
            act(&mut scene, &[6]);
        }
        let before = (scene.output(0), scene.output(1), scene.output(2));
        assert_eq!(scene.act(6), 2);
        assert_eq!((scene.output(0), scene.output(1), scene.output(2)), before);
        assert_eq!(scene.act(0), 0);
        assert_eq!(scene.configuration(), configuration);
        assert!(cable(&scene.core.perspective(TECHNICIAN)).is_some());
    }

    #[test]
    fn semantic_matching_requires_codes_sources_and_content_variants() {
        fn confirm(core: &mut Core, source: Entity, description: &str) -> RecordId {
            let action = core
                .evaluate(source, |_| {
                    Some(Proposal {
                        request: "synthetic test execution".into(),
                        causes: vec![],
                        rule: "Host test executed this synthetic action".into(),
                    })
                })
                .unwrap()
                .unwrap();
            core.accept(action).unwrap();
            core.confirm(action, description.into()).unwrap()
        }

        let mut core = Core::new(0);
        let basis = core.observe(DONOR, REPORT_CODE.into());
        core.account(DONOR, basis, REPORT.into()).unwrap();
        let basis_from_stock = core.observe(STOCK, "test basis".into());
        let wrong_source = core
            .account(STOCK, basis_from_stock, REPORT_CODE.into())
            .unwrap();
        core.schedule(STOCK, DONOR, wrong_source, 0).unwrap();
        core.advance_to(0).unwrap();
        assert!(account(&core.perspective(DONOR), DONOR, REPORT_CODE).is_none());
        assert!(inspection_policy(core.perspective(DONOR)).is_none());
        let report = core.account(DONOR, basis, REPORT_CODE.into()).unwrap();
        assert_eq!(
            account(&core.perspective(DONOR), DONOR, REPORT_CODE),
            Some(report)
        );
        assert!(inspection_policy(core.perspective(DONOR)).is_some());

        for (source, code, prose) in [
            (TECHNICIAN, DIAGNOSIS_CODE, DIAGNOSIS),
            (TECHNICIAN, READY_CODE, READY),
            (COLLECTION, COLLECTED_CODE, COLLECTED),
        ] {
            let mut core = Core::new(0);
            confirm(&mut core, source, prose);
            core.observe(source, code.into());
            let wrong_source = confirm(&mut core, STOCK, code);
            core.schedule(STOCK, source, wrong_source, 0).unwrap();
            core.advance_to(0).unwrap();
            assert!(outcome(&core.perspective(source), source, code).is_none());
            let record = confirm(&mut core, source, code);
            assert_eq!(
                outcome(&core.perspective(source), source, code),
                Some(record)
            );
        }

        let mut core = Core::new(0);
        let prose = core.observe(STOCK, CABLE.into());
        core.account(STOCK, prose, CABLE_CODE.into()).unwrap();
        let wrong_source = core.observe(DONOR, CABLE_CODE.into());
        core.schedule(DONOR, STOCK, wrong_source, 0).unwrap();
        core.advance_to(0).unwrap();
        assert!(cable(&core.perspective(STOCK)).is_none());
        let receipt = core.observe(STOCK, CABLE_CODE.into());
        assert_eq!(cable(&core.perspective(STOCK)), Some(receipt));
    }

    #[test]
    fn semantic_codes_are_rendered_as_prose_in_player_output() {
        let mut scene = Repair::new();
        act(&mut scene, &[1, 6, 6, 2, 3, 6, 6, 3, 6, 6, 4]);
        let receipt = cable(&scene.core.perspective(STOCK)).unwrap();
        scene.receive(STOCK, receipt);
        for prose in [REPORT, DIAGNOSIS, CABLE, READY, COLLECTED] {
            assert!(scene.journal.iter().any(|entry| entry == prose));
        }
        for field in [0, 1] {
            let output = scene.output(field).unwrap();
            for code in [
                REPORT_CODE,
                DIAGNOSIS_CODE,
                CABLE_CODE,
                READY_CODE,
                COLLECTED_CODE,
            ] {
                assert!(!output.contains(code));
            }
        }
    }

    #[test]
    fn delayed_reports_rejection_and_collection_require_local_encounters() {
        let mut scene = Repair::new();
        assert!(cable(&scene.core.perspective(TECHNICIAN)).is_none());
        act(&mut scene, &[1, 2]);
        assert!(scene.dialogue.contains("no donor report"));
        let original = scene.core.record(scene.report.unwrap()).unwrap().clone();
        let journal = scene.journal.clone();
        act(&mut scene, &[6]);
        assert_eq!(scene.journal, journal);
        assert!(account(&scene.core.perspective(INTAKE), DONOR, REPORT_CODE).is_some());
        assert!(account(&scene.core.perspective(TECHNICIAN), DONOR, REPORT_CODE).is_none());
        assert!(scene.enabled(2));
        act(&mut scene, &[6, 2, 3]);
        let rejected = scene.rejected_repair.unwrap();
        assert!(matches!(
            scene.core.action(rejected).unwrap().status,
            Status::Rejected(_)
        ));
        assert!(!scene.repaired);
        assert!(outcome(&scene.core.perspective(TECHNICIAN), TECHNICIAN, READY_CODE).is_none());
        act(&mut scene, &[5]);
        assert!(scene.dialogue.contains("only the donor's"));
        let journal = scene.journal.clone();
        act(&mut scene, &[6, 6]);
        assert_eq!(scene.journal, journal);
        assert!(cable(&scene.core.perspective(TECHNICIAN)).is_some());
        act(&mut scene, &[3]);
        assert!(scene.repaired);
        act(&mut scene, &[5, 4]);
        assert!(scene.dialogue.contains("no confirmed readiness"));
        act(&mut scene, &[5]);
        assert!(scene.dialogue.contains("no confirmed repair receipt"));
        let journal = scene.journal.clone();
        act(&mut scene, &[6]);
        assert_eq!(scene.journal, journal);
        act(&mut scene, &[5]);
        assert!(
            scene
                .dialogue
                .contains("confirmed repair receipt has arrived")
        );
        act(&mut scene, &[4]);
        assert!(!scene.collected);
        act(&mut scene, &[6, 4]);
        assert!(scene.collected);
        assert_eq!(scene.dialogue, COLLECTED);
        assert!(matches!(
            scene.core.action(rejected).unwrap().status,
            Status::Rejected(_)
        ));
        assert!(scene.journal.iter().any(|entry| entry == REJECTION));
        assert_eq!(scene.core.record(original.id), Some(&original));
    }

    #[test]
    fn correct_causes_survive_earlier_encounters_and_repeated_actions() {
        let mut scene = Repair::new();
        act(&mut scene, &[5, 2, 3, 4, 1, 6, 6, 2, 3]);
        let diagnosis = scene.diagnosis.unwrap();
        let Content::ConfirmedOutcome { action, .. } =
            scene.core.record(diagnosis).unwrap().content
        else {
            panic!("expected confirmed inspection");
        };
        assert_eq!(
            scene.core.action(action).unwrap().proposal.causes,
            vec![scene.report.unwrap()]
        );
        let rejected = scene.rejected_repair.unwrap();
        assert_eq!(
            scene.core.action(rejected).unwrap().proposal.causes,
            vec![diagnosis]
        );
        let trace = scene.core.trace().clone();
        let journal = scene.journal.clone();
        for _ in 0..10 {
            assert_eq!(scene.act(1), 1);
            assert_eq!(scene.act(2), 1);
            assert_eq!(scene.act(3), 0);
        }
        assert_eq!(scene.core.trace(), &trace);
        assert_eq!(scene.journal, journal);
        act(&mut scene, &[6, 6, 3]);
        let ready = outcome(&scene.core.perspective(TECHNICIAN), TECHNICIAN, READY_CODE).unwrap();
        let Content::ConfirmedOutcome { action, .. } = scene.core.record(ready).unwrap().content
        else {
            panic!("expected confirmed repair");
        };
        assert_eq!(
            scene.core.action(action).unwrap().proposal.causes,
            vec![
                diagnosis,
                cable(&scene.core.perspective(TECHNICIAN)).unwrap()
            ]
        );
        act(&mut scene, &[6, 6, 4]);
        let receipt = outcome(
            &scene.core.perspective(COLLECTION),
            COLLECTION,
            COLLECTED_CODE,
        )
        .unwrap();
        let Content::ConfirmedOutcome { action, .. } = scene.core.record(receipt).unwrap().content
        else {
            panic!("expected confirmed collection");
        };
        assert_eq!(
            scene.core.action(action).unwrap().proposal.causes,
            vec![ready]
        );
        let trace = scene.core.trace().clone();
        assert_eq!(scene.act(3), 1);
        assert_eq!(scene.act(4), 1);
        assert_eq!(scene.core.trace(), &trace);
    }

    #[test]
    fn reset_replay_bounds_and_recipient_delivery_times() {
        let replay = |scene: &mut Repair| {
            act(scene, &[1, 6, 6, 2, 3, 6, 6, 3, 5, 6, 5, 4, 6, 4]);
            (scene.journal.clone(), scene.core.trace().clone())
        };
        let mut scene = Repair::new();
        let expected = replay(&mut scene);
        assert_eq!(scene.act(0), 0);
        assert_eq!(scene.time(), 0);
        assert!(!scene.repaired && !scene.collected && scene.journal.is_empty());
        assert_eq!(replay(&mut scene), expected);
        let ready = outcome(&scene.core.perspective(TECHNICIAN), TECHNICIAN, READY_CODE).unwrap();
        let delivered = scene
            .core
            .trace()
            .iter()
            .filter_map(|entry| match &entry.event {
                Event::Delivered(delivery) if delivery.record == ready => {
                    Some((delivery.to, entry.time))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(delivered.contains(&(INTAKE, 5)));
        assert!(delivered.contains(&(COLLECTION, 6)));
        let trace = scene.core.trace().clone();
        for _ in 0..20 {
            act(&mut scene, &[5]);
        }
        // Intake's first ready encounter already happened during replay.
        assert_eq!(scene.core.trace(), &trace);
        while scene.time() < 1000 {
            act(&mut scene, &[6]);
        }
        assert_eq!(scene.act(6), 2);
        assert!(!scene.enabled(6));
        assert_eq!(scene.act(99), 1);
        assert_eq!(scene.output(99), None);
        assert!(scene.core.trace().len() <= 128);
    }
}
