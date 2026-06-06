---
name: design-worker
description: Use for screenshot-driven Flutter design work that must prove visual quality through Requirements-native claims, sanctioned Design Lab capture, and anti-slop evidence. [skill-hash:7b1c9e3]
---

# Design Worker

Use this skill for design-focused Flutter work that needs screenshot proof.

Load only the role reference that matches the task:

- Orchestrator assigning or merge-gating design work: `references/orchestrator.md`.
- Worker executing design work: `references/worker.md`.
- Design Lab setup or capture details: `references/design-lab.md`.

## Required Gate

Design completion is gated only by Requirements-native proof:

- Attach or require `design-non-negotiables` for design work.
- Final claims must include screenshot evidence paths, viewport/device, scope contract, reference image path when applicable, primary job statement, and anti-slop self-review assertions.
- Text-only visual review is not acceptable.
- Requirements reviewer verdicts are the review path.

## Screenshot Tooling

Use sanctioned capture tooling:

```sh
design-lab-capture --workdir <project-root> --story <story> --shell <shell> --fixture <fixture> --viewport <viewport> --out /tmp/<name>.png
```

Use `design-lab-capture` for projects with Design Lab. Use simulator/runtime capture only when the task requires runtime/device proof; load `$designer-runtime` for that path.
