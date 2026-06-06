# Design Worker Orchestration

Use this when assigning or accepting design-focused Flutter work.

## Assignment Contract

Give the worker:

- screen/component/flow;
- product goal and primary user job;
- content-vs-shell boundaries;
- target viewport/device;
- fake-data policy;
- reference image path when one exists;
- required screenshots or states.

Attach `design-non-negotiables` to the worker Requirements for design work.

## Gate

The only design gate is Requirements-native design proof:

- final worker claims include screenshot evidence paths and capture method;
- claims identify viewport/device and reference image when applicable;
- claims include scope contract and anti-slop self-review;
- Requirements reviewer verdicts decide pass/fail against the RequirementSet.

Do not ask workers to run an external visual-review workflow. Do not accept text-only visual review.

## Screenshot Evidence

For Design Lab projects, require `design-lab-capture` evidence. Do not accept Flutter tester pixels, persistent web servers, tmux sessions, manual start/shot/reload/stop loops, or bypassed readiness as visual gate proof.

For runtime/device proof, explicitly assign simulator capture and the device path. Use `$designer-runtime` guidance.

If Requirements image routing cannot inspect pixels directly, require owner-visible screenshot artifacts and owner visual approval as the explicit non-text-only review mechanism.
