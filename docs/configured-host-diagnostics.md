# Configured-host diagnostics — frozen profile 1

Question: what latency and whole-process memory does the existing bounded native
configured Convoy use on this disclosed host, including its actual CLI and local
persistence? This is diagnostic testing. **250 ms p95 and 64 MiB peak RSS are
provisional evaluation targets**, not adopted product guarantees, compatibility
commitments, a browser budget or complete MVP acceptance.

This profile, its compiled fixtures and measurement code must be committed before
the first sample. A target failure remains a failure of this profile; retain it
and investigate. Do not raise targets or discard slow samples to manufacture a
pass. A later changed profile needs its own revision and preserved earlier data.

## Target and measured boundaries

The selected environment is Linux 6.18.55 x86_64/glibc 2.41, reporting an AMD EPYC
9V74 physical CPU, five eligible logical CPUs, a four-CPU cgroup quota and 16 GiB
cgroup memory. The save filesystem is overlayfs. The report captures actual CPU,
memory, affinity, quota, kernel, filesystem, Python and compiler information;
changes must be disclosed. The physical core count is not usable parallelism.
Shared scheduling, throttling, warm caches and underlying storage are not
controlled. Fresh process is not cold disk.

Use pinned Rust 1.99.1 release builds and unchanged lockfiles. Custom Rust flags,
profile/compiler wrappers and custom build targets are rejected. External Cargo
configuration files require profile review before measurement; the driver does
not modify or silently ignore them. Compilation is
outside timing. Source must be clean and the driver builds the actual release CLI
and its test worker from that source, recording their hashes. Python 3's existing
standard-library `resource` module is measurement tooling only; no package,
interpreter installation or simulation runtime dependency is added.

Each latency sample measures launch through exit of one real Rust process with
`perf_counter_ns`, including spawning, parsing, actual host operations, JSON output
to a local capture file, and declared native file I/O/sync. Python interpreter
startup is outside that boundary. Each sample uses a fresh Python wrapper with
exactly one Rust child; Linux `RUSAGE_CHILDREN.ru_maxrss * 1024` records that child's
full-process peak RSS without polling races or cumulative peaks from earlier
children. It includes the native host, allocator, parser/DTOs, validation and
reporting; it does not attribute memory to Core alone.

**Every cell:** five excluded latency warmups, then 50 measured samples, in the
fixed order below. All 55 RSS values participate in the 64 MiB target. Latency
uses nearest-rank p95: the 48th ordered measured value. Retain raw warmups and
samples, median, p95, maximum, exits, functional failures and raw outputs.
The proposed targets reflect a quarter-second native operation and small-process
footprint goal for this tiny synthetic encounter, not conclusions from observed
Convoy performance. Internal restore duration is diagnostic only.

| Cell | Actual command/work and target |
| --- | --- |
| `visit` | Fresh CLI loads default held-at-hour-3 snapshot, visits and reports refusal; 250 ms p95 |
| `equal_due_batch` | Fresh CLI loads three routes due at hour 1, advances once and reports the real ordered receipts; 250 ms p95 |
| `save_pending` | Fresh CLI loads pending correction and saves a new native file including validation, JSON, file sync, rename and directory sync; 250 ms p95 |
| `fresh_load_pending` | Fresh CLI reads/validates/reports pending state with no existing host; 250 ms p95 |
| `live_restore_into` | Fresh Rust test process retains a loaded pressure host, calls actual `restore_into` with the valid old near-text-limit candidate and generates player output; 250 ms p95 for the full process |
| `catchup_to_1000` | Fresh CLI loads hour 3 pending correction, executes 997 ordinary waits then visits; parsing/reporting and all intermediate decisions included; 250 ms p95 |
| `capacity_refusal` | Fresh CLI loads repetition pressure, attempts another question and reports typed unchanged-state refusal; expected exit 1, 250 ms p95 |
| `old_text_load` | Fresh CLI validates/reports historical admitted Core text at 1 MiB minus one byte; 250 ms p95 |
| `old_text_save` | Fresh CLI reads that historical fixture and saves it unchanged; 250 ms p95 |

Every cell also has the provisional 64 MiB whole-process RSS target. The live
restore cell deliberately differs from CLI read: the old host remains alive
during candidate construction. Its full-process timing/RSS also include test
harness setup and canonical verification, so they are conservative process
measurements. The separately reported `inner_ns` measures only actual replacement
and player output, with no independent target.

## Frozen correctness and state fixtures

The worker uses actual shared Rust, strict typed configuration, Core/host snapshots
and file persistence. There is no Python or JavaScript simulation. All use Mara
as observer and the existing schema-2 vocabulary. Numbers below are initial,
routine and intermediary delivery ticks, respectively.

| Configuration | Behavior verified before timing |
| --- | --- |
| Default: incomplete, restrict, trusted, 3/3/1, trace 128 | Both correction routes; held-at-3 visit, pending correction, inert read and equivalent continuation |
| Precise/immediate: precise, restrict, trusted, 0/0/0 | Service remains available; no fabricated restriction/restoration |
| Confidence mismatch: incomplete, warn, unverified, 0/3/1 | Intermediary receipt alone cannot resolve warning; later gate receipt can |
| Equal-time: incomplete, restrict, trusted, 1/1/1 | Hold, release and handoff before advancing; declared receipt ordering and current-state settlement |
| Default lifecycle | Unloaded receipt followed by explicit loaded execution; separately unavailable-at-due terminal rejection survives recovery without resend |
| Repetition pressure, traces 1/128/1024 | Stop at actual configured admission; preserve rejected state and reserve correction/return/save/reset |
| Historical near-text fixture, trace 1024 | Full native factory accepts 1 MiB minus one byte; visit refusal is atomic and save/reset remain usable |

Historical admitted state is not claimed reachable through fresh bounded player
commands. Fixture source, action sequences and correctness expectations are frozen
in the compiled worker; exact generated configuration/snapshot hashes and boundary
resource counts are recorded before timing. All semantic cases receive untimed
checks; the nine declared performance cells are not a Cartesian benchmark of every
configuration. Trace, reference, record, text, queue and file ceilings remain the
existing contracts. Record actual serialized trace/file sizes and dropped-entry
counts; serialized bytes are not live memory measurements.

## Run and preserve evidence

First run the affected normal correctness suite and measurement helper self-tests.
After a clean profile commit, activate the existing pinned toolchain/cache and run:

```sh
python3 scripts/qualify-convoy.py --self-test
python3 scripts/qualify-convoy.py --output /absolute/new/directory/outside/checkout
```

The driver refuses an existing output directory, builds before timing, runs fixture
verification, then records 450 measured and 45 warmup Rust processes. Each raw
sample is appended and synced to `raw-samples.jsonl` before functional validation.
The report preserves partial evidence if an execution/functional error occurs.
`report.json` distinguishes execution error, within provisional targets, and
outside provisional targets. An expected typed refusal is not a launch failure.
The driver exits 0 within targets, 1 outside targets, and fails on execution error.
Raw stdout/stderr, source/binary/fixture hashes and reports stay outside Git.

No timing threshold is asserted in ordinary tests. These results cannot qualify
browser input-to-paint, file dialogs, keyboard/accessibility, cold storage,
independent authoring, human value, other targets or larger worlds. Actual browser
qualification remains separate; WebAssembly page size alone omits JS objects,
replay arrays, DOM and other browser memory. No Site or transfer retry is part of
this diagnostic workflow.
