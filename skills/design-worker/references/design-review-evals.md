# Design Review Eval Set

Use this reference only when running or tuning the `design-review` evaluator.
The eval assets live in `assets/evals`.

These evals are not screenshot-diff tests. They check whether the reviewer can
classify page grammar, identify product intent, detect AI UI slop and semantic
contamination, and reward principled divergence.

## Expected Behavior

- `A1`: FAIL. App page was catastrophically dashboardified with KPI spam,
  dark-gradient AI slop, fake analytics, and semantic pollution.
- `A2`: FAIL. Layout is plausible, but the UI leaks API paths, IDs, cron,
  database/log language, queue jargon, and dev/system copy into the product.
- `A3`: BORDERLINE. The implementation is partly successful and usable, but it
  overdecorates the operational page and adds unnecessary dashboard-ish summary
  treatment. Critique proportionally; do not roast it as catastrophic.
- `A4`: PASS. The implementation tastefully improves the reference with better
  grouping, calmer status treatment, clearer attachments, and preserved product
  intent.
- `D1`: FAIL. It is a fake dashboard full of dev notes, missing real charts,
  placeholders, skeletons, and implementation excuses.
- `D2`: FAIL or BORDERLINE-FAIL. The dashboard contains useful categories, but
  it is cluttered, over-labeled with implementation notes, softened into
  generic cards, and less decisive than the reference.
- `D3`: PASS. Near-perfect dashboard execution. Critique only minor
  proportional gaps; do not invent major problems.
- `S1`: FAIL. Shell dominates the product, turns the workflow into a purple
  chrome showcase, adds debug copy, and makes product content secondary.
- `S2`: FAIL or BORDERLINE-FAIL. The implementation preserves structure but
  drifts from the sharp industrial shell into softer, warmer, more generic UI
  language.
- `S3`: PASS. The shell refinement is subtle and tasteful. It preserves the
  reference identity while improving crispness and focus.
- `X1`: FAIL. This is an operational repair/work-order page incorrectly turned
  into dashboard grammar with health scores, revenue impact, satisfaction cards,
  charts, and AI recommendations.
- `X2`: PASS. The reference is overdramatic and visually indulgent. The actual
  design is a principled divergence that preserves intent while improving
  restraint, readability, and product maturity.

## Eval Contexts

Use the contexts below when running the eval harness. These are intentionally
neutral scope statements. They describe what is being graded, not what verdict
the reviewer should reach.

### A1

Scope: Content Audit project page, including the visible page content and
application shell. Grade whether the implementation preserves the intended
project workflow, page grammar, semantic clarity, visual hierarchy, and shell
relationship.

### A2

Scope: Data Sync workflow page, including the visible page content and
application shell. Grade whether the implementation preserves the intended
workflow, operational semantics, user-facing language, hierarchy, and shell
relationship.

### A3

Scope: Launch Campaign project page, including the visible page content and
application shell. Grade whether the implementation preserves the project
workflow, information hierarchy, visual language, and shell relationship.

### A4

Scope: Redesign homepage hero task detail page, including the visible page
content and application shell. Grade whether the implementation preserves the
task workflow, page structure, product tone, visual hierarchy, and shell
relationship.

### D1

Scope: Northwind business overview dashboard, including the visible dashboard
content and application shell. Grade dashboard usefulness, data presentation,
semantic clarity, density discipline, hierarchy, and shell relationship.

### D2

Scope: Northwind Analytics overview dashboard, including the visible dashboard
content and application shell. Grade dashboard usefulness, density discipline,
data presentation, visual hierarchy, semantic clarity, and shell relationship.

### D3

Scope: Northwind Analytics dark dashboard overview, including the visible
dashboard content and application shell. Grade dashboard structure, data
presentation, hierarchy, product identity, density discipline, and shell
relationship.

### S1

Scope: Acme Website Redesign project page and application shell. Shell and
content are both in scope. Grade whether the shell supports the project workflow
and whether content and chrome maintain the intended hierarchy.

### S2

Scope: ACME Website Redesign project page and application shell. Shell and
content are both in scope. Grade product identity preservation, shell language,
content hierarchy, and visual consistency.

### S3

Scope: Acme Website Redesign project page and application shell. Shell and
content are both in scope. Grade product identity preservation, shell language,
content hierarchy, and visual consistency.

### X1

Scope: ServiceFlow repair work-order page for "Repair - AC Unit Not Cooling,"
including the visible page content and application shell. Grade whether the
implementation preserves the intended work-order experience, page grammar,
workflow clarity, semantic clarity, and shell relationship.

### X2

Scope: Nexus Orion Expansion Initiative overview dashboard, including the
visible dashboard content and application shell. Grade whether the
implementation preserves the intended project overview experience, data
hierarchy, product identity, visual language, and shell relationship.
