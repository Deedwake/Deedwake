//! Domain behavior is deliberately here, not in the core.
use deedwake_core::{Content, Core, Entity, Perspective, Proposal, RecordId};

const PLAYER: Entity = Entity(1);
const WITNESS: Entity = Entity(2);
const PROVIDER: Entity = Entity(3);
const INTERMEDIARY: Entity = Entity(4);
const REPORT: &str =
    "I saw the player hold the convoy; the reason and later events were out of view.";
const RELEASE: &str = "Convoy released after a strap check";
const VERIFIED: &str = "Verified release receipt";

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

struct Convoy {
    core: Core,
    held: bool,
    access: bool,
    report: RecordId,
    release: Option<RecordId>,
    refused: bool,
    journal: Vec<String>,
}
impl Convoy {
    fn new() -> Self {
        let mut core = Core::new(128);
        let strap = core.observe(PLAYER, "Loose load strap".into());
        let action = core
            .evaluate(PLAYER, |_| {
                Some(Proposal {
                    request: "hold convoy".into(),
                    causes: vec![strap],
                    rule: "Player checks the strap".into(),
                })
            })
            .unwrap()
            .unwrap();
        core.accept(action).unwrap();
        let held = true; // The host executes before confirming, not the policy.
        core.confirm(action, "Convoy held".into()).unwrap();
        let seen = core.observe(
            WITNESS,
            "Player at gate; convoy stopped; reason out of view".into(),
        );
        let report = core.account(WITNESS, seen, REPORT.into()).unwrap();
        core.schedule(WITNESS, PROVIDER, report, 3).unwrap();
        Self {
            core,
            held,
            access: true,
            report,
            release: None,
            refused: false,
            journal: vec!["You held the convoy to check the strap.".into()],
        }
    }
    fn question(&mut self) -> String {
        // A conversation is an explicit delivery, not privileged state in the UI.
        self.core
            .schedule(WITNESS, PLAYER, self.report, self.core.time())
            .unwrap();
        self.core.advance_to(self.core.time()).unwrap();
        let account = self
            .core
            .perspective(PLAYER)
            .records()
            .iter()
            .find(|record| record.id == self.report)
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
        let id = self
            .core
            .evaluate(PLAYER, |view| {
                Some(Proposal {
                    request: "release convoy".into(),
                    causes: vec![view.records()[0].id],
                    rule: "Player completed strap check".into(),
                })
            })
            .unwrap()
            .unwrap();
        self.core.accept(id).unwrap();
        self.held = false;
        let receipt = self.core.confirm(id, RELEASE.into()).unwrap();
        self.core
            .schedule(PLAYER, PROVIDER, receipt, self.core.time() + 3)
            .unwrap();
        self.release = Some(receipt);
        self.journal
            .push("You released the convoy and received a gate receipt.".into());
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
            .schedule(INTERMEDIARY, PROVIDER, verification, self.core.time() + 1)
            .unwrap();
        self.journal
            .push("Sela sent your verified receipt by priority pouch.".into());
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
        self.journal.push(message.clone());
        message
    }
}
fn main() {
    for intervene in [false, true] {
        let mut scene = Convoy::new();
        println!("Mara: {}", scene.question());
        scene.wait_until(3);
        println!("{}", scene.visit());
        scene.release();
        if intervene {
            scene.intervene();
        }
        scene.wait_until(if intervene { 4 } else { 6 });
        println!("Hour {}: {}", scene.core.time(), scene.visit());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_core_drives_both_paths_without_remote_knowledge_leaks() {
        for intervene in [false, true] {
            let mut scene = Convoy::new();
            let testimony = scene.question();
            assert_eq!(scene.question(), testimony);
            let sent = scene.core.record(scene.report).unwrap().clone();
            scene.wait_until(2);
            assert!(scene.access);
            scene.wait_until(3);
            assert!(!scene.access);
            let refusal = scene.visit();
            scene.release();
            scene.release();
            if intervene {
                scene.intervene();
                scene.intervene();
            }
            let journal = scene.journal.clone();
            scene.wait_until(4);
            assert_eq!(scene.access, intervene);
            assert_eq!(scene.journal, journal);
            if !intervene {
                scene.wait_until(6);
            }
            assert!(scene.access);
            assert_eq!(scene.journal, journal);
            assert!(scene.visit().contains("remember refusing"));
            assert!(scene.journal.contains(&refusal));
            assert_eq!(scene.core.record(scene.report), Some(&sent));
        }
    }
    #[test]
    fn restart_and_replay_are_deterministic() {
        let replay = || {
            let mut scene = Convoy::new();
            scene.wait_until(3);
            scene.visit();
            scene.release();
            scene.intervene();
            scene.wait_until(4);
            scene.visit();
            (scene.journal, scene.core.trace().clone())
        };
        assert_eq!(replay(), replay());
        let restarted = Convoy::new();
        assert_eq!(restarted.core.time(), 0);
        assert!(restarted.access && restarted.held);
        assert!(!restarted.refused);
        assert_eq!(restarted.release, None);
    }
}
