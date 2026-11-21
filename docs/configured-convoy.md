# Author the configured convoy

`/configured.html` runs one build-time authored convoy profile through the actual
Rust/Wasm host. `/configured-developer.html` adds read-only configuration and the
causal trace. The ordinary page exposes only learned encounters and journal
entries. Existing `/`, current/control trial pages, study pages, and the root
native convoy example retain their fixed profiles.

Edit `example-web/convoy.toml`, rebuild, then restart. There is no runtime editor,
script, expression, include mechanism, policy source, network call or model call.
The reference package has four stable identities: player, Mara, Ivo and Sela.
Only the existing observer role is selectable; this is not a character editor.

| Required field | Supported values |
| --- | --- |
| `schema_version` | Integer `2`; legacy schema 1 remains supported as described below |
| `profile_id` | 1–64 lowercase ASCII letters, digits, `_` or `-` |
| `observer` | `"mara"` or `"sela"` |
| `retention` | `"precise"` or `"incomplete"` |
| `sela_confidence` | `"trusted"` or `"unverified"`; schema 2 only |
| `reaction` | `"restrict_until_correction"` or `"warn_and_continue"` |
| `trace_capacity` | Integer `0..=1024`; zero disables retained trace |
| `initial_delivery_ticks` | Integer `0..=1000`, relative to the incident |
| `routine_delivery_ticks` | Integer `0..=1000`, relative to release |
| `intermediary_delivery_ticks` | Integer `0..=1000`, relative to handoff |

Schema 2 requires all ten fields. Schema 1 accepts exactly the previous nine
fields, without `sela_confidence`, and retains trusted behavior. Adding that key
to schema 1 is an error; unknown versions are rejected. Direct Rust construction
uses the same validation: schema 1 supports trusted only, while schema 2 permits
both confidence values. The checked-in schema-2 profile defaults to trusted.

Mara or Sela observes the same visible hold/strap inspection, without private
motives or later release. Precise retention preserves that visible context;
incomplete retention omits it with explicit uncertainty. Sela can be both the
observer and the intermediary. Her earlier sent account remains unchanged when
she later receives and verifies the release receipt. Ask-observer labels and
dialogue follow the selected identity.

An arrived incomplete report produces the selected compiled reaction. Restriction
withholds loading until correction arrives. Warning keeps loading available and
records an unresolved warning, later resolved by sufficient arrived correction. Precise
context keeps service available without inventing either an adverse decision or
a warning. An earlier correction can prevent a later report from creating a
restriction/warning. Ivo has one directed, fixed confidence setting for Sela's release verification.
With `trusted`, her arrived verification is sufficient, preserving existing
behavior. With `unverified`, that same account remains received history but does
not resolve or prevent a restriction/warning; an arrived gate receipt remains
sufficient. This setting does not invalidate Sela's witnessed report or change
what she actually observed, and it is not a global reputation score. No automatic
trust evolution is implemented. Ivo explains insufficient verification when visited;
clock advancement does not reveal his confidence or response in the player journal.

Policies use received semantic facts; display prose is not
a policy predicate. Correction and warning resolution append outcomes rather than
rewriting accounts or past refusal/warning encounters. The host still accepts
and executes requests before confirming actual outcomes.

Zero delay means receipt in the dispatch action's current hour. Reading and other
choices never advance fictional time; only the advance choice does. Equal-time
receipts follow insertion order. Time stops at hour 1000; a late send can remain
pending beyond that horizon. Limits are support bounds, not promises that every
valid configuration completes its workflow before the horizon. Restart discards
progress and retains the compiled profile. Remote arrivals never add journal
entries; visit Ivo to learn his response.

## Build, diagnose and compare

```sh
npm ci --ignore-scripts
cargo test -p deedwake-example-web --test config_compile --locked --offline
cargo run -p deedwake-example-web --example convoy_configured --locked --offline -- --actions 1,2,6,6,6,5,3,4,6,5 --trace
npm run build
npm test
npm run preview
```

Unknown/missing/duplicate keys, wrong types, unsupported selections and out-of-range
values fail the build. Diagnostics include a stable code, field where available,
filename, one-based line/column and byte span. Build output is ordinary escaped
Rust construction under Cargo's `OUT_DIR`; builds never rewrite tracked source.
Direct construction and generated construction use the same validated model.
Changing `profile_id` names the authored profile; it is not authentication.

The native `convoy_configured` example and Wasm consume the same generated values
and scene. The older `cargo run --example convoy` remains the fixed legacy
regression entry point. Built-in tests exercise the checked-in default plus direct
Rust variants; configuration-specific timing assertions may need corresponding
updates for intentional changes. No second TypeScript simulation exists.

Only the already pinned TOML 1.1.8 and Serde 1.0.229 build/test declarations were
added to `example-web`. Their existing locked dependencies supply parsing and
source diagnostics. The normal browser and default Core runtime dependency graphs
remain standard-library-only. No parser executes inside the simulation.

## Configured save and resume

The ordinary configured page downloads and loads schema-2 checkpoints with exactly
`magic`, `schema`, `exactWasmSHA256`, `profileId` and `actions`. It replays successful
inputs into a fresh instance of the retained actual module. Schema 2 is exclusive
to configured profiles; schema-1 legacy checkpoints remain on the legacy path.
Core snapshots and native Repair persistence formats are unchanged.

The configured checkpoint remains schema 2: the exact Wasm digest covers the
generated configuration, including confidence, and code. A file from a
different artifact, even with the same profile name, is rejected. No migration is
promised. The digest checks compatibility, not authenticity. Changing this build
also changes the artifact identity of legacy checkpoints; older binary checkpoints
remain incompatible as documented.

At most 4096 successful inputs and 64 KiB are supported. Configured sessions
now refuse growth before runtime or replay capacity is consumed, with room for
unfinished story work and a final visit; save/reset remain available. Optional
repetition can stop earlier than the absolute replay cap. History is never
truncated, and failed actions are not recorded. See the
[session limit contract](configured-session-limits.md). Loading checks UTF-8, exact keys,
duplicates, schema, profile, digest and bounds, then replays into an isolated
candidate. Every input must succeed. Only a fully validated candidate replaces
live state. Actions, restart and overlapping loads are blocked during loading;
errors leave progress and the previous saveability state intact. Nothing uploads
to a service. Web Crypto SHA-256 is required for checkpoints, not ordinary play.

This remains synthetic-fixture replay, not durable Core restoration or a mechanism
for repeating external transactions. The separate native snapshot API is the
supported state-restoration boundary.

## Implementation and evaluation

The owner's sequence is to finish the defined MVP implementation before seeking
participant feedback, then consider a separate, more complex game. Implementation
completion and evaluation completion are different claims. This slice implements
reference authoring and configured save/resume; it does not declare the entire
MVP implemented, validate player value, or satisfy the outside developer exercise.
Those evidence gates remain pending. The explicit relationship dimension is now implemented for this narrow source
relationship. Reference-host lifecycle integration, operational limits and
packaging still need an implementation completeness review. A larger game is not part of this slice.

The optional example-owned `dw_convoy_confidence()` metadata export reports trusted
(1), unverified (2), or no active configured profile (maximum u32). Existing
configuration metadata version 1 and its bounds are unchanged. The TypeScript
adapter treats an older artifact without this extension as trusted, its only
supported behavior. Present malformed metadata is rejected. This is read-only
developer metadata; it does not add a player choice or change the generic ABI.

## Configured host lifecycle

Configured sessions register the same four identities. `set_provider_state` is a
privileged Rust host operation for Ivo only; it does not move the clock, deliver
messages, run policy, change dialogue or write the journal. The other identities
stay loaded and available. `provider_state` and `provider_delivery_receipt` are
read-only host inspection, separate from ordinary learned information. Legacy
conditions/studies remain unmanaged and reject provider updates.

The three fixed provider routes use managed Input IDs 1 (initial report), 2
(routine correction), and 3 (intermediary verification), sharing a capacity of
three. IDs and terminal receipts are never evicted during a session. Available
but unloaded Ivo can receive an abstract account; service execution requires him
to be loaded and available. Loading alone does not execute work. The next explicit
host/player step evaluates received evidence at that step's actual hour. A visit
while blocked returns unavailable without changing the session or hiding choices
based on privileged state.

Unavailable, incapable or retired recipients reject queued delivery at its due
time, without gaining the record or producing a delivered event. Terminal
rejection stays rejected after a later state change; acknowledging the same ID
never resends. If Ivo was already retired before dispatch, admission fails before
creating an ID receipt. Local hold/release/handoff and account creation can still
complete; route-duration feedback is the same across lifecycle states and does
not promise admission or arrival. A missing receipt is not proof of delivery.
There is no automatic resend or deferred delivery policy. Retirement permanently
reserves the identity until fixture restart.

Restart resets lifecycle, receipts and progress while retaining configuration.
The ordinary browser has no lifecycle controls; its configured session starts
with everyone loaded and available. Browser checkpoints still replay only player
inputs in the exact artifact and are not a persistence mechanism for external
host lifecycle operations. No snapshot schema or generic ABI changed in this slice.
