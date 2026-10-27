# Deedwake

A small deterministic Rust core and browser-text examples. The original convoy example is: **The gate
receipt**. A fixed, incomplete report travels to a service provider; a release
receipt can correct it by routine or intermediary delivery. Earlier refusals remain
history. The current browser host runs the actual Rust core as WebAssembly.

## Build and run

Install Rustup from [official Rust sources](https://rust-lang.org/tools/install)
and use Node 24 with npm. `rust-toolchain.toml` pins Rust **1.99.1**, rustfmt,
Clippy, and the WebAssembly standard-library target. The Rust version was verified
on 2026-10-24 against the [stable channel](https://static.rust-lang.org/dist/channel-rust-stable.toml)
and [official releases](https://blog.rust-lang.org/releases/).

The only direct web tooling dependencies are exact pins: **TypeScript 7.0.2** and
**Vite 8.3.5**, verified against their npm registry `latest` tags on 2026-10-24.
`package-lock.json` locks their normal transitive dependencies. No frontend
framework is used. Default Core and both browser runtime dependency graphs use
only the standard library; native snapshot tooling opts into serialization below.

For a clean, exact-commit source export, dependency-isolation checks and the
supported single-host qualification boundary, see the
[reference package workflow](docs/reference-package.md).

The opt-in [configured-host diagnostic profile](docs/configured-host-diagnostics.md)
measures actual native operations and process memory against frozen provisional
evaluation targets. It does not establish browser or human acceptance.
The [first diagnostic record](docs/configured-host-diagnostic-results.md) preserves
its source, aggregate results and the process-memory interpretation caveat.

```sh
npm ci --ignore-scripts
npm run build
npm test
npm run preview
```

Open the printed local URL. `npm run build` compiles the example's Rust WebAssembly
module, runs a separate TypeScript typecheck, then builds static `dist/` with Vite.
The authored web source and tests are TypeScript; browser JavaScript is generated.
For development, run `npm run build:wasm` once, then `npm run dev`. Rust changes
require rebuilding the module. Opening HTML directly as a file is unsupported.

The ordinary page contains only dialogue, choices, explicit fictional time, and
learned journal entries. Reading and choosing take no time; only the advance-time
choice moves the clock. Restart discards all fixture state. The ordinary convoy
page can explicitly download/load a bounded checkpoint; comparison and repair
pages still restart from a fixture. No external service calls, telemetry, or model
calls are included. The bounded example stops time advancement at hour 1000;
restart starts a new session.

`/developer.html` runs the same fixture with an additional read-only causal trace.
Do not use that page for playtests. The ordinary page does not read the trace.
Client-side source and WebAssembly inspection are not a secrecy boundary.

## Validate

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --all-targets --locked --offline
cargo run --example convoy --locked --offline
npm run build
npm test
node --test historical/js-g1/simulation.test.mjs
```

Rust tests cover permitted perspectives, immutable records, delivery order,
action acceptance/rejection/confirmation, bounded trace, repeat actions, history,
and reset. TypeScript tests instantiate the compiled Rust module and test its
ABI and both interaction paths. Production checks compare the exported binary
with the compiled Rust output and exclude the historical JavaScript simulation.
A Vite build alone is not a TypeScript check; `npm run typecheck` is independent.

Manual paths from restart:

1. Hold convoy; ask Mara; advance three hours; visit Ivo (refusal); release;
   give receipt to Sela; advance one hour; return (service at hour 4).
2. Hold convoy; advance three hours; visit Ivo (refusal); release; advance three
   hours; return (service at hour 6). At hours 4 and 5 visits still refuse service.

Remote deliveries do not update player knowledge until an encounter. The receipt
and sending feedback are learned locally; Ivo's response is learned on a visit.
Check keyboard focus, errors, clock, choices, journal, return dialogue, and trace
separately. Human player-value validation is separate from automated correctness.

## Configurable reference convoy

Open `/configured.html` for the build-time authored profile, or
`/configured-developer.html` for separate read-only configuration and trace.
`example-web/convoy.toml` selects the observer, precise/incomplete retention,
bounded delivery delays, and restrict-until-correction or warn-and-continue
reaction. Schema 2 also selects Ivo's directed confidence in Sela's verification:
trusted preserves the faster correction route; unverified retains her account but
requires the gate receipt. Schema-1 configuration remains trusted and accepts its
original fields. Native and actual Wasm hosts share the generated Rust configuration;
legacy presets remain unchanged. Configured save/resume uses its own schema-2
checkpoint with profile identity and exact artifact checking.

See [the authoring guide](docs/configured-convoy.md) for supported fields, native
commands, validation errors and compatibility limits. The owner has selected MVP
implementation before participant evaluation; human/adopter validation remains
pending and is not implied by implementation or passing tests.

The configured Rust host also exposes provider lifecycle updates and read-only
managed delivery receipts. Available offscreen Ivo receives accounts, while
service execution waits for a loaded/available explicit step. Lifecycle updates
do not execute work or reveal remote knowledge in the journal; terminal delivery
rejections do not resend automatically. Legacy presets remain unmanaged. See the
[host lifecycle contract](docs/configured-convoy.md#configured-host-lifecycle).

Configured sessions also enforce [fixed operating limits](docs/configured-session-limits.md)
before mutation. Optional repetition leaves room for unfinished story work and a
final visit; typed capacity refusal preserves state and keeps save/reset usable.
`Core::usage()` is a privileged read-only accounting API, with derived counters
rebuilt on restoration; generic Core and legacy fixtures keep their existing
behavior and snapshot formats. These limits are not capacity claims for a future
larger game.

The separate [ordinary flags baseline](docs/flags-authoring-baseline.md) implements
the same closed all-available encounter for later authoring comparison, without
calling Core simulation. It reuses the validated input/presentation contract and
can be run as `convoy-baseline` in the native tooling package. Its narrower scope
excludes lifecycle, durable restoration and runtime admission. Observable-path
comparisons do not establish internal Core correctness or human authoring value.

## Convoy delivery comparison

`/trial-a.html` and `/trial-b.html` provide ordinary, matched trial pages for the
same Rust convoy scene. A uses the current three-hour routine and one-hour
intermediary correction; B uses three hours for both. Timing labels come from
the selected compiled condition. Initial report delivery, observations, policies,
permitted actions, historical records and fictional clock remain shared.

The existing default convoy stays in the current condition. Restart clears the
session while retaining the selected condition. Developer traces remain separate:
`/developer.html` for current and `/convoy-control-developer.html` for control.
Use the [short facilitator guide](docs/convoy-comparison-guide.md) for trial order,
questions and expected timing checks. This slice is a bounded timing comparison,
not a completed evaluation or a claim of measured superiority.

The example-owned condition enum has only two compiled choices; no runtime
configuration language or editable delays are introduced. An optional convoy
selection capability starts a fresh fixture and exposes condition/delay metadata.
Invalid IDs leave state unchanged. The generic player ABI remains version 1;
legacy default loading does not require this extension.

Native examples can exercise each route, including the same causal trace as Wasm:

```sh
cargo run --example convoy --locked --offline -- --condition current --route intermediary --trace
cargo run --example convoy --locked --offline -- --condition control --route routine --trace
```

Tests cover all four combinations, actual delivery rather than creation time,
unchanged remote journal/choice projections, preserved accounts/refusals,
condition-preserving restart, and native/Wasm parity. No human results are
invented. Browser interaction and accessibility still require browser review.

## Matched memory and delivery study

The ordinary `/memory-delivery-a.html` through `/memory-delivery-d.html` pages
run four compiled Rust profiles: precise/immediate, incomplete/immediate,
precise/scheduled, and incomplete/scheduled. All share the same observed strap
inspection, action vocabulary, service policy and layout. Precise retention
preserves observed inspection context; incomplete retention omits it with
explicit uncertainty. Neither profile obtains hidden motives or later events.

Immediate delivery settles at the current hour inside the triggering action.
Scheduled delivery uses three hours for the initial report and routine correction,
and one for Sela. Only arrived evidence affects Ivo. Precise conditions can keep
service available throughout; the encounter explains received context without
inventing a restriction or confirmed restoration. Remote arrivals do not update
the player journal or enabled choices. Restart retains the chosen profile.

The separate `/memory-delivery-developer.html` offers condition selection and a
read-only trace; condition changes restart the fixture. Do not use it for player
trials. Study pages have no checkpoint UI. Existing default/current/control pages,
the condition-v1 capability and convoy checkpoint interpretation remain unchanged.
A separate optional example-owned study capability rejects unsupported selections
and metadata; it does not change the generic Core or player ABI.

The native example accepts the same compiled profiles and an optional bounded
choice list for reproducible validation:

```sh
cargo run --example convoy --locked --offline -- \
  --study incomplete-immediate --actions 1,2,5,3,5 --trace
```

`--actions` accepts at most 2,128 fixed action IDs (`0` restart through `6`
advance). It prints each attempted result/hour/dialogue, final learned journal,
choice availability and an optional trace. This is a fixture input harness;
there are no expressions, scripts or arbitrary policies. Without `--actions`,
the existing narrated routine/intermediary path runs; `--condition` and `--study`
cannot be combined.

See the [facilitator guide](docs/memory-delivery-guide.md) for matched procedures,
counterbalanced order, questions and expected results. These pages enable the
required comparison; they supply no human results or completed player-value gate.
Outside authoring work, a competent simpler authoring baseline and measured limits
remain to be evaluated. No new packages, runtime configuration language or
additional mechanics are included.

## Evidence in Ivo’s return encounter

Visiting Ivo now names the evidence actually cited by his last confirmed service
change and the fictional hour of that decision. A routine gate release receipt
and Sela’s verified receipt are distinct causes. When both arrive together, the
shared policy’s selected cause is reported; sending a later pouch cannot rewrite
an earlier decision. The hour describes the service decision, not record creation
or an inferred delivery timestamp.

Ivo explains only his received evidence and confirmed outcome. The ordinary page
learns this through a visit, with no remote journal update, hidden account basis,
or developer-trace read. Earlier refusals remain historical entries. If service
was restored before the first visit, he explains restoration without remembering
a refusal that never happened in an encounter. If service was never restricted,
he does not claim it was restored. Repeated visits retain the same explanation;
restart clears it and retains the selected delivery condition.

This is player-facing causal feedback for the existing incident, intervention
and return loop. It does not change service rules or establish human comprehension
or player value. Test those separately using the ordinary trial pages.

## Convoy checkpoint save/resume

On the ordinary `/` page, use **Save · download checkpoint** to keep a local JSON
file. After reloading the page, choose that file and select **Load file · replace
session**. A successful load replaces current progress. A rejected file leaves
the complete current session intact. Nothing is uploaded to a service, and no
autosave or browser storage is used. Keep the downloaded file yourself; Restart
does not delete it. Comparison, developer and repair pages have no checkpoint UI.

This is same-artifact save/resume for the deterministic, synthetic convoy fixture.
The TypeScript host records successful inputs and reconstructs a fresh instance of
the retained **actual Rust/Wasm module**. It does not implement a second simulation,
restore from a debug trace, or supply a generic core snapshot API. Replayed actions
have only fixture-local effects; this mechanism must not replay real external
transactions or stand in for external-effect idempotency.

The schema-1 checkpoint has exactly `magic`, `schema`, `exactWasmSHA256`,
`condition`, and `actions`. SHA-256 covers the exact bytes used to compile the
retained module; loading never refetches a possibly changed binary. A different
artifact or condition is rejected, with no cross-version migration promise.
Changing checkpoint interpretation requires a schema change. No expressions,
custom policies or scripting language are accepted. A valid edited checkpoint is
not authenticated proof of genuine play, and its digest is compatibility checking,
not a signature or anti-cheat mechanism.

All successful inputs matter, including repeated questions and visits; rejected
or unavailable choices are excluded. Restart begins a new history. At most
**4,096 successful choices** are saveable, with a **64 KiB input-file limit**.
Choice 4,097 can still execute, but saving becomes visibly unavailable until a
restart or supported load. History is never silently clipped. This cap bounds
checkpoint support; it does not bound all core history or solve queue limits.

Loading checks file size before reading, strict UTF-8/JSON, exact fields, schema,
artifact, condition and command bounds. Every recorded choice must succeed in an
isolated candidate, whose output is validated before replacement. Player choices,
Restart and overlapping loads are blocked until completion. Web Crypto SHA-256
is required for checkpoints; when unavailable, ordinary play remains available.

Tests compare uninterrupted and resumed runs before reports arrive, with a
correction pending, and after restoration. They cover both conditions/routes,
repeated loads/questions/visits, immutable historical refusals, confirmed decision
causes, equal-time/late arrivals, trace eviction, cap boundaries and atomic errors.
Native tests also compare the complete shared Rust state and each continuation.
This browser checkpoint remains fixture input replay. The separate native snapshot
API below captures state, including lifecycle and receipts; it does not extend this
browser format. No complete MVP gate is claimed. Browser file interaction and human
usability still require browser review.

For a private preview check: save after the hour-three refusal and release, reload,
load the file, and confirm time, dialogue and journal match. Send the receipt to
Sela, advance one hour and return; service should restore at hour four. Load the
same earlier file again and repeat. Try a foreign-build or malformed file while
partway through a different session and confirm that progress remains unchanged.

## Repair café example

Open `/repair.html` after building. For the native demonstration run
`cargo run -p deedwake-repair-web --example repair_cafe --locked --offline`. `/repair-developer.html` adds
the separate read-only trace. All data and actions are synthetic; this example
controls no real appliance and performs no external messages or transactions.

Three ordinary policy functions compose through the existing public core API:
inspection needs a received donor report; repair/readiness needs a confirmed
inspection and host execution; collection needs its own delivered readiness
record. Rejection of a repair request does not create a completed repair.

From restart: submit the donor report at hour 0; advance to hour 2; inspect;
request repair (rejected because the cable receipt has not reached the technician);
advance to hour 4 and retry (host confirms repair). Intake receives that readiness
at hour 5, and collection at hour 6. Ask intake or visit collection to learn what
each desk knows. Remote delivery arrivals change no learned journal entries or
choice availability; the explicit hour-1000 limit disables time advancement. Original accounts and the rejected request remain history.

The donor report reaches intake at hour 1 and the technician at hour 2. Inspection
findings reach intake one hour after inspection and collection two hours after.
Ready outcomes use those same relative delays. The synthetic cable stock receipt
reaches the technician at hour 4. UI labels render Rust state; no TypeScript
repair simulation is present. Both fixtures share the same scalar ABI adapter.

`src/lib.rs` rustdoc includes a minimal consumer example. Generate the public
reference with `cargo doc -p deedwake-core --no-deps --offline`; run its executable
snippet with `cargo test --workspace --doc --locked --offline`. In a local Rust
consumer, use `deedwake-core = { path = "../Deedwake" }` as a path dependency.
The library has no registry release. Scheduling a record twice creates two
trace deliveries but one known record; record order in a perspective is creation
order, not delivery order. Mutating the host remains separate from proposals.

## Build-time repair configuration

Configuration belongs to the repair café example; the generic core remains
independent of its TOML parser and example-specific fields.

Edit only `repair-web/repair.toml` to configure this example. All eight keys are
required; unknown keys, wrong types, duplicate keys, unsupported references,
invalid ranges, and clock overflow fail the build. There are no expressions,
source snippets, dynamic policy code, include paths, or external actions.

| Field | Supported value |
| --- | --- |
| `schema_version` | Integer `1` |
| `policy_set` | String `"repair_cafe_v1"`, selecting the existing compiled policies |
| `trace_capacity` | Integer `0..=1024`; zero disables trace retention |
| `report_to_intake_ticks` | Integer `0..=1000`, relative to submission |
| `report_to_technician_ticks` | Integer `0..=1000`, relative to submission |
| `technician_to_intake_ticks` | Integer `0..=1000`, relative to finding/readiness |
| `technician_to_collection_ticks` | Integer `0..=1000`, relative to finding/readiness |
| `cable_receipt_at_tick` | Integer `0..=1000`, absolute from restart |

Roles and the hour-1000 limit stay fixed. Zero delays deliver at the current tick.
A valid configuration need not complete every workflow before the fixed horizon;
late actions can leave deliveries beyond it. Runtime transition checks remain.
Policy records carry stable semantic codes; prose is rendered separately and is
not an eligibility predicate. Display text is not a configurable field here.

Validate with `cargo check -p deedwake-repair-web --locked --offline`. The build
script reports filename, one-based line/column, stable error code, and a field
name or parser explanation. For example, an unsupported policy reports
`[unsupported_policy] policy_set: policy_set must be repair_cafe_v1`, pointing to
its TOML value. Syntax errors may stop at the first error; independent semantic
errors are reported in fixed field order with byte spans. Invalid UTF-8 and file
read failures are explicit build errors.

`repair-web/build.rs` reads this fixed file and generates `repair_config.rs` only
under Cargo's `OUT_DIR`, with precise rerun inputs. It never edits tracked source.
The generated constructor uses the same std-only `Input`/`ValidatedConfig`
validator as direct Rust construction. Validation runs at initialization, with no
TOML parser or configuration interpretation during simulation. The default build
and source generator are deterministic; build-side parsing is not a runtime
loader. Native and Wasm hosts consume the same generated values.

The only added registry dependencies are exact `toml = 1.1.8+spec-1.1.0` and
`serde = 1.0.229`, plus their locked transitive dependencies. They are build/test
only; toml has only `parse`, `serde`, and `std` features enabled. `serde` enables
`derive`. `cargo tree -p deedwake-core -e normal` has no dependencies, and the
repair host's normal Wasm tree contains only the core. A few optional packages
remain represented in Cargo.lock but do not enter the active runtime graph.

Useful checks for humans and agents:

```sh
cargo test -p deedwake-repair-web --test config_compile --locked --offline
cargo test -p deedwake-repair-web --lib --locked --offline
npm run build
cargo build -p deedwake-repair-web --example repair_cafe --locked --offline
node --experimental-strip-types --test tests/config-parity.test.ts
cargo run -p deedwake-repair-web --example repair_cafe --locked --offline -- --trace
```

The native CLI uses a fixed demonstration action sequence and reports unavailable
choices without claiming execution. It is not a planner for arbitrary timings.
Generated/direct and native/Wasm checks compare the actual configuration, whereas
existing browser scenario regressions use the checked-in default timing expectations;
intentional timing changes may require updated scenario expectations. Reset retains
the selected configuration. Test variants cover changed and zero delays, trace
limits, correction/history, and rejection without false completion.

For a small release measurement run
`cargo run -p deedwake-repair-web --example repair_config_measure --release --locked --offline`.
It reports mean fixture startup and repeated intake-query times for five trials,
comparing direct construction with generated construction of the same values.
It excludes compilation and TOML parsing. Results depend on the host and workload;
this is not a zero-overhead or general performance guarantee.

## Repair developer tool

The host-only `repair-tool` checks configuration and runs five compiled Rust
scenarios against the same repair scene used by the native and WebAssembly hosts.
It builds independently of `repair-web/build.rs`, so an invalid checked-in
`repair-web/repair.toml` does not prevent the validator from building.

```sh
cargo build -p deedwake-repair-tool --locked --offline
./target/debug/repair-tool check repair-web/repair.toml
./target/debug/repair-tool check repair-web/repair.toml --json
./target/debug/repair-tool test repair-web/repair.toml --case delayed-recipients --json
./target/debug/repair-tool explain repair-web/repair.toml --case rejection-then-success
./target/debug/repair-tool --help
```

Use `target/debug/repair-tool.exe` on Windows. A fresh environment must first
fetch the pinned Cargo dependencies before using `--offline`.

`check` reuses the build-time parser and typed validator. `test` and `explain`
execute a fresh, isolated fixture using exactly the supplied validated values;
they do not rewrite configuration or choose timings that make a case pass.
The allowlisted cases are `delayed-recipients`, `rejection-then-success`,
`equal-time`, `repeated-actions`, and `reset`. Different report delays are needed
for `delayed-recipients`; `rejection-then-success` needs the cable receipt after
the technician's report; `equal-time` needs matching report delays and matching
technician-to-desk delays. A valid configuration can fail a scenario expectation
or leave insufficient time to complete it before tick 1000. Failures identify
the case, step, tick, assertion code, expected result, and actual result.

Both readable text and `--json` use exit codes: `0` success, `1` failed scenario
expectation, `2` configuration/read error, and `64` invalid command arguments.
JSON has `schema_version: 1`, fixed report fields, source-located diagnostics,
steps, assertions, actual action states, desk knowledge, and a developer trace.
There are no wall-clock timestamps, random values, or debug-string parsing in that format.
The report's session identifies the final snapshot and its causal references;
reset steps from an earlier session have null record references rather than
linking to reused identities. Assertions have `passed`, `failed`, or
`not_checked` status. Missing or truncated ordering history is `not_checked`
with `ok: null`; exit `0` means no failed assertion, not that unavailable
history was verified.

Explanations distinguish prospective policy/host decisions from recorded action
states. A handled interaction is not proof of completed work. Actual statuses
include `proposed`, `host_rejected`, `accepted_unconfirmed`, and `confirmed`;
this synthetic host normally confirms accepted work within the same interaction.
The accepted-but-unconfirmed projection is tested against an actual core action,
without fabricating such an intermediate state in a completed scenario.
Read-only inspection does not advance time, create records, or change knowledge.
All tool output is privileged developer information, separate from the ordinary
browser journal. Trace retention and dropped entries are reported explicitly.

Inputs are regular files of at most 65,536 bytes, with bounded argument sizes.
Cases take at most 2,128 interactions across at most two sessions, each ending
at tick 1000; trace retention is at most 1,024 entries. These are data and
simulation bounds, not hard deadlines for operating-system file I/O.
The tool accepts no user-authored action scripts, expressions, or policy code.

Only the host tool adds `serde_json = 1.0.154`, verified against official registry
metadata on 2026-10-25, with locked `itoa`, `memchr`, and `zmij` dependencies.
It reuses the existing TOML/Serde pins. The core and both browser simulation
crates retain their existing normal runtime dependencies. No new browser page
is needed for these command-line operations.

## Repair browser inspection

After `npm run build`, open `/repair-developer.html` with `npm run preview`.
The developer page shows the eight compiled configuration values, current
prospective decisions for inspection/repair/collection, their typed reasons and
permitted causes, actual action history, and trace retention. It reads the same
live Rust fixture as the player controls. It does not run a TypeScript policy
model, change configuration, or execute actions when inspected.

From restart, inspection has no proposal because the technician has no report.
Submit the report, advance twice, inspect, and request repair: the history shows
a host rejection. The prospective repair row now says
`no_proposal / prior_rejection_unchanged`, preserving the earlier rejection.
Advance twice more and repair: the new confirmed outcome appears alongside it.
Collection remains unavailable to its policy until that desk receives readiness;
advance twice and collect. Restart clears history and returns to tick zero while
retaining the compiled configuration. These timings describe the checked-in
default; the configuration table shows the values actually compiled into a build.

Prospective eligibility is distinct from actual execution. Cause timestamps are
record creation times, not delivery times; the cable receipt is recorded at zero
but is unavailable to the technician until its configured arrival. The ordinary
`/repair.html` page does not read these privileged inspection fields. Use that
page for player-perspective feedback. Client-side binaries remain inspectable;
this separation is a UI contract, not a secrecy boundary.

Repair Wasm adds the optional version-1 scalar capability
`dw_repair_inspect(section, row, column)`. Sections select metadata, compiled
configuration, prospective decisions, causes, or actual history. Invalid or absent
coordinates return `u32::MAX`. The decoder checks capability arity/version,
numeric bounds, codes, lifecycle fields, and absent coordinates before rendering.
This repair-specific protocol assumes at most two causes per decision and four
actual action rows; these are current fixture invariants, not generic core limits.
Changing them requires reviewing the protocol and decoder together. The original
player ABI remains version 1, and ordinary use does not require this capability.

`tests/inspection.test.ts` exercises the actual compiled Wasm boundary and
`tests/inspection-parity.test.ts` compares it with native CLI explanations and
outcomes. Neither test is a replacement simulation. Browser interaction,
accessibility, and layout still need a supported browser or human review.

## Browser scenario checks

On `/repair-developer.html`, select one of the five built-in cases and click
**Run check**. The shared Rust runner creates a fresh validation fixture using
the compiled configuration. It does not change the interactive encounter,
fictional clock, journal, trace, action history, or inspection above it.
The last separate result remains visible while you use the interactive controls;
another run replaces that result rather than continuing the previous case.

The report shows case assertions, their step and fictional tick, stable code,
expected and actual values, session identifiers, and trace retention. A failed
expectation is evidence to inspect, not automatically an engine bug. With the
checked-in configuration, `equal-time` fails before executing actions because
the report delays and technician-to-desk delays differ. The other four cases
pass. A valid configuration need not satisfy every case's assumptions or leave
enough time to finish before tick 1000. `not_checked` assertions stay explicit;
the browser labels a result with unavailable checks as incomplete, preserving
the CLI's existing exit semantics.

`examples/repair/scenarios.rs` owns the existing case logic and std-only result
types. The CLI keeps file access, parsing and JSON serialization; the Wasm host
keeps only the latest isolated run. CLI JSON schema v1, output and exit codes
are unchanged. The browser decodes Rust results rather than planning or testing
a second simulation in TypeScript.

The optional developer check capability has a finite allowlist of case IDs
1–5, numeric metadata/assertion/step accessors, and bounded UTF-8 assertion
fields. Invalid IDs leave both the previous result and interactive state intact.
There are at most 2,128 steps, 128 projected assertions, 1,024 bytes per text
field, and two sessions, each bounded at tick 1000. Ordinary player pages never
request this capability. No runtime TOML editor, user-authored action sequence,
expression evaluator, external dependency, or generic core extension is added.

`tests/checks-parity.test.ts` compares all five browser/Wasm reports with the
native CLI; `tests/checks.test.ts` covers isolation and invalid boundaries.
These run as part of `npm test` against the built Rust modules. Use the normal
Rust tests for additional configuration and runner coverage. Browser interaction
and accessibility still require a supported browser or human review.

## Core and host boundary

`src/lib.rs` contains domain-neutral identity, typed records, delivery, perspective,
policy evaluation, action, and outcome types. The host owns stable entity IDs,
explicit time, execution, and confirmations. Policies receive only permitted
records and propose requests. Confirmation appends a new outcome. Trace retention
is capacity-bounded; records and action history remain available in this small
session. Hosts must bound inputs for larger workloads. IDs belong to one session.

Convoy policy, dialogue, journal, and host state live in `examples/convoy/scene.rs`,
shared by the native example and `example-web` WebAssembly crate. The tiny scalar
ABI uses commands and read-only UTF-8 bytes; invalid commands, disabled choices,
and out-of-range reads have explicit error values. TypeScript only renders and
validates that boundary. There is no second TypeScript simulation. The original
JavaScript proof is retained under `historical/js-g1/`, outside production output.

## Identified inputs and retries

Trusted host adapters can opt into an experimental, bounded, session-local
accepted-input ledger:

```rust
use deedwake_core::{Core, Entity, InputId};
fn main() -> Result<(), deedwake_core::InputError> {
    let mut core = Core::with_input_capacity(32, 128);
    let first = core.observe_identified(InputId(1), Entity(7), "partial inspection".into())?;
    let retry = core.observe_identified(InputId(1), Entity(7), "partial inspection".into())?;
    assert!(!first.duplicate);
    assert!(retry.duplicate);
    assert_eq!(first.value, retry.value);
    Ok(())
}
```

`observe_identified`, `account_identified`, `schedule_identified`, and
`confirm_identified` take an `InputId` followed by the corresponding legacy
method's arguments. They return `InputReceipt<T> { value, duplicate }`:
`RecordId` for record-producing operations and `()` for scheduling. One ID
namespace covers these methods and managed scheduling below. Exact accepted
retries return the original
receipt without changing records, time, queue order, trace, or dropped-trace
counts. Reusing an accepted ID with different arguments or another operation
returns `InputError::Conflict`. Different IDs remain distinct even when their
text is identical; corrections need new IDs and append new records.

Accepted-ID lookup happens before capacity and live preconditions. A scheduling
retry therefore still succeeds after its due time or delivery, and a confirmation
retry returns its original outcome after confirmation. A fresh ID confirming an
already confirmed action still fails the existing transition guard. Receipts
survive trace eviction, including when trace capacity is zero.
Keep the original arguments when retrying, including the absolute delivery tick;
recomputing a relative delay supplies changed arguments and conflicts.

`input_capacity` limits the number of accepted IDs, not the number of records,
pending events, payload bytes, or diagnostic entries. A full ledger returns
`InputError::CapacityReached` for new IDs before effects. Accepted IDs are never
evicted, and their retries remain available at capacity. Failed initial operations
return `InputError::Core(Error)` without reserving an ID or changing state, so
prerequisites can be corrected and the operation retried. Record-backed receipts
refer to retained immutable records; the ledger does not duplicate their strings.

Existing `Core::new(trace_capacity)` and legacy methods keep their behavior.
`Core::new` provides zero identified-input capacity; opt in with
`Core::with_input_capacity(trace_capacity, input_capacity)`. Legacy operations do
not reserve IDs or gain retry semantics implicitly. Use the identified variants
consistently at the supported retry boundary. Existing `Error` remains unchanged;
identified-input failures use the separate `InputError` type.

The host assigns IDs uniquely within one Core session. These IDs are not
authentication, authorization, or proof of session ownership. A fresh Core starts
a fresh namespace; delayed retries from an old session must not be forwarded to
it as new work. Existing record/action IDs also require the correct host/session.
There is no persisted acknowledgment ledger, cross-restart guarantee, or exactly-once
external effect. A receipt acknowledges a core input, not independent evidence
that an external world action ran: the host still validates and executes actions
before confirming outcomes. The native snapshot API below preserves this ledger
consistently with Core state; hosts must restore their corresponding state too.

Public integration tests cover the complete observation/account/delivery/outcome
path and its retries. Internal tests check the complete unchanged state for
retries and failures. Existing native/Wasm, checkpoint, CLI and production tests
remain regression coverage; the browser fixtures do not invoke the new methods,
so those tests are not presented as browser proof of this new API. No new UI,
Wasm ABI capability or lifecycle policy is added by identified inputs. Their
ledger is included in the optional native snapshot API below.

## Actor lifecycle and execution eligibility

The experimental, opt-in lifecycle contract keeps host residency separate from
availability. `Core::with_lifecycle_capacity(trace_capacity, input_capacity,
entity_capacity)` enables a bounded registry. Existing constructors provide zero
entity capacity; all legacy methods remain unmanaged and keep their behavior.
Use the guarded methods consistently for the supported lifecycle boundary.

The host explicitly calls `register_entity(entity, residency, availability)`.
`Residency` is `Loaded` or `Unloaded`; `Availability` is `Available`, `Unavailable`
or `Incapable`. Update an existing identity with `set_entity_state`, using
`EntityState::Present { residency, availability }` or `EntityState::Retired`.
`entity_state` is privileged host inspection and is not supplied to policies.

Registration never silently creates a second person under an existing ID.
Duplicate registration fails, and retirement is terminal for this Core session.
Tombstones retain identity/history and count toward `entity_capacity`; no identity
is evicted to make room. Repeating the retired state is harmless, but a retired
identity cannot return to Present. A fresh Core is a new session, not durable
identity storage or proof that an ID belongs to this session.

`evaluate_guarded` checks a registered, available actor before invoking its
policy. `accept_guarded` checks the actor again before accepting a proposal.
`check_execution(action, mode)` is a read-only check of an accepted action and its
actor's current eligibility. `ExecutionMode::Direct` requires loaded residency;
`DelegatedAbstract` can permit an unloaded, available actor. That explicit mode
is not permission from the toolkit: the host must authorize the particular
abstract operation and validate its own current mechanics and target conditions.
Unloaded does not mean unavailable, dead, or safe to act without delegation.

For example, this synthetic host owns a counter; the core records the outcome
only after the host changes it:

```rust
use deedwake_core::{Availability, Core, Entity, ExecutionMode, LifecycleError,
    Proposal, Residency};
fn main() -> Result<(), LifecycleError> {
    let mut core = Core::with_lifecycle_capacity(32, 128, 6);
    let worker = Entity(7);
    core.register_entity(worker, Residency::Loaded, Availability::Available)?;
    let action = core.evaluate_guarded(worker, |_| Some(Proposal {
        request: "advance host counter".into(),
        causes: vec![],
        rule: "Host permits a synthetic counter step".into(),
    }))?.expect("the example policy proposes a step");
    core.accept_guarded(action)?;
    let mut host_counter = 0;
    core.check_execution(action, ExecutionMode::Direct)?;
    host_counter += 1;
    core.confirm(action, format!("Counter is {host_counter}"))
        .map_err(LifecycleError::Core)?;
    assert_eq!(host_counter, 1);
    Ok(())
}
```

The supported host operation is serialized: check eligibility and domain/target
preconditions, execute, then confirm. A guard is a momentary check, not a
reservation, executor, concurrent transaction, or exactly-once external effect.
The core cannot infer target permissions from freeform proposal text. A host
that bypasses the guarded methods also bypasses their lifecycle guarantees.

Temporary unavailability or incapacity blocks guarded progression but leaves
Proposed and Accepted actions intact. Recovery never executes them automatically;
the host deliberately rechecks and continues. Retirement permanently blocks new
execution but also leaves pending statuses intact: Accepted is not proof that
work never ran. The host explicitly rejects known-unexecuted work or retains it
unresolved until reconciled. There is no automatic cancellation or expiration.

Confirmation remains a factual acknowledgment. An already completed action can
be confirmed after a lifecycle change; previously accepted identified-input
receipts still acknowledge their original result. Lifecycle metadata creates no
observations, accounts or NPC knowledge, and does not erase history or receipts.
Failures use the separate `LifecycleError` type, preserving existing `Error`,
`Event` and `Status` enums. Registration capacity is an identity count, not a
bound on total record storage, pending deliveries or payload bytes.

The actor guards do not manage delivery. Legacy scheduling and delivery remain
unmanaged, including for unavailable recipients; opt-in managed delivery is
described below. Native Core/host snapshots are described below; there is no
browser lifecycle UI.
Public Rust host tests exercise this API directly. Legacy condition/study browser
fixtures remain unmanaged; the configured browser uses the managed host integration
with everyone initially loaded and available and no lifecycle controls. Native
lifecycle tests cover host updates; default native/Wasm/checkpoint parity does not
prove browser lifecycle interaction. No complete MVP gate is claimed.

## Managed delivery and lifecycle

The experimental managed-delivery API checks recipient lifecycle at the declared
due tick. It uses the existing lifecycle registry, `InputId` namespace and
`input_capacity`; it adds no new identifier, constructor or capacity setting.
The scheduling receipt acknowledges accepted input, **not successful arrival**.
Inspect `delivery_receipt(input)` separately for the current transmission result.

```rust
use deedwake_core::{Availability, Core, DeliveryMode, DeliveryState, Entity,
    InputId, Residency};
fn main() {
    let mut core = Core::with_lifecycle_capacity(16, 8, 1);
    let sender = Entity(1);
    let recipient = Entity(2);
    core.register_entity(recipient, Residency::Loaded, Availability::Available).unwrap();
    let report = core.observe(sender, "partial inspection".into());
    let input = InputId(1);
    let accepted = core.schedule_managed_identified(input, sender, recipient,
        report, 3, DeliveryMode::LoadedRecipient).unwrap();
    assert!(!accepted.duplicate);
    assert_eq!(core.delivery_receipt(input).unwrap().state, DeliveryState::Pending);
    core.advance_to(3).unwrap();
    assert_eq!(core.delivery_receipt(input).unwrap().state,
        DeliveryState::Delivered { at: 3 });
}
```

`DeliveryReceipt` retains the original `Delivery`, `mode`, and `state`:
`Pending`, `Delivered { at }`, or `Rejected { at, reason }`. The query is privileged
host inspection; it returns `None` for unknown IDs and IDs accepted by other
operations. It creates no character observations or knowledge.

A new managed transmission requires a registered, nonretired recipient. It may
be queued while that recipient is temporarily unavailable, incapable or unloaded.
At the due tick, the current lifecycle state controls receipt:

| Recipient at due time | LoadedRecipient | AbstractReceipt |
| --- | --- | --- |
| Loaded and available | Delivered | Delivered |
| Unloaded and available | Rejected: Unloaded | Delivered |
| Unavailable | Rejected: Unavailable | Rejected: Unavailable |
| Incapable | Rejected: Incapable | Rejected: Incapable |
| Retired | Rejected: Retired | Rejected: Retired |

`AbstractReceipt` is the host's explicit choice to model offscreen receipt, not
permission inferred by the toolkit. The host still supplies valid transport and
game-specific conditions. Sender lifecycle is not a delivery-time condition:
retiring the sender after dispatch does not cancel an accepted transmission or
change the immutable record it carries.

This is a **reject-at-due** policy. Rejection is permanent for that transmission,
even if the recipient recovers later. It grants no knowledge, records no successful
`Delivered` event, and fabricates no observation, action or outcome. Its typed
rejection reason remains available in the receipt even with no retained trace.
A deliberate resend uses a fresh input ID. There is no automatic deferral, retry,
deadline, cancellation, expiration or redirection.

Every clock advancement uses the same internal dispatcher. Ordinary `advance_to`
therefore enforces managed checks; it cannot bypass them. Legacy transmissions
retain their unmanaged behavior, including delivery to unregistered or unavailable
recipients. Both kinds share stable `(due, insertion order)` processing; a rejected
entry does not block later entries. Terminal `at` values are the due tick, even
when the host advances across several ticks at once.
The host orders lifecycle updates against clock advancement explicitly. An
advance uses the state supplied at that boundary; it does not infer a history
of lifecycle changes across a time jump.

Exact retries match original sender, recipient, record, due tick and mode, before
checking current capacity, lifecycle, clock or sequence. They return the original
unit acknowledgment with `duplicate: true` after success or rejection, without
resending. Changing arguments or reusing a managed ID for another input kind
conflicts. Nonidentical submissions need fresh IDs, even if they carry the same
record. Admission failures reserve no ID and change no queue, knowledge or trace.
`DeliveryError::Core` wraps original scheduling errors; `Input` wraps input-ledger
errors; `UnknownRecipient` and `RecipientRetired` describe admission failures.

Each accepted managed transmission consumes one entry in the shared input ledger.
Terminal receipts are retained and never free that budget; unrelated identified
inputs consume it too. Capacity rejection happens before effects, while accepted
retries and read-only receipts remain available at capacity. This bounds managed
pending work and retained managed receipts, not legacy queues, total record
storage or payload bytes. IDs and receipts remain session-local and host-managed;
managed scheduling alone is not authentication or exactly-once external execution.
The native snapshot API below includes these receipts and their pending state.

Public Rust tests cover lifecycle changes before receipt, sender retirement,
mixed equal-time queues, rejection without knowledge, terminal retries, conflicts,
capacity, trace loss and ordinary-clock enforcement. Internal tests verify complete
state preservation on failures and retries. Browser fixtures still use legacy
scheduling; their native/Wasm/checkpoint/CLI/production tests are regression
coverage, not direct browser proof of this new API. No new UI, Wasm capability,
browser dependency is added by managed delivery. Native snapshots are described
below; no complete MVP gate is claimed.

## Native durable snapshots

`Core::snapshot(&limits)` captures a privileged, versioned `CoreSnapshot` data
transfer object. `Core::from_snapshot(snapshot, &limits)` validates an isolated
candidate before returning a new Core. The live Core never implements
`Deserialize`. Loading does not advance time, deliver records, evaluate policies,
execute actions or confirm results. Record/action positions remain session-local;
checked lookup helpers resolve restored host references without unchecked IDs.

```rust
use deedwake_core::{Core, Entity, SnapshotLimits};
let limits = SnapshotLimits::default();
let mut live = Core::new(16);
let record = live.observe(Entity(1), "limited inspection".into());
let position = live.record_position(record).unwrap();
let saved = live.snapshot(&limits).unwrap();
let candidate = Core::from_snapshot(saved, &limits).unwrap();
let restored_record = candidate.record_id(position).unwrap();
assert_eq!(candidate.record(restored_record).unwrap().source, Entity(1));
```

Snapshots retain the clock, immutable records, knowledge, actions and confirmed
outcomes, pending deliveries and insertion counter, lifecycle entries and retired
identities, complete identified-input ledger and managed receipts, retained trace,
drop count and capacities. Ordered entry lists preserve duplicate detection.
Legacy unregistered activity and evicted trace history remain supported. A valid
snapshot establishes structural consistency, not authenticity, permission to
execute, or proof of historical external activity.

Capture Core and host state at one serialized, quiescent boundary: no operation
may be executing or have an uncertain external result. Accepted work known to be
unexecuted may remain pending, and restoring it never executes it automatically.
A host with an uncertain external result needs reconciliation outside this API.
This is not a durable external transaction or exactly-once execution protocol.

The native Repair tool stores one envelope containing Core and the matching
validated configuration, flags, references, action history, journal and dialogue.
Its synthetic actions are synchronous and have no external effects. Restore
validates both halves before replacing the whole session. Core schema, host
schema, envelope and example content compatibility versions must match; there
is no migration promise. The Repair host rejects times above hour 1000 while
Core retains its `u64` clock.

The default safety profile rejects more than 16,384 records, pending deliveries
or input entries, 8,192 actions, 4,096 lifecycle entries or trace capacity,
16,384 knowledge owners or 131,072 total knowledge/proposal/trace references, a 64 KiB string, or 4 MiB
of aggregate Core text. Saved capacities are bounded too. Export checks bounds
before copying; nothing is silently truncated. The native codec additionally
limits the actual file read to 8 MiB and rejects JSON nesting beyond 64 levels
while retaining the codec’s own recursion guard.
These are initial safety ceilings, not throughput or latency guarantees.

Serialization is opt-in through `snapshot-serde`, using pinned Serde **1.0.229**
on DTOs only. The native tool uses pinned serde_json **1.0.154**. No version was
upgraded; the default Core and browser runtime graphs remain free of external
crates. JSON contains exact Rust integers; this native format makes no JavaScript
number-precision or browser save compatibility promise.

The smaller fixed Repair host additionally limits Core state to 256 records or
pending deliveries, 128 actions, six knowledge owners and 1,536 total references;
its identified-input and lifecycle capacities remain zero. Host history/journal
contain at most 128 entries each, with 4 KiB per string and 128 KiB total host
text. Configuration retains its existing trace limit of 1,024. These narrower
bounds support this example, rather than changing the generic Core limits.

Run one process to save with reports pending, then a separate process to resume:

```sh
cargo run -p deedwake-repair-tool --locked --offline -- session \
  --config repair-web/repair.toml --action submit --action wait \
  --save /tmp/repair-session.json --json
cargo run -p deedwake-repair-tool --locked --offline -- session \
  --load /tmp/repair-session.json --action wait --action inspect \
  --action repair --action wait --action wait --action repair \
  --action wait --action wait --action collect --json
```

`session` accepts only the existing named actions: `reset`, `submit`, `inspect`,
`repair`, `collect`, `intake`, and `wait`, at most 2,128 per invocation. Choose
exactly one `--config` or `--load` source; `--save` optionally replaces a local
snapshot. Readable and schema-1 JSON reports use the existing diagnostic shape.
Invalid session input, restore or save exits 2; invalid command usage exits 64.
The existing `check`, `test`, and `explain` contracts remain unchanged.

On Linux, the native writer uses a newly created sibling temporary file, synchronizes it,
then atomically replaces the target on the supported local filesystem. Errors
before replacement preserve an existing save; an error after replacement must
be reported as an uncertain durability result, not as proof the old file remains.
Concurrent writers, universal power-loss guarantees, migrations and browser
persistence are outside this slice.

## Native acceptance measurements

An opt-in ignored release test measures the existing native Repair host,
including pending-session save/read and continuation. It uses predeclared,
provisional revision-1 engineering budgets, three fixed configurations, actual filesystem persistence and the shared
Rust implementation. Ordinary tests do not assert wall-clock performance.

See [native acceptance budgets](docs/native-acceptance-budgets.md) for exact
boundaries, numeric caps and the clean-source command. The versioned JSON record
includes source/machine/filesystem provenance, raw samples and resource counts.
A failed timing budget remains a failed report; limits are not silently raised.
No production command, Core API, dependency or persistence format is added.

This measures one bounded native path. Browser rendering, live heap usage,
independent adoption and human player value remain separate acceptance work.

## Static export

Build from the intended commit with the pinned toolchain and lockfiles. Publish
only `dist/` as the static root, preserving `convoy.wasm`, `repair.wasm`, and
hashed `assets/`.
Serve `.wasm` as `application/wasm` and assets with their normal MIME types. All
HTML entries are included; omit `developer.html`, `repair-developer.html`,
`convoy-control-developer.html`, `memory-delivery-developer.html`, and
`configured-developer.html` when making a playtest-only copy.
No backend, runtime environment variables, or repository hosting metadata is needed.
An archive is a build artifact; consumers in another environment must transfer it
through a supported route or rebuild the same commit and verify the binary.

## License

Deedwake-owned code is licensed under the [Zero-Clause BSD license (0BSD)](LICENSE).
Third-party material retains its own license: the adapted Ponytail skill in
[`.agents/skills/ponytail/`](.agents/skills/ponytail/) remains under its existing
[MIT license](.agents/skills/ponytail/LICENSE), with its copyright and attribution
notices preserved.
## Native configured-Convoy restoration

The existing native tooling package now also provides `convoy-tool`. It saves
and restores the configured convoy's actual Core and host state together, including
lifecycle, managed receipts, semantic facts, confidence and pending deliveries.
Loading is inert and validated before replacement; it does not replay actions.
The package's default remains `repair-tool`, preserving existing commands.

See [native Convoy restoration](docs/native-convoy-restoration.md) for the
cross-process workflow, ordered host controls, supported synchronous boundary,
versioning, admission ceilings and Linux atomic-save limitations. Browser replay
checkpoints and native Repair formats remain separate and unchanged.
