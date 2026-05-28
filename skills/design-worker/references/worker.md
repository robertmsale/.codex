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

For projects with `clients/design_lab`, produce visual proof through Design Lab
Bun/WebView screenshots:

- Capture screenshots with `design-lab-capture`. It builds the Design Lab web
  artifact, starts an ephemeral local static server, runs the project's
  `npm run bun:shot`, and shuts the server down automatically.
- Do not pass `--port`, `--out`, `--backend`, or readiness bypasses such as
  `--skipReady` through `design-lab-capture`. Those flags bypass the sanctioned
  build/serve/readiness contract and can produce blank Flutter loading frames.
  Alternate `--url` values are allowed for sanctioned web-client capture when
  the target page exposes the Design Lab readiness signal.
- Do not start persistent web-server sessions, tmux panes, or manual
  start/shot/reload/stop loops for merge-grade proof.
- Do not use Flutter tester pixels as visual proof for Design Lab work.
- Do not create or update golden assets by default.
- Do not use `--update-goldens` unless the orchestrator explicitly requests maintained golden baselines.
- Do not require emulator, simulator, container stack, or broker/device harness for this workflow unless the task requires real-device proof.
- If the task explicitly requires live simulator capture, load `$designer-runtime` and use `designer-drive` with the provided device ID.
- Do not use `flutter-sim`, `flutter-drive`, or managed broker reservation commands for design-worker simulator capture unless the operator explicitly assigns a QA broker-managed slot.
- If the page scrolls, capture multiple scroll positions so the full page is represented.

Use `flutter test` for behavioral assertions when appropriate, but not as
merge-grade visual evidence for a Design Lab task. If the page is not available
in Design Lab, first wire the page body/renderer into Design Lab or report a
blocker; do not substitute widget-test screenshots for design review.

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

Your second delivered response should provide the existing UI screenshot artifact
from Design Lab Bun/WebView capture when the project has a Design Lab, including
the output path and any important context about what it shows.

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

- Capture a fresh implemented UI image with `design-lab-capture` and write it to `/tmp`.
- View the implemented image.
- Compare the actual pixels against the approved reference before running
  review. Do not submit if the page only has the same sections but is visibly
  lower fidelity, harsher, more wireframe-like, more cramped, less polished, or
  less data-rich than the reference.
- Run `design-review <reference-image> <implemented-image> [context...]`.
- The context is mandatory for useful review, but it must be neutral. State what screen/region is being graded, not what verdict the reviewer should return.
- If the reference image is a composite with multiple pages or concepts, crop to the relevant page/region before running `design-review` whenever practical. If you cannot crop, state exactly which panel/region of the composite is in scope.
- Explicitly say whether shell, nav, app chrome, global headers, sidebars, breadcrumbs, device frames, and surrounding scaffold are out of scope.
- If the generated reference includes a custom shell but the task is page-content only, say: `Shell/nav/chrome in the reference is out of scope; grade only the page content surface inside the real Ezra shell.`
- State unavailable backend/product contracts that should not be required, such as fake rows/actions/results that were only visual placeholders.
- If fake live data is forbidden, say so explicitly. Ask the reviewer to grade locked/skeleton/unavailable/readiness states for geometry, rhythm, hierarchy, and polish rather than requiring populated fake rows/results.
- Do not use data or backend constraints to excuse missing dominant page composition. If the reference's main artifact is a map, calendar, table/list, dashboard canvas, inspector rail, editor canvas, or similar core surface, the actual must preserve that core surface as a polished real/empty/readiness state.
- Tell the reviewer what neutral fidelity target matters, for example pane proportions, table/list density, inspector layout, non-clipping iPad layout, typography, card rhythm, surface treatment, border/shadow/radius polish, chart/data fidelity, copy/data quality, page grammar, semantic cleanliness, workflow clarity, or shell restraint.
- Do not tell the reviewer your implementation is better, tasteful, principled, acceptable, close enough, intentionally improved, or already approved.
- Do not tell the reviewer what defect to find or what score/verdict to return. The prompt may say `Scope: Accounting overview page content inside the existing Ezra shell`; it must not say `Reward this calmer implementation` or `Ignore that the reference is overdone`.
- Include the full `design-review` verdict, page type classification, content verdict/score, shell verdict/score, full-reference likeness score, semantic cleanliness, slop/dashboardification notes, style drift notes, intelligent divergence, acceptable scope/product deviations, and required fixes in your report.
- The reviewer returns separate content and shell grades. If your task was page-content only, use the content grade for merge readiness and treat shell grade as diagnostic only. If your task was shell-only, use the shell grade.
- If `design-review` returns `FAIL`, fix the visual gaps and rerun the implemented screenshot plus `design-review` before requesting merge approval.
- If you believe a `FAIL` is wrong because the reviewer misunderstood an explicit task boundary, explain that boundary clearly and stop for orchestrator judgment.
- Report what matches, what intentionally differs, and any remaining design risk.

The goal is high-confidence visual alignment, not pixel-perfect identity.
Structural similarity is not high-confidence visual alignment. A design is not
ready for merge when it preserves section inventory but loses the reference's
production polish, visual hierarchy, spacing rhythm, chart/data credibility, or
perceived completeness.

For Design Lab projects, high-confidence visual alignment requires comparing the
reference against a Bun/WebView screenshot produced by `design-lab-capture`.
Flutter tester screenshots are not acceptable for this merge gate unless the
task explicitly opts out of Design Lab.

## Design-Review Prompt Contract

The third argument to `design-review` should be a concise scope contract:

- Name the screen, page, component, or flow.
- Say whether content, shell, or both are in scope.
- Identify the exact reference region if the reference is cropped or composite.
- Identify the exact actual region if the implementation screenshot includes extra surrounding UI.
- State hard product constraints: no shell changes, no fake live data, unavailable backend behavior, locked/readiness states, or viewport/device target.
- State dominant page artifacts neutrally when they are central to the reference: route map/navigation cockpit, calendar grid, table/list, dashboard data canvas, inspector/detail rail, editor canvas, etc.
- State neutral review dimensions: product intent, page grammar, workflow clarity, semantic cleanliness, density discipline, visual hierarchy, style drift, shell restraint.

Good examples:

```text
Scope: Accounting overview page content inside the existing Ezra shell. Shell/nav/chrome are out of scope. Grade product intent, page grammar, pane proportions, table density, semantic cleanliness, visual hierarchy, and non-clipping iPad layout. Fake live QBO data is forbidden; locked/readiness rows should be graded for geometry and polish, not treated as required populated data.
```

```text
Scope: Technician GPS Navigation page content inside the existing Ezra shell. Shell/nav/chrome are out of scope. The route map/navigation cockpit is the dominant in-scope page artifact. Grade whether the actual preserves route-map composition, guidance affordances, job context, hierarchy, semantic cleanliness, density discipline, and non-clipping iPad layout. Real GPS data may be unavailable, but unavailable/readiness states must preserve the designed map/navigation geometry and core affordances.
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
