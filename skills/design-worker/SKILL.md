---
name: design-worker
description: Use for orchestrating or executing screenshot-driven Flutter redesign work where workers generate reference images, capture atomic Design Lab Bun/WebView proof when available, run visual design review, and implement only after visual approval. [skill-hash:2c4d9f0]
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

For projects with a Design Lab, workers use one atomic capture command for
visual proof:

- `design-lab-capture --workdir <project-root> --story <story> --shell <shell> --fixture <fixture> --viewport <viewport> --out /tmp/<name>.png`

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
- Structural similarity is not enough. Design review must fail when the actual
  screenshot has the same sections but materially worse polish, harsher
  border/shadow/radius treatment, weaker spacing/density/hierarchy, broken or
  low-fidelity charts/data surfaces, or thin empty/copy states that make the UI
  feel unfinished compared with the reference.
- For Design Lab projects, `design-lab-capture` is the visual proof surface. It
  builds the web artifact, serves it ephemerally, captures a Bun/WebView
  screenshot, and cleans up automatically. `flutter test` screenshots are not
  acceptable as design-review merge-gate evidence unless the task explicitly
  says it is not a Design Lab task and accepts widget-rendered artifacts.
- Do not pass screenshot-script bypass flags through `design-lab-capture`.
  `--port`, `--out`, `--backend`, and readiness bypasses such as `--skipReady`
  are owned or forbidden by the wrapper because they defeat the atomic Design
  Lab proof contract. Alternate `--url` values are allowed for sanctioned
  web-client capture when the target page exposes the Design Lab readiness
  signal.
- Flutter tests may still prove behavior, logic, state, and widget contracts.
  They do not replace Design Lab screenshots for visual confirmation.
- Do not use `--update-goldens` by default. Design review proof should write
  temporary viewable artifacts to `/tmp`, not update tracked golden baselines.
- If the page scrolls, capture enough screenshots to understand the whole page before asking for a full-page redesign.
- Do not redesign the application shell/chrome unless the task explicitly includes shell work.
- If the design needs backend behavior that does not exist, finish the design with safe mocked data and report the backend gap for a separate worker.
- If a page is not available in Design Lab, wire the page body/renderer into
  Design Lab first or report a blocker. Do not fall back to Flutter tester pixels
  for design-review evidence.
- Do not use emulator, device harness, QA broker, persistent web servers, tmux, or container stack unless the task specifically requires runtime/device proof. Prefer `design-lab-capture` for visual proof.
- If runtime simulator capture is explicitly required for a designer/design-worker, use `$designer-runtime`; do not use `flutter-sim`, `flutter-drive`, or the managed reservation path.

## Design Lab Reference

Use `references/design-lab.md` when creating or operating a project's Design Lab.
