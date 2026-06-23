# Instruction Ownership

This repository is a live Codex/Robdex control plane. Instructions should have
one canonical home and may be referenced from other surfaces when needed.

## Canonical Homes

- Global rules: `AGENTS.md`
  - Applies to every agent working in `/Users/robertsale/.codex`.
  - Owns repository-wide search boundaries and basename skill-script usage.
- Base/default Codex engineering behavior: `roles/hidden.md`
  - Owns default Codex engineering behavior, editing constraints, review stance,
    user updates, and final response discipline.
- Operator behavior: `roles/operator.md`
  - Owns operator runtime stewardship, direct engineering behavior, control-plane
    repair discipline, and the explicit boundary that operators cannot manage
    agent lifecycle.
- Orchestrator behavior: `roles/orchestrator.md`
  - Owns worker/QA lifecycle, merge gates, Requirements timing, blocker
    adjudication, and Robdex control-plane responsibilities.
- Worker behavior: `roles/worker.md`
  - Owns assigned local-worktree discipline, validation, Requirements Review
    proof, and local commit expectations for implementers. In the `.codex`
    Robdex flow, workers do not publish branches, open pull requests, or write
    local review artifacts.
- QA behavior: `roles/qa.md`
  - Owns user-story piloting, bug classification, retry discipline, and
    non-implementation boundaries.
- Designer behavior: `roles/designer.md`
  - Owns product-design judgment, hierarchy, visual reduction, screenshot
    discipline, and anti-slop review expectations for design tasks.
- Design proof requirements: `requirements/composables/design-non-negotiables.yaml`
  - Owns the Requirements-native design gate, screenshot evidence contract,
    scope contract, and non-text-only visual proof requirements.
- Requirements reviewer behavior: `roles/requirements-reviewer.md`
  - Owns adversarial comparison of structured claims against evidence.
- Skill usage: `skills/*/SKILL.md`
  - Owns when and how to use a specific local tool surface.
  - Skill docs should not redefine role responsibilities unless the skill is
    explicitly role-scoped.
- Privileged execution command shape: `skills/privileged-exec/SKILL.md`
  - Owns how agents respond to approval prompts, sandbox friction, and
    privileged-exec rejection.
- Robdex CLI usage: `skills/robdex-orchestrator/SKILL.md`
  - Owns the public `robdex` script surface and shared Robdex usage rules.
  - Role behavior stays in role files.
- Project-specific rules: project `AGENTS.md` files and project-local skills.
  - Own repo-specific workflow details outside this control-plane repo.

## Duplicated Or Conflicting Instructions

- Search boundaries appear in `AGENTS.md` and in higher-level prompts. Keep the
  canonical rule in `AGENTS.md`; other surfaces should refer to it rather than
  restating it.
- Editing constraints appear in `roles/hidden.md` and `roles/worker.md`.
  Keep generic editing rules in `roles/hidden.md`; keep only worktree-specific
  worker constraints in `roles/worker.md`.
- Build/test parallelization appears in base and worker instructions. Keep the
  global rule in `roles/hidden.md`; worker-specific proof requirements may
  repeat the practical consequence.
- Requirements workflow appears in `roles/orchestrator.md` and
  `skills/robdex-orchestrator/SKILL.md`. Keep behavior and timing in the role;
  keep command syntax in the skill.
- Design workflow duplication belongs in `requirements/composables/design-non-negotiables.yaml` and `skills/design-worker`; role files may require screenshot discipline but must not define a separate design gate.
- Privileged-exec command-shape rules belong in `skills/privileged-exec/SKILL.md`.
  Role files may say to use the skill, but should not duplicate every shell
  restriction.
- Command-parser instructions are deprecated. Retained references should be
  compatibility notes only, not active workflow guidance.

## Role Boundaries

Cleanup may reduce duplicated wording, but must not merge distinct roles:

- `requirements-reviewer` remains a non-implementing evidence reviewer.
- `qa` remains a non-implementing user-story pilot.
- `designer` remains a product-design specialist.
- `worker` remains the scoped implementer.
- `orchestrator` remains the worker/QA control plane and merge authority.
