# Deedwake

A small deterministic Rust core and a browser-text convoy example: **The gate
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
framework or external Rust crates are used.

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
only `dist/` as the static root, preserving `convoy.wasm` and hashed `assets/`.
Serve `.wasm` as `application/wasm` and assets with their normal MIME types. Both
HTML entries are included; omit `developer.html` when making a playtest-only copy.
No backend, runtime environment variables, or repository hosting metadata is needed.
An archive is a build artifact; consumers in another environment must transfer it
through a supported route or rebuild the same commit and verify the binary.

## License

Deedwake-owned code is licensed under the [Zero-Clause BSD license (0BSD)](LICENSE).
Third-party material retains its own license: the adapted Ponytail skill in
[`.agents/skills/ponytail/`](.agents/skills/ponytail/) remains under its existing
[MIT license](.agents/skills/ponytail/LICENSE), with its copyright and attribution
notices preserved.
