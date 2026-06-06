# Robdex Requirements System

This document describes the Robdex Requirements system as a control-plane feature: what it is for, how it is represented in state, how it uses structured-output constrained decoding, how review is routed, and why the system behaves the way it does.

The short version:

Requirements turn the operator-approved task outcome into an explicit completion contract. When active, Robdex gives the source agent a structured output schema for every turn. Mid-turn commentary can use `requirements: null`, but a final completion claim must fill a per-requirement claim object. Robdex then routes that claim packet to a bound Requirements reviewer, whose own structured output schema forces a per-requirement verdict. Passing verdicts shrink future worker claim schemas, but the reviewer always checks the full canonical RequirementSet and can re-fail previously passed requirements if later work regresses them.

## Why Requirements Exist

The Requirements system exists because ordinary prompt prose is too soft for multi-agent project work.

The failure modes it is designed to prevent are concrete:

- Agents claim a task is complete when only part of it is done.
- Agents implement a technically adjacent but operator-unapproved alternative.
- Workers steer orchestrators into a smaller or easier """first slice""" that does not satisfy the operator request.
- Orchestrators and workers forget the top-level outcome during long or adversarial review loops.
- Review becomes noisy because every worker hammers the orchestrator with defensive arguments.
- Evidence is vague: """tests passed""", """looks good""", """done""", or """implemented""" without exact proof.
- Fake UI, fake data, disabled checks, documentation-only compromises, or manual workarounds sneak through as """completion""".

Requirements make completion expensive to fake. The agent must explicitly address each requirement in a machine-checkable shape, and a separate reviewer must verdict each requirement before the gate passes.

## Core Design Principles

Requirements are not a task planner and not a suggestion list.

They are:

- **Authoritative**: a feasible operator request is the source of truth. Requirements represent that approved outcome.
- **Structured**: active Requirements produce JSON schemas that are supplied to the Codex app-server as output schemas.
- **Per-turn enforced**: the schema is attached at `turn/start`; it cannot change an already-running turn.
- **Review-gated**: final claim packets are routed to a requirements-reviewer agent for adversarial review.
- **Canonical**: the full RequirementSet remains the source of truth even when worker claim schemas shrink.
- **Progressive**: after partial review success, future worker schemas include only unresolved requirements to reduce output tokens.
- **Regression-aware**: reviewers still evaluate every canonical requirement and can re-fail previously passed requirements.
- **Role-aware**: workers and QA do not set Requirements on other agents; orchestrators set worker Requirements; GUI/operator paths can directly manage selected-thread Requirements.
- **System-managed**: workers do not request review manually. The bridge decides when a claim is reviewable and routes it.

## Main State Objects

The persisted Robdex state stores Requirements on agents. The relevant model is in the bridge state model.

### RequirementSet

Conceptually:

```json
{
  "id": "sales-dashboard-redesign",
  "active": true,
  "enforceOnTurns": true,
  "reviewerThreadId": "optional-explicit-reviewer",
  "requirements": [
    {
      "key": "matchReferenceFidelity",
      "statement": "The implemented dashboard must match the approved reference with production-quality visual fidelity.",
      "severity": "blocker",
      "verificationMethod": "screenshotReview"
    }
  ],
  "reviewProgress": {
    "matchReferenceFidelity": {
      "status": "passed",
      "updatedAt": 1760000000
    }
  }
}
```

Fields:

- `id`: stable label for the requirement package.
- `active`: whether Requirements are currently active for the agent.
- `enforceOnTurns`: whether Robdex should supply an output schema to the app-server.
- `reviewerThreadId`: optional explicit reviewer thread.
- `requirements`: canonical list of requirements. This list is never reduced merely because some requirements passed.
- `reviewProgress`: per-requirement lifecycle state derived from reviewer verdicts.

### Requirement

Each requirement has:

- `key`: semantic camelCase identifier used as the JSON property name in claim and verdict schemas.
- `statement`: human-readable obligation.
- `severity`: typically `blocker`, `high`, `medium`, or `low`.
- `verificationMethod`: expected proof type, such as `diffReview`, `sourceInspection`, `commandEvidence`, `manualEvidence`, `screenshotReview`, or `designComparison`.
- optional schema descriptions for specialized claim/verdict guidance.

The `key` matters. It becomes a required JSON object property. Stable, semantic keys make review packets readable and let progress tracking survive multiple review passes.

### Requirement Review Binding

When a review is in progress, the source agent stores a binding:

```json
{
  "sourceThreadId": "worker-thread",
  "reviewerThreadId": "reviewer-thread",
  "requirementSetId": "sales-dashboard-redesign",
  "status": "inReview",
  "latestClaimPacket": {},
  "latestVerdictPacket": {},
  "updatedAt": 1760000000
}
```

This tells Robdex which reviewer is bound to the source, what the latest claim was, and what the latest verdict was.

### Requirement Packets

Robdex records notable claim/verdict packets for inspection:

- `claim`: reviewable worker claim packet routed to reviewer.
- `claimNull`: worker used `requirements: null` when a final claim was expected.
- `claimContinuation`: worker claimed every currently required requirement as `notSatisfied`, so Robdex skipped review and told the worker to continue.
- `verdict`: final reviewer verdict packet.
- `verdictNull`: reviewer commentary/progress packet with `requirements: null`.

These packets are not the canonical contract. The RequirementSet is.

## Structured Output And Constrained Decoding

The important technical mechanism is the OpenAI structured output schema passed through the Codex app-server as an `output_schema`.

When Requirements are active and `enforceOnTurns` is true, Robdex supplies a JSON schema at `turn/start`. Structured output does not merely ask the model nicely to format JSON. It constrains decoding so the model can only emit tokens that keep the output valid according to the supplied schema.

That is the critical enforcement leverage:

- The agent cannot finish with arbitrary prose if the app-server is enforcing the output schema.
- The top-level object must include the required fields.
- Required requirement keys must be present.
- Enum values must be selected from allowed values.
- Additional properties are rejected by schema.

This does not prove the work is actually complete. It forces the agent to make explicit claims in a predictable shape. The reviewer then evaluates those claims.

In other words:

- Structured output forces **claim surface area**.
- Requirements review evaluates **truthfulness**.

The system uses both. Schema alone is not enough; review alone is too easy to starve or confuse with vague evidence.

## Worker Claim Schema

The worker schema has the top-level shape:

```json
{
  "summary": "Concise global outcome or progress note.",
  "requirements": null
}
```

or, for a final claim:

```json
{
  "summary": "Implemented the assigned behavior and validated it.",
  "requirements": {
    "requirementKey": {
      "claim": "satisfied",
      "justification": "Why the requirement is satisfied.",
      "evidence": [
        "Exact command, file, screenshot, or manual proof."
      ],
      "risk": "low"
    }
  }
}
```

The top-level `summary` is always required.

The top-level `requirements` field is always required and is either:

- `null` for mid-turn commentary/progress, or
- an object for an end-of-turn claim packet.

Each currently required claim object requires:

- `claim`: one of `satisfied`, `notSatisfied`, `blocked`, `notApplicable`.
- `justification`: concise explanation.
- `evidence`: array of exact proof strings.
- `risk`: one of `none`, `low`, `medium`, `high`, `unknown`.

The worker schema uses `additionalProperties: false`. The agent cannot add random properties to avoid the intended format.

### Commentary Versus Final Claim

`requirements: null` is allowed only for commentary/progress while Requirements remain active.

Example commentary:

```json
{
  "summary": "I found the failing test path and am updating the parser.",
  "requirements": null
}
```

If the worker appears to end a turn under active Requirements with `requirements: null`, Robdex sends a follow-up telling the source agent that active Requirements are still attached and it must provide a final claim packet with the currently required claims.

### All `notSatisfied` Is Not Reviewable

If every currently required claim is `notSatisfied`, Robdex does not route to a reviewer.

That is deliberate. Otherwise the worker and reviewer can loop pointlessly:

1. Worker: """Nothing is satisfied."""
2. Reviewer: """Correct, nothing is satisfied."""
3. Worker: """Nothing is satisfied."""
4. Reviewer: """Correct, nothing is satisfied."""

Instead, the bridge sends an explicit continuation message:

```text
This is the owner. Your final claim packet marked every currently required claim as `notSatisfied`, so Robdex did not request Requirements Review. Continue working until at least one currently required requirement can be claimed `satisfied`, `blocked`, or `notApplicable`, then provide an updated final Requirements claim packet. An all-`notSatisfied` packet is absolutely unacceptable. If you are blocked, use the `blocked` claim on the specific blocked requirement and provide concrete blocker evidence. If you submit another final message with all claims as `notSatisfied` then you will be terminated.
```

If the agent is truly blocked, it must use the `blocked` claim on the specific blocked requirement with evidence.

## Progressive Worker Schema Reduction

The first active Requirements pass usually requires the worker to claim every requirement.

After review progress exists, worker schemas are reduced to only currently unresolved requirements. A requirement is considered resolved for the worker claim schema when review progress marks it:

- `passed`
- `blocked`
- `waived`

Resolved requirements are omitted from the next worker claim schema to reduce output tokens.

Example:

Canonical set:

- `noLegacyLeftBehind`
- `backendRouteImplemented`
- `frontendControlWired`
- `testsPass`

First claim schema includes all four.

Reviewer verdict:

- `noLegacyLeftBehind`: pass
- `backendRouteImplemented`: pass
- `frontendControlWired`: fail
- `testsPass`: fail

Next worker claim schema includes only:

- `frontendControlWired`
- `testsPass`

However, the canonical RequirementSet still contains all four. The worker remains bound by all four. If the fix for `frontendControlWired` reintroduces legacy behavior, the reviewer can re-fail `noLegacyLeftBehind` even though the worker did not have to re-claim it in the reduced schema.

This is the token optimization:

- Workers stop repeatedly producing evidence for already-passed requirements.
- Reviewers still protect against regressions.

## Reviewer Verdict Schema

The reviewer also receives a structured output schema. It has the same top-level pattern:

```json
{
  "summary": "Concise reviewer summary.",
  "requirements": null
}
```

or a final verdict:

```json
{
  "summary": "Requirements review failed because one item lacks evidence.",
  "requirements": {
    "requirementKey": {
      "verdict": "fail",
      "reason": "The implementation does not prove the required behavior.",
      "evidenceAssessment": "The cited test only covers a helper, not the real route.",
      "requiredCorrection": "Add route-level proof and rerun the relevant test."
    },
    "overallVerdict": "fail",
    "route": {
      "destination": "sourceAgent",
      "message": "Fix the route-level validation gap and provide exact command evidence."
    }
  }
}
```

Each requirement verdict normally requires:

- `verdict`: one of `pass`, `fail`, `acceptedBlocked`, `rejectedBlocked`, `waiverRequired`, `waiverAccepted`.
- `reason`: why the verdict was assigned.
- `evidenceAssessment`: what evidence supports or fails to support the claim.
- `requiredCorrection`: exact correction when needed, or empty string when not needed.

The final verdict object also requires:

- `overallVerdict`
- `route`

`overallVerdict` can be:

- `pass`
- `fail`
- `acceptedBlocked`
- `rejectedBlocked`
- `needsHumanWaiver`
- `waiverAccepted`

The `route` object contains:

- `destination`: `sourceAgent`, `orchestrator`, `owner`, or `none`.
- `message`: curated routing message.

## Reviewer `stillPassing` Shorthand

Reviewer output can be expensive because the reviewer must check the full canonical RequirementSet every time.

To reduce output cost, Robdex modifies the reviewer schema for requirements that have previously passed. For those keys only, the verdict property becomes an `anyOf`:

1. the full verdict object, or
2. a shorthand object:

```json
{
  "verdict": "stillPassing"
}
```

This is only available for requirements whose review progress is already `passed`.

The reviewer prompt instructs:

- use `stillPassing` only after checking that the previously passed requirement still passes for the same reason;
- do not use it for new, failed, blocked, waived, changed, or insufficiently evidenced requirements;
- keep full evidence for changed or questionable requirements.

The bridge also defends this in lifecycle handling. If a reviewer somehow emits `stillPassing` for a requirement that was not previously passed, that is treated as invalid/failing rather than as success.

## Canonical Versus Reduced Schemas

There are two different schema strategies:

### Worker

The worker claim schema is reduced after progress exists. It includes only currently unresolved claims.

Purpose: reduce worker output tokens.

### Reviewer

The reviewer verdict schema always covers every canonical requirement. Some previously passed requirement values may allow the short `stillPassing` alternative.

Purpose: keep regression protection while reducing reviewer output tokens.

This asymmetry is intentional.

Workers should not repeatedly claim what already passed. Reviewers must still guard the whole contract.

## Review Routing Lifecycle

The bridge watches app-server turn completion events. When an assistant turn completes, the runtime evaluates whether Requirements handling applies.

### Source Agent Turn Completion

For non-reviewer source agents:

1. Robdex checks whether the thread has active Requirements.
2. It ignores requirements-reviewer threads as sources.
3. It fetches the latest assistant text for the completed turn.
4. It parses that text as JSON.
5. It classifies the payload:
   - valid final claim;
   - `requirements: null` commentary;
   - invalid payload;
   - all `notSatisfied`.
6. Commentary and invalid payloads trigger corrective prompts rather than review.
7. All-`notSatisfied` packets trigger continuation rather than review.
8. A valid reviewable claim is recorded.
9. Robdex ensures a reviewer exists and is bound to the source.
10. Robdex starts a reviewer turn with the Requirements review prompt and verdict schema.
11. Source state is marked `inReview`.

### Reviewer Creation

If no reviewer is already bound, Robdex spawns one with role `requirements-reviewer`.

Important reviewer defaults:

- `parentAgentId` is the source thread.
- `approvalPolicy` is forced to `never`.
- `sandboxMode` is `workspace-write`.
- `networkAccess` is false.
- reviewer model/reasoning can come from project role defaults for `requirements-reviewer`.
- otherwise model/reasoning inherit from the source agent.
- reviewer is hidden from normal peer lists.

Requirements reviewers are not implementers. They should inspect the claim and available transcript/context; they should not edit repos or run unsanctioned commands.

If a reviewer still produces an approval request, the bridge has a fallback: it sends a steer telling the reviewer to stop running unsanctioned commands and auto-declines the approval.

### Reviewer Turn Completion

When a requirements-reviewer turn completes:

1. Robdex finds the source thread bound to the reviewer.
2. It parses the latest reviewer assistant text.
3. It accepts `requirements: null` as reviewer commentary/progress and records it.
4. It accepts a final verdict object as review output.
5. It updates source `requirementReview`.
6. It updates per-requirement `reviewProgress`.
7. It routes the review result.

### Verdict Routing

Routing by overall verdict:

- `fail`: route correction message to the source agent.
- `rejectedBlocked`: route correction message to the source agent.
- `needsHumanWaiver`: route to the project orchestrator/owner path for human decision.
- `pass`: route pass message to orchestrator when available, otherwise source.
- `acceptedBlocked`: route to orchestrator when available, otherwise source.
- `waiverAccepted`: route to orchestrator when available, otherwise source.

On `pass`, the active RequirementSet is deactivated and the reviewer is archived or detached/hidden.

On `waiverAccepted`, the RequirementSet is also deactivated, but the review binding records the waiver outcome.

## Requirement Progress Semantics

Per-requirement `reviewProgress` is derived from reviewer verdicts:

- `pass` -> `passed`
- `stillPassing` when previously passed -> `passed`
- invalid `stillPassing` -> `failed`
- `fail` -> `failed`
- `rejectedBlocked` -> `failed`
- `acceptedBlocked` -> `blocked`
- `waiverRequired` -> `blocked`
- `waiverAccepted` -> `waived`
- unknown -> `unresolved`

The overall review binding status is derived from `overallVerdict`:

- `pass` -> `passed`
- `fail` -> `failed`
- `acceptedBlocked` -> `blocked`
- `rejectedBlocked` -> `failed`
- `needsHumanWaiver` -> `waiverRequired`
- `waiverAccepted` -> `waiverAccepted`
- missing/unknown -> `inReview`

## Composable Requirements

Composable Requirements are reusable requirement packs.

They live in:

- global scope: `~/.codex/requirements/composables`
- project scope: `<project>/.codex/requirements/composables`

Supported file extensions:

- `.json`
- `.yaml`
- `.yml`

The current global composables are YAML.

Conceptually:

```yaml
id: no-legacy
title: No Legacy
description: Manual opt-in for clean-slate apps or non-production systems.
appliesTo:
  - code
  - UI
requirements:
  - key: noLegacyLeftBehind
    statement: Do not leave obsolete code paths, docs, flags, config, tests, or UI affordances behind.
    severity: blocker
    verificationMethod: diffReview
```

Composables are merged deterministically:

1. permanent project composables;
2. explicitly included composables;
3. task-specific requirements.

Duplicate requirement keys with identical definitions are deduped. Duplicate keys with conflicting definitions fail.

### Permanent Project Composables

Project settings can mark composables as permanent.

Permanent composables are server-enforced. That means the bridge merges them into every Requirements set/update for agents in that project, even if the GUI, CLI, or orchestrator omits them.

This is important because mandatory policy should not depend on agent memory.

Composable discovery marks permanent composables so GUI and CLI users can see which packs are automatically included.

## GUI Entry Points

The GUI can manage Requirements directly from:

- composer Add/Replace Requirements;
- inspector Add/Replace Requirements;
- project settings permanent composables.

GUI Requirements set/clear actions are owner/direct operations. They target an exact `recipientThreadId` and do not pretend to be the selected agent.

This distinction matters because agent authorization rules are role-scoped:

- workers and QA cannot set Requirements on themselves;
- orchestrators can set Requirements on workers;
- non-worker/non-QA agents may set Requirements on themselves through the sanctioned self path;
- operator/GUI direct management is a separate owner action.

Requirements are separate from communication visibility. Hidden agents may be targeted for Requirements by owner/direct GUI paths even when they are hidden from normal peer communication lists.

## CLI Entry Points

The Robdex CLI exposes Requirements workflows for orchestrators/operators.

Common commands:

```bash
robdex requirements-composables list --name "<agent>"
robdex requirements-composables show no-legacy --name "<agent>"
robdex requirements-from-prose --title "<title>" --text-stdin
robdex requirements-from-prose --title "<title>" --include-composable no-legacy --text-stdin --attach --name "<agent>"
robdex set-requirements --name "<agent>" --requirements-file /tmp/requirements.json
robdex requirements-status --name "<agent>"
```

`requirements-from-prose` turns each non-empty line or bullet into one requirement. This is useful for human-authored task contracts.

Important: `--text-stdin` must be paired with a heredoc, pipe, or redirected file. Running it without input can leave the command waiting interactively.

Example:

```bash
robdex requirements-from-prose --title "Parser cleanup" --include-composable non-negotiables --text-stdin --attach --name "Worker" <<'EOF'
- Remove the obsolete parser path completely.
- Preserve existing documented CLI behavior.
- Add targeted tests for the new parser behavior.
EOF
```

For running workers:

```bash
robdex requirements-from-prose --title "Parser cleanup" --text-stdin --interrupt --name "Worker" <<'EOF'
- Replace the stale requirements with this complete work package.
EOF
```

`--interrupt` sequence:

1. interrupt target;
2. set Requirements on target;
3. send `Requirements updated`.

For setting Requirements on self:

```bash
robdex requirements-from-prose --title "Operator task" --text-stdin --to-self <<'EOF'
- Complete the assigned operator task.
EOF
```

`--to-self` sequence:

1. set Requirements on self;
2. brief delay;
3. interrupt self;
4. send `Begin`.

`--interrupt` and `--to-self` are mutually exclusive.

## Requirements And Turn Start

Requirements enforcement happens at turn start.

When Robdex starts a turn for a thread, it asks:

1. Does the thread have active Requirements?
2. Is `enforceOnTurns` true?
3. Are there unresolved requirements?
4. What schema should be supplied?

If active schema exists, Robdex sends it through the app-server turn parameters.

If Requirements are inactive or absent, Robdex explicitly sends a null output schema so the app-server does not accidentally retain an old structured schema for that thread.

This is why changing Requirements during an already-running turn cannot alter that turn. The schema is already attached. A replacement needs an interrupt and a new turn.


## Design Proof Requirements

Design work must use Requirements-native proof. Attach `design-non-negotiables` to design tasks. Final design claims must include sanctioned screenshot evidence paths, capture method, viewport or device, reference image path when applicable, scope contract, primary job statement, and anti-slop self-review assertions. Text-only visual review is not acceptable. If reviewer image routing cannot inspect pixels directly, the claim must provide owner-visible screenshot artifacts and identify owner visual approval as the explicit non-text-only review mechanism.

## What Requirements Do Not Do

Requirements do not:

- prove code correctness by themselves;
- replace testing;
- replace human judgment;
- magically know whether evidence is true;
- let workers negotiate scope;
- make every tiny task worth gating.

They force explicit claims and route those claims through adversarial review.

## Common Edge Cases

### `requirements: null` Final Packet

If a source agent finishes with `requirements: null` while active Requirements remain, Robdex sends a corrective message. `requirements: null` is only for commentary/progress.

### Invalid JSON Or Invalid Shape

Structured output should make invalid shape rare. If it happens, Robdex sends a corrective prompt asking for a valid claim packet.

### All Claims `notSatisfied`

Robdex skips review and tells the worker to continue. All-`notSatisfied` is never terminal.

### True Blocker

A worker should use `blocked` for the specific blocked requirement. The reviewer can accept or reject the blocker. Accepted blockers route to orchestrator/owner instead of pretending the task passed.

### Human Waiver

If the reviewer determines a requirement needs an explicit owner waiver, it routes a waiver request. Waiver accepted is terminal; waiver rejected or absent means the work remains blocked or failed.

### Passing Review

On pass, Robdex deactivates the RequirementSet and clears the active schema path. The worker should no longer continue using Requirements JSON unless new Requirements are set.

### Hidden Agents

Hidden communication visibility is separate from Requirements state. Owner/direct GUI operations can set Requirements on hidden agents by exact thread ID.

### Missing Project Composable Directory

Missing project-scoped composable directories are not errors. The bridge should still return global composables and any permanent composable metadata it can resolve from project config.

## Operational Guidance

Use Requirements when the task has a meaningful completion contract:

- implementation gates;
- risky refactors;
- cross-cutting behavior changes;
- cleanup where """no legacy""" matters;
- design or UX work where visual proof matters;
- deployment or infrastructure changes;
- tasks where previous agents have drifted or under-completed.

Avoid Requirements for tiny one-command status checks or pure brainstorming.

For large work, do not micro-slice Requirements. Fan out by complete responsibility boundary:

- contracts/schema/API;
- backend implementation/storage;
- frontend integration;
- design polish;
- QA validation.

Each worker package should have Requirements covering the full assigned responsibility, not only the first small step.

## Mental Model

Think of Requirements as a contract compiler:

1. Operator intent becomes a RequirementSet.
2. The bridge compiles the RequirementSet into a worker output schema.
3. Constrained decoding forces the worker to produce a structured claim surface.
4. The bridge routes the claim to a reviewer.
5. The bridge compiles the canonical RequirementSet into a reviewer verdict schema.
6. The reviewer emits a structured verdict.
7. The bridge updates progress and routes the result.
8. Passing progress reduces future worker output, while canonical review prevents regressions.

The important distinction is that Requirements are not about making the agent say JSON for its own sake. They are about turning vague completion into a constrained, reviewable protocol that the bridge can enforce and route.
