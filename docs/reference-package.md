# Single-host reference package

This is a source reference for one deterministic, synthetic, single-process host.
It combines the reusable Core, configured convoy, native Core/host restoration,
bounded session admission, independent flags comparison and actual Rust/Wasm web
examples. It is not a registry release, engine integration, distributed service,
stable SDK or qualification for a larger world.

## Build a clean source copy

Use an exact reviewed commit, not a branch name that can move. From a Git checkout:

```sh
git rev-parse HEAD HEAD^{tree}
git status --porcelain
git archive --format=tar HEAD > ../deedwake-source.tar
sha256sum ../deedwake-source.tar
mkdir ../deedwake-reference
tar -xf ../deedwake-source.tar -C ../deedwake-reference
cd ../deedwake-reference
```

Require an empty status before packaging your changes. `git archive` exports
tracked committed files only; it includes the root license and bundled third-party
notices. It excludes Git metadata and local ignored build outputs. Do not copy a
working directory containing caches, credentials, generated Wasm or private files.
Keep the exact commit, tree and archive hash beside the archive. Do not describe
an archive from a dirty checkout as containing its uncommitted work.

The source archive intentionally contains no `node_modules`, `target`, generated
`public/*.wasm` or `dist`. Build these from the source. Preserve `LICENSE` and the
MIT notices under `.agents/skills/ponytail` when redistributing the source; the
third-party exception in the root README still applies.

## Required tools and dependencies

The reference uses Rust **1.99.1**, rustfmt, Clippy and the
`wasm32-unknown-unknown` standard-library target from `rust-toolchain.toml`.
The exercised native host is **Linux x86_64**, Node **24.19.0** and npm **11.9.0**.
The web tooling remains pinned to TypeScript **7.0.2** and Vite **8.3.5** by the
existing manifest/lockfile. A source archive does not install those tools.

Install the pinned tools from their official sources. Populate the locked Cargo
cache once with `cargo fetch --locked`; thereafter the commands below use offline
Cargo. `npm ci --ignore-scripts` installs exactly the npm lockfile without package
lifecycle scripts. No project package upgrade is part of this workflow.

```sh
npm ci --ignore-scripts
cargo fmt --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --all-targets --locked --offline
cargo test --workspace --doc --locked --offline
npm run build
npm test
node --test historical/js-g1/simulation.test.mjs
```

Run the build before web tests: production checks compare `dist` with actual
compiled Wasm. `npm run build` creates `public` itself, builds the Rust modules,
runs the separate TypeScript typecheck and emits static `dist`. It does not use a
second JavaScript implementation as evidence for Rust behavior.

Workspace host tooling opts Core into `snapshot-serde`. Check feature isolation
separately; a workspace-wide build alone cannot establish a dependency-free
default runtime:

```sh
cargo check -p deedwake-core --no-default-features --locked --offline
cargo test -p deedwake-core --lib --no-default-features --locked --offline
cargo check -p deedwake-core --features snapshot-serde --locked --offline
cargo tree -p deedwake-core --no-default-features -e normal --locked --offline
cargo tree -p deedwake-example-web --target wasm32-unknown-unknown -e normal --locked --offline
cargo tree -p deedwake-repair-web --target wasm32-unknown-unknown -e normal --locked --offline
```

The first dependency tree should contain only Core, and the browser normal trees
only their example crate and Core. TOML/Serde build dependencies and native
snapshot tooling remain outside those normal Wasm runtime graphs.

## Native host and browser boundaries

Run `cargo run -p deedwake-repair-tool --bin convoy-tool --locked --offline -- --help` for the native
configured host. For example, save pending correction, terminate the process,
then restore and continue with a separate invocation:

```sh
cargo run -p deedwake-repair-tool --bin convoy-tool --locked --offline -- \
  --config example-web/convoy.toml --action hold --action wait --action wait \
  --action wait --action visit --action release --action handoff \
  --save ../convoy-session.json --json
cargo run -p deedwake-repair-tool --bin convoy-tool --locked --offline -- \
  --load ../convoy-session.json --action wait --action visit --json
```

Native files restore validated Core and host state together without replay or
executing pending work. Loading is inert. The current format is same-version,
quiescent and consistency-checked, not authenticated. On the supported Linux local
filesystem, rename atomically replaces the destination. Failures before rename
preserve the previous file; directory-sync failure afterward reports committed
replacement with uncertain durability. Concurrent writers, migration and uncertain
external-effect recovery are unsupported. Privileged provider controls are host
operations, not ordinary player choices. See [the host contract](configured-convoy.md)
and [session limits](configured-session-limits.md).

Serve the complete `dist/` directory over HTTP with `npm run preview`. Use
`/configured.html` for the ordinary configured game, and keep developer pages
separate. The web game executes Rust/Wasm. Its checkpoint is bounded successful-
input replay into the exact retained Wasm artifact; it is **not** the native
Core/host snapshot format. Changing Wasm changes checkpoint compatibility even
when a source change appears unrelated. Repair/comparison pages retain their
documented fixture-only behavior. Browser source inspection is not a secrecy
boundary.

## Record qualification evidence

Keep an external evidence bundle with the source commit/tree/archive hash,
tool versions, host OS/architecture, commands, exit statuses and raw logs. Include
SHA-256 and byte sizes for every static file, the two release Wasm modules and
the installed license/notices. Hash the complete `dist`, not just the HTML. Preserve applicable distribution
notices beside the built export: the root 0BSD license, the pinned Rust toolchain
standard-library copyright/license files, and the installed Vite license for its
generated browser helper. These are distinct from build-only tool dependencies;
do not label all compiler or npm dependencies as shipped browser code.
Compare `dist/convoy.wasm` and `dist/repair.wasm` with their release outputs.
Record any intentionally varied configuration separately from the default build.

Automated qualification covers public Core contracts, native cross-process
restoration, strict input failures, capacity atomicity, actual native/Wasm paths,
checkpoint isolation, CLI compatibility and production-file identity. It does not
establish interactive DOM/accessibility behavior, browser download/upload UX,
power-loss durability on arbitrary filesystems, other native targets or large-world
performance. Browser interaction and private preview delivery may remain blocked
by the execution environment; disclose that instead of weakening sandbox controls.
The simpler baseline is for later authoring evaluation, not proof of Core internals.

This package advances implementation and reproducible host qualification. Actual
outside-authoring and player-value evidence remain later work under the owner's
MVP-first sequence; no completed human acceptance or product-wide performance
claim follows from passing these checks.
