---
name: project-workflow
description: Apply Deedwake's established model preferences and planning, implementation, correctness review, and complexity review roles when organizing engineering work.
---

# Project workflow

Use Ponytail Full by default. Use task-specific Ultra only when warranted,
then return to Full. Preserve the user's task scope and repository instructions.

## Model preferences

- Lead, implementation, and routine reviews: GPT-6.1-Sol, high reasoning.
- Planning and difficult decisions: GPT-6-Astra, high reasoning.

These are preferences for supported model selection, not a claim that reading
this file switches the active model. If selection is unavailable, report that
limitation when relevant; do not claim to have used a different model.

## Roles

- Planning: establish the smallest complete scope, reuse existing patterns,
  identify decisions needing approval, and specify useful validation.
- Implementation: make the smallest complete authorized change, preserve
  existing work, and run the checks appropriate to it.
- Correctness review: independently check behavior, callers, failure paths,
  security boundaries, and meaningful test coverage against the request.
- Complexity review: challenge unnecessary scope, dependencies, abstractions,
  and maintenance cost without cutting required behavior or validation.

Use explicit hosted delegation when the task and available tools authorize it.
Do not treat local `.codex/agents` TOML files as hosted role registration or
launch nested Codex processes to test hosted roles. Routine tasks need not
create separate agents for every role. Do not enforce custom concurrent-agent
caps; platform limits, permissions, and task-specific constraints still apply.
