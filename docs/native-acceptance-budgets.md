# Native Repair acceptance budgets — revision 1 (provisional)

Question: can the existing native Repair host complete its supported interaction,
pending-session restoration and bounded catch-up within these initial engineering
budgets on the disclosed machine? These are provisional engineering baselines,
not owner-approved product acceptance criteria. This is an opt-in measurement
run, not a new runtime feature, production guarantee or complete MVP gate.

These caps, workloads and sampling rules are declared before the first measured
run of this revision. Do not raise caps after seeing a failure and call that same
run a pass. A changed budget needs a new revision and a preserved earlier result.

## Budgets and measured boundaries

| Operation | p95 ceiling | Included |
| --- | ---: | --- |
| Representative inspection interaction | 5 ms | Real `Repair::act` inspection and player dialogue/journal string generation |
| Wait with a due delivery | 5 ms | Real host wait action, due delivery handling and player output generation |
| Complete supported sequence | 50 ms | Host initialization, rejection-then-success, collection and repeated intake questions with player output |
| Native session save | 100 ms | Existing snapshot validation, JSON encoding, sibling-file write, sync and atomic replacement |
| Native session read | 50 ms | Actual file read, JSON decoding and full Core/host validation |
| Catch-up to hour 1000 | 100 ms | Supported wait actions from a pending session through the existing horizon |

The initial target is the selected Linux x86_64 Cloud environment with pinned Rust
1.99.1, the documented Cargo release invocation and its disclosed save filesystem. The
report records the actual CPU, OS, available parallelism and machine memory
information. Shared-host contention and warm filesystem caches affect results;
this run neither isolates the CPU nor simulates a cold disk. Moving to another
machine requires its own recorded run, not an inferred performance claim.

Each of three fixed configurations receives three warmups and twenty measured
samples. Record every sample as integer nanoseconds, plus nearest-rank median,
p95 and maximum. Budget decisions use the p95 ceiling; retain the maximum and
failures too. Ordinary correctness tests have no hardware timing assertions.

The measured path excludes compilation, process launch, TOML parsing, full CLI
report formatting, test-only state inspection and browser/DOM rendering. Save
and read include the real existing persistence implementation. Measurements use
actual shared Rust; no alternative simulation or mocked storage supplies proof.

## Fixed workloads and safety bounds

All configurations retain `repair_cafe_v1` and the same synthetic roles/actions:

| Configuration | Trace capacity | Report to intake / technician | Technician to intake / collection | Cable receipt |
| --- | ---: | --- | --- | ---: |
| Default | 128 | 1 / 2 | 1 / 2 | 4 |
| Equal-time | 128 | 2 / 2 | 1 / 1 | 4 |
| Evicted trace | 1 | 1 / 2 | 1 / 2 | 4 |

At hour 2, inspection has completed and repair was rejected because the cable
receipt has not arrived. The saved session has pending deliveries. Loading must
leave the hour, full state and player output unchanged. Applying the same suffix
to restored and uninterrupted sessions must produce identical complete snapshots
and player output. The full sequence later confirms repair and collection;
repeated questions must preserve learned history. The equal-time configuration
checks declared ordering. Evicted traces expose dropped-entry counts.

Catch-up uses ordinary host wait actions to hour 1000. A further wait must return
the existing horizon result without altering state. The bounded workload must
not exceed 2,128 steps per session. Existing snapshot rejection/atomicity and
trace-limit regressions remain required ordinary checks; timing alone is not
correctness evidence.

The native snapshot contract admits at most 256 records or pending deliveries,
128 actions, six knowledge owners, 1,536 total references and trace capacity
1,024. The encoded input limit is 8 MiB. These are snapshot admission bounds,
**not runtime queue or heap caps**. The report records the actual workload's
record/action/pending high-water counts, retained/dropped trace entries,
serialized trace bytes and snapshot bytes. A small passing workload says nothing
about maximum-capacity latency or large populations.

Serialized bytes measure storage, not live heap usage. Heap/RSS, process startup,
browser/rendered latency and whole-game memory acceptance remain unmeasured.

## Reproduce

First run the ordinary correctness suite on the intended clean source revision:

```sh
cargo test --workspace --all-targets --locked --offline
```

Choose an existing writable directory on the filesystem you intend to measure.
Use a new absolute report filename outside the Git checkout; the test refuses
to overwrite an existing file. For example, after creating `/workspace/deedwake-acceptance-results`:

```sh
DEEDWAKE_ACCEPTANCE_REPORT=/workspace/deedwake-acceptance-results/native-repair-v1.json \
  cargo test -p deedwake-repair-tool --release --locked --offline --bin repair-tool \
  session::acceptance::native_repair_budgets -- \
  --ignored --exact --nocapture --test-threads=1
```

The test requires Linux, a release build and a clean Git checkout. It creates a
unique sibling work directory for real save/read operations and removes its own
work files afterward. The selected filesystem is recorded; it is not silently
replaced by an in-memory store. The report persists even when a timing budget
fails. A failed operation is not a fast successful sample.

The versioned JSON record identifies source commit/tree and cleanliness, package,
content and snapshot versions, Rust/target/release information, actual machine,
filesystem, fixed configuration, workload bounds, samples, correctness checks,
resource observations and failed budgets. Git provenance describes the checkout
at invocation; the binary does not embed a source revision or optimization flags.
Use the documented fresh Cargo command on that clean checkout, rather than
launching a stale test binary. Do not treat a caller label or a Cargo package
version as a substitute for the actual source revision. Missing machine
information must remain unavailable, rather than invented. Environment variables,
credentials, hostnames and user identities are not an evidence requirement.

## Interpret the result

This maintainer-run measurement partially advances the native operating-budget
requirements. It is not the outside developer exercise, a comparison against
ordinary flags/scripts, human player-value evidence, or browser performance
acceptance. The four-condition human study and its technically blocked private
preview handoff remain separate. No upload, Site change or distribution decision
is performed by this test.

Preserve each report with its exact source and build. Record any failed budget,
workaround or assistance; decide whether to simplify, repeat under disclosed
conditions, or revise a future budget. A passing native timing run is permission
to discuss only the measured native path and workload.
