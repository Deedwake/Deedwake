//! Fixed configured-session admission. Core remains an unmanaged, reusable engine.
use super::*;

pub const MAX_RECORDS: usize = 4096;
pub const MAX_TEXT_BYTES: usize = 1024 * 1024;
pub const MAX_JOURNAL: usize = 128;
pub const MAX_ACTIONS: usize = 4;
pub const MAX_PENDING: usize = 3;
pub const MAX_TRANSIENT_PENDING: usize = 4;
pub const MAX_REFERENCES: usize = 32768;
pub const MAX_DELIVERY_SEQUENCE: usize = 4096;
pub const MAX_HOST_TEXT_BYTES: usize = 128 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum LimitResource {
    Records = 1,
    TextBytes = 2,
    Journal = 3,
    Actions = 4,
    Pending = 5,
    References = 6,
    DeliverySequence = 7,
    HostTextBytes = 8,
}
impl LimitResource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Records => "records",
            Self::TextBytes => "text_bytes",
            Self::Journal => "journal",
            Self::Actions => "actions",
            Self::Pending => "pending",
            Self::References => "references",
            Self::DeliverySequence => "delivery_sequence",
            Self::HostTextBytes => "host_text_bytes",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandError {
    InvalidAction,
    ClockLimit,
    Capacity(LimitResource),
}
impl CommandError {
    /// Configured command ABI: legacy statuses stay 0/1/2; capacity is 17..24.
    pub fn code(self) -> u32 {
        match self {
            Self::InvalidAction => 1,
            Self::ClockLimit => 2,
            Self::Capacity(resource) => 16 + resource as u32,
        }
    }
}

// Compiled record/visit text fits 512 bytes; request+rule fits 160; a trace
// rule fits 128. Formatting uses fixed names and times <=1000 (due <=2000).
// Restored custom strings are already included in Core::usage(), and a custom
// report spoken to the player is accounted at its actual length below.
const RECORD_TEXT: usize = 512;
const ACTION_TEXT: usize = 160;
const TRACE_TEXT: usize = 128;
const DIALOGUE_TEXT: usize = 1024;

impl Convoy {
    fn remaining_service_outcomes(&self) -> usize {
        use deedwake_core::DeliveryState;
        if self.retention() == Retention::Precise
            || self.provider_state() == Some(EntityState::Retired)
        {
            return 0;
        }
        let delivered = |kind| {
            self.provider_delivery_receipt(kind)
                .is_some_and(|receipt| matches!(receipt.state, DeliveryState::Delivered { .. }))
        };
        // A terminal rejected route cannot be resent. Pending routes beyond the
        // host clock ceiling also cannot generate a future service outcome.
        let possible = |kind, not_sent, delay| match self.provider_delivery_receipt(kind) {
            Some(receipt) => match receipt.state {
                DeliveryState::Delivered { .. } => true,
                DeliveryState::Pending => receipt.delivery.due <= u64::from(config::MAX_TIME),
                DeliveryState::Rejected { .. } => false,
            },
            None => not_sent && self.core.time() + u64::from(delay) <= u64::from(config::MAX_TIME),
        };
        let trusted = self.confidence() == Confidence::Trusted;
        let correction_arrived = delivered(ProviderDelivery::Routine)
            || (trusted && delivered(ProviderDelivery::Intermediary));
        let correction_possible = possible(
            ProviderDelivery::Routine,
            self.release.is_none(),
            self.routine_delay(),
        ) || (trusted
            && possible(
                ProviderDelivery::Intermediary,
                self.verification.is_none(),
                self.intermediary_delay(),
            ));
        match self.service_decision.and_then(|id| self.core.action(id)) {
            Some(action) => match action.status {
                Status::Confirmed(record) => match self.facts.get(&record) {
                    Some(Fact::Service {
                        operation: ServiceOperation::Restrict | ServiceOperation::Warn,
                        ..
                    }) => usize::from(correction_possible),
                    Some(Fact::Service {
                        operation: ServiceOperation::Restore | ServiceOperation::ResolveWarning,
                        ..
                    }) => 0,
                    _ => 2,
                },
                _ => 2,
            },
            None if correction_arrived => 0,
            None if possible(
                ProviderDelivery::Report,
                self.report.is_none(),
                self.initial_delay(),
            ) =>
            {
                1 + usize::from(correction_possible)
            }
            None => 0,
        }
    }

    pub(crate) fn preflight(&self, action: u32) -> Result<(), CommandError> {
        if action == 0 || self.config.is_none() {
            return Ok(());
        }
        let usage = self.core.usage();
        let service = self.remaining_service_outcomes();
        let hold = usize::from(self.report.is_none());
        let release = usize::from(self.release.is_none());
        let handoff = usize::from(self.verification.is_none());
        let (records, actions, schedules, events) = match action {
            1 => (4, 1, 1, 8),
            2 => (0, 0, usize::from(self.report.is_some()), 2),
            3 => (1, 1, 1, 5),
            4 => (1, 0, 2, 5),
            5 => (1, 0, 0, 1),
            6 => (0, 0, 0, 0),
            _ => unreachable!("action eligibility checked before preflight"),
        };
        // One command can settle at both ends: release/wait/handoff can produce
        // two service outcomes, whereas hold/question/visit can produce one.
        let command_service = service.min(if matches!(action, 3 | 4 | 6) { 2 } else { 1 });
        let mut required_records = records + command_service;
        let mut required_actions = actions + command_service;
        let mut required_schedules = schedules;
        let mut required_events = events + 3 * command_service + usage.pending;
        let mut journal_entries = usize::from(matches!(action, 1..=5));
        let mut journal_bytes = journal_entries * RECORD_TEXT;
        let mut dialogue_bytes = DIALOGUE_TEXT;
        if action == 2 {
            if let Some(report) = self.report {
                if let Some(deedwake_core::Record {
                    content: Content::Account { summary, .. },
                    ..
                }) = self.core.record(report)
                {
                    journal_entries = usize::from(!self.journal.contains(summary));
                    journal_bytes = journal_entries * summary.len();
                    dialogue_bytes = dialogue_bytes.max(summary.len());
                }
            } else {
                journal_entries = 0;
                journal_bytes = 0;
            }
        }
        // Optional repeat questions/visits must leave unfinished one-off story
        // operations (fresh: 6 records, 2 actions, 4 schedules), all remaining
        // service outcomes (at most 2), and a final visit available. Taking max
        // avoids counting those same outcomes twice in the current command.
        let story_records = 4 * hold + release + handoff + service;
        let story_actions = hold + release + service;
        let story_schedules = hold + release + 2 * handoff;
        let story_events = 8 * hold + 5 * release + 5 * handoff + 3 * service + usage.pending;
        let optional_records = usize::from(action == 5);
        let optional_schedules = if action == 2 { schedules } else { 0 };
        let optional_events = if matches!(action, 2 | 5) { events } else { 0 };
        let final_visit = usize::from(story_records > 0 || story_schedules > 0);
        required_records = required_records.max(story_records + optional_records + final_visit);
        required_actions = required_actions.max(story_actions);
        required_schedules = required_schedules.max(story_schedules + optional_schedules);
        required_events = required_events.max(story_events + optional_events + final_visit);
        let story_journal = hold + release + handoff + final_visit;
        journal_entries = journal_entries.max(story_journal + usize::from(matches!(action, 2 | 5)));
        journal_bytes = journal_bytes.max(
            story_journal * RECORD_TEXT
                + if matches!(action, 2 | 5) {
                    journal_bytes
                } else {
                    0
                },
        );
        let text = required_records * RECORD_TEXT
            + required_actions * ACTION_TEXT
            + required_events * TRACE_TEXT;
        // Each new record adds owner knowledge, each schedule can add recipient
        // knowledge, each action has one cause, and every event has <=1 cause.
        let refs = required_records
            + required_schedules
            + usage.pending
            + required_actions
            + required_events;
        for (current, growth, max, resource) in [
            (
                usage.records,
                required_records,
                MAX_RECORDS,
                LimitResource::Records,
            ),
            (
                usage.text_bytes,
                text,
                MAX_TEXT_BYTES,
                LimitResource::TextBytes,
            ),
            (
                self.journal.len(),
                journal_entries,
                MAX_JOURNAL,
                LimitResource::Journal,
            ),
            (
                usage.actions,
                required_actions,
                MAX_ACTIONS,
                LimitResource::Actions,
            ),
            (
                usage.reference_edges,
                refs,
                MAX_REFERENCES,
                LimitResource::References,
            ),
            (
                usage.next_delivery,
                required_schedules,
                MAX_DELIVERY_SEQUENCE,
                LimitResource::DeliverySequence,
            ),
            (
                self.journal.iter().map(String::len).sum::<usize>(),
                journal_bytes + dialogue_bytes,
                MAX_HOST_TEXT_BYTES,
                LimitResource::HostTextBytes,
            ),
        ] {
            if current.checked_add(growth).is_none_or(|total| total > max) {
                return Err(CommandError::Capacity(resource));
            }
        }
        // A same-time question may briefly coexist with all three managed
        // deliveries; it is drained before the command returns to quiescence.
        let transient = if matches!(action, 2 | 4) { 1 } else { 0 };
        let retained = usize::from(matches!(action, 1 | 3 | 4));
        if usage.pending + transient > MAX_TRANSIENT_PENDING
            || usage.pending + retained > MAX_TRANSIENT_PENDING
        {
            return Err(CommandError::Capacity(LimitResource::Pending));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deedwake_core::{CoreSnapshot, SnapshotLimits};

    fn configured() -> Convoy {
        Convoy::with_config(config::validate(config::DEFAULT_INPUT).unwrap())
    }
    fn core_snapshot(scene: &Convoy) -> CoreSnapshot {
        scene.core.snapshot(&SnapshotLimits::default()).unwrap()
    }
    fn fork(scene: &Convoy) -> Convoy {
        Convoy {
            condition: scene.condition,
            study: scene.study,
            config: scene.config.clone(),
            facts: scene.facts.clone(),
            verification: scene.verification,
            core: Core::from_snapshot(core_snapshot(scene), &SnapshotLimits::default()).unwrap(),
            held: scene.held,
            access: scene.access,
            report: scene.report,
            hold: scene.hold,
            release: scene.release,
            service_decision: scene.service_decision,
            refused: scene.refused,
            journal: scene.journal.clone(),
            dialogue: scene.dialogue.clone(),
        }
    }
    fn same(left: &Convoy, right: &Convoy) {
        assert_eq!(core_snapshot(left), core_snapshot(right));
        assert_eq!(left.core.usage(), right.core.usage());
        assert_eq!(
            (
                left.held,
                left.access,
                left.report,
                left.hold,
                left.release,
                left.verification,
                left.service_decision,
                left.refused
            ),
            (
                right.held,
                right.access,
                right.report,
                right.hold,
                right.release,
                right.verification,
                right.service_decision,
                right.refused
            )
        );
        assert_eq!(left.facts, right.facts);
        assert_eq!(left.journal, right.journal);
        assert_eq!(left.dialogue, right.dialogue);
        for field in 0..3 {
            assert_eq!(left.output(field), right.output(field));
        }
    }
    fn rejected_unchanged(scene: &mut Convoy, action: u32, resource: LimitResource) {
        let mut untouched = fork(scene);
        assert_eq!(
            scene.act_checked(action),
            Err(CommandError::Capacity(resource))
        );
        same(scene, &untouched);
        // Reset and future replay still agree: failed commands do not poison state.
        assert_eq!(scene.act_checked(0), Ok(()));
        assert_eq!(untouched.act_checked(0), Ok(()));
        for next in [1, 6, 6, 6, 3, 4, 6, 5] {
            assert_eq!(scene.act_checked(next), untouched.act_checked(next));
        }
        same(scene, &untouched);
    }
    #[test]
    fn optional_visits_leave_the_full_story_and_last_visit_at_exact_record_capacity() {
        for retention in ["incomplete", "precise"] {
            for sela_confidence in ["trusted", "unverified"] {
                for reaction in ["restrict_until_correction", "warn_and_continue"] {
                    let mut scene = Convoy::with_config(
                        config::validate(config::Input {
                            schema_version: 2,
                            retention,
                            sela_confidence,
                            reaction,
                            trace_capacity: 0,
                            ..config::DEFAULT_INPUT
                        })
                        .unwrap(),
                    );
                    while scene.act_checked(5).is_ok() {}
                    let before = fork(&scene);
                    assert_eq!(
                        scene.act_checked(5),
                        Err(CommandError::Capacity(LimitResource::Records))
                    );
                    same(&scene, &before);
                    for action in [1, 6, 6, 6, 3, 4, 6, 6, 6, 5] {
                        assert_eq!(
                            scene.act_checked(action),
                            Ok(()),
                            "{retention}, {sela_confidence}, {reaction}, {action}"
                        );
                    }
                    assert_eq!(scene.core.usage().records, MAX_RECORDS);
                    assert!(scene.access);
                    assert!(scene.core.usage().actions <= MAX_ACTIONS);
                    assert_eq!(scene.act_checked(0), Ok(()));
                    assert_eq!(scene.core.usage().records, 0);
                }
            }
        }
    }
    #[test]
    fn final_visit_survives_overtaking_and_corrections_beyond_the_host_clock() {
        for late in [false, true] {
            let mut scene = Convoy::with_config(
                config::validate(config::Input {
                    initial_delivery_ticks: if late { 1000 } else { 3 },
                    routine_delivery_ticks: if late { 1000 } else { 0 },
                    intermediary_delivery_ticks: if late { 1000 } else { 0 },
                    trace_capacity: 0,
                    ..config::DEFAULT_INPUT
                })
                .unwrap(),
            );
            while scene.act_checked(5).is_ok() {}
            assert_eq!(scene.act_checked(1), Ok(()));
            if late {
                for _ in 0..1000 {
                    assert_eq!(scene.act_checked(6), Ok(()));
                }
            }
            assert_eq!(scene.act_checked(3), Ok(()));
            assert_eq!(scene.act_checked(4), Ok(()));
            assert_eq!(scene.act_checked(5), Ok(()));
            assert_eq!(scene.access, !late);
            while scene.act_checked(5).is_ok() {}
            assert_eq!(scene.core.usage().records, MAX_RECORDS);
        }
    }
    #[test]
    fn old_near_capacity_state_is_admitted_on_restore_but_command_rejection_is_atomic() {
        let mut scene = configured();
        // Existing generic Core operations remain unmanaged; these model a
        // historical valid save with no runtime reserve, without narrowing DTOs.
        for _ in 0..MAX_RECORDS {
            scene.core.observe(PLAYER, "old".into());
        }
        let mut restored = fork(&scene);
        assert_eq!(restored.core.usage().records, MAX_RECORDS);
        rejected_unchanged(&mut restored, 1, LimitResource::Records);
    }
    #[test]
    fn text_preflight_includes_custom_retained_trace_and_proposal_strings() {
        let mut scene = configured();
        scene.core = Core::with_lifecycle_capacity(1024, 3, 4);
        for e in [PLAYER, WITNESS, PROVIDER, INTERMEDIARY] {
            scene
                .core
                .register_entity(e, Residency::Loaded, Availability::Available)
                .unwrap();
        }
        let record = scene.core.observe(PLAYER, "x".repeat(4096));
        // Fill aggregate text through retained custom trace rules: record-only
        // accounting would leave this scene looking almost empty.
        for _ in 0..126 {
            let id = scene
                .core
                .evaluate(PLAYER, |_| {
                    Some(Proposal {
                        request: "x".repeat(4096),
                        rule: "y".repeat(4096),
                        causes: vec![record],
                    })
                })
                .unwrap()
                .unwrap();
            scene.core.reject(id, "z".repeat(4096)).unwrap();
        }
        assert!(scene.core.usage().text_bytes > MAX_TEXT_BYTES);
        rejected_unchanged(&mut scene, 5, LimitResource::TextBytes);
    }
    #[test]
    fn command_budget_precedes_any_due_delivery_or_policy_settlement() {
        let mut scene = configured();
        assert_eq!(scene.act_checked(1), Ok(()));
        // A due-now delivery is deliberately left queued. The rejected visit
        // must leave its receipt Pending and provider knowledge unchanged.
        scene.core.advance_to(2).unwrap();
        scene.journal = vec!["old".into(); MAX_JOURNAL];
        rejected_unchanged(&mut scene, 6, LimitResource::Journal);
    }
    #[test]
    fn question_temporarily_coexists_with_three_protected_managed_routes() {
        let mut scene = Convoy::with_config(
            config::validate(config::Input {
                initial_delivery_ticks: 1000,
                routine_delivery_ticks: 1000,
                intermediary_delivery_ticks: 1000,
                ..config::DEFAULT_INPUT
            })
            .unwrap(),
        );
        for action in [1, 3, 4] {
            assert_eq!(scene.act_checked(action), Ok(()));
        }
        assert_eq!(scene.core.usage().pending, 3);
        assert_eq!(scene.act_checked(2), Ok(()));
        assert_eq!(scene.core.usage().pending, 3);
        assert_eq!(scene.core.usage().inputs, 3);
        assert!(
            scene
                .core
                .perspective(PLAYER)
                .records()
                .iter()
                .any(|r| Some(r.id) == scene.report)
        );
        for kind in [
            ProviderDelivery::Report,
            ProviderDelivery::Routine,
            ProviderDelivery::Intermediary,
        ] {
            assert_eq!(
                scene.provider_delivery_receipt(kind).unwrap().state,
                deedwake_core::DeliveryState::Pending
            );
        }
    }
    #[test]
    fn optional_questions_leave_all_four_story_schedules_available() {
        let mut scene = configured();
        assert_eq!(scene.act_checked(1), Ok(()));
        while scene.act_checked(2).is_ok() {}
        assert_eq!(scene.core.usage().next_delivery, MAX_DELIVERY_SEQUENCE - 3);
        let before = fork(&scene);
        assert_eq!(
            scene.act_checked(2),
            Err(CommandError::Capacity(LimitResource::DeliverySequence))
        );
        same(&scene, &before);
        for action in [6, 6, 6, 3, 4, 6, 5] {
            assert_eq!(scene.act_checked(action), Ok(()));
        }
        assert_eq!(scene.core.usage().next_delivery, MAX_DELIVERY_SEQUENCE);
        assert!(scene.access);
    }
    #[test]
    fn journal_and_host_text_reserves_reject_at_the_admission_boundary() {
        let mut scene = configured();
        // Fresh optional visits reserve hold/release/handoff/final-visit journal
        // entries as well as the current encounter: five entries total.
        scene.journal = vec!["historical".into(); MAX_JOURNAL - 5];
        assert_eq!(scene.act_checked(5), Ok(()));
        rejected_unchanged(&mut scene, 5, LimitResource::Journal);

        let mut scene = configured();
        scene.journal = vec!["x".repeat(4096); 31];
        scene.journal.push("x".repeat(512));
        assert_eq!(scene.act_checked(5), Ok(()));
        rejected_unchanged(&mut scene, 5, LimitResource::HostTextBytes);
    }

    #[test]
    fn release_can_confirm_both_pre_and_post_service_outcomes_and_wait_can_confirm_two() {
        for action in [3, 6] {
            let mut scene = Convoy::with_config(
                config::validate(config::Input {
                    initial_delivery_ticks: 0,
                    routine_delivery_ticks: if action == 6 { 1 } else { 0 },
                    ..config::DEFAULT_INPUT
                })
                .unwrap(),
            );
            // Build a valid prefix with a due-now report, before settlement.
            scene.delay();
            if action == 6 {
                scene.release();
            }
            let before = scene.core.usage();
            assert_eq!(scene.act_checked(action), Ok(()));
            let after = scene.core.usage();
            if action == 3 {
                assert_eq!(after.records - before.records, 3);
                assert_eq!(after.actions - before.actions, 3);
            } else {
                assert_eq!(after.records - before.records, 2);
                assert_eq!(after.actions - before.actions, 2);
            }
            assert!(after.actions <= MAX_ACTIONS && scene.access);
        }
    }
}
