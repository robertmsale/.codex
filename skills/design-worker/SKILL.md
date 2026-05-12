---
name: design-worker
description: Use for orchestrating or executing screenshot-driven Flutter redesign work where workers generate reference images, compare against rendered UI screenshots, run visual design review, and implement only after visual approval. [skill-hash:ddda03a]
---

# Design Worker

Use this skill when a worker is assigned a design-focused Flutter task and the workflow should produce visual reference artifacts before implementation.

This skill is for orchestrators and workers. Load only the role reference that matches your responsibility:

- Orchestrator: read `references/orchestrator.md`.
- Worker: read `references/worker.md`.

## Scripts

Workers use `design-review <reference-image> <actual-image> [context...]` before
requesting merge approval. The first image must be the generated reference; the
second image must be the actual rendered implementation.

Use `design-review-eval <case-id>` or `design-review-eval --all` only when
tuning the reviewer against the curated local eval assets.

## Required Quality Source

Use `$impeccable-ui` when judging or shaping the design direction.

Recommended dispatches:

- `impeccable-ui-dispatch impeccable shape critique polish`
- Add `overdrive` when the task explicitly asks for a high-ambition redesign.
- Add `arrange typeset colorize` when layout, typography, or palette are the main problem.

## Core Rules

- The reference image is direction, not a pixel-perfect contract.
- Default screenshot tests write temporary viewable artifacts to `/tmp`; do not add golden images to version control unless the task explicitly asks for golden tests.
- Do not use `--update-goldens` by default. A design-worker screenshot test should render and write PNG artifacts to `/tmp`, not update tracked golden baselines.
- If the page scrolls, capture enough screenshots to understand the whole page before asking for a full-page redesign.
- Do not redesign the application shell/chrome unless the task explicitly includes shell work.
- If the design needs backend behavior that does not exist, finish the design with safe mocked data and report the backend gap for a separate worker.
- Do not use emulator, device harness, QA broker, or container stack unless the task specifically requires runtime/device proof. Prefer Flutter tests that render widgets and write screenshots.
- If runtime simulator capture is explicitly required for a designer/design-worker, use `$designer-runtime`; do not use `flutter-sim`, `flutter-drive`, or the managed reservation path.
