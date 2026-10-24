# Deedwake

Local G1 browser-text proof: **The gate receipt**. One convoy misunderstanding,
three NPCs, delayed reports and correction, and a faster evidence intervention.
No packages, backend, build step, external assets, telemetry, or runtime models.

## Rust core (native)

The std-only Rust library is in `src/lib.rs`; domain-specific convoy behavior is
in `examples/convoy.rs`. The existing JavaScript browser proof below remains
unchanged and does **not** exercise the Rust core. Browser integration is pending
owner approval; no Rust-backed Site is claimed.

`rust-toolchain.toml` pins **1.99.1**, verified on 2026-10-24 against the
[official stable channel](https://static.rust-lang.org/dist/channel-rust-stable.toml)
and [release list](https://blog.rust-lang.org/releases/). With Rustup installed
from the official Rust distribution, run:

```sh
rustup toolchain install 1.99.1 --profile minimal --component rustfmt --component clippy
cargo fmt --check
cargo clippy --all-targets --locked --offline -- -D warnings
cargo test --all-targets --locked --offline
cargo run --example convoy --locked --offline
```

There are no crate dependencies. The example uses the actual Rust core and
prints both routine-correction and faster-intervention paths. Its tests cover
refusal history, repeated actions, learned-only journal updates, and reset.
Core tests cover restricted perspectives, immutable records, delivery order,
host acceptance/rejection/confirmation, and bounded trace retention.

The host owns `Core`, stable `Entity` identifiers, and explicit time. Policies
are ordinary functions receiving only a `Perspective`; they propose actions.
Only the host accepts or rejects requests and confirms completed execution.
Confirmation creates a new typed outcome; it never rewrites an earlier account.
Scheduling does not disclose content to the recipient until `advance_to` delivers
it. Equal-time deliveries use insertion order. Advancing drains the due batch;
the host chooses when to evaluate policies afterward. The example evaluates each
fictional hour. Policies are trusted functions, not a sandbox for untrusted code.

IDs are local to one core session. Construct a new core to reset; discard old IDs.
The trace is read-only and capacity-bounded, with an explicit dropped-entry count.
Records/actions remain available for this small session; bound host inputs for
larger workloads. Privileged record/trace access is for host diagnostics, not
player UI. No filesystem, wall-clock, network, or renderer is used by the core.

## Run the existing browser proof locally

From this directory, using existing Python 3:

```sh
python3 -m http.server 8000 --bind 127.0.0.1
```

Open `http://127.0.0.1:8000/` in a browser. Native modules need HTTP; opening the
HTML directly as a file is not supported. State lives only in this page session.
“Restart fixture” resets it completely. Only the explicit advance-time choice
moves fictional time; reading speed does not matter.

For the separate developer fixture, open
`http://127.0.0.1:8000/developer.html`. Its trace is read-only and records actual
outcomes, causes, and rules. Use the ordinary page for playtests: it has no trace
or developer link. This is UI separation, not secrecy against source inspection;
all simulation code is necessarily available in this local client-only proof.

## Checks

```sh
node --test
node --check app.mjs
node --check simulation.mjs
```

Tested with existing Node 24.19.0 and Python 3.12.14. Tests cover delayed-receipt
isolation, fixed testimony, immutable sent accounts, deterministic equal-time
ordering, earlier intervention, historical refusals, repeated actions, player
projection, trace causality, and restart.

Two manual paths, each from restart:

1. Hold convoy; ask Mara; advance three hours; visit Ivo (refusal); release;
   give receipt to Sela; advance one hour; return (service available at hour 4).
2. Hold convoy; advance three hours; visit Ivo (refusal); release; advance three
   hours; return (service available at hour 6). At hours 4 and 5, visits still
   refuse service. Earlier refusals remain history after restoration.

Check keyboard navigation, disabled completed actions, journal disclosures,
clock, return dialogue, and the developer trace separately. No human playtest
or player-value validation is claimed. Browser rendering and console checks in
this task container are blocked: installed Chromium aborts because its SUID
sandbox helper is misconfigured. No sandbox bypass or security change was made.

Scope is this bounded, local-only proof. G0 legal/risk review is unresolved:
no legal clearance or private-prototype exemption is claimed. Scoped guidance
and owner decisions remain necessary for unresolved activity boundaries before
external distribution or substantial expansion. No G2 comparison, M0 toolkit,
save/load, or publication is included.

## License

Deedwake-owned code is licensed under the [Zero-Clause BSD license (0BSD)](LICENSE).
Third-party material retains its own license: the adapted Ponytail skill in
[`.agents/skills/ponytail/`](.agents/skills/ponytail/) remains under its existing
[MIT license](.agents/skills/ponytail/LICENSE), with its copyright and attribution
notices preserved.
