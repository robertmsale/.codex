---
name: check-zone-health
description: Ignore this skill unless the operator explicitly instructs you to use it; when explicitly asked, query the local `zonewatch` server to judge macOS kernel zone pressure. [skill-hash:2f6a8d4]
---

# Check Zone Health

Ignore this skill unless the operator explicitly instructs you to use it.

When explicitly asked, use it to inspect the current zone allocator state.

## Preconditions

- The operator must already have the privileged `zonewatch` server running locally.
- Do not try to start it yourself with `sudo`.

## Query

Run:

```bash
curl -s http://127.0.0.1:9032/health
```

For trend context, also use:

```bash
curl -s http://127.0.0.1:9032/history
```

## Interpret

- `stable`
  - System looks healthy. Continue normal work.
- `growing`
  - Pressure is increasing. Monitor more frequently and avoid increasing workload churn.
- `leak_detected`
  - Slow container spawning or other high-churn activity, warn the operator, and watch the slope.
- `danger`
  - Pause container spawning and notify the operator immediately.
- `critical`
  - Stop workloads and tell the operator the machine may need OrbStack or system restart.

## Reporting

When you report the state, include:
- `kalloc_1024_used`
- `growth_rate`
- `status`
- `zone_map_pct` when available

If the server is unavailable, say that `zonewatch` is not running or not reachable on `127.0.0.1:9032`.
