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
2. Second delivered artifact: an existing UI screenshot from Design Lab
   Bun/WebView capture when the project has `clients/design_lab`, produced by a
   repeatable command that can be rerun.

The worker may internally capture and view the existing UI before generating the reference image. The external approval gate remains: reference design first, existing UI render second, then orchestration approval or iteration.

## Reference Review Process

Compare:

- Existing rendered UI screenshot.
- Generated reference design image.
- Task boundaries and product goals.
- Impeccable UI guidance relevant to the task.

Approve implementation only if the reference image is strong enough to guide useful work. If it misses the mark, send the worker back to generate another reference image before touching implementation code.

The worker runs `design-review` during pre-merge proof. Do not make the
orchestrator the default initiator of this tool; consume the worker's review
output at the merge gate.

## Implementation Authorization

Once approved, tell the worker:

- Implement toward the approved reference image.
- Preserve controllers, state flow, and backend contracts unless the task explicitly authorizes deeper changes.
- Avoid shell/chrome changes unless authorized.
- Keep screenshots/test artifacts in `/tmp` unless golden assets are explicitly requested.
- Reject `--update-goldens` as the default screenshot path. Use it only when the task explicitly requests maintained golden baselines.
- For projects with Design Lab, use `design-lab-capture` for visual proof. Do
  not accept Flutter tester screenshots, tmux web-server sessions, or manual
  start/shot/reload/stop loops as design-review evidence for Design Lab tasks.
- Do not accept `design-lab-capture` evidence produced with pass-through
  `--port`, `--out`, `--backend`, or readiness bypasses such as `--skipReady`.
  Those flags bypass the wrapper-owned build/serve/readiness contract and can
  produce blank Flutter loading frames. Alternate `--url` values are acceptable
  for sanctioned web-client capture when the target page exposes the Design Lab
  readiness signal.
- For designer/design-worker live simulator capture, instruct workers to use `$designer-runtime` with `designer-drive` and the provided device ID. Do not send them through `flutter-sim`/`flutter-drive` unless you are intentionally using the QA broker-managed path.

## Pre-Merge Visual Gate

Before merge approval, require:

- A fresh Design Lab Bun/WebView screenshot of the implemented UI for projects
  with `clients/design_lab`.
- The worker's `design-review` output comparing the approved reference image to the implemented screenshot.
- The exact `design-review` context the worker passed, including in-scope and out-of-scope regions.
- For composite references, either a cropped reference image or explicit context identifying the reviewed panel/region.
- A comparison against the approved reference image.
- A short written note covering intentional deviations.

Do not allow a worker to replace Design Lab proof with Flutter tester pixels
unless the assignment explicitly says it is not a Design Lab task and accepts
widget-rendered artifacts. `flutter test` remains valid for behavior, logic,
state, and widget contract assertions; it is not the visual merge gate for
Design Lab work.

Do not accept proof that requires workers to keep a persistent server, tmux
session, or port alive. `design-lab-capture` owns build, ephemeral serving,
Bun/WebView screenshot capture, cleanup, and log reporting in one command.

If the page is not available in Design Lab, route the worker to wire the page
body/renderer into Design Lab or report a blocker. Do not accept a fallback
design review against Flutter tester screenshots.

Review the worker's `design-review` context before accepting the result. It must
be a neutral scope contract, not a persuasive argument. Reject and rerun the
review if the worker tells the reviewer what answer to give.

Acceptable context includes:

- screen/page/component name
- whether content, shell, or both are in scope
- exact reference/actual region to grade
- explicit exclusions such as shell out of scope, device frame out of scope, or fake live data forbidden
- dominant in-scope page artifact, such as route map/navigation cockpit, calendar grid, table/list, dashboard data canvas, inspector/detail rail, or editor canvas
- neutral grading dimensions such as product intent, page grammar, workflow clarity, semantic cleanliness, density discipline, style drift, and shell restraint

Unacceptable context includes:

- `the actual is better`
- `the reference is wrong`
- `reward this divergence`
- `ignore this mismatch`
- `this should pass`
- `only fail if...`
- any statement that pre-judges the verdict, score, or defect list

The reviewer returns separate content and shell grades. For page-content work,
use the content grade as the merge gate and treat shell grade as diagnostic
unless shell changes were authorized. For shell work, use the shell grade. When
both are in scope, the weaker grade is usually blocking.

Treat a scoped `FAIL` verdict as design-blocking unless you explicitly decide
the reviewer misunderstood the task boundary. If the worker claims the verdict
is wrong, require them to explain the boundary mismatch and show the relevant
images.

If the review criticizes shell/nav/chrome that was not in scope, send the worker
back to rerun `design-review` with explicit context excluding the shell instead
of treating that critique as an implementation requirement.

If the review demands fake populated rows, generated results, schedules, owners,
metrics, providers, charts, exports, or actions that the task forbids, treat that
as a calibration/context issue. Ask the worker to rerun `design-review` with
explicit honest-data constraints and require the review to grade unavailable or
readiness states for geometry and polish.

Do not accept honest-data constraints as an excuse for missing the dominant
reference composition. If a GPS/navigation page has a blank map placeholder
instead of a route-map/navigation cockpit, or any page omits its primary product
artifact, require a FAIL or rerun with a neutral scope that names the missing
core artifact.

Use visual judgment, not pixel-perfect matching. The question is whether the implementation strongly fulfills the reference direction and product goal.

## Visual Fidelity Enforcement

Do not accept a design-review PASS that only proves the same sections exist.
The review must compare the actual screenshot against the approved reference's
visible quality bar: surface treatment, borders, shadows, radius, spacing,
typography, density, proportions, chart/data rendering, copy/data richness, and
overall polish.

Reject the review result and require rerun or fixes when:

- The implementation is structurally similar but materially worse in polish,
  hierarchy, density, spacing, or proportions.
- Borders, shadows, radii, or token use make the page look wireframe-like,
  debug-like, harshly outlined, cramped, or unfinished compared with the
  reference.
- Charts, maps, tables, graphs, or other data surfaces are visibly broken or
  low-fidelity.
- Empty states, unavailable states, copy changes, or data substitutions make the
  page feel thinner, less trustworthy, or less production-ready than the
  reference.
- The reviewer cannot clearly articulate concrete pixel-visible differences
  between the reference and actual screenshot.

Treat these as design-blocking even if the macro layout is recognizable. The
merge gate is production-fidelity alignment with the approved reference, not
section inventory.

## Backend Gaps

If the worker reports missing backend functionality:

- Let the design worker finish with mocked data if that produces a useful design.
- Assign a separate backend worker for the missing behavior.
- After backend and design work land, assign a follow-up integration worker to wire real data into the approved design.

Do not let the design worker invent backend behavior unless the task explicitly includes backend implementation.
