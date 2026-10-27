# Configured session limits

The configured reference host now checks capacity before an entire command,
including its before/after settlement. The reusable Core supplies a privileged,
constant-time `usage()` report; it does not apply this example's policy to generic
or legacy callers. No new dependencies, configuration knobs or snapshot fields
are required. These bounds qualify a small reference session, not a larger game.

| Resource | Configured ceiling |
| --- | --- |
| Records / actions | 4,096 / 4 |
| Pending deliveries | 3 at command/save boundaries; 4 transiently |
| Input slots / registered identities / knowledge owners | 3 / 4 / 4 |
| Aggregate knowledge, proposal and trace references | 32,768 |
| Retained trace | Configured capacity, at most 1,024; existing eviction |
| Semantic facts / delivery sequence | 5 / 4,096 |
| Clock / permitted route due time | 1,000 / 2,000 |
| Individual supported strings / aggregate Core text | 4 KiB / 1 MiB |
| Journal / aggregate journal plus dialogue | 128 entries / 128 KiB |

Core text includes record content, action requests/rules/rejections, and retained
trace rules. Reference accounting matches the existing snapshot limit definition.
Derived text/reference counters update with mutations and trace eviction and are
rebuilt during validated restoration. They are not serialized. A record-only
counter would miss admitted historical action/trace text.

## Admission and completion room

Compiled conservative growth estimates cover the complete next command, incoming
deliveries and possible service outcomes. Optional repeated questions/visits also
leave room for outstanding one-off story operations and a final visit. Remaining
service reservations shrink after actual resolution, early sufficient correction,
terminal rejection or routes that cannot arrive within the clock horizon.

Admission reads counts and fixed-size host facts rather than cloning/replaying
Core or walking its record history. Journal checks scan at most 128 entries.
Conservative admission can refuse before every last byte/slot is used. A refusal
changes no world state, clock, receipts, records, counters, dialogue or journal.
No historical evidence is deleted. Retirement and terminal-delivery rules still
apply; reserved capacity cannot repair a rejected delivery or extend the clock.

`Convoy::act_checked` returns a typed `CommandError::Capacity(LimitResource)`.
Existing `act` statuses stay 0/1/2; capacity maps to unavailable for older callers.
The configured Wasm capability adds typed command codes 17–24 and static ceiling
metadata. Its adapter treats these as recoverable capacity errors. Ordinary UI
shows a generic capacity message outside the journal and keeps save/reset usable;
it does not render privileged Core usage, receipt state or counter totals.
Malformed ABI statuses remain fatal errors, not capacity refusals. Native
`convoy-tool` steps add a typed `capacity_resource` field when admission fails.

Reset always succeeds and clears usage while retaining configuration. Save remains
available for admitted states. Existing valid native files remain loadable under
the unchanged format; older files may lack the protected headroom and have some
next commands refused. The completion-room guarantee applies to fresh bounded
sessions under otherwise feasible routes/lifecycle, not retroactively to every
old file or every timing configuration.

## Browser replay capacity

The configured checkpoint path also admits replay inputs before executing them.
It supports at most 4,096 successful inputs and a 64 KiB file. Optional questions
and repeat visits leave room for remaining one-off choices, a possible visit after
each, all remaining waits and a visit after each wait. The reservation uses only
the public clock/choice flags and the player's own accepted command sequence.
It reads no private knowledge or usage totals.

Thus optional repetition may stop before 4,096 inputs; protected choices and a
fresh return visit retain their slots. A rejected input is never logged and never
executes. Reset clears the log. Loading validates the same admission rules at each
prefix in the isolated actual-module replay and publishes the derived visit flag
only on success. Invalid files leave the live session and saveability unchanged.

Legacy conditions/studies and their old checkpoint path are unchanged. Configured
checkpoint schema 2 retains the same fields and command meanings; exact Wasm
identity rejects older builds. Optional-heavy files from an older admission policy
may be refused rather than silently clipped. Native Core+host restoration remains
actual state restoration and does not use this replay log.

## Verification and limits

Tests independently recompute usage from snapshots across successful/failed
operations, identified retries, duplicate deliveries, trace eviction and restore.
Host checks cover exact capacities, a four-message transient queue, before/after
service outcomes, protected story completion and atomic refusals. A structurally
valid native save with custom action/trace prose at 1 MiB minus one byte loads
inertly, refuses additional growth and still saves byte-for-byte. Browser tests
exercise real Wasm refusal, replay reserve, save/reset and malformed capability
statuses. Human/adopter value and larger-game capacity are not established by
these tests. Browser DOM/file-dialog qualification remains a separate limit.
