# Deedwake

A small deterministic Rust core and browser-text examples. The original convoy example is: **The gate
receipt**. A fixed, incomplete report travels to a service provider; a release
receipt can correct it by routine or priority delivery. Earlier refusals remain
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
framework or external runtime Rust crates are used.

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
choice moves the clock. Restart discards all fixture state. No save/load,
external calls, telemetry, or model calls are included. The bounded example stops
time advancement at hour 1000; restart starts a new session.

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

## Static export

Build from the intended commit with the pinned toolchain and lockfiles. Publish
only `dist/` as the static root, preserving `convoy.wasm`, `repair.wasm`, and
hashed `assets/`.
Serve `.wasm` as `application/wasm` and assets with their normal MIME types. All
HTML entries are included; omit both `developer.html` and `repair-developer.html`
when making a playtest-only copy.
No backend, runtime environment variables, or repository hosting metadata is needed.
An archive is a build artifact; consumers in another environment must transfer it
through a supported route or rebuild the same commit and verify the binary.

## License

Deedwake-owned code is licensed under the [Zero-Clause BSD license (0BSD)](LICENSE).
Third-party material retains its own license: the adapted Ponytail skill in
[`.agents/skills/ponytail/`](.agents/skills/ponytail/) remains under its existing
[MIT license](.agents/skills/ponytail/LICENSE), with its copyright and attribution
notices preserved.
