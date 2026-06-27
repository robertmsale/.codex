# Robdex Requirements System

This document describes the Robdex Requirements system as a control-plane feature: what it is for, how it is represented in state, how it uses structured-output constrained decoding, how review is routed, and why the system behaves the way it does.

The short version:

Requirements turn the operator-approved task outcome into an explicit completion contract. When active, Robdex gives the source agent a structured output schema for every turn. Mid-turn commentary can use `requirements: null`, but a final completion claim must fill a per-requirement claim object. Robdex then routes that claim packet to a bound Requirements reviewer, whose own structured output schema forces a per-requirement verdict for the reviewable keys in the claim packet. Passing verdicts shrink future worker claim schemas, and later reviewable claims can re-fail previously passed requirements if later work regresses them.

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
- **Role-aware**: workers and QA do not set Requirements on other agents; orchestrators set worker Requirements; planners may set Requirements on non-hidden agents in their project; GUI/operator paths can directly manage selected-thread Requirements.
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
- `claimContinuation`: worker sent a cheap `notFinished` continuation packet, so Robdex kept Requirements active without reviewer dispatch.
- `claimCorrection`: worker sent a malformed or low-quality sparse claim packet, so Robdex routed correction without reviewer dispatch.
- `verdict`: final reviewer verdict packet.
- `verdictCorrection`: reviewer sent output outside the minimal full-set verdict schema; Robdex preserves the raw rejected payload for audit, does not mutate source review progress, and sends the owner-authority correction back to the reviewer thread only.

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
      "kind": "satisfied",
      "summary": "Why the requirement is satisfied.",
      "evidence": [
        {
          "type": "testsRun",
          "value": "Exact command, file, screenshot, or manual proof."
        }
      ]
    }
  }
}
```

The top-level `summary` is always required.

The top-level `requirements` field is always required and is one of:

- `null` for mid-turn commentary/progress, or
- `{ "notFinished": true }` for a cheap continue packet, or
- a sparse object for an end-of-turn claim packet.

Sparse claim objects may omit every canonical requirement that is unchanged for that turn. Omitted requirement keys mean unchanged and unclaimed; previously passed requirements remain binding in persisted bridge state. A per-key `{ "notFinished": true }` entry is also a cheap continue marker and is stripped before reviewer dispatch.

Each completed claim object requires:

- `kind`: one of `satisfied`, `blocked`, `notApplicable`.
- `summary`: concise per-requirement result.
- `evidence`: non-empty array of typed evidence objects shaped exactly as `{ "type": "<evidence-type>", "value": "<concrete proof>" }`.

Allowed evidence types are exactly `changedFiles`, `testsRun`, `sourceInspection`, `artifact`, `screenshot`, `commandOutput`, `migration`, and `searchProof`.

`kind: "blocked"` also requires:

- `blocker`: the concrete external dependency.
- `ownerDecisionNeeded`: the exact owner/orchestrator decision required.

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

If the worker appears to end a turn under active Requirements with `requirements: null`, Robdex sends a follow-up telling the source agent that active Requirements are still attached and it must provide a final claim packet.

`{ "requirements": { "notFinished": true } }` and sparse packets containing only omitted keys plus per-key `{ "notFinished": true }` are cheap continue packets. Robdex records minimal packet metadata for traceability, keeps the active RequirementSet attached, mutates no progress, and dispatches no reviewer.

Robdex rejects or routes correction for source claim packets whose typed evidence is empty, placeholder text, circular restatement, generic proof, or lacks concrete file, test, artifact, source-inspection, migration, screenshot, command-output, or search-proof details.

## Sparse Worker Claims

The active worker schema exposes all canonical requirement keys as optional sparse properties. The worker claims only requirements with new evidence, a true blocker, or a not-applicable decision.

Review progress is persisted separately from the sparse claim packet. A requirement remains terminal only when review progress marks it:

- `passed`
- `blocked`
- `waived`

Resolved requirements do not need to be re-claimed by the worker. The canonical RequirementSet still contains every key, and `requirements-status` derives pass/fail/blocked/waived/pending counts from persisted progress instead of assuming the latest sparse packet contains all keys.

Example:

Canonical set:

- `noLegacyLeftBehind`
- `backendRouteImplemented`
- `frontendControlWired`
- `testsPass`

The worker claims the two requirements it has evidence for and omits the rest.

Reviewer verdict:

- `noLegacyLeftBehind`: pass
- `backendRouteImplemented`: pass
- `frontendControlWired`: fail
- `testsPass`: fail

The next worker turn claims only:

- `frontendControlWired`
- `testsPass`

However, the canonical RequirementSet still contains all four. The worker remains bound by all four. If the fix for `frontendControlWired` reintroduces legacy behavior, a later scoped review can re-fail `noLegacyLeftBehind` when that key is in review scope.

This is the token optimization:

- Workers stop repeatedly producing unchanged evidence.
- Reviewers receive only the keys claimed in the current source packet.

## Reviewer Verdict Schema

The reviewer also receives a structured output schema. It has one top-level object shape:

```json
{
  "requirements": {
    "passedRequirement": {
      "verdict": "pass",
      "evidence": [
        {
          "type": "testsRun",
          "value": "Inspected the cited route-level test output from this review turn."
        }
      ]
    },
    "failedRequirement": {
      "verdict": "fail",
      "reason": "The implementation does not prove the required behavior.",
      "evidenceAssessment": "The cited test only covers a helper, not the real route.",
      "requiredCorrection": "Add route-level proof and rerun the relevant test."
    }
  }
}
```

The reviewer packet has exactly one top-level property: `requirements`. That object must contain every canonical requirement key in the active RequirementSet and must not contain extra keys.

Each requirement uses exactly one of two shapes:

- Evidence-backed accepted verdict: `pass`, `acceptedBlocked`, or `waiverAccepted` with a non-empty `evidence` array. Each evidence item must be reviewer-authored inspection proof shaped as `{ "type": "changedFiles|testsRun|sourceInspection|artifact|screenshot|commandOutput|migration|searchProof", "value": "<what the reviewer inspected>" }`.
- Explained rejected verdict: `fail` or `rejectedBlocked` with `reason`, `evidenceAssessment`, and `requiredCorrection`.

Reviewer output does not contain reviewer `summary`, reviewer `route`, reviewer-authored destination metadata, deferral verdicts, risk fields, null requirement packets, or reviewer-owned overall verdicts. Robdex rejects invalid reviewer output before progress mutation and sends an owner-prefixed correction to the reviewer thread only.

## Canonical Full-Set Verdicts

The reviewer schema is full-set and canonical on every review turn. Sparse worker/source claims remain allowed where the source claim schema permits them, but the reviewer must render a verdict for every canonical key required by the active schema. Terminal success is never reviewer-authored for a subset; it is derived after applying persisted per-requirement progress across the full RequirementSet.

## Worker Sparse Claims And Reviewer Full-Set Verdicts

The worker claim schema is sparse. All canonical requirement keys are optional; omitted keys are unchanged for that turn. Whole-packet and per-key `notFinished` shortcuts are accepted as cheap continue signals.

The reviewer verdict schema is full-set. It carries every canonical requirement key from the active RequirementSet. The reviewer prompt input is exactly the current compacted source claim packet text. It does not add a subject line, headings, reviewer instructions, requirement-key inventory, canonical requirement prose, source IDs, turn IDs, prior statuses, or global `previouslyPassed`, `currentlyUnresolved`, or `previousFailuresBlockersWaivers` lists.

The full canonical RequirementSet and all progress history remain in Rust-owned persisted bridge state for audit, `requirements-status`, routing, and warm handoff.

Workers should not repeatedly claim unchanged work. Reviewers must verdict every canonical key required by the schema; passing or accepted outcomes include reviewer-authored evidence for the current review turn, while failed or rejected-blocker outcomes include concrete assessment and correction.

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
   - `notFinished` cheap continuation;
   - invalid payload;
   - low-quality evidence.
6. Commentary, invalid payloads, and low-quality evidence trigger corrective prompts rather than review.
7. Cheap continuation packets are recorded without reviewer dispatch.
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
3. It accepts only the top-level `{ "requirements": { ... } }` full-set verdict packet shape.
4. It rejects invalid reviewer output before source progress mutation, records a `verdictCorrection` packet, and sends the owner-prefixed correction to the reviewer thread only.
5. It updates per-requirement `reviewProgress` from valid verdicts.
6. It derives the full RequirementSet status from persisted progress across every canonical requirement.
7. It routes only from Rust-derived status and persisted verdict details.

### Verdict Routing

Routing is Rust-derived. Reviewer packets do not choose destinations.

- `fail` and `rejectedBlocked` become failed progress and route synthesized correction text to the source worker from `reason`, `evidenceAssessment`, and `requiredCorrection`.
- `acceptedBlocked` becomes blocked progress. Proven owner/human decision blockers do not route as normal completion. Other true external blockers route only through configured project blocked-routing when a distinct route exists.
- A fully successful final review routes to the orchestrator only when project auto-routing is enabled and a distinct orchestrator exists. If auto-routing is disabled or no distinct orchestrator exists, Robdex deactivates/completes Requirements without starting an orchestrator turn.
- `waiverAccepted` contributes to terminal success and deactivates the RequirementSet when every requirement is passed or waived.

### Requirement Progress Semantics

Per-requirement `reviewProgress` is derived from reviewer verdicts:

- `pass` -> `passed`
- `fail` -> `failed`
- `rejectedBlocked` -> `failed`
- `acceptedBlocked` -> `blocked`
- `waiverAccepted` -> `waived`
- unknown -> `unresolved`

The overall review binding status is derived from persisted progress across every canonical requirement. All passed/waived requirements produce terminal success; any missing, failed, blocked, pending, or unresolved requirement keeps the set nonterminal.

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
- planners can set Requirements on non-hidden agents in their project, but not on themselves;
- non-worker/non-QA/non-planner agents may set Requirements on themselves through the sanctioned self path;
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

### True Blocker

A worker should use `blocked` for the specific blocked requirement. The reviewer can accept or reject the blocker. Accepted blockers route to orchestrator/owner instead of pretending the task passed.

### Human Or Owner Decision

If review identifies a true owner/human decision blocker, the reviewer uses `acceptedBlocked` only when the source proved the external blocker. Rust-derived blocked routing must not treat that as normal orchestrator completion.

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
