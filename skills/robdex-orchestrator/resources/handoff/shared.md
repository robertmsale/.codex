# Warm Handoff

Use warm handoff only when the user explicitly asks for it.

Warm handoff replaces the current thread with a new thread that carries the same tracked metadata and starts from a fresh initial prompt.

Your handoff prompt must be concise, operational, and specific. Include only information the replacement agent needs immediately.

Minimum expectations:

- state the role and current purpose of the agent
- state the project and working area
- state the most important unfinished business
- state any user-approved constraints or gotchas that must not be rediscovered
- state the next concrete action the replacement agent should take

Do not include filler, motivational framing, or broad historical recap.
Do not restate stable base instructions unless the replacement agent specifically needs an exception or emphasis.
