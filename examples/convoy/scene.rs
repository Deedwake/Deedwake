//! Convoy example host shared by native and WebAssembly entry points.
use deedwake_core::{ActionId, Content, Core, Entity, Perspective, Proposal, RecordId, Status};
use std::collections::BTreeMap;

#[path = "config.rs"]
pub mod config;
use config::{Observer, Reaction, Retention, ValidatedConfig};

const PLAYER: Entity = Entity(1);
const WITNESS: Entity = Entity(2);
const PROVIDER: Entity = Entity(3);
const INTERMEDIARY: Entity = Entity(4);
const REPORT: &str =
    "I saw the player hold the convoy; the reason and later events were out of view.";
const STUDY_OBSERVATION: &str = "Player held the convoy and visibly inspected a loose load strap; private purpose and later release out of view";
const PRECISE_REPORT: &str = "I saw the player hold the convoy and inspect a loose load strap; private purpose and later events were out of view.";
const INCOMPLETE_REPORT: &str = "I saw the player hold the convoy; I do not retain the surrounding context, and private purpose and later events were out of view.";
const RELEASE: &str = "Convoy released after a strap check";
const VERIFIED: &str = "Verified release receipt";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Condition {
    Current = 1,
    Control = 2,
}
impl Condition {
    pub fn from_id(id: u32) -> Option<Self> {
        match id {
            1 => Some(Self::Current),
            2 => Some(Self::Control),
            _ => None,
        }
    }
    pub fn id(self) -> u32 {
        self as u32
    }
    pub fn routine_delay(self) -> u32 {
        3
    }
    pub fn intermediary_delay(self) -> u32 {
        match self {
            Self::Current => 1,
            Self::Control => 3,
        }
    }
    fn intermediary_duration(self) -> &'static str {
        match self {
            Self::Current => "one fictional hour",
            Self::Control => "three fictional hours",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Study {
    PreciseImmediate = 1,
    IncompleteImmediate = 2,
    PreciseScheduled = 3,
    IncompleteScheduled = 4,
}
impl Study {
    pub fn from_id(id: u32) -> Option<Self> {
        match id {
            1 => Some(Self::PreciseImmediate),
            2 => Some(Self::IncompleteImmediate),
            3 => Some(Self::PreciseScheduled),
            4 => Some(Self::IncompleteScheduled),
            _ => None,
        }
    }
    pub fn id(self) -> u32 {
        self as u32
    }
    pub fn retention(self) -> u32 {
        match self {
            Self::PreciseImmediate | Self::PreciseScheduled => 1,
            Self::IncompleteImmediate | Self::IncompleteScheduled => 2,
        }
    }
    pub fn initial_delay(self) -> u32 {
        match self {
            Self::PreciseImmediate | Self::IncompleteImmediate => 0,
            Self::PreciseScheduled | Self::IncompleteScheduled => 3,
        }
    }
    pub fn routine_delay(self) -> u32 {
        self.initial_delay()
    }
    pub fn intermediary_delay(self) -> u32 {
        if self.initial_delay() == 0 { 0 } else { 1 }
    }
    #[cfg(test)]
    fn report(self) -> &'static str {
        if self.retention() == 1 {
            PRECISE_REPORT
        } else {
            INCOMPLETE_REPORT
        }
    }
}
fn delivery_timing(delay: u32) -> String {
    match delay {
        0 => "immediately, without advancing fictional time".into(),
        1 => "in one fictional hour".into(),
        3 => "in three fictional hours".into(),
        _ => format!("in {delay} fictional hours"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ServiceOperation {
    Restrict,
    Restore,
    Warn,
    ResolveWarning,
}
impl ServiceOperation {
    fn request(self) -> &'static str {
        match self {
            Self::Restrict => "restrict loading",
            Self::Restore => "restore loading",
            Self::Warn => "warn and continue loading",
            Self::ResolveWarning => "resolve loading warning",
        }
    }
    fn description(self) -> &'static str {
        match self {
            Self::Restrict => "Loading restricted",
            Self::Restore => "Loading restored",
            Self::Warn => "Loading warning issued; service remains available",
            Self::ResolveWarning => "Loading warning resolved; service remains available",
        }
    }
}

// Domain meaning belongs to host-created identities, independently of display prose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fact {
    Report {
        observer: Entity,
        retention: Retention,
    },
    Release,
    Verification,
    Service {
        operation: ServiceOperation,
        action: ActionId,
    },
}
fn received_fact(facts: &BTreeMap<RecordId, Fact>, record: &deedwake_core::Record) -> Option<Fact> {
    let fact = *facts.get(&record.id)?;
    match (fact, record.source, &record.content) {
        (Fact::Report { observer, .. }, source, Content::Account { .. }) if observer == source => {
            Some(fact)
        }
        (Fact::Release, PLAYER, Content::ConfirmedOutcome { .. }) => Some(fact),
        (Fact::Verification, INTERMEDIARY, Content::Account { .. }) => Some(fact),
        (
            Fact::Service { action, .. },
            PROVIDER,
            Content::ConfirmedOutcome {
                action: confirmed, ..
            },
        ) if action == *confirmed => Some(fact),
        _ => None,
    }
}
fn service_policy(
    view: &Perspective<'_>,
    facts: &BTreeMap<RecordId, Fact>,
    reaction: Reaction,
) -> Option<(ServiceOperation, Proposal)> {
    let report = view.records().iter().find(|record| {
        matches!(
            received_fact(facts, record),
            Some(Fact::Report {
                retention: Retention::Incomplete,
                ..
            })
        )
    });
    let correction = view.records().iter().find(|record| {
        matches!(
            received_fact(facts, record),
            Some(Fact::Release | Fact::Verification)
        )
    });
    let current =
        view.records()
            .iter()
            .rev()
            .find_map(|record| match received_fact(facts, record) {
                Some(Fact::Service { operation, .. }) => Some(operation),
                _ => None,
            });
    let (operation, cause, rule) = match (current, report, correction) {
        (None, Some(report), None) => match reaction {
            Reaction::RestrictUntilCorrection => (
                ServiceOperation::Restrict,
                report.id,
                "An arrived witness hold report restricts service until release confirmation arrives",
            ),
            Reaction::WarnAndContinue => (
                ServiceOperation::Warn,
                report.id,
                "An arrived incomplete witness account warrants a warning while loading remains available",
            ),
        },
        (Some(ServiceOperation::Restrict), _, Some(correction)) => (
            ServiceOperation::Restore,
            correction.id,
            "An arrived gate receipt or intermediary verification permits restoration",
        ),
        (Some(ServiceOperation::Warn), _, Some(correction)) => (
            ServiceOperation::ResolveWarning,
            correction.id,
            "An arrived gate receipt or intermediary verification resolves the warning while loading remains available",
        ),
        _ => return None,
    };
    Some((
        operation,
        Proposal {
            request: operation.request().into(),
            causes: vec![cause],
            rule: rule.into(),
        },
    ))
}

pub struct Convoy {
    condition: Condition,
    study: Option<Study>,
    config: Option<ValidatedConfig>,
    facts: BTreeMap<RecordId, Fact>,
    verification: Option<RecordId>,
    core: Core,
    held: bool,
    access: bool,
    report: Option<RecordId>,
    hold: Option<RecordId>,
    release: Option<RecordId>,
    service_decision: Option<ActionId>,
    refused: bool,
    journal: Vec<String>,
    dialogue: String,
}
impl Convoy {
    pub fn new() -> Self {
        Self::with_condition(Condition::Current)
    }
    pub fn with_condition(condition: Condition) -> Self {
        Self { condition, study: None, config: None, facts: BTreeMap::new(), verification: None, core: Core::new(128), held: false, access: true, report: None, hold: None,
            release: None, service_decision: None, refused: false, journal: vec![],
            dialogue: "At the gate, a load strap looks loose. Mara watches across the road. Ivo runs the depot; Sela is a trusted intermediary. Hold the convoy to check the strap, then follow the account to the depot.".into() }
    }
    pub fn with_study(study: Study) -> Self {
        let mut scene = Self::new();
        scene.study = Some(study);
        scene.dialogue = "At the gate, a load strap looks loose. Mara watches across the road. Ivo runs the depot; Sela is a trusted intermediary. Hold the convoy and inspect the strap, then follow the account to the depot.".into();
        scene
    }
    pub fn with_config(config: ValidatedConfig) -> Self {
        let mut scene = Self::new();
        scene.core = Core::new(config.trace_capacity());
        scene.config = Some(config);
        scene.dialogue = format!(
            "At the gate, a load strap looks loose. {} watches across the road. Ivo runs the depot; Sela is a trusted intermediary. Hold the convoy and inspect the strap, then follow the account to the depot.",
            scene.observer_name()
        );
        scene
    }
    pub fn config(&self) -> Option<&ValidatedConfig> {
        self.config.as_ref()
    }
    fn observer(&self) -> Entity {
        match self.config.as_ref().map(ValidatedConfig::observer) {
            Some(Observer::Sela) => INTERMEDIARY,
            _ => WITNESS,
        }
    }
    fn observer_name(&self) -> &'static str {
        if self.observer() == INTERMEDIARY {
            "Sela"
        } else {
            "Mara"
        }
    }
    fn retention(&self) -> Retention {
        self.config.as_ref().map_or_else(
            || {
                if self.study.is_some_and(|study| study.retention() == 1) {
                    Retention::Precise
                } else {
                    Retention::Incomplete
                }
            },
            ValidatedConfig::retention,
        )
    }
    fn sent_report(&self) -> &'static str {
        if self.config.is_none() && self.study.is_none() {
            REPORT
        } else if self.retention() == Retention::Precise {
            PRECISE_REPORT
        } else {
            INCOMPLETE_REPORT
        }
    }
    fn contextual(&self) -> bool {
        self.study.is_some() || self.config.is_some()
    }
    pub fn study(&self) -> Option<Study> {
        self.study
    }
    fn initial_delay(&self) -> u32 {
        self.config.as_ref().map_or_else(
            || self.study.map_or(3, Study::initial_delay),
            ValidatedConfig::initial_delivery_ticks,
        )
    }
    fn routine_delay(&self) -> u32 {
        self.config.as_ref().map_or_else(
            || {
                self.study
                    .map_or(self.condition.routine_delay(), Study::routine_delay)
            },
            ValidatedConfig::routine_delivery_ticks,
        )
    }
    fn intermediary_delay(&self) -> u32 {
        self.config.as_ref().map_or_else(
            || {
                self.study.map_or(
                    self.condition.intermediary_delay(),
                    Study::intermediary_delay,
                )
            },
            ValidatedConfig::intermediary_delivery_ticks,
        )
    }
    pub fn condition(&self) -> Condition {
        self.condition
    }
    fn delay(&mut self) {
        if self.report.is_some() {
            return;
        }
        let strap = self.core.observe(PLAYER, "Loose load strap".into());
        let action = self
            .core
            .evaluate(PLAYER, |_| {
                Some(Proposal {
                    request: "hold convoy".into(),
                    causes: vec![strap],
                    rule: "Player checks the strap".into(),
                })
            })
            .unwrap()
            .unwrap();
        self.core.accept(action).unwrap();
        self.held = true;
        self.hold = Some(self.core.confirm(action, "Convoy held".into()).unwrap());
        let seen = self.core.observe(
            self.observer(),
            if self.contextual() {
                STUDY_OBSERVATION
            } else {
                "Player at gate; convoy stopped; reason out of view"
            }
            .into(),
        );
        let report = self
            .core
            .account(self.observer(), seen, self.sent_report().into())
            .unwrap();
        self.core
            .schedule(
                self.observer(),
                PROVIDER,
                report,
                self.core.time() + u64::from(self.initial_delay()),
            )
            .unwrap();
        self.report = Some(report);
        self.facts.insert(
            report,
            Fact::Report {
                observer: self.observer(),
                retention: self.retention(),
            },
        );
        self.journal
            .push("You held the convoy to check the strap.".into());
        self.dialogue = if self.contextual() {
            format!(
                "You hold the convoy and inspect the loose strap. {} sends her fixed account to Ivo; it arrives {}. Ask {} to learn what she sent.",
                self.observer_name(),
                delivery_timing(self.initial_delay()),
                self.observer_name()
            )
        } else {
            "You hold the convoy and secure the strap. Mara sends a fixed, incomplete report in the gate–depot pouch. It takes three fictional hours to arrive.".into()
        };
    }
    fn question(&mut self) -> String {
        let Some(report) = self.report else {
            return format!(
                "{}: I have not seen an incident here.",
                self.observer_name()
            );
        };
        // A conversation is an explicit delivery, not privileged state in the UI.
        self.core
            .schedule(self.observer(), PLAYER, report, self.core.time())
            .unwrap();
        self.core.advance_to(self.core.time()).unwrap();
        let account = self
            .core
            .perspective(PLAYER)
            .records()
            .iter()
            .find(|record| record.id == report)
            .unwrap()
            .content
            .clone();
        let Content::Account { summary, .. } = account else {
            unreachable!()
        };
        if !self.journal.contains(&summary) {
            self.journal.push(summary.clone());
        }
        summary
    }
    fn settle(&mut self, time: u64) {
        self.core.advance_to(time).unwrap();
        let reaction = self
            .config
            .as_ref()
            .map_or(Reaction::RestrictUntilCorrection, ValidatedConfig::reaction);
        let decision = service_policy(&self.core.perspective(PROVIDER), &self.facts, reaction);
        if let Some((operation, proposal)) = decision {
            let id = self
                .core
                .evaluate(PROVIDER, |_| Some(proposal))
                .unwrap()
                .unwrap();
            self.core.accept(id).unwrap();
            self.access = operation != ServiceOperation::Restrict;
            let outcome = self
                .core
                .confirm(id, operation.description().into())
                .unwrap();
            self.facts.insert(
                outcome,
                Fact::Service {
                    operation,
                    action: id,
                },
            );
            self.service_decision = Some(id);
        }
    }
    fn wait_until(&mut self, time: u64) {
        while self.core.time() < time {
            self.settle(self.core.time() + 1);
        }
    }
    fn release(&mut self) {
        if !self.held {
            return;
        }
        let hold = self.hold.expect("held convoy has confirmed hold record");
        let id = self
            .core
            .evaluate(PLAYER, |_| {
                Some(Proposal {
                    request: "release convoy".into(),
                    causes: vec![hold],
                    rule: "Player completed strap check".into(),
                })
            })
            .unwrap()
            .unwrap();
        self.core.accept(id).unwrap();
        self.held = false;
        let receipt = self.core.confirm(id, RELEASE.into()).unwrap();
        self.core
            .schedule(
                PLAYER,
                PROVIDER,
                receipt,
                self.core.time() + u64::from(self.routine_delay()),
            )
            .unwrap();
        self.release = Some(receipt);
        self.facts.insert(receipt, Fact::Release);
        self.journal
            .push("You released the convoy and received a gate receipt.".into());
        if self.contextual() {
            self.dialogue = format!(
                "You release the convoy and take the gate receipt. A correction reaches Ivo {}; Sela can verify your receipt and send an intermediary account {}. {}",
                delivery_timing(self.routine_delay()),
                delivery_timing(self.intermediary_delay()),
                if self.config.is_some() {
                    "Earlier reports and service decisions remain history."
                } else {
                    "Earlier reports and refusals remain history."
                }
            );
        } else {
            self.dialogue = format!(
                "You release the convoy and take the gate receipt. A correction travels to Ivo in three fictional hours; Sela can verify your receipt and send an intermediary pouch in {}. Earlier reports and refusals remain history.",
                self.condition.intermediary_duration()
            );
        }
    }
    fn intervene(&mut self) {
        let Some(receipt) = self.release else {
            return;
        };
        if self.verification.is_some() {
            return;
        }
        self.core
            .schedule(PLAYER, INTERMEDIARY, receipt, self.core.time())
            .unwrap();
        self.core.advance_to(self.core.time()).unwrap();
        let verification = self
            .core
            .account(INTERMEDIARY, receipt, VERIFIED.into())
            .unwrap();
        self.core
            .schedule(
                INTERMEDIARY,
                PROVIDER,
                verification,
                self.core.time() + u64::from(self.intermediary_delay()),
            )
            .unwrap();
        self.verification = Some(verification);
        self.facts.insert(verification, Fact::Verification);
        self.journal
            .push("Sela sent your verified receipt by intermediary pouch.".into());
        if self.contextual() {
            self.dialogue = format!(
                "Sela: I verified your release receipt. My intermediary account reaches Ivo {}. Ivo acts only on evidence that has arrived.",
                delivery_timing(self.intermediary_delay())
            );
        } else {
            self.dialogue = format!(
                "Sela: I verified your release receipt. My intermediary pouch takes {} to reach Ivo. It does not repair access before arrival.",
                self.condition.intermediary_duration()
            );
        }
    }
    fn service_explanation(&self) -> Option<String> {
        let view = self.core.perspective(PROVIDER);
        let Some(id) = self.service_decision else {
            let report = view.records().iter().find(|record| {
                matches!(
                    received_fact(&self.facts, record),
                    Some(Fact::Report {
                        retention: Retention::Precise,
                        ..
                    })
                )
            })?;
            let observer = if report.source == INTERMEDIARY {
                "Sela"
            } else {
                "Mara"
            };
            return Some(format!(
                "Ivo: {observer}'s account arrived. She saw you hold the convoy and inspect a loose strap; your private purpose and later events were out of her view. That inspection context keeps loading service available."
            ));
        };
        let action = self.core.action(id)?;
        let Status::Confirmed(outcome_id) = action.status else {
            return None;
        };
        if action.actor != PROVIDER {
            return None;
        }
        let outcome = view
            .records()
            .iter()
            .find(|record| record.id == outcome_id)?;
        let Some(Fact::Service {
            operation,
            action: confirmed,
        }) = received_fact(&self.facts, outcome)
        else {
            return None;
        };
        if confirmed != id {
            return None;
        }
        let [cause_id] = action.proposal.causes.as_slice() else {
            return None;
        };
        let cause = view
            .records()
            .iter()
            .find(|record| record.id == *cause_id)?;
        let (prefix, verb, evidence) = match (operation, received_fact(&self.facts, cause)) {
            (
                ServiceOperation::Restrict,
                Some(Fact::Report {
                    observer,
                    retention: Retention::Incomplete,
                }),
            ) => {
                let name = if observer == INTERMEDIARY {
                    "Sela"
                } else {
                    "Mara"
                };
                (
                    format!(
                        "Ivo: {name}'s incomplete report arrived. Service is restricted until a gate receipt or Sela's verification arrives."
                    ),
                    "restricted",
                    format!("{name}'s incomplete account"),
                )
            }
            (
                ServiceOperation::Warn,
                Some(Fact::Report {
                    observer,
                    retention: Retention::Incomplete,
                }),
            ) => {
                let name = if observer == INTERMEDIARY {
                    "Sela"
                } else {
                    "Mara"
                };
                (
                    format!(
                        "Ivo: {name}'s incomplete report arrived. I issued a warning; loading service remains available. The warning awaits a gate receipt or Sela's verification."
                    ),
                    "issued a warning about",
                    format!("{name}'s incomplete account"),
                )
            }
            (
                ServiceOperation::Restore | ServiceOperation::ResolveWarning,
                Some(correction @ (Fact::Release | Fact::Verification)),
            ) => {
                let evidence = if correction == Fact::Release {
                    "your gate release receipt"
                } else {
                    "Sela's verified release receipt"
                };
                if operation == ServiceOperation::Restore {
                    (
                        "Ivo: Service is restored.".into(),
                        "restored",
                        evidence.into(),
                    )
                } else {
                    ("Ivo: The warning is resolved; loading service remains available. The earlier warning remains history.".into(), "resolved the warning about", evidence.into())
                }
            }
            _ => return None,
        };
        let memory = if operation == ServiceOperation::Restore && self.refused {
            if self.contextual() {
                " I remember refusing you; that refusal remains history."
            } else {
                " I remember refusing you; that delay still happened."
            }
        } else {
            ""
        };
        Some(format!(
            "{prefix}{memory} At hour {}, I {verb} loading based on {evidence}.",
            outcome.time
        ))
    }
    fn visit(&mut self) -> String {
        let message = self.service_explanation().unwrap_or_else(|| {
            if self.access {
                "Ivo: Loading service is available."
            } else {
                "Ivo: Service is restricted until a gate receipt or Sela's verification arrives."
            }
            .into()
        });
        if !self.access {
            self.refused = true;
        }
        self.core.observe(PLAYER, message.clone());
        if !self.journal.contains(&message) {
            self.journal.push(message.clone());
        }
        message
    }
}
impl Default for Convoy {
    fn default() -> Self {
        Self::new()
    }
}
impl Convoy {
    pub fn time(&self) -> u32 {
        self.core.time() as u32
    }
    pub fn enabled(&self, action: u32) -> bool {
        match action {
            0 | 2 | 5 => true,
            1 => self.report.is_none(),
            3 => self.held,
            4 => self.release.is_some() && self.verification.is_none(),
            6 => self.time() < 1000,
            _ => false,
        }
    }
    pub fn act(&mut self, action: u32) -> u32 {
        if action == 6 && self.time() == 1000 {
            return 2;
        }
        if !self.enabled(action) {
            return 1;
        }
        if self.contextual() {
            self.settle(self.core.time());
        }
        match action {
            0 => {
                *self = if let Some(config) = &self.config {
                    Self::with_config(config.clone())
                } else {
                    self.study
                        .map_or_else(|| Self::with_condition(self.condition), Self::with_study)
                }
            }
            1 => self.delay(),
            2 => self.dialogue = self.question(),
            3 => self.release(),
            4 => self.intervene(),
            5 => self.dialogue = self.visit(),
            6 => {
                self.wait_until(self.core.time() + 1);
                self.dialogue =
                    "One fictional hour passes. Return to Ivo to learn his current response."
                        .into();
            }
            _ => unreachable!(),
        }
        if self.contextual() {
            self.settle(self.core.time());
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
#[cfg(test)]
mod tests {
    use super::*;

    const STUDIES: [Study; 4] = [
        Study::PreciseImmediate,
        Study::IncompleteImmediate,
        Study::PreciseScheduled,
        Study::IncompleteScheduled,
    ];

    fn configured(
        observer: &str,
        retention: &str,
        reaction: &str,
        delays: [u64; 3],
        trace_capacity: u64,
    ) -> Convoy {
        Convoy::with_config(
            config::validate(config::Input {
                observer,
                retention,
                reaction,
                trace_capacity,
                initial_delivery_ticks: delays[0],
                routine_delivery_ticks: delays[1],
                intermediary_delivery_ticks: delays[2],
                ..config::DEFAULT_INPUT
            })
            .unwrap(),
        )
    }
    fn operations(scene: &Convoy) -> Vec<(u64, ServiceOperation)> {
        scene
            .core
            .perspective(PROVIDER)
            .records()
            .iter()
            .filter_map(|record| match received_fact(&scene.facts, record) {
                Some(Fact::Service { operation, .. }) => Some((record.time, operation)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn configured_observers_retain_the_shared_observation_and_fixed_testimony() {
        for observer in ["mara", "sela"] {
            for retention in ["precise", "incomplete"] {
                let mut scene =
                    configured(observer, retention, "warn_and_continue", [3, 3, 1], 128);
                let expected_observer = if observer == "sela" {
                    INTERMEDIARY
                } else {
                    WITNESS
                };
                assert_eq!(scene.observer(), expected_observer);
                assert!(scene.dialogue.contains(if observer == "sela" {
                    "Sela watches"
                } else {
                    "Mara watches"
                }));
                perform(&mut scene, &[1]);
                let sent = scene.core.record(scene.report.unwrap()).unwrap().clone();
                let Content::Account { basis, summary } = &sent.content else {
                    panic!("account")
                };
                let observation = scene.core.record(*basis).unwrap().clone();
                assert_eq!(sent.source, expected_observer);
                assert_eq!(observation.source, expected_observer);
                assert_eq!(
                    observation.content,
                    Content::Observation(STUDY_OBSERVATION.into())
                );
                assert_eq!(
                    summary,
                    if retention == "precise" {
                        PRECISE_REPORT
                    } else {
                        INCOMPLETE_REPORT
                    }
                );
                for entity in [PLAYER, PROVIDER] {
                    assert!(
                        !scene
                            .core
                            .perspective(entity)
                            .records()
                            .iter()
                            .any(|record| record.id == observation.id || record.id == sent.id)
                    );
                }
                perform(&mut scene, &[2, 2]);
                let journal = scene.journal.clone();
                perform(&mut scene, &[2]);
                assert_eq!(scene.journal, journal);
                assert_eq!(&scene.dialogue, summary);
                perform(&mut scene, &[3, 4, 6, 6, 6, 2, 2]);
                assert_eq!(&scene.dialogue, summary);
                assert_eq!(scene.core.record(sent.id), Some(&sent));
                assert_eq!(scene.core.record(observation.id), Some(&observation));
                assert!(
                    !scene
                        .core
                        .perspective(PLAYER)
                        .records()
                        .iter()
                        .any(|record| record.id == observation.id)
                );
                assert!(
                    !scene
                        .core
                        .perspective(PROVIDER)
                        .records()
                        .iter()
                        .any(|record| record.id == observation.id)
                );
                // Sela can receive the receipt as intermediary without rewriting her earlier report.
                assert!(scene.verification.is_some());
            }
        }
    }

    #[test]
    fn configured_reaction_matrix_uses_only_arrived_evidence_and_retains_history() {
        for observer in ["mara", "sela"] {
            for retention in ["precise", "incomplete"] {
                for reaction in ["restrict_until_correction", "warn_and_continue"] {
                    for intermediary in [false, true] {
                        let mut scene = configured(observer, retention, reaction, [3, 3, 1], 128);
                        perform(&mut scene, &[1, 5]);
                        assert_eq!(scene.dialogue, "Ivo: Loading service is available.");
                        let journal = scene.journal.clone();
                        for _ in 0..2 {
                            perform(&mut scene, &[6]);
                            assert!(scene.access);
                            assert!(operations(&scene).is_empty());
                            assert_eq!(scene.journal, journal);
                            assert!(!scene.core.trace().iter().any(|entry| matches!(entry.event, deedwake_core::Event::Proposed(id) if scene.core.action(id).unwrap().actor == PROVIDER)));
                        }
                        perform(&mut scene, &[6]);
                        assert_eq!(scene.journal, journal);
                        let incomplete = retention == "incomplete";
                        let restrict = reaction == "restrict_until_correction";
                        assert_eq!(scene.access, !(incomplete && restrict));
                        assert_eq!(
                            operations(&scene),
                            if incomplete {
                                vec![(
                                    3,
                                    if restrict {
                                        ServiceOperation::Restrict
                                    } else {
                                        ServiceOperation::Warn
                                    },
                                )]
                            } else {
                                vec![]
                            }
                        );
                        perform(&mut scene, &[5]);
                        let encountered = scene.dialogue.clone();
                        assert!(encountered.contains(if observer == "sela" {
                            "Sela's"
                        } else {
                            "Mara's"
                        }));
                        if !incomplete {
                            assert!(
                                encountered
                                    .contains("inspection context keeps loading service available")
                            );
                        } else if !restrict {
                            assert!(
                                encountered.contains("warning; loading service remains available")
                            );
                            assert!(
                                !encountered.contains("restricted")
                                    && !encountered.contains("restored")
                            );
                        }
                        assert_eq!(scene.refused, incomplete && restrict);
                        perform(&mut scene, &[3]);
                        if intermediary {
                            perform(&mut scene, &[4]);
                        }
                        let journal = scene.journal.clone();
                        let arrival = if intermediary { 4 } else { 6 };
                        while scene.time() < arrival {
                            perform(&mut scene, &[6]);
                            assert_eq!(scene.journal, journal);
                            if scene.time() < arrival {
                                assert_eq!(scene.access, !(incomplete && restrict));
                            }
                        }
                        assert!(scene.access);
                        perform(&mut scene, &[5]);
                        let final_response = scene.dialogue.clone();
                        if incomplete {
                            let resolved = if restrict {
                                ServiceOperation::Restore
                            } else {
                                ServiceOperation::ResolveWarning
                            };
                            assert_eq!(
                                operations(&scene),
                                vec![
                                    (
                                        3,
                                        if restrict {
                                            ServiceOperation::Restrict
                                        } else {
                                            ServiceOperation::Warn
                                        }
                                    ),
                                    (u64::from(arrival), resolved)
                                ]
                            );
                            assert!(final_response.contains(if restrict {
                                "Service is restored."
                            } else {
                                "warning is resolved; loading service remains available"
                            }));
                            assert!(scene.journal.contains(&encountered));
                            if !restrict {
                                assert!(
                                    !scene.refused
                                        && !final_response.contains("restored")
                                        && !final_response.contains("refusing")
                                );
                            }
                        } else {
                            assert!(operations(&scene).is_empty());
                            assert_eq!(final_response, encountered);
                        }
                        let history = operations(&scene);
                        perform(&mut scene, &[2, 2, 6, 6, 6, 5]);
                        assert_eq!(operations(&scene), history);
                        assert_eq!(scene.dialogue, final_response);
                        for record in scene.core.perspective(PROVIDER).records() {
                            if let Some(Fact::Service { action, .. }) =
                                received_fact(&scene.facts, record)
                            {
                                let action = scene.core.action(action).unwrap();
                                assert_eq!(action.actor, PROVIDER);
                                assert_eq!(action.status, Status::Confirmed(record.id));
                                assert!(action.proposal.causes.iter().all(|cause| {
                                    scene
                                        .core
                                        .perspective(PROVIDER)
                                        .records()
                                        .iter()
                                        .any(|known| known.id == *cause)
                                }));
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn configured_zero_equal_time_overtaking_and_late_corrections_are_stable() {
        for reaction in ["restrict_until_correction", "warn_and_continue"] {
            let opening = if reaction == "warn_and_continue" {
                ServiceOperation::Warn
            } else {
                ServiceOperation::Restrict
            };
            let resolution = if reaction == "warn_and_continue" {
                ServiceOperation::ResolveWarning
            } else {
                ServiceOperation::Restore
            };
            let mut immediate = configured("sela", "incomplete", reaction, [0, 0, 0], 128);
            perform(&mut immediate, &[1, 5, 3, 4, 5]);
            assert_eq!(immediate.time(), 0);
            assert_eq!(operations(&immediate), vec![(0, opening), (0, resolution)]);
            assert!(immediate.access);
            let response = immediate.dialogue.clone();
            let history = operations(&immediate);
            perform(&mut immediate, &[2, 5, 6, 5]);
            assert_eq!(immediate.dialogue, response);
            assert_eq!(operations(&immediate), history);
            for (delays, commands) in [
                ([3, 3, 1], vec![1, 3, 6, 6, 6, 5]),
                ([3, 3, 1], vec![1, 3, 4, 6, 6, 6, 5]),
                ([1000, 0, 0], vec![1, 3, 4, 6, 5]),
            ] {
                let mut scene = configured("sela", "incomplete", reaction, delays, 128);
                perform(&mut scene, &commands);
                assert!(scene.access && !scene.refused);
                assert!(operations(&scene).is_empty());
            }
            let mut late = configured("mara", "incomplete", reaction, [2, 4, 1], 128);
            perform(&mut late, &[1, 6, 6, 5, 3, 6, 6, 6, 6, 5]);
            let decision = late.service_decision;
            let explanation = late.dialogue.clone();
            let history = operations(&late);
            assert_eq!(history, vec![(2, opening), (6, resolution)]);
            perform(&mut late, &[4, 6, 5]);
            assert_eq!(late.service_decision, decision);
            assert_eq!(late.dialogue, explanation);
            assert_eq!(operations(&late), history);
        }
    }

    #[test]
    fn configured_maximum_delay_reaches_the_clock_boundary_without_early_reaction() {
        for reaction in ["restrict_until_correction", "warn_and_continue"] {
            let mut scene = configured("sela", "incomplete", reaction, [1000, 1000, 1000], 0);
            perform(&mut scene, &[1]);
            let journal = scene.journal.clone();
            for _ in 0..999 {
                perform(&mut scene, &[6]);
            }
            assert!(operations(&scene).is_empty());
            assert_eq!(scene.journal, journal);
            perform(&mut scene, &[6]);
            assert_eq!(scene.time(), 1000);
            assert_eq!(operations(&scene).len(), 1);
            assert_eq!(operations(&scene)[0].0, 1000);
            assert!(!scene.enabled(6));
            let before = (scene.output(0), scene.output(1), scene.output(2));
            assert_eq!(scene.act(6), 2);
            assert_eq!((scene.output(0), scene.output(1), scene.output(2)), before);
            perform(&mut scene, &[3, 4]);
            assert_eq!(operations(&scene).len(), 1);
            assert_eq!(scene.access, reaction == "warn_and_continue");
            assert!(scene.dialogue.contains("1000 fictional hours"));
            assert_eq!(scene.core.trace().len(), 0);
        }
    }

    #[test]
    fn configured_trace_bounds_restart_and_invalid_inputs_preserve_the_config() {
        for observer in ["mara", "sela"] {
            for reaction in ["restrict_until_correction", "warn_and_continue"] {
                let replay = |capacity| {
                    let mut scene =
                        configured(observer, "incomplete", reaction, [1, 3, 1], capacity);
                    perform(&mut scene, &[1, 2, 6, 5, 3, 4, 6, 5]);
                    for _ in 0..80 {
                        perform(&mut scene, &[2, 5]);
                    }
                    assert!(scene.core.dropped_trace() > 0);
                    assert!(scene.core.trace().len() <= capacity as usize);
                    (
                        scene.dialogue.clone(),
                        scene.journal.clone(),
                        operations(&scene),
                    )
                };
                assert_eq!(replay(0), replay(1));
                assert_eq!(replay(1), replay(128));
                let mut scene = configured(observer, "incomplete", reaction, [1, 3, 1], 1);
                let original = scene.config().unwrap().clone();
                perform(&mut scene, &[1, 2, 6, 5]);
                let before = (
                    scene.time(),
                    scene.output(0),
                    scene.output(1),
                    scene.output(2),
                );
                for invalid in [1, 4, 7, u32::MAX] {
                    assert_eq!(scene.act(invalid), 1);
                }
                assert_eq!(
                    (
                        scene.time(),
                        scene.output(0),
                        scene.output(1),
                        scene.output(2)
                    ),
                    before
                );
                perform(&mut scene, &[0]);
                assert_eq!(scene.config(), Some(&original));
                assert_eq!(scene.study(), None);
                let fresh = Convoy::with_config(original);
                assert_same_replay_state(&scene, &fresh);
                assert!(scene.facts.is_empty() && scene.verification.is_none());
                perform(&mut scene, &[1]);
                assert_eq!(scene.core.trace().len(), 1);
            }
        }
    }

    #[test]
    fn configured_semantics_do_not_interpret_display_words_or_hidden_basis() {
        let mut scene = configured("sela", "incomplete", "warn_and_continue", [3, 3, 1], 128);
        perform(&mut scene, &[1]);
        let report = scene.report.unwrap();
        let Content::Account { basis, .. } = scene.core.record(report).unwrap().content else {
            panic!("account")
        };
        // A policy sees only delivered identities; even a classified hidden record is unavailable.
        let view = scene.core.perspective(PROVIDER);
        assert!(service_policy(&view, &scene.facts, Reaction::WarnAndContinue).is_none());
        assert!(!view.records().iter().any(|record| record.id == basis));
        // Matching words and record kind from the right entity are still not a host report identity.
        let seen = scene.core.observe(INTERMEDIARY, "unrelated".into());
        let imitation = scene
            .core
            .account(INTERMEDIARY, seen, INCOMPLETE_REPORT.into())
            .unwrap();
        scene
            .core
            .schedule(INTERMEDIARY, PROVIDER, imitation, 0)
            .unwrap();
        scene.settle(0);
        assert!(operations(&scene).is_empty());
        // Semantic meaning survives a different displayed summary without consulting its basis.
        let reworded = scene
            .core
            .account(INTERMEDIARY, seen, "Independent display wording".into())
            .unwrap();
        scene
            .core
            .schedule(INTERMEDIARY, PROVIDER, reworded, 0)
            .unwrap();
        scene.core.advance_to(0).unwrap();
        let mut facts = scene.facts.clone();
        facts.insert(
            reworded,
            Fact::Report {
                observer: INTERMEDIARY,
                retention: Retention::Incomplete,
            },
        );
        let decision = service_policy(
            &scene.core.perspective(PROVIDER),
            &facts,
            Reaction::WarnAndContinue,
        )
        .unwrap();
        assert_eq!(decision.0, ServiceOperation::Warn);
        assert_eq!(decision.1.causes, vec![reworded]);
        assert!(
            !scene
                .core
                .perspective(PROVIDER)
                .records()
                .iter()
                .any(|record| record.id == seen)
        );
        perform(&mut scene, &[6, 6, 6]);
        let confirmed = scene.service_decision.unwrap();
        let proposal = scene.core.action(confirmed).unwrap().proposal.clone();
        let forged = scene
            .core
            .evaluate(PROVIDER, |_| Some(proposal))
            .unwrap()
            .unwrap();
        scene.core.accept(forged).unwrap();
        scene
            .core
            .confirm(forged, ServiceOperation::Warn.description().into())
            .unwrap();
        scene.service_decision = Some(forged);
        assert_eq!(scene.service_explanation(), None);
        scene.service_decision = Some(confirmed);
        assert!(
            scene
                .service_explanation()
                .unwrap()
                .contains("Sela's incomplete report arrived")
        );
    }

    fn perform(scene: &mut Convoy, actions: &[u32]) {
        for &action in actions {
            assert_eq!(scene.act(action), 0);
        }
    }
    fn decisions(scene: &Convoy) -> Vec<(u64, String)> {
        scene
            .core
            .perspective(PROVIDER)
            .records()
            .iter()
            .filter_map(|record| {
                if record.source == PROVIDER
                    && let Content::ConfirmedOutcome { description, .. } = &record.content
                {
                    Some((record.time, description.clone()))
                } else {
                    None
                }
            })
            .collect()
    }

    #[test]
    fn studies_share_observation_and_keep_the_sent_retention_fixed() {
        for study in STUDIES {
            let mut scene = Convoy::with_study(study);
            perform(&mut scene, &[1]);
            let sent = scene.core.record(scene.report.unwrap()).unwrap().clone();
            let Content::Account { summary, basis } = &sent.content else {
                panic!("account")
            };
            assert_eq!(summary, study.report());
            let observed = scene.core.record(*basis).unwrap().clone();
            assert_eq!(observed.source, WITNESS);
            assert_eq!(
                observed.content,
                Content::Observation(STUDY_OBSERVATION.into())
            );
            assert!(
                !scene
                    .core
                    .perspective(PLAYER)
                    .records()
                    .iter()
                    .any(|record| record.id == sent.id || record.id == observed.id)
            );
            perform(&mut scene, &[2]);
            assert_eq!(scene.dialogue, study.report());
            let journal = scene.journal.clone();
            perform(&mut scene, &[2]);
            assert_eq!(scene.journal, journal);
            assert!(
                !scene
                    .core
                    .perspective(PLAYER)
                    .records()
                    .iter()
                    .any(|record| record.id == observed.id)
            );
            perform(&mut scene, &[3, 4, 6, 6, 6, 2, 2]);
            assert_eq!(scene.dialogue, study.report());
            assert_eq!(scene.core.record(sent.id), Some(&sent));
            assert_eq!(scene.core.record(observed.id), Some(&observed));
            assert!(
                !scene
                    .core
                    .perspective(WITNESS)
                    .records()
                    .iter()
                    .any(|record| Some(record.id) == scene.release)
            );
        }
    }

    #[test]
    fn study_delivery_and_policy_are_matched_without_remote_journal_leaks() {
        for study in STUDIES {
            for intermediary in [false, true] {
                let mut scene = Convoy::with_study(study);
                perform(&mut scene, &[1]);
                let journal = scene.journal.clone();
                if study.initial_delay() == 3 {
                    perform(&mut scene, &[5]);
                    assert_eq!(scene.dialogue, "Ivo: Loading service is available.");
                    let before_arrival = scene.journal.clone();
                    perform(&mut scene, &[6, 6]);
                    assert!(scene.access);
                    assert_eq!(scene.journal, before_arrival);
                    perform(&mut scene, &[6]);
                    assert_eq!(scene.journal, before_arrival);
                } else {
                    assert_eq!(scene.time(), 0);
                    assert_eq!(scene.journal, journal);
                }
                assert_eq!(scene.access, study.retention() == 1);
                perform(&mut scene, &[5]);
                let encountered = scene.dialogue.clone();
                if study.retention() == 1 {
                    assert!(
                        encountered.contains("inspection context keeps loading service available")
                    );
                    assert_eq!(scene.service_decision, None);
                    assert!(!scene.refused);
                } else {
                    assert!(encountered.contains(&format!(
                        "At hour {}, I restricted loading",
                        study.initial_delay()
                    )));
                    assert!(scene.refused);
                }
                let release_hour = scene.time();
                perform(&mut scene, &[3]);
                if intermediary {
                    perform(&mut scene, &[4]);
                }
                let journal = scene.journal.clone();
                let arrives = release_hour
                    + if intermediary {
                        study.intermediary_delay()
                    } else {
                        study.routine_delay()
                    };
                while scene.time() < arrives {
                    perform(&mut scene, &[6]);
                    assert_eq!(scene.journal, journal);
                    if scene.time() < arrives {
                        assert_eq!(scene.access, study.retention() == 1);
                    }
                }
                assert!(scene.access);
                perform(&mut scene, &[5]);
                if study.retention() == 1 {
                    assert_eq!(scene.dialogue, encountered);
                    assert!(decisions(&scene).is_empty());
                } else {
                    assert!(
                        scene
                            .dialogue
                            .contains(&format!("At hour {arrives}, I restored loading"))
                    );
                    assert!(scene.dialogue.contains("I remember refusing you"));
                    assert!(scene.journal.contains(&encountered));
                    assert_eq!(
                        decisions(&scene),
                        vec![
                            (
                                u64::from(study.initial_delay()),
                                "Loading restricted".into()
                            ),
                            (u64::from(arrives), "Loading restored".into())
                        ]
                    );
                }
                perform(&mut scene, &[6, 6, 6]);
                assert!(scene.access);
            }
        }
    }

    #[test]
    fn scheduled_study_overtake_equal_time_and_late_handoff_keep_history() {
        for study in [Study::PreciseScheduled, Study::IncompleteScheduled] {
            // Release and hold accounts arrive together; verification can also overtake the hold.
            for actions in [&[1, 3, 6, 6, 6, 5][..], &[1, 3, 4, 6, 5, 6, 6, 5][..]] {
                let mut scene = Convoy::with_study(study);
                perform(&mut scene, actions);
                assert!(scene.access);
                assert!(!scene.refused);
                assert!(decisions(&scene).is_empty());
            }
            let mut scene = Convoy::with_study(study);
            perform(&mut scene, &[1, 6, 6, 6, 5, 3, 6, 6, 6, 5]);
            let decision = scene.service_decision;
            let explanation = scene.dialogue.clone();
            let history = decisions(&scene);
            perform(&mut scene, &[4, 6, 5]);
            assert_eq!(scene.service_decision, decision);
            assert_eq!(scene.dialogue, explanation);
            assert_eq!(decisions(&scene), history);
        }
    }

    #[test]
    fn study_current_hour_settlement_preserves_both_decisions_and_host_authority() {
        let mut scene = Convoy::with_study(Study::IncompleteImmediate);
        perform(&mut scene, &[1, 5, 3, 4, 5]);
        assert_eq!(scene.time(), 0);
        assert_eq!(
            decisions(&scene),
            vec![
                (0, "Loading restricted".into()),
                (0, "Loading restored".into())
            ]
        );
        for record in scene.core.perspective(PROVIDER).records() {
            if record.source == PROVIDER
                && let Content::ConfirmedOutcome { action, .. } = record.content
            {
                let confirmed = scene.core.action(action).unwrap();
                assert_eq!(confirmed.status, Status::Confirmed(record.id));
                assert_eq!(confirmed.actor, PROVIDER);
                assert!(confirmed.proposal.causes.iter().all(|id| {
                    scene
                        .core
                        .perspective(PROVIDER)
                        .records()
                        .iter()
                        .any(|known| known.id == *id)
                }));
            }
        }
        assert!(scene.dialogue.contains("At hour 0, I restored loading"));
        assert!(scene.dialogue.contains("I remember refusing you"));
    }

    #[test]
    fn study_explanations_and_reset_survive_trace_eviction_and_invalid_actions() {
        for study in STUDIES {
            let replay = |capacity| {
                let mut scene = Convoy::with_study(study);
                scene.core = Core::new(capacity);
                perform(&mut scene, &[1, 2, 6, 6, 6, 5, 3, 4, 6, 6, 6, 5]);
                for _ in 0..80 {
                    perform(&mut scene, &[2, 5]);
                }
                assert!(scene.core.dropped_trace() > 0);
                (
                    scene.dialogue.clone(),
                    scene.journal.clone(),
                    decisions(&scene),
                )
            };
            assert_eq!(replay(0), replay(1));
            let mut scene = Convoy::with_study(study);
            perform(&mut scene, &[1, 2, 6]);
            let before = (
                scene.time(),
                scene.output(0),
                scene.output(1),
                scene.output(2),
            );
            for invalid in [1, 4, 7, u32::MAX] {
                assert_eq!(scene.act(invalid), 1);
            }
            assert_eq!(
                (
                    scene.time(),
                    scene.output(0),
                    scene.output(1),
                    scene.output(2)
                ),
                before
            );
            perform(&mut scene, &[0]);
            assert_eq!(scene.study(), Some(study));
            assert_eq!(scene.time(), 0);
            assert!(scene.access && !scene.refused);
            assert_eq!(scene.service_decision, None);
            for field in 0..3 {
                assert_eq!(scene.output(field), Convoy::with_study(study).output(field));
            }
        }
    }

    fn assert_same_replay_state(replayed: &Convoy, uninterrupted: &Convoy) {
        assert_eq!(
            (
                replayed.condition,
                replayed.held,
                replayed.access,
                replayed.report,
                replayed.hold,
                replayed.release,
                replayed.service_decision,
                replayed.refused,
                &replayed.journal,
                &replayed.dialogue,
            ),
            (
                uninterrupted.condition,
                uninterrupted.held,
                uninterrupted.access,
                uninterrupted.report,
                uninterrupted.hold,
                uninterrupted.release,
                uninterrupted.service_decision,
                uninterrupted.refused,
                &uninterrupted.journal,
                &uninterrupted.dialogue,
            ),
        );
        // Same-build test inspection includes private pending, knowledge, records and actions.
        assert_eq!(
            format!("{:?}", replayed.core),
            format!("{:?}", uninterrupted.core),
        );
        for action in 0..=7 {
            assert_eq!(replayed.enabled(action), uninterrupted.enabled(action));
        }
        for field in 0..3 {
            assert_eq!(replayed.output(field), uninterrupted.output(field));
        }
    }

    #[test]
    fn accepted_prefix_replay_reconstructs_state_and_future_continuation() {
        let cases: &[(&str, &[u32])] = &[
            (
                "routine",
                &[
                    1, 2, 2, 5, 6, 6, 6, 5, 5, 3, 5, 6, 5, 6, 5, 6, 5, 2, 2, 6, 5,
                ],
            ),
            (
                "intermediary",
                &[
                    1, 2, 2, 5, 6, 6, 6, 5, 5, 3, 4, 5, 6, 5, 6, 5, 6, 5, 2, 2, 6, 5,
                ],
            ),
            ("unwitnessed refusal routine", &[1, 6, 6, 6, 3, 6, 6, 6, 5]),
            (
                "unwitnessed refusal intermediary",
                &[1, 6, 6, 6, 3, 4, 6, 6, 6, 5],
            ),
            (
                "simultaneous report and receipt",
                &[5, 1, 2, 2, 3, 6, 6, 6, 5],
            ),
            ("early verification", &[5, 1, 3, 4, 6, 6, 6, 5]),
            ("late handoff", &[1, 6, 6, 6, 5, 3, 6, 6, 6, 5, 4, 6, 6, 5]),
        ];
        for condition in [Condition::Current, Condition::Control] {
            for &(name, commands) in cases {
                for prefix in 0..=commands.len() {
                    let mut uninterrupted = Convoy::with_condition(condition);
                    for &command in &commands[..prefix] {
                        assert_eq!(uninterrupted.act(command), 0);
                    }
                    let mut replayed = Convoy::with_condition(condition);
                    for &command in &commands[..prefix] {
                        assert_eq!(replayed.act(command), 0, "{name}: replay {prefix}");
                    }
                    assert_same_replay_state(&replayed, &uninterrupted);
                    for &command in &commands[prefix..] {
                        assert_eq!(replayed.act(command), 0, "{name}: continuation {prefix}");
                        assert_eq!(uninterrupted.act(command), 0);
                        assert_same_replay_state(&replayed, &uninterrupted);
                    }
                }
            }
        }
    }

    #[test]
    fn accepted_prefix_replay_retains_repeated_commands_after_trace_eviction() {
        for condition in [Condition::Current, Condition::Control] {
            for intermediary in [false, true] {
                let mut commands = vec![1];
                commands.extend([2, 5].repeat(80));
                commands.extend([6, 6, 6, 5, 5, 3]);
                if intermediary {
                    commands.push(4);
                }
                commands.extend([6, 5, 6, 5, 6, 5, 2, 2, 6, 5]);
                for prefix in [1, 81, 161, 164, 167, 168, commands.len()] {
                    let mut uninterrupted = Convoy::with_condition(condition);
                    for &command in &commands[..prefix] {
                        assert_eq!(uninterrupted.act(command), 0);
                    }
                    let mut replayed = Convoy::with_condition(condition);
                    for &command in &commands[..prefix] {
                        assert_eq!(replayed.act(command), 0);
                    }
                    assert_same_replay_state(&replayed, &uninterrupted);
                    if prefix >= 161 {
                        assert!(replayed.core.dropped_trace() > 0);
                    }
                    for &command in &commands[prefix..] {
                        assert_eq!(uninterrupted.act(command), 0);
                        assert_eq!(replayed.act(command), 0);
                        assert_same_replay_state(&replayed, &uninterrupted);
                    }
                    assert!(replayed.refused && replayed.access);
                    assert!(replayed.dialogue.contains("I remember refusing you"));
                    assert_eq!(replayed.core.trace().len(), 128);
                }
            }
        }
    }

    #[test]
    fn release_cites_hold_even_after_an_earlier_depot_visit() {
        let mut scene = Convoy::new();
        for action in [5, 1, 3] {
            assert_eq!(scene.act(action), 0);
        }
        let receipt = scene.core.record(scene.release.unwrap()).unwrap();
        let Content::ConfirmedOutcome { action, .. } = receipt.content else {
            panic!("expected confirmed outcome")
        };
        let causes = &scene.core.action(action).unwrap().proposal.causes;
        assert_eq!(causes, &vec![scene.hold.unwrap()]);
        assert!(
            matches!(&scene.core.record(causes[0]).unwrap().content, Content::ConfirmedOutcome { description, .. } if description == "Convoy held")
        );
    }
    #[test]
    fn native_core_drives_both_paths_without_remote_knowledge_leaks() {
        for condition in [Condition::Current, Condition::Control] {
            for intervene in [false, true] {
                let mut scene = Convoy::with_condition(condition);
                scene.delay();
                let testimony = scene.question();
                assert_eq!(scene.question(), testimony);
                let sent = scene.core.record(scene.report.unwrap()).unwrap().clone();
                scene.wait_until(2);
                assert!(scene.access);
                scene.wait_until(3);
                assert!(!scene.access);
                let refusal = scene.visit();
                assert!(refusal.contains(
                    "At hour 3, I restricted loading based on Mara's incomplete account."
                ));
                assert_eq!(sent.time, 0);
                let refusal_journal = scene.journal.clone();
                assert_eq!(scene.visit(), refusal);
                assert_eq!(scene.time(), 3);
                assert_eq!(scene.journal, refusal_journal);
                let refusal_record = scene.core.perspective(PLAYER).records().iter().find(|record|
                    matches!(&record.content, Content::Observation(message) if message == &refusal)
                ).map(|record| (*record).clone()).unwrap();
                scene.release();
                scene.release();
                if intervene {
                    scene.intervene();
                    scene.intervene();
                }
                assert!(!scene.access);
                let receipt = scene.core.record(scene.release.unwrap()).unwrap().clone();
                assert_eq!(receipt.time, 3);
                let verification = scene.core.perspective(INTERMEDIARY).records().iter().find(|record|
                matches!(&record.content, Content::Account { summary, .. } if summary == VERIFIED)
            ).map(|record| (*record).clone());
                if intervene {
                    assert_eq!(verification.as_ref().unwrap().time, 3);
                }
                let journal = scene.journal.clone();
                let arrives = 3 + if intervene {
                    condition.intermediary_delay()
                } else {
                    condition.routine_delay()
                };
                for time in 4..arrives {
                    scene.wait_until(u64::from(time));
                    assert!(!scene.access);
                    assert_eq!(scene.journal, journal);
                    assert!(
                        !scene
                            .core
                            .perspective(PROVIDER)
                            .records()
                            .iter()
                            .any(|r| r.id == receipt.id
                                || verification.as_ref().is_some_and(|v| r.id == v.id))
                    );
                }
                scene.wait_until(u64::from(arrives));
                assert!(scene.access);
                assert_eq!(scene.journal, journal);
                let deliveries: Vec<_> = scene
                    .core
                    .trace()
                    .iter()
                    .filter_map(|entry| match &entry.event {
                        deedwake_core::Event::Delivered(delivery)
                            if delivery.to == PROVIDER
                                && (delivery.record == receipt.id
                                    || verification
                                        .as_ref()
                                        .is_some_and(|v| delivery.record == v.id)) =>
                        {
                            Some((entry.time, delivery.clone()))
                        }
                        _ => None,
                    })
                    .collect();
                assert!(!deliveries.is_empty());
                assert_eq!(deliveries[0].0, u64::from(arrives));
                if condition == Condition::Control && intervene {
                    assert_eq!(deliveries.len(), 2);
                    assert_eq!(deliveries[0].1.record, receipt.id);
                    assert_eq!(deliveries[1].1.record, verification.as_ref().unwrap().id);
                    assert!(deliveries[0].1.order < deliveries[1].1.order);
                }
                let restored = scene.visit();
                assert!(restored.contains("Service is restored. I remember refusing you"));
                let evidence = if condition == Condition::Current && intervene {
                    "Sela's verified release receipt"
                } else {
                    "your gate release receipt"
                };
                assert!(restored.contains(&format!(
                    "At hour {arrives}, I restored loading based on {evidence}."
                )));
                let decision = scene.service_decision;
                let journal = scene.journal.clone();
                assert_eq!(scene.visit(), restored);
                assert_eq!(scene.time(), arrives);
                assert_eq!(scene.journal, journal);
                scene.wait_until(8);
                assert_eq!(scene.service_decision, decision);
                assert_eq!(scene.visit(), restored);
                assert_eq!(scene.journal, journal);
                assert!(scene.journal.contains(&refusal));
                assert_eq!(scene.core.record(scene.report.unwrap()), Some(&sent));
                assert_eq!(scene.core.record(receipt.id), Some(&receipt));
                assert_eq!(scene.core.record(refusal_record.id), Some(&refusal_record));
                if let Some(verification) = verification {
                    assert_eq!(scene.core.record(verification.id), Some(&verification));
                }
            }
        }
    }
    #[test]
    fn conditions_match_before_correction_and_restart_keeps_metadata() {
        let mut current = Convoy::new();
        let mut control = Convoy::with_condition(Condition::Control);
        for action in [1, 2, 6, 6, 6, 5] {
            assert_eq!(current.act(action), control.act(action));
            for field in 0..3 {
                assert_eq!(current.output(field), control.output(field));
            }
        }
        for condition in [Condition::Current, Condition::Control] {
            assert_eq!(Condition::from_id(condition.id()), Some(condition));
            let mut scene = Convoy::with_condition(condition);
            for action in [1, 6, 0] {
                assert_eq!(scene.act(action), 0);
            }
            assert_eq!(scene.condition(), condition);
            assert_eq!(scene.time(), 0);
            assert_eq!(scene.service_decision, None);
            let fresh = Convoy::with_condition(condition);
            for field in 0..3 {
                assert_eq!(scene.output(field), fresh.output(field));
            }
        }
        for invalid in [0, 3, u32::MAX] {
            assert_eq!(Condition::from_id(invalid), None);
        }
    }
    #[test]
    fn restoration_before_first_visit_does_not_invent_a_refusal() {
        let mut scene = Convoy::new();
        scene.delay();
        scene.wait_until(3);
        scene.release();
        scene.intervene();
        scene.wait_until(4);
        assert_eq!(
            scene.visit(),
            "Ivo: Service is restored. At hour 4, I restored loading based on Sela's verified release receipt."
        );
        assert!(!scene.refused);
        assert!(!scene.journal.iter().any(|entry| entry.contains("refusing")));
    }
    #[test]
    fn simultaneous_report_and_receipt_without_restriction_stays_available() {
        for condition in [Condition::Current, Condition::Control] {
            let mut scene = Convoy::with_condition(condition);
            assert_eq!(scene.visit(), "Ivo: Loading service is available.");
            scene.delay();
            scene.release();
            let journal = scene.journal.clone();
            scene.wait_until(2);
            assert_eq!(scene.journal, journal);
            assert_eq!(scene.service_decision, None);
            assert_eq!(scene.visit(), "Ivo: Loading service is available.");
            scene.wait_until(3);
            assert_eq!(scene.service_decision, None);
            assert_eq!(scene.visit(), "Ivo: Loading service is available.");
            assert!(!scene.refused);
        }
    }
    #[test]
    fn late_handoff_keeps_the_receipt_cited_by_the_confirmed_restoration() {
        let mut scene = Convoy::new();
        scene.delay();
        scene.wait_until(3);
        scene.visit();
        scene.release();
        scene.wait_until(6);
        let restored = scene.visit();
        let decision = scene.service_decision;
        let action = scene.core.action(decision.unwrap()).unwrap().clone();
        assert!(
            restored.contains("At hour 6, I restored loading based on your gate release receipt.")
        );
        scene.intervene();
        let journal = scene.journal.clone();
        scene.wait_until(7);
        assert_eq!(scene.service_decision, decision);
        assert_eq!(scene.core.action(decision.unwrap()), Some(&action));
        assert_eq!(scene.visit(), restored);
        assert_eq!(scene.journal, journal);
    }
    #[test]
    fn explanations_survive_trace_eviction_and_disabled_tracing() {
        let replay = |condition, intervene, capacity| {
            let mut scene = Convoy::with_condition(condition);
            scene.core = Core::new(capacity);
            scene.delay();
            scene.wait_until(3);
            let refusal = scene.visit();
            scene.release();
            if intervene {
                scene.intervene();
            }
            scene.wait_until(6);
            assert!(scene.core.dropped_trace() > 0);
            (refusal, scene.visit(), scene.journal)
        };
        for condition in [Condition::Current, Condition::Control] {
            for intervene in [false, true] {
                assert_eq!(
                    replay(condition, intervene, 0),
                    replay(condition, intervene, 1)
                );
            }
        }
    }
    #[test]
    fn only_confirmed_provider_decisions_with_the_right_cited_evidence_explain_completion() {
        let mut scene = Convoy::new();
        scene.delay();
        scene.wait_until(3);
        let confirmed = scene.service_decision.unwrap();
        let proposal = scene.core.action(confirmed).unwrap().proposal.clone();
        let pending = scene
            .core
            .evaluate(PROVIDER, |_| Some(proposal))
            .unwrap()
            .unwrap();
        scene.service_decision = Some(pending);
        assert_eq!(scene.service_explanation(), None);
        scene.core.accept(pending).unwrap();
        assert_eq!(scene.service_explanation(), None);
        scene.core.reject(pending, "Not executed".into()).unwrap();
        assert_eq!(scene.service_explanation(), None);
        scene.service_decision = Some(confirmed);
        assert!(scene.service_explanation().is_some());

        // Exact words alone cannot turn an observation or a wrong source into evidence.
        for (source, account) in [(WITNESS, false), (PLAYER, true)] {
            let observed = scene.core.observe(source, REPORT.into());
            let cause = if account {
                scene.core.account(source, observed, REPORT.into()).unwrap()
            } else {
                observed
            };
            scene
                .core
                .schedule(source, PROVIDER, cause, scene.core.time())
                .unwrap();
            scene.core.advance_to(scene.core.time()).unwrap();
            let id = scene
                .core
                .evaluate(PROVIDER, |_| {
                    Some(Proposal {
                        request: "restrict loading".into(),
                        causes: vec![cause],
                        rule: "test unsupported evidence".into(),
                    })
                })
                .unwrap()
                .unwrap();
            scene.core.accept(id).unwrap();
            scene.core.confirm(id, "Loading restricted".into()).unwrap();
            scene.service_decision = Some(id);
            assert_eq!(scene.service_explanation(), None);
        }
    }
    #[test]
    fn restart_and_fresh_condition_clear_confirmed_decision_and_refusal() {
        for condition in [Condition::Current, Condition::Control] {
            let mut scene = Convoy::with_condition(condition);
            scene.delay();
            scene.wait_until(3);
            scene.visit();
            assert!(scene.service_decision.is_some() && scene.refused);
            scene.act(0);
            assert_eq!(scene.service_decision, None);
            assert!(!scene.refused);
            assert_eq!(scene.visit(), "Ivo: Loading service is available.");
            let mut fresh = Convoy::with_condition(condition);
            assert_eq!(fresh.service_decision, None);
            assert_eq!(fresh.visit(), "Ivo: Loading service is available.");
        }
    }
    #[test]
    fn restart_and_replay_are_deterministic() {
        let replay = |condition, intervene| {
            let mut scene = Convoy::with_condition(condition);
            scene.delay();
            scene.wait_until(3);
            scene.visit();
            scene.release();
            if intervene {
                scene.intervene();
            }
            scene.wait_until(6);
            scene.visit();
            (scene.journal, scene.core.trace().clone())
        };
        for condition in [Condition::Current, Condition::Control] {
            for intervene in [false, true] {
                assert_eq!(replay(condition, intervene), replay(condition, intervene));
            }
        }
        let restarted = Convoy::new();
        assert_eq!(restarted.core.time(), 0);
        assert!(restarted.access && !restarted.held);
        assert!(!restarted.refused);
        assert_eq!(restarted.release, None);
    }
}
