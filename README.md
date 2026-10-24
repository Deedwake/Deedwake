# Deedwake

Local G1 browser-text proof: **The gate receipt**. One convoy misunderstanding,
three NPCs, delayed reports and correction, and a faster evidence intervention.
No packages, backend, build step, external assets, telemetry, or runtime models.

## Run locally

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
