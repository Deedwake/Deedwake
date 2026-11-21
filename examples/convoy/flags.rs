//! Independent, closed Convoy authoring baseline: plain flags and three routes.
//! Shares configuration validation and authored display text, never Core state.
#[path = "config.rs"]
pub mod config;
use config::{Confidence, Observer, Reaction, Retention, ValidatedConfig};

const PRECISE_REPORT: &str = "I saw the player hold the convoy and inspect a loose load strap; private purpose and later events were out of view.";
const INCOMPLETE_REPORT: &str = "I saw the player hold the convoy; I do not retain the surrounding context, and private purpose and later events were out of view.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Route {
    Report = 0,
    Gate = 1,
    Sela = 2,
}
#[derive(Clone, Copy, Debug)]
struct Pending {
    route: Route,
    due: u32,
    sequence: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Service {
    Restrict,
    Warn,
    Restore,
    ResolveWarning,
}
#[derive(Clone, Copy, Debug)]
struct Decision {
    service: Service,
    at: u32,
    gate: bool,
}

pub struct FlagsConvoy {
    config: ValidatedConfig,
    time: u32,
    held: bool,
    sent_account: Option<&'static str>,
    released: bool,
    sela_verified: bool,
    received_report: bool,
    received_gate: bool,
    received_sela: bool,
    available: bool,
    warning: bool,
    refused: bool,
    decision: Option<Decision>,
    pending: [Option<Pending>; 3],
    receipts: [Option<(u32, Option<u32>)>; 3],
    next_sequence: u32,
    journal: Vec<String>,
    dialogue: String,
}

fn timing(delay: u32) -> String {
    match delay {
        0 => "immediately, without advancing fictional time".into(),
        1 => "in one fictional hour".into(),
        3 => "in three fictional hours".into(),
        _ => format!("in {delay} fictional hours"),
    }
}

impl FlagsConvoy {
    pub fn with_config(config: ValidatedConfig) -> Self {
        let mut scene = Self {
            config,
            time: 0,
            held: false,
            sent_account: None,
            released: false,
            sela_verified: false,
            received_report: false,
            received_gate: false,
            received_sela: false,
            available: true,
            warning: false,
            refused: false,
            decision: None,
            pending: [None; 3],
            receipts: [None; 3],
            next_sequence: 0,
            journal: vec![],
            dialogue: String::new(),
        };
        scene.dialogue = format!(
            "At the gate, a load strap looks loose. {} watches across the road. Ivo runs the depot; {} Hold the convoy and inspect the strap, then follow the account to the depot.",
            scene.observer_name(),
            if scene.trusted() {
                "Sela is a trusted intermediary."
            } else {
                "Sela can carry intermediary accounts."
            }
        );
        scene
    }
    fn observer_name(&self) -> &'static str {
        match self.config.observer() {
            Observer::Mara => "Mara",
            Observer::Sela => "Sela",
        }
    }
    fn trusted(&self) -> bool {
        self.config.confidence() == Confidence::Trusted
    }
    fn dispatch(&mut self, route: Route, delay: u32) {
        let slot = route as usize;
        debug_assert!(self.receipts[slot].is_none());
        let due = self.time + delay;
        self.pending[slot] = Some(Pending {
            route,
            due,
            sequence: self.next_sequence,
        });
        self.receipts[slot] = Some((due, None));
        self.next_sequence += 1;
    }
    fn settle(&mut self) {
        // All due routes arrive before the one authored policy evaluation.
        loop {
            let next = self
                .pending
                .iter()
                .flatten()
                .filter(|event| event.due <= self.time)
                .min_by_key(|event| (event.due, event.sequence))
                .copied();
            let Some(event) = next else { break };
            self.pending[event.route as usize] = None;
            self.receipts[event.route as usize] = Some((event.due, Some(self.time)));
            match event.route {
                Route::Report => self.received_report = true,
                Route::Gate => self.received_gate = true,
                Route::Sela => self.received_sela = true,
            }
        }
        let corrected = self.received_gate || (self.received_sela && self.trusted());
        let service = match self.decision.map(|decision| decision.service) {
            None if self.received_report
                && self.config.retention() == Retention::Incomplete
                && !corrected =>
            {
                Some(match self.config.reaction() {
                    Reaction::RestrictUntilCorrection => Service::Restrict,
                    Reaction::WarnAndContinue => Service::Warn,
                })
            }
            Some(Service::Restrict) if corrected => Some(Service::Restore),
            Some(Service::Warn) if corrected => Some(Service::ResolveWarning),
            _ => None,
        };
        if let Some(service) = service {
            self.available = service != Service::Restrict;
            self.warning = service == Service::Warn;
            // Gate receipt is created first, so it wins when both are present.
            self.decision = Some(Decision {
                service,
                at: self.time,
                gate: self.received_gate,
            });
        }
    }
    fn hold(&mut self) {
        self.held = true;
        // Freeze what the observer sent; later receipts never rewrite it.
        self.sent_account = Some(match self.config.retention() {
            Retention::Precise => PRECISE_REPORT,
            Retention::Incomplete => INCOMPLETE_REPORT,
        });
        self.dispatch(Route::Report, self.config.initial_delivery_ticks());
        self.journal
            .push("You held the convoy to check the strap.".into());
        self.dialogue = format!(
            "You hold the convoy and inspect the loose strap. {} prepares her fixed account for the route to Ivo; the route normally delivers {}, but arrival is not guaranteed. Ask {} to hear her account.",
            self.observer_name(),
            timing(self.config.initial_delivery_ticks()),
            self.observer_name()
        );
    }
    fn remember(&mut self, message: String) -> String {
        if !self.journal.contains(&message) {
            self.journal.push(message.clone());
        }
        message
    }
    fn question(&mut self) -> String {
        match self.sent_account {
            Some(account) => self.remember(account.into()),
            None => format!(
                "{}: I have not seen an incident here.",
                self.observer_name()
            ),
        }
    }
    fn release(&mut self) {
        self.held = false;
        self.released = true;
        self.dispatch(Route::Gate, self.config.routine_delivery_ticks());
        self.journal
            .push("You released the convoy and received a gate receipt.".into());
        self.dialogue = format!(
            "You release the convoy and take the gate receipt. The correction route to Ivo normally delivers {}; Sela can verify your receipt and prepare an intermediary account whose route normally delivers {}. Arrival is not guaranteed. Earlier reports and service decisions remain history.",
            timing(self.config.routine_delivery_ticks()),
            timing(self.config.intermediary_delivery_ticks())
        );
    }
    fn handoff(&mut self) {
        self.sela_verified = true;
        self.dispatch(Route::Sela, self.config.intermediary_delivery_ticks());
        self.journal
            .push("Sela prepared your verified receipt for the intermediary pouch.".into());
        self.dialogue = format!(
            "Sela: I verified your release receipt and prepared my intermediary account for the route to Ivo. The route normally delivers {}, but arrival is not guaranteed. Ivo acts only on evidence that has arrived.",
            timing(self.config.intermediary_delivery_ticks())
        );
    }
    fn explanation(&self) -> String {
        let name = self.observer_name();
        let Some(decision) = self.decision else {
            return if self.received_report && self.config.retention() == Retention::Precise {
                format!(
                    "Ivo: {name}'s account arrived. She saw you hold the convoy and inspect a loose strap; your private purpose and later events were out of her view. That inspection context keeps loading service available."
                )
            } else {
                "Ivo: Loading service is available.".into()
            };
        };
        let (prefix, verb, evidence) = match decision.service {
            Service::Restrict => (
                format!("Ivo: {name}'s incomplete report arrived. Service is restricted until {}.", if self.trusted() { "a gate receipt or Sela's verification arrives" } else { "a gate receipt arrives; I have not established confidence in Sela's verification" }),
                "restricted", format!("{name}'s incomplete account")
            ),
            Service::Warn => (
                format!("Ivo: {name}'s incomplete report arrived. I issued a warning; loading service remains available. The warning awaits {}.", if self.trusted() { "a gate receipt or Sela's verification" } else { "a gate receipt; I have not established confidence in Sela's verification" }),
                "issued a warning about", format!("{name}'s incomplete account")
            ),
            Service::Restore => ("Ivo: Service is restored.".into(), "restored", if decision.gate { "your gate release receipt" } else { "Sela's verified release receipt" }.into()),
            Service::ResolveWarning => ("Ivo: The warning is resolved; loading service remains available. The earlier warning remains history.".into(), "resolved the warning about", if decision.gate { "your gate release receipt" } else { "Sela's verified release receipt" }.into()),
        };
        let memory = if decision.service == Service::Restore && self.refused {
            " I remember refusing you; that refusal remains history."
        } else {
            ""
        };
        format!(
            "{prefix}{memory} At hour {}, I {verb} loading based on {evidence}.",
            decision.at
        )
    }
    fn visit(&mut self) -> String {
        let mut message = self.explanation();
        if !self.trusted() && self.received_sela {
            message.push_str(" I received Sela's account, but her verification alone is unverified for me; it does not replace the gate receipt.");
        }
        if !self.available {
            self.refused = true;
        }
        self.remember(message)
    }
    pub fn time(&self) -> u32 {
        self.time
    }
    pub fn enabled(&self, action: u32) -> bool {
        match action {
            0 | 2 | 5 => true,
            1 => self.sent_account.is_none(),
            3 => self.held,
            4 => self.released && !self.sela_verified,
            6 => self.time < config::MAX_TIME,
            _ => false,
        }
    }
    pub fn act(&mut self, action: u32) -> u32 {
        if action == 6 && self.time == config::MAX_TIME {
            return 2;
        }
        if !self.enabled(action) {
            return 1;
        }
        self.settle();
        match action {
            0 => *self = Self::with_config(self.config.clone()),
            1 => self.hold(),
            2 => self.dialogue = self.question(),
            3 => self.release(),
            4 => self.handoff(),
            5 => self.dialogue = self.visit(),
            6 => {
                self.time += 1;
                self.settle();
                self.dialogue =
                    "One fictional hour passes. Return to Ivo to learn his current response."
                        .into();
            }
            _ => unreachable!(),
        }
        self.settle();
        0
    }
    pub fn output(&self, field: u32) -> Option<String> {
        match field {
            0 => Some(self.dialogue.clone()),
            1 => Some(self.journal.join("\n")),
            _ => None,
        }
    }
    /// Developer inspection of this baseline only; unrelated to Core traces.
    pub fn debug_trace(&self) -> String {
        format!(
            "flags_baseline time={} received_report={} received_gate={} received_sela={} available={} warning={} refused={}\npending={:?}\nreceipts(report,gate,sela)={:?}\ndecision={:?}",
            self.time,
            self.received_report,
            self.received_gate,
            self.received_sela,
            self.available,
            self.warning,
            self.refused,
            self.pending,
            self.receipts,
            self.decision
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn configured(
        observer: &str,
        retention: &str,
        reaction: &str,
        confidence: &str,
        delays: [u64; 3],
    ) -> FlagsConvoy {
        FlagsConvoy::with_config(
            config::validate(config::Input {
                observer,
                retention,
                reaction,
                sela_confidence: confidence,
                initial_delivery_ticks: delays[0],
                routine_delivery_ticks: delays[1],
                intermediary_delivery_ticks: delays[2],
                ..config::DEFAULT_INPUT
            })
            .unwrap(),
        )
    }
    fn run(scene: &mut FlagsConvoy, actions: &[u32]) {
        for action in actions {
            assert_eq!(scene.act(*action), 0);
        }
    }
    #[test]
    fn knowledge_is_local_and_remote_arrival_does_not_reveal_it_in_the_journal() {
        let mut scene = configured(
            "sela",
            "incomplete",
            "restrict_until_correction",
            "trusted",
            [3, 3, 1],
        );
        run(&mut scene, &[1]);
        assert!(!scene.received_report && scene.available);
        assert!(!scene.journal.iter().any(|entry| entry == INCOMPLETE_REPORT));
        run(&mut scene, &[5]);
        assert_eq!(scene.dialogue, "Ivo: Loading service is available.");
        run(&mut scene, &[2]);
        assert_eq!(scene.dialogue, INCOMPLETE_REPORT);
        assert!(!scene.received_report);
        let journal = scene.journal.clone();
        run(&mut scene, &[6, 6]);
        assert!(scene.available);
        run(&mut scene, &[6]);
        assert!(scene.received_report && !scene.available);
        assert_eq!(scene.journal, journal);
        run(&mut scene, &[5]);
        assert!(scene.dialogue.contains("Sela's incomplete report arrived"));
    }
    #[test]
    fn sent_account_is_immutable_after_correction_and_repeated_questions() {
        for retention in ["precise", "incomplete"] {
            let mut scene = configured(
                "mara",
                retention,
                "restrict_until_correction",
                "trusted",
                [0, 0, 0],
            );
            run(&mut scene, &[1, 2]);
            let account = scene.dialogue.clone();
            run(&mut scene, &[3, 4, 5, 2]);
            assert_eq!(scene.dialogue, account);
            let journal = scene.journal.clone();
            run(&mut scene, &[2, 2]);
            assert_eq!(scene.journal, journal);
            assert_eq!(scene.sent_account, Some(account.as_str()));
        }
    }
    #[test]
    fn correction_can_overtake_report_without_creating_a_refusal() {
        let mut scene = configured(
            "mara",
            "incomplete",
            "restrict_until_correction",
            "trusted",
            [10, 3, 1],
        );
        run(&mut scene, &[1, 3, 4, 6]);
        assert!(scene.received_sela && !scene.received_gate && !scene.received_report);
        assert!(scene.available && scene.decision.is_none());
        for _ in 0..9 {
            run(&mut scene, &[6]);
        }
        run(&mut scene, &[5]);
        assert!(scene.received_report && scene.available && !scene.refused);
        assert_eq!(scene.dialogue, "Ivo: Loading service is available.");
        assert!(scene.decision.is_none());
    }
    #[test]
    fn equal_time_routes_arrive_before_policy_and_gate_wins_correction_tie() {
        let mut scene = configured(
            "mara",
            "incomplete",
            "restrict_until_correction",
            "trusted",
            [1, 1, 1],
        );
        run(&mut scene, &[1, 3, 4]);
        assert_eq!(
            scene
                .pending
                .iter()
                .flatten()
                .map(|event| (event.due, event.sequence))
                .collect::<Vec<_>>(),
            vec![(1, 0), (1, 1), (1, 2)]
        );
        run(&mut scene, &[6]);
        assert!(scene.received_report && scene.received_gate && scene.received_sela);
        assert!(scene.decision.is_none() && scene.available);
        assert_eq!(scene.receipts, [Some((1, Some(1))); 3]);

        let mut scene = configured(
            "mara",
            "incomplete",
            "restrict_until_correction",
            "trusted",
            [0, 1, 1],
        );
        run(&mut scene, &[1, 5, 3, 4, 6, 5]);
        assert!(scene.dialogue.contains("your gate release receipt"));
        assert!(scene.dialogue.contains("I remember refusing you"));
        assert_eq!(scene.decision.unwrap().at, 1);
    }
    #[test]
    fn immediate_routes_settle_without_time_and_refusal_or_warning_stays_history() {
        for reaction in ["restrict_until_correction", "warn_and_continue"] {
            let mut scene = configured("mara", "incomplete", reaction, "trusted", [0, 0, 0]);
            run(&mut scene, &[1, 5]);
            let earlier = scene.dialogue.clone();
            assert!(scene.received_report);
            run(&mut scene, &[3, 5]);
            assert_eq!(scene.time, 0);
            assert!(scene.received_gate && scene.available && !scene.warning);
            assert!(scene.journal.contains(&earlier));
            if reaction == "restrict_until_correction" {
                assert!(scene.dialogue.contains("At hour 0, I restored loading"));
                assert!(scene.dialogue.contains("I remember refusing you"));
            } else {
                assert!(
                    scene
                        .dialogue
                        .contains("The earlier warning remains history")
                );
            }
        }
    }
    #[test]
    fn confidence_only_controls_sela_verification_across_observers_precision_and_reactions() {
        for observer in ["mara", "sela"] {
            for retention in ["precise", "incomplete"] {
                for reaction in ["restrict_until_correction", "warn_and_continue"] {
                    for confidence in ["trusted", "unverified"] {
                        let mut scene =
                            configured(observer, retention, reaction, confidence, [0, 3, 1]);
                        run(&mut scene, &[1, 5]);
                        let earlier = scene.dialogue.clone();
                        let incomplete = retention == "incomplete";
                        assert_eq!(
                            scene.available,
                            !incomplete || reaction == "warn_and_continue"
                        );
                        run(&mut scene, &[3, 4]);
                        assert!(!scene.received_gate && !scene.received_sela);
                        run(&mut scene, &[6, 5]);
                        assert!(scene.received_sela && !scene.received_gate);
                        if confidence == "unverified" {
                            assert!(scene.dialogue.contains("unverified for me"));
                            assert_eq!(
                                scene.available,
                                !incomplete || reaction == "warn_and_continue"
                            );
                            assert_eq!(
                                scene.warning,
                                incomplete && reaction == "warn_and_continue"
                            );
                        } else {
                            assert!(scene.available && !scene.warning);
                            if incomplete {
                                assert!(scene.dialogue.contains("Sela's verified release receipt"));
                            }
                        }
                        run(&mut scene, &[6, 6, 5]);
                        assert!(scene.received_gate && scene.available && !scene.warning);
                        assert!(scene.journal.contains(&earlier));
                        if incomplete && confidence == "unverified" {
                            assert!(scene.dialogue.contains("your gate release receipt"));
                        }
                        if !incomplete {
                            assert!(scene.decision.is_none());
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn repeated_or_invalid_inputs_are_inert_and_restart_retains_only_config() {
        let mut scene = configured(
            "sela",
            "incomplete",
            "warn_and_continue",
            "unverified",
            [3, 3, 1],
        );
        let initial = scene.dialogue.clone();
        run(&mut scene, &[2]);
        assert_eq!(scene.dialogue, "Sela: I have not seen an incident here.");
        run(&mut scene, &[1, 3, 4, 2]);
        for action in [1, 3, 4, 7, u32::MAX] {
            let before = (
                scene.dialogue.clone(),
                scene.journal.clone(),
                scene.debug_trace(),
            );
            assert_eq!(scene.act(action), 1);
            assert_eq!(
                (
                    scene.dialogue.clone(),
                    scene.journal.clone(),
                    scene.debug_trace()
                ),
                before
            );
        }
        run(&mut scene, &[0]);
        assert_eq!(scene.dialogue, initial);
        assert!(scene.journal.is_empty() && scene.pending.iter().all(Option::is_none));
        assert_eq!(scene.receipts, [None; 3]);
        assert!(scene.available && !scene.warning && !scene.refused);
        assert_eq!(scene.time, 0);
        assert!(scene.enabled(1) && !scene.enabled(3) && !scene.enabled(4));
    }
    #[test]
    fn explicit_clock_stops_at_1000_and_future_routes_remain_pending() {
        let mut scene = configured(
            "mara",
            "incomplete",
            "restrict_until_correction",
            "trusted",
            [1000, 1000, 1000],
        );
        run(&mut scene, &[6, 1, 3, 4]);
        for _ in 1..1000 {
            run(&mut scene, &[6]);
        }
        assert_eq!(scene.time(), 1000);
        assert!(!scene.received_report && scene.pending.iter().all(Option::is_some));
        let before = scene.debug_trace();
        assert_eq!(scene.act(6), 2);
        assert_eq!(scene.debug_trace(), before);
        assert!(!scene.enabled(6));
        assert_eq!(scene.output(2), None);
        run(&mut scene, &[0]);
        assert_eq!(scene.time(), 0);
    }
}
