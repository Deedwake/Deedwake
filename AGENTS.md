# Deedwake project instructions

## Engineering approach

Read and apply `.agents/skills/ponytail/SKILL.md` for coding, review, dependency,
and implementation decisions. Use Ponytail Full by default. Honor an explicit
task-specific Lite, Ultra, or off request; use targeted Ultra when warranted to
challenge unjustified scope, then return to Full.

Read `.agents/skills/project-workflow/SKILL.md` when choosing planning,
implementation, correctness review, or complexity review roles.

## Working boundaries

- Inspect the repository and applicable instructions before changing files.
  Preserve project-specific setup and existing work.
- Use a normal Git worktree or feature branch for repository changes. Keep work
  local pending publication approval. Do not merge or push to main, release, or
  deploy without explicit authorization.
- Ask before project package additions or upgrades, unclear major decisions,
  or architecture changes. Do not infer a product stack from installed tools.
- Provision only existing pinned tooling and lockfiles. Preserve existing
  setup and maintenance scripts; verify both initial setup and repeat runs.
  Do not copy another project's paths or packages. Use explicit shell
  activation when needed; do not alter managed `CODEX_HOME`, `BASH_ENV`,
  trust, credentials, or security settings.
- Do not copy secrets between projects or expand security, billing, or access.
- Use repository-owned skills under `.agents/skills/`. Do not install a global
  CLI or ZIP setup, change managed-home trust, or launch nested Codex to test
  hosted roles. `.codex/agents` TOML files do not register hosted cloud roles;
  use explicit hosted delegation only when authorized and available.
- Verify changes reproducibly and report validation limits. Distinguish task
  container changes from saved-environment changes; never claim persistence
  without confirmation.

## Current project state

This repository contains a Rust core and Rust-backed TypeScript browser examples,
with Cargo and npm manifests, pinned tooling, and build/test commands in README.md.
Environment preparation does not authorize additional product implementation or
selection of a framework. Recheck the repository as the project evolves.
