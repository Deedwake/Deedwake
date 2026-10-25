//! Convoy example host shared by native and WebAssembly entry points.
use deedwake_core::{Content, Core, Entity, Perspective, Proposal, RecordId};

const PLAYER: Entity = Entity(1);
const WITNESS: Entity = Entity(2);
const PROVIDER: Entity = Entity(3);
const INTERMEDIARY: Entity = Entity(4);
const REPORT: &str =
    "I saw the player hold the convoy; the reason and later events were out of view.";
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

fn service_policy(view: Perspective<'_>) -> Option<Proposal> {
    let report = view.records().iter().find(|record| {
        record.source == WITNESS
            && matches!(&record.content, Content::Account { summary, .. } if summary == REPORT)
    });
    let correction = view.records().iter().find(|record| {
        (record.source == PLAYER && matches!(&record.content, Content::ConfirmedOutcome { description, .. } if description == RELEASE))
            || (record.source == INTERMEDIARY && matches!(&record.content, Content::Account { summary, .. } if summary == VERIFIED))
    });
    let restricted = view
        .records()
        .iter()
        .rev()
        .find_map(|record| {
            if record.source == PROVIDER
                && let Content::ConfirmedOutcome { description, .. } = &record.content
            {
                return match description.as_str() {
                    "Loading restricted" => Some(true),
                    "Loading restored" => Some(false),
                    _ => None,
                };
            }
            None
        })
        .unwrap_or(false);
    match (restricted, report, correction) {
        (false, Some(report), None) => Some(Proposal {
            request: "restrict loading".into(), causes: vec![report.id],
            rule: "An arrived witness hold report restricts service until release confirmation arrives".into(),
        }),
        (true, _, Some(correction)) => Some(Proposal {
            request: "restore loading".into(), causes: vec![correction.id],
            rule: "An arrived gate receipt or intermediary verification permits restoration".into(),
        }),
        _ => None,
    }
}

pub struct Convoy {
    condition: Condition,
    core: Core,
    held: bool,
    access: bool,
    report: Option<RecordId>,
    hold: Option<RecordId>,
    release: Option<RecordId>,
    refused: bool,
    journal: Vec<String>,
    dialogue: String,
}
impl Convoy {
    pub fn new() -> Self {
        Self::with_condition(Condition::Current)
    }
    pub fn with_condition(condition: Condition) -> Self {
        Self { condition, core: Core::new(128), held: false, access: true, report: None, hold: None,
            release: None, refused: false, journal: vec![],
            dialogue: "At the gate, a load strap looks loose. Mara watches across the road. Ivo runs the depot; Sela is a trusted intermediary. Hold the convoy to check the strap, then follow the account to the depot.".into() }
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
            WITNESS,
            "Player at gate; convoy stopped; reason out of view".into(),
        );
        let report = self.core.account(WITNESS, seen, REPORT.into()).unwrap();
        self.core
            .schedule(WITNESS, PROVIDER, report, self.core.time() + 3)
            .unwrap();
        self.report = Some(report);
        self.journal
            .push("You held the convoy to check the strap.".into());
        self.dialogue = "You hold the convoy and secure the strap. Mara sends a fixed, incomplete report in the gate–depot pouch. It takes three fictional hours to arrive.".into();
    }
    fn question(&mut self) -> String {
        let Some(report) = self.report else {
            return "Mara: I have not seen an incident here.".into();
        };
        // A conversation is an explicit delivery, not privileged state in the UI.
        self.core
            .schedule(WITNESS, PLAYER, report, self.core.time())
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
    fn wait_until(&mut self, time: u64) {
        while self.core.time() < time {
            self.core.advance_to(self.core.time() + 1).unwrap();
            if let Some(id) = self.core.evaluate(PROVIDER, service_policy).unwrap() {
                let request = self.core.action(id).unwrap().proposal.request.clone();
                self.core.accept(id).unwrap();
                match request.as_str() {
                    "restrict loading" => {
                        self.access = false;
                        self.core.confirm(id, "Loading restricted".into()).unwrap();
                    }
                    "restore loading" => {
                        self.access = true;
                        self.core.confirm(id, "Loading restored".into()).unwrap();
                    }
                    _ => {
                        self.core
                            .reject(id, "Unsupported host operation".into())
                            .unwrap();
                    }
                }
            }
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
                self.core.time() + u64::from(self.condition.routine_delay()),
            )
            .unwrap();
        self.release = Some(receipt);
        self.journal
            .push("You released the convoy and received a gate receipt.".into());
        self.dialogue = format!(
            "You release the convoy and take the gate receipt. A correction travels to Ivo in three fictional hours; Sela can verify your receipt and send an intermediary pouch in {}. Earlier reports and refusals remain history.",
            self.condition.intermediary_duration()
        );
    }
    fn intervene(&mut self) {
        let Some(receipt) = self.release else {
            return;
        };
        if self
            .core
            .perspective(INTERMEDIARY)
            .records()
            .iter()
            .any(|r| matches!(&r.content, Content::Account { summary, .. } if summary == VERIFIED))
        {
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
                self.core.time() + u64::from(self.condition.intermediary_delay()),
            )
            .unwrap();
        self.journal
            .push("Sela sent your verified receipt by intermediary pouch.".into());
        self.dialogue = format!(
            "Sela: I verified your release receipt. My intermediary pouch takes {} to reach Ivo. It does not repair access before arrival.",
            self.condition.intermediary_duration()
        );
    }
    fn visit(&mut self) -> String {
        let message = if self.access {
            if self.refused { "Ivo: Service is restored. I remember refusing you; that delay still happened." }
            else { "Ivo: Loading service is available." }
        } else {
            self.refused = true;
            "Ivo: Mara's incomplete report arrived. Service is restricted until a gate receipt or Sela's verification arrives."
        }.to_string();
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
            4 => self.release.is_some() && !self.core.perspective(INTERMEDIARY).records().iter().any(|r| matches!(&r.content, Content::Account { summary, .. } if summary == VERIFIED)),
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
        match action {
            0 => *self = Self::with_condition(self.condition),
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
                assert!(scene.visit().contains("remember refusing"));
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
