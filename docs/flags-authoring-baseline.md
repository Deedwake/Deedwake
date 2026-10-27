# Ordinary flags authoring baseline

`convoy-baseline` is a competent small implementation of the same closed,
all-available convoy encounter using flags and authored rules. It supplies the
comparison work product for later authoring evaluation. It is not a test double
used to establish Core correctness, a replacement engine, or evidence that either
approach is easier to author.

```sh
cargo run -p deedwake-repair-tool --bin convoy-baseline --locked --offline -- \
  --config example-web/convoy.toml --actions 1,2,6,6,6,5,3,4,6,5 --json
```

The baseline simulation in `examples/convoy/flags.rs` imports no Core types or
functions. It uses a fixed three-route pending array, explicit receipt flags,
loading/warning state, the fixed sent account, one current confirmed decision,
refusal memory and the learned journal. Route processing follows due time and
dispatch order; all due routes arrive before the authored service rule runs.
Only action 6 advances the clock, which stops at hour 1000. Restart clears the
fixture while keeping its configuration.

The common input harness reuses the existing pure validated configuration model
and TOML parser: observer, retention, delays, reaction and Sela confidence have the
same meanings. Both implementations deliberately share the authored presentation
contract. This holds the supported content edits and encounter presentation
constant. It also means the cost of creating that common validator/presentation
is not independently measured by this comparison; record that limitation in any
later authoring exercise.

An incomplete arrived account can restrict loading or issue a warning. A precise
account retains the visible inspection context without inventing an adverse
outcome. Sela's verification only resolves/prevents the issue when trusted and
received; a received gate receipt remains sufficient. Later correction preserves
past accounts and encounter entries. Repeated questions do not reroll testimony,
and remote receipts do not write the player's journal.

## Supported comparison and exclusions

The baseline covers the normal loaded/available encounter and supported authoring
variations. It does not implement Core's reusable lifecycle, identified-input
ledger, durable snapshots, runtime resource admission or external-effect safety.
Its terminal workflow has no save/load. Those omissions must be recorded rather
than treated as equivalent total integration cost. The actual Rust/Wasm browser
continues to be the playable reference; no cosmetic baseline UI is presented as
proof of the reusable engine.

The CLI accepts at most 2128 comma-separated compiled action IDs 0..6, plus one
`--config PATH`, optional `--json` and `--trace`. There are no expressions, arbitrary
scripts or a new test language. Configuration reads are capped at 64 KiB and use
the shared strict parser. Exit codes are 0 success, 1 unavailable action, 2
configuration/output failure, and 64 usage. An unavailable action is reported;
later explicitly listed choices can continue.

JSON reports use schema version 1 and `implementation: "flags_baseline"`. Steps
contain action, result, time and dialogue. The final player projection contains
time, dialogue, learned journal and enabled flags for actions 0..6. `--trace`
adds baseline-only internal flag/route inspection, never a Core causal trace.
Do not show that developer inspection during later player trials.

## Evidence procedure

Automated checks compare the observable encounter contract against actual
Rust/Wasm on controlled, all-available paths: ordinary and intermediary correction,
early correction, equal-time receipt, disabled repeats, retained history, restart
and clock bounds. Core's own correctness/persistence/lifecycle tests remain
separate. Agreement on these selected observations is not equivalence for all
worlds or a validation of internal Core semantics.

For later independent authoring work, give both versions the same changes:
observer, permitted retained context, delivery timing and supported reaction.
Record changed files, editing effort, mistakes, workarounds, assistance and any
core changes. Do not count an agent's implementation run as an outside human
exercise, invent timings, or infer superiority from source size or passing tests.
Lifecycle/persistence authoring requires a separately scoped matched comparison.
No human results, performance improvement or completed acceptance gate is claimed.
