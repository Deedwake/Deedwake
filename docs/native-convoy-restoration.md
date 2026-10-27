# Native configured-Convoy restoration

The native `convoy-tool` stores configured Convoy Core and host state together.
It is a second binary in the existing `deedwake-repair-tool` package, reusing its
pinned dependencies. `repair-tool` remains that package's default binary. The
browser remains the ordinary playable host; this CLI is a privileged host runner,
not a replacement player interface or a new scripting language.

```sh
cargo run -p deedwake-repair-tool --bin convoy-tool --locked --offline -- \
  --config example-web/convoy.toml --action hold --action ask --action wait \
  --provider unloaded --action wait --action wait --save /tmp/convoy.json --json
cargo run -p deedwake-repair-tool --bin convoy-tool --locked --offline -- \
  --load /tmp/convoy.json --save /tmp/convoy-copy.json --json
cargo run -p deedwake-repair-tool --bin convoy-tool --locked --offline -- \
  --load /tmp/convoy.json --provider loaded --action visit --action release \
  --action handoff --action wait --action visit --json
```

The first invocation saves at hour 3: Ivo has received the report offscreen but
has not executed a service decision. The second invocation restores and saves
without executing any operation; canonical save bytes remain identical. The
third explicitly loads Ivo and visits him at hour 3, then continues. Trusted
verification can resolve the issue at hour 4; unverified verification requires
sufficient routine correction instead. Loading alone never applies that outcome.

Choose exactly one `--config PATH` or `--load SNAPSHOT`. Ordered operations are
`--action reset|hold|ask|release|handoff|visit|wait` and
`--provider loaded|unloaded|unavailable|incapable|retired`. The latter are privileged
host controls for Ivo, never new player choices. Loaded/unloaded mean available;
unavailable/incapable explicitly select loaded residency with that availability.
Retirement is irreversible until fixture restart. Configuration and retired
identity rules are shared with the actual Rust browser scene.

At most 2,128 operations run per invocation, in argument order. A failed operation
is reported and later explicit operations can still run; an invocation is not a
transaction over its operations. `--save PATH` writes the resulting valid state.
Exit codes are 0 for successful operations, 1 for an unavailable operation, 2 for
configuration/load/save failure, and 64 for usage. JSON reports have schema version
1 and are labeled privileged host reports. They include receipt/lifecycle state
outside player knowledge. `--trace` additionally includes the bounded developer
trace. The saved journal and dialogue remain the ordinary learned presentation.

## Coherent boundary and compatibility

Capture is supported only at a completed synchronous host operation boundary,
with no executing or uncertain external result. The configured Convoy host leaves
all created actions confirmed at those boundaries. Proposed, Accepted or Rejected
actions are therefore rejected by this narrower native format. This does not
change the generic Core snapshot contract or Repair's supported pending actions.
Delivery queues need not be empty, and a pending due time equal to the saved clock
is preserved without delivery on load.

The envelope has format `deedwake.convoy.session`, format version 1, Core snapshot
version 1, host version 1, and content version `configured_convoy_v1`. The editable
profile ID is not a compatibility or authenticity guarantee. Unsupported versions
are rejected; migrations, legacy/study snapshots and removed-content repair are
not provided. Configuration is saved in canonical schema 2, including confidence;
schema-1 TOML input normalizes to its trusted equivalent.

The envelope contains the complete Core snapshot: clock, record/action identities,
records, knowledge, outcomes, pending queue and insertion counter, four lifecycle
entries, the three-slot input ledger, managed receipts, trace/drop count and
capacities. Its host half contains validated configuration, a typed fact list,
verification/report/hold/release/service references, held/access/refusal flags,
journal and dialogue. Fact meanings are checked against source, record kind,
action/outcome backlinks and permitted causal relationships; prose does not assign
the meaning of a fact. No actor or queue is reconstructed by replaying inputs.

Restore validates an isolated Core and matching host before replacing a live
bundle once. It checks actual receipt times rather than inferring arrival from
record creation or current time. Unavailable/incapable rejection may legitimately
remain after Ivo becomes available. Already-retired admission can legitimately
leave a prepared route fact with no receipt. Accepted delivery IDs never resend.
Available offscreen receipt and delayed loaded execution remain distinct.

Loading invokes no policy, time advance, delivery, action execution or
confirmation. Missing/duplicate facts, unknown or duplicate JSON fields, bad
references, wrong routes/modes/IDs, forged knowledge, conflicting flags and
incompatible versions fail without replacing live state. Retained trace can be
empty or evicted; validation does not invent a complete interaction/lifecycle
history. Structural consistency is not authenticity or proof of external effects.

Core snapshot versions, native Repair envelopes and browser checkpoint schemas
are unchanged. Browser checkpoints remain exact-artifact player-input replay and
cannot load these native state files. Native restoration does not add browser
lifecycle controls, browser state persistence or an external transaction protocol.

## Limits and file replacement

These ceilings validate native snapshots. The configured host also enforces
[runtime admission limits](configured-session-limits.md) before command mutation.
Neither contract is a performance budget or promise of unlimited resumed play:

| State | Maximum |
| --- | --- |
| Snapshot file / nesting | 8 MiB / 64 levels |
| Core records / actions | 4,096 / 4 |
| Pending deliveries / input slots | 3 / 3 |
| Registered identities / knowledge owners | 4 / 4 |
| Trace capacity / fact entries | 1,024 / 5 |
| Core references / next delivery counter | 32,768 / 4,096 |
| Individual strings / aggregate Core text | 4 KiB / 1 MiB |
| Journal entries / aggregate host text | 128 / 128 KiB |
| Host clock / latest supported route due | 1,000 / 2,000 |

Fresh configured sessions reserve capacity for unfinished story work and a final
visit. An older valid save may load without that reserved headroom; a later
capacity refusal preserves the session and leaves save/reset available. Save
still validates the snapshot ceilings without trimming state; validation failure
leaves the earlier destination untouched. Successful save or runtime admission
does not establish a resource-performance claim.

Atomic replacement is supported on Linux: validate and serialize first, write a
unique sibling temporary file, sync the file, rename over the destination, then
sync the directory. Failures before rename preserve the previous destination and
clean the temporary file. A failure after rename reports
`snapshot_save_uncertain`: replacement committed, durability uncertain. Concurrent
writers, cross-version recovery and uncertain external-effect reconciliation are
unsupported. Paths are user-selected local files; there is no service upload.

Tests include separate processes, no-op byte identity, pending correction and
terminal receipts, confidence/reaction variants, offscreen receipt followed by
loaded execution, corruption and failed replacement. Shared disk-failure injection
also preserves the existing Repair guarantees. These are implementation checks,
not human/adopter evaluation or a declaration that every MVP gate is complete.
