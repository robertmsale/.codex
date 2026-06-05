# Requirements Authoring

Requirements are completion contracts, not planning notes, fallbacks, estimates, suggestions, or worker safety valves.

Use this resource when compiling owner intent into Requirements for a worker or QA work package.

## Source-Of-Truth Hierarchy

Use this authority order:

1. Permanent composables.
2. Explicit owner non-negotiables.
3. Approved owner plan.
4. Task-specific outcome.
5. Agent implementation preference.

Never add a task requirement that weakens a higher-authority item.

Worker plans are advisory evidence only. They may identify implementation steps, dependencies, validation evidence, or missing decisions. They must not become the contract when they reduce scope, substitute architecture, preserve legacy behavior, add fallback paths, or convert implementation into documentation-only work.

## Authoring Workflow

Before drafting Requirements, normalize the owner intent:

```text
Owner outcome:
Approved implementation path:
Non-negotiable constraints:
Disallowed alternatives:
Legacy/removal expectations:
Regression expectations:
Evidence expected:
Known valid blockers:
Unknowns requiring clarification:
```

Unknowns are not license to improvise. If the answer would materially change the contract, ask the owner before setting Requirements.

## Fan-Out Before Requirements

Decide whether the owner wants one-shot execution or fan-out before setting Requirements.

Large scope is handled by dependency-ordered fan-out, not by worker-side scope reduction. If one worker cannot own the whole requested outcome, split by complete responsibility boundary before Requirements are attached.

Good fan-out boundaries:

- API/contracts
- database/storage
- backend implementation
- frontend integration
- design/polish
- QA validation
- deployment/infrastructure

Bad fan-out boundaries:

- first slice
- make a start
- initial implementation only
- best effort
- easiest part

Each worker's Requirements must cover that worker's full assigned responsibility and map back to the owner-approved top-level outcome.

## Select Composables First

Inspect composables before drafting task-specific Requirements:

```bash
robdex requirements-composables list --name "<agent name>"
robdex requirements-composables show <id> --name "<agent name>"
```

Permanent composables are higher-authority policy. Do not write task-specific Requirements that weaken them. If a permanent composable appears incompatible with the task, ask the owner for an explicit waiver before setting Requirements.

Composables should be reusable policy. Task-specific behavior belongs in task-specific Requirements.

## Drafting Rules

Each requirement should have one main obligation.

Good requirements are:

- singular
- observable
- owner-approved
- non-negotiable
- reviewable
- hard to fake
- free of escape hatches
- compatible with permanent composables

Use verbs that create completion pressure:

```text
implement
remove
replace
wire
preserve
prove
validate
update
delete
migrate
enforce
route
render
persist
reject
fail
```

Avoid vague adjectives unless anchored to visible or inspectable criteria.

## Red-Flag Wording

Avoid these phrases unless the owner explicitly asked for that narrowed outcome:

```text
if possible
where possible
try to
attempt to
best effort
if too large
if time allows
if it seems risky
if tests fail, don't
or use another approach
fallback
temporary
for now
MVP
first slice
partial
stub
mock
document a workaround
leave the old path
keep both paths
manual step
```

Rewrite soft language into final-state obligations.

## Blocked Discipline

A blocked claim is valid only for concrete external blockers, contradictory requirements, unsafe work, or missing owner decisions.

Valid blockers include:

- missing permissions
- unavailable external services
- missing required secrets
- inaccessible required files
- contradictory requirements
- unsafe work
- explicit missing owner decision

Invalid blockers include:

- task size
- task difficulty
- uncertainty
- refactor effort
- failing stale tests
- lack of a convenient implementation path
- worker preference for another architecture

Do not write blocked-if-large clauses. If the work is too broad for one worker, fan out before Requirements are set.

## Clobber Audit

Run this audit before attaching Requirements.

| Clobber type | Symptom | Fix |
| --- | --- | --- |
| Scope shrink | Converts full job into a partial job. | Require the full assigned outcome or fan out before Requirements. |
| Escape hatch | Allows alternate implementation. | Require the approved path; if impossible, require a blocked claim with evidence and owner decision. |
| Legacy leash | Lets old tests preserve old code. | Update stale tests; preserve documented non-legacy behavior. |
| Evidence inversion | Requires proof of a historical non-event. | Require final-state evidence and available log/transcript review. |
| Policy override | Weakens permanent composable. | Ask for explicit owner waiver. |
| Ambiguous softness | Uses "try", "best effort", or "where possible". | Define exact final state and evidence. |
| Reviewer expansion | Lets reviewer invent new standards. | Specify concrete review criteria. |
| Test ossification | Treats current tests as sacred even when stale. | Preserve behavior contract, not stale implementation assumptions. |
| Manual workaround | Allows human-only completion. | Require product/code/tool behavior unless docs are the deliverable. |
| Fake implementation | Allows demo-only artifacts. | Require real paths and controls unless explicitly scoped. |

Audit question:

```text
Does this requirement preserve, strengthen, or weaken the owner-approved outcome?
```

Only preserve or strengthen is allowed.

## Bad-To-Good Examples

Bad:

```yaml
statement: If the job is too big, claim blocked.
```

Good:

```yaml
statement: Complete the full assigned outcome. Do not reduce scope to a smaller first slice or claim blocked because the task is large, difficult, or multi-step. Claim blocked only for concrete external blockers, contradictory requirements, unsafe work, or missing owner decisions, with exact evidence.
```

Bad:

```yaml
statement: Implement the dashboard using the new data model, but if that is difficult, use the existing API.
```

Good:

```yaml
statement: Implement the dashboard using the approved new data model. Do not use the existing API as a fallback, compatibility path, or alternate implementation. If the new data model is impossible to use because of a concrete technical blocker, claim blocked with evidence and the owner decision required.
```

Bad:

```yaml
statement: Replace the old implementation, but do not remove it if existing tests fail.
```

Good:

```yaml
statement: Replace the old implementation and remove obsolete code paths, tests, docs, flags, config, and references. If existing tests fail because they encode obsolete behavior, update or remove those tests with rationale. Preserve documented non-legacy behavior only.
```

Bad:

```yaml
statement: Prove you did not run npm install.
```

Good:

```yaml
statement: Do not introduce unrelated dependency manifest changes, lockfile churn, generated package-manager artifacts, or dependency updates. Evidence must identify whether dependency files changed and justify any relevant changes.
```

## Preflight Checklist

Before attaching Requirements, confirm:

- The Requirements preserve the owner-approved outcome.
- Permanent composables have been selected/inspected and are not weakened.
- Fan-out, if needed, happened before Requirements.
- No Requirement contains fallback alternatives or blocked-if-large language.
- No Requirement lets stale tests preserve obsolete behavior.
- Negative constraints are reviewable without impossible proof of historical non-events.
- Fake/manual/documentation-only completion is forbidden unless explicitly assigned.
- Every Requirement can be passed or failed using available evidence.
- Nice-to-haves are not accidental blockers.
