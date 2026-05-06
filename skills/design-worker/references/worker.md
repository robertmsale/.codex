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
- Compare it to the approved reference image.
- Report what matches, what intentionally differs, and any remaining design risk.

The goal is high-confidence visual alignment, not pixel-perfect identity.

## If Blocked

If screenshot tooling, image generation, or Flutter rendering is blocked:

- Report the exact command/test, expected result, and actual output.
- Do not switch to a device stack or broker path unless explicitly authorized.
- Do not replace the design process with prose-only speculation unless the orchestrator explicitly accepts that fallback.
