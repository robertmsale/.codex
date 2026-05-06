# Design Worker Orchestrator Workflow

Use this when assigning, approving, or merge-gating a design worker.

## Assignment Shape

Give the worker:

- The exact screen, page, component, or flow to redesign.
- The product goal and audience.
- The boundaries: what may change and what must stay stable.
- Whether application shell/chrome may be changed. If not explicit, shell changes are forbidden.
- Any real data states that must be represented.
- Whether mocked data is acceptable for missing backend behavior.

## Pre-Implementation Gate

Require the worker to complete a two-artifact plan before implementation:

1. First delivered artifact: a generated reference design image.
2. Second delivered artifact: a Flutter test-rendered screenshot of the existing UI, produced by a test that can be rerun.

The worker may internally capture and view the existing UI before generating the reference image. The external approval gate remains: reference design first, existing UI render second, then orchestration approval or iteration.

## Review Process

Compare:

- Existing rendered UI screenshot.
- Generated reference design image.
- Task boundaries and product goals.
- Impeccable UI guidance relevant to the task.

Approve implementation only if the reference image is strong enough to guide useful work. If it misses the mark, send the worker back to generate another reference image before touching implementation code.

## Implementation Authorization

Once approved, tell the worker:

- Implement toward the approved reference image.
- Preserve controllers, state flow, and backend contracts unless the task explicitly authorizes deeper changes.
- Avoid shell/chrome changes unless authorized.
- Keep screenshots/test artifacts in `/tmp` unless golden assets are explicitly requested.

## Pre-Merge Visual Gate

Before merge approval, require:

- A fresh test-rendered screenshot of the implemented UI.
- A comparison against the approved reference image.
- A short written note covering intentional deviations.

Use visual judgment, not pixel-perfect matching. The question is whether the implementation strongly fulfills the reference direction and product goal.

## Backend Gaps

If the worker reports missing backend functionality:

- Let the design worker finish with mocked data if that produces a useful design.
- Assign a separate backend worker for the missing behavior.
- After backend and design work land, assign a follow-up integration worker to wire real data into the approved design.

Do not let the design worker invent backend behavior unless the task explicitly includes backend implementation.
