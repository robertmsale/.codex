---
name: product-usability-qa
description: Use this when QA is piloting product goals, golden paths, simulator flows, or workflow-scale usability for a real user persona.
---

# Product Usability QA

## Core Standard

Completion is not enough.

A story passes only when a real target user can complete it with reasonable effort, obvious navigation, clear state, and no inappropriate developer knowledge.

## Step Budgets

Simple tasks: 3-5 expected, >7 usability bug, 10+ severe usability.

Medium tasks: 6-12 expected, >15 usability bug, 20+ severe usability.

Complex tasks: 15-30 allowed only when guided, previewable where applicable, recoverable, and appropriate for advanced configuration.

## AI Assistance Rule

AI assist is appropriate for advanced configuration, rules, workflow design, package/interface generation, marketing copy, complex automation, and import/mapping.

AI assist is not acceptable for basic operational tasks such as creating records, adding contacts, finding records, editing basic info, assigning people, sending items, approving or rejecting items, or marking work complete.

If a simple task appears to require AI assistance, report Severe Usability.

## Required QA Output

Use [`assets/product-usability-pilot-report.md`](assets/product-usability-pilot-report.md) when a full report artifact is requested.

Every pilot report must include:
- persona
- starting state
- task
- expected step budget
- actual step count
- step trace
- friction score
- product bugs
- usability bugs
- tooling bugs
- final judgment

## Friction Score

Use [`references/product-usability-task-classes.md`](references/product-usability-task-classes.md) when task class or AI-assistance suitability is unclear.

Score each 0-3:
- navigation clarity
- information visibility
- form/input efficiency
- error recovery
- terminology/user fit
- visual hierarchy
- state confidence
- mobile suitability

0-4 acceptable.
5-8 needs polish.
9-14 usability bug cluster.
15+ severe usability failure.

## Anti-Pattern

Do not say "passed" merely because the task eventually succeeded.

If the task took unreasonable effort, report PASS WITH USABILITY BUGS or FAIL - SEVERE USABILITY.
