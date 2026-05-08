# Design Worker Workflow

Use this when you are the worker executing a design-focused Flutter task.

## Starting Contract

Before implementation, clarify the design boundary:

- What screen, component, or flow is in scope.
- Whether the application shell/chrome is in scope. If not explicit, do not change it.
- What existing controllers, state, and data contracts must remain stable.
- What data states must be visually represented.
- Whether missing backend behavior may be mocked for design proof.

Use `$impeccable-ui` for design standards before generating or judging the reference.

## Capture Existing UI Context

Produce viewable screenshots with Flutter tests where possible:

- Prefer widget/test rendering that writes PNGs to `/tmp`.
- Use `pumpAndSettle` only when appropriate and bounded by the test’s normal behavior.
- Do not create or update golden assets by default.
- Do not use `--update-goldens` unless the orchestrator explicitly requests maintained golden baselines.
- Do not require emulator, simulator, container stack, or broker/device harness for this workflow unless the task requires real-device proof.
- If the task explicitly requires live simulator capture, load `$designer-runtime` and use `designer-drive` with the provided device ID.
- Do not use `flutter-sim`, `flutter-drive`, or managed broker reservation commands for design-worker simulator capture unless the operator explicitly assigns a QA broker-managed slot.
- If the page scrolls, capture multiple scroll positions so the full page is represented.

After each screenshot is written, view it so the model has the actual pixels in context before asking for a redesign.

## Generate The Reference Design

Generate a reference image before implementation.

The image-generation prompt should be self-contained:

- Say it is redesigning the attached/viewed UI.
- Include the product goal and tone.
- Include constraints from the task.
- Say which parts must not change, especially shell/chrome if out of scope.
- Ask for a strong, intentional interface, not a safe generic restyle.
- Mention that implementation will be best-effort Flutter, not pixel-perfect.

If the design task needs multiple states, generate enough references to cover the important states.

## Pre-Implementation Response Sequence

Your first delivered response should be the generated reference design image.

Your second delivered response should provide the existing UI screenshot artifact from the Flutter render test, including the output path and any important context about what it shows.

Stop for orchestrator approval before implementation.

## Implementation Rules

After approval:

- Implement toward the approved reference image.
- Keep controllers, state flow, navigation, and backend contracts stable unless explicitly authorized.
- Do not redesign the application shell/chrome unless explicitly authorized.
- Prefer strong visual hierarchy, deliberate typography, purposeful color, and meaningful spacing.
- Avoid generic AI UI tells: default purple gradients, decorative glassmorphism, gradient text, identical card grids, and random ornamental borders.
- If backend behavior is missing, use safe mocked data only for design proof and report the backend gap clearly.

## Pre-Merge Proof

Before review/merge:

- Rerun the screenshot-rendering test and write a fresh implemented UI image to `/tmp`.
- View the implemented image.
- Run `design-review <reference-image> <implemented-image> [context...]`.
- The context is mandatory for useful review, but it must be neutral. State what screen/region is being graded, not what verdict the reviewer should return.
- If the reference image is a composite with multiple pages or concepts, crop to the relevant page/region before running `design-review` whenever practical. If you cannot crop, state exactly which panel/region of the composite is in scope.
- Explicitly say whether shell, nav, app chrome, global headers, sidebars, breadcrumbs, device frames, and surrounding scaffold are out of scope.
- If the generated reference includes a custom shell but the task is page-content only, say: `Shell/nav/chrome in the reference is out of scope; grade only the page content surface inside the real Ezra shell.`
- State unavailable backend/product contracts that should not be required, such as fake rows/actions/results that were only visual placeholders.
- If fake live data is forbidden, say so explicitly. Ask the reviewer to grade locked/skeleton/unavailable/readiness states for geometry, rhythm, hierarchy, and polish rather than requiring populated fake rows/results.
- Tell the reviewer what neutral fidelity target matters, for example pane proportions, table/list density, inspector layout, non-clipping iPad layout, typography, card rhythm, page grammar, semantic cleanliness, workflow clarity, or shell restraint.
- Do not tell the reviewer your implementation is better, tasteful, principled, acceptable, close enough, intentionally improved, or already approved.
- Do not tell the reviewer what defect to find or what score/verdict to return. The prompt may say `Scope: Accounting overview page content inside the existing Ezra shell`; it must not say `Reward this calmer implementation` or `Ignore that the reference is overdone`.
- Include the full `design-review` verdict, page type classification, content verdict/score, shell verdict/score, full-reference likeness score, semantic cleanliness, slop/dashboardification notes, style drift notes, intelligent divergence, acceptable scope/product deviations, and required fixes in your report.
- The reviewer returns separate content and shell grades. If your task was page-content only, use the content grade for merge readiness and treat shell grade as diagnostic only. If your task was shell-only, use the shell grade.
- If `design-review` returns `FAIL`, fix the visual gaps and rerun the implemented screenshot plus `design-review` before requesting merge approval.
- If you believe a `FAIL` is wrong because the reviewer misunderstood an explicit task boundary, explain that boundary clearly and stop for orchestrator judgment.
- Report what matches, what intentionally differs, and any remaining design risk.

The goal is high-confidence visual alignment, not pixel-perfect identity.

## Design-Review Prompt Contract

The third argument to `design-review` should be a concise scope contract:

- Name the screen, page, component, or flow.
- Say whether content, shell, or both are in scope.
- Identify the exact reference region if the reference is cropped or composite.
- Identify the exact actual region if the implementation screenshot includes extra surrounding UI.
- State hard product constraints: no shell changes, no fake live data, unavailable backend behavior, locked/readiness states, or viewport/device target.
- State neutral review dimensions: product intent, page grammar, workflow clarity, semantic cleanliness, density discipline, visual hierarchy, style drift, shell restraint.

Good examples:

```text
Scope: Accounting overview page content inside the existing Ezra shell. Shell/nav/chrome are out of scope. Grade product intent, page grammar, pane proportions, table density, semantic cleanliness, visual hierarchy, and non-clipping iPad layout. Fake live QBO data is forbidden; locked/readiness rows should be graded for geometry and polish, not treated as required populated data.
```

```text
Scope: Nexus Orion Expansion Initiative overview dashboard, including the visible application shell. Grade dashboard structure, data hierarchy, product identity, semantic cleanliness, style drift, shell restraint, and workflow clarity.
```

Bad examples:

```text
This implementation is a principled restraint pass and the reference is overwrought. Reward the actual even though full-reference likeness is lower.
```

```text
The only problem is shell mismatch; ignore everything else and pass if the page content seems close.
```

If you need to explain why you disagree with a review, do it after the review
returns. Do not preload the reviewer with your conclusion.

## If Blocked

If screenshot tooling, image generation, or Flutter rendering is blocked:

- Report the exact command/test, expected result, and actual output.
- Do not switch to a device stack or broker path unless explicitly authorized.
- Do not replace the design process with prose-only speculation unless the orchestrator explicitly accepts that fallback.
