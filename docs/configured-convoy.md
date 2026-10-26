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
| `schema_version` | Integer `1` |
| `profile_id` | 1–64 lowercase ASCII letters, digits, `_` or `-` |
| `observer` | `"mara"` or `"sela"` |
| `retention` | `"precise"` or `"incomplete"` |
| `reaction` | `"restrict_until_correction"` or `"warn_and_continue"` |
| `trace_capacity` | Integer `0..=1024`; zero disables retained trace |
| `initial_delivery_ticks` | Integer `0..=1000`, relative to the incident |
| `routine_delivery_ticks` | Integer `0..=1000`, relative to release |
| `intermediary_delivery_ticks` | Integer `0..=1000`, relative to handoff |

Mara or Sela observes the same visible hold/strap inspection, without private
motives or later release. Precise retention preserves that visible context;
incomplete retention omits it with explicit uncertainty. Sela can be both the
observer and the intermediary. Her earlier sent account remains unchanged when
she later receives and verifies the release receipt. Ask-observer labels and
dialogue follow the selected identity.

An arrived incomplete report produces the selected compiled reaction. Restriction
withholds loading until correction arrives. Warning keeps loading available and
records an unresolved warning, later resolved by arrived correction. Precise
context keeps service available without inventing either an adverse decision or
a warning. An earlier correction can prevent a later report from creating a
restriction/warning. Policies use received semantic facts; display prose is not
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

The exact Wasm digest covers the generated configuration and code. A file from a
different artifact, even with the same profile name, is rejected. No migration is
promised. The digest checks compatibility, not authenticity. Changing this build
also changes the artifact identity of legacy checkpoints; older binary checkpoints
remain incompatible as documented.

At most 4096 successful inputs and 64 KiB are supported. At the next successful
input, play continues but saving becomes visibly unavailable; history is never
truncated. Failed actions are not recorded. Loading checks UTF-8, exact keys,
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
Those evidence gates remain pending. The explicit relationship dimension,
reference-host lifecycle integration, operational limits and packaging still need
an implementation completeness review. A larger game is not part of this slice.
