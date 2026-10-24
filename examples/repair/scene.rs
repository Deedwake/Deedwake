//! Bounded, synthetic repair café host shared by native and WebAssembly callers.
//! No real appliance control, external messages, purchases, or safety advice.
use deedwake_core::{ActionId, Content, Core, Entity, Perspective, Proposal, RecordId};

const PLAYER: Entity = Entity(1);
const DONOR: Entity = Entity(2);
const INTAKE: Entity = Entity(3);
const TECHNICIAN: Entity = Entity(4);
const STOCK: Entity = Entity(5);
const COLLECTION: Entity = Entity(6);
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
            && matches!(&record.content, Content::Observation(text) if text == CABLE))
        .then_some(record.id)
    })
}

fn inspection_policy(view: Perspective<'_>) -> Option<Proposal> {
    if outcome(&view, TECHNICIAN, DIAGNOSIS).is_some() {
        return None;
    }
    Some(Proposal {
        request: "inspect donated lamp".into(),
        causes: vec![account(&view, DONOR, REPORT)?],
        rule: "An arrived donor report permits a synthetic inspection, not a diagnosis by itself"
            .into(),
    })
}

fn repair_policy(view: Perspective<'_>) -> Option<Proposal> {
    if outcome(&view, TECHNICIAN, READY).is_some() {
        return None;
    }
    let mut causes = vec![outcome(&view, TECHNICIAN, DIAGNOSIS)?];
    causes.extend(cable(&view));
    Some(Proposal {
        request: "replace cable and mark ready".into(),
        causes,
        rule: "A cable-only diagnosis proposes repair; the host must check the cable receipt and execute it"
            .into(),
    })
}

fn collection_policy(view: Perspective<'_>) -> Option<Proposal> {
    if outcome(&view, COLLECTION, COLLECTED).is_some() {
        return None;
    }
    Some(Proposal {
        request: "hand over repaired lamp".into(),
        causes: vec![outcome(&view, TECHNICIAN, READY)?],
        rule: "Only an arrived confirmed readiness record permits a collection proposal".into(),
    })
}

pub struct Repair {
    core: Core,
    report: Option<RecordId>,
    diagnosis: Option<RecordId>,
    rejected_repair: Option<ActionId>,
    repaired: bool,
    collected: bool,
    journal: Vec<String>,
    dialogue: String,
}

impl Repair {
    pub fn new() -> Self {
        let mut core = Core::new(128);
        // Created is not received: only the stock source knows this until hour four.
        let receipt = core.observe(STOCK, CABLE.into());
        core.schedule(STOCK, TECHNICIAN, receipt, 4).unwrap();
        Self {
            core,
            report: None,
            diagnosis: None,
            rejected_repair: None,
            repaired: false,
            collected: false,
            journal: vec![],
            dialogue: "A synthetic repair café: submit a donated fictional lamp, visit the technician, and follow the delayed reports to intake and collection. Everything here is a bounded simulation.".into(),
        }
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
        let message = match &known.content {
            Content::Observation(text) => text.clone(),
            Content::Account { summary, .. } => summary.clone(),
            Content::ConfirmedOutcome { description, .. } => description.clone(),
        };
        self.learn(&message);
    }

    fn submit(&mut self) {
        let observed = self.core.observe(
            DONOR,
            "Fictional lamp does not light; cause not inspected".into(),
        );
        let report = self.core.account(DONOR, observed, REPORT.into()).unwrap();
        self.core
            .schedule(DONOR, INTAKE, report, self.core.time() + 1)
            .unwrap();
        self.core
            .schedule(DONOR, TECHNICIAN, report, self.core.time() + 2)
            .unwrap();
        self.report = Some(report);
        self.receive(DONOR, report);
        self.learn("You submit the fictional lamp. The fixed, incomplete donor report reaches intake in one hour and the technician in two hours.");
    }

    fn inspect(&mut self) {
        let Some(action) = self.core.evaluate(TECHNICIAN, inspection_policy).unwrap() else {
            self.learn("Technician: no donor report has reached me, so I cannot propose an inspection yet.");
            return;
        };
        self.core.accept(action).unwrap();
        // The synthetic host performs the inspection before confirming its finding.
        let finding = self.core.confirm(action, DIAGNOSIS.into()).unwrap();
        self.diagnosis = Some(finding);
        self.core
            .schedule(TECHNICIAN, INTAKE, finding, self.core.time() + 1)
            .unwrap();
        self.core
            .schedule(TECHNICIAN, COLLECTION, finding, self.core.time() + 2)
            .unwrap();
        self.receive(TECHNICIAN, finding);
    }

    fn repair(&mut self) {
        let has_cable = cable(&self.core.perspective(TECHNICIAN)).is_some();
        if !has_cable && self.rejected_repair.is_some() {
            self.learn(REJECTION);
            return;
        }
        let Some(action) = self.core.evaluate(TECHNICIAN, repair_policy).unwrap() else {
            self.learn("Technician: I have no confirmed inspection diagnosis on which to request a repair.");
            return;
        };
        if !has_cable {
            self.core.reject(action, REJECTION.into()).unwrap();
            self.rejected_repair = Some(action);
            self.learn(REJECTION);
            return;
        }
        self.core.accept(action).unwrap();
        self.repaired = true;
        let ready = self.core.confirm(action, READY.into()).unwrap();
        self.core
            .schedule(TECHNICIAN, INTAKE, ready, self.core.time() + 1)
            .unwrap();
        self.core
            .schedule(TECHNICIAN, COLLECTION, ready, self.core.time() + 2)
            .unwrap();
        self.receive(TECHNICIAN, ready);
    }

    fn collect(&mut self) {
        let Some(action) = self.core.evaluate(COLLECTION, collection_policy).unwrap() else {
            self.learn("Collection: no confirmed readiness record has reached this desk. I cannot hand over the lamp.");
            return;
        };
        if !self.repaired {
            self.core
                .reject(action, "Synthetic host has not executed the repair".into())
                .unwrap();
            self.learn("Collection: the synthetic host cannot confirm a completed repair.");
            return;
        }
        self.core.accept(action).unwrap();
        self.collected = true;
        let receipt = self.core.confirm(action, COLLECTED.into()).unwrap();
        self.receive(COLLECTION, receipt);
    }

    fn ask_intake(&mut self) {
        let view = self.core.perspective(INTAKE);
        let known = outcome(&view, TECHNICIAN, READY)
            .or_else(|| outcome(&view, TECHNICIAN, DIAGNOSIS))
            .or_else(|| account(&view, DONOR, REPORT));
        let message = if outcome(&view, TECHNICIAN, READY).is_some() {
            "Intake: a confirmed repair receipt has arrived. Our desk knows the lamp is ready; collection has its own delivery delay."
        } else if outcome(&view, TECHNICIAN, DIAGNOSIS).is_some() {
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
            0 => *self = Self::new(),
            1 => self.submit(),
            2 => self.inspect(),
            3 => self.repair(),
            4 => self.collect(),
            5 => self.ask_intake(),
            6 => {
                self.core.advance_to(self.core.time() + 1).unwrap();
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
    fn delayed_reports_rejection_and_collection_require_local_encounters() {
        let mut scene = Repair::new();
        assert!(cable(&scene.core.perspective(TECHNICIAN)).is_none());
        act(&mut scene, &[1, 2]);
        assert!(scene.dialogue.contains("no donor report"));
        let original = scene.core.record(scene.report.unwrap()).unwrap().clone();
        let journal = scene.journal.clone();
        act(&mut scene, &[6]);
        assert_eq!(scene.journal, journal);
        assert!(account(&scene.core.perspective(INTAKE), DONOR, REPORT).is_some());
        assert!(account(&scene.core.perspective(TECHNICIAN), DONOR, REPORT).is_none());
        assert!(scene.enabled(2));
        act(&mut scene, &[6, 2, 3]);
        let rejected = scene.rejected_repair.unwrap();
        assert!(matches!(
            scene.core.action(rejected).unwrap().status,
            Status::Rejected(_)
        ));
        assert!(!scene.repaired);
        assert!(outcome(&scene.core.perspective(TECHNICIAN), TECHNICIAN, READY).is_none());
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
        let ready = outcome(&scene.core.perspective(TECHNICIAN), TECHNICIAN, READY).unwrap();
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
        let receipt = outcome(&scene.core.perspective(COLLECTION), COLLECTION, COLLECTED).unwrap();
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
        let ready = outcome(&scene.core.perspective(TECHNICIAN), TECHNICIAN, READY).unwrap();
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
