# Design Worker Execution

Use this when you are implementing screenshot-driven design work.

## 1. Establish The Scope Contract

Before implementation, identify:

- screen, component, or flow in scope;
- content-vs-shell boundaries;
- target viewport or device;
- fake-data policy;
- primary job of the screen;
- reference image path when a reference exists.

If scope is unclear, ask. Do not redesign shell/chrome unless assigned.

## 2. Capture Visual Evidence

For Design Lab projects, use:

```sh
design-lab-capture --workdir <project-root> --story <story> --shell <shell> --fixture <fixture> --viewport <viewport> --out /tmp/<name>.png
```

Rules:

- Keep screenshot artifacts in `/tmp` unless golden assets are explicitly requested.
- Do not use `--update-goldens` by default.
- Do not pass wrapper-owned or readiness-bypass options through `design-lab-capture`.
- Do not replace Design Lab proof with Flutter tester pixels.
- If runtime/device proof is required, load `$designer-runtime` and use sanctioned simulator capture.
- If the page scrolls, capture enough screenshots to review the whole surface.

## 3. Implement With Product Honesty

- Share the same design-system/source components between Design Lab and client app where applicable.
- Preserve controllers, state flow, navigation, and backend contracts unless the task authorizes changes.
- Use safe mocked data only for Design Lab/reference proof or when the task explicitly allows it.
- Do not add decorative controls, fake metrics, dashboard panels, or inactive UI.

## 4. Anti-Slop Self-Review

Inspect the screenshot before claiming completion. Fix failures before final claim.

Required assertions:

- primary job is obvious;
- hierarchy has one dominant path;
- no nested cards/cards-inside-cards on the main surface;
- workflow pages are not dashboardified;
- no decorative metrics/badges/charts/KPI strips unless required;
- no developer/internal copy leakage;
- no copy sludge or control-plane fan fiction;
- no clipping, overlap, overflow, unreadable contrast, dead empty states, or unfinished components.

## 5. Requirements Claim Shape

Final Requirements claims must include:

- screenshot path(s) and capture command/method;
- viewport or device;
- reference image path or explicit statement that no reference exists;
- scope contract;
- primary job statement;
- anti-slop self-review results;
- remaining risks or exact blockers.

Text-only visual review is rejected. If the Requirements reviewer cannot inspect pixels directly, provide owner-visible screenshot evidence and state that owner visual approval is the non-text-only review mechanism.
