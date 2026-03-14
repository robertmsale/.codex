---
name: check-zone-health
description: Ignore this skill unless the operator explicitly instructs you to use it; when explicitly asked, query the local `zonewatch` server with bounded history reads to judge macOS kernel zone pressure. [skill-hash:6b412e1]
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

For trend context, use bounded history reads. The endpoint is paginated by default.

```bash
curl -s 'http://127.0.0.1:9032/history'
```

To request a smaller page explicitly:

```bash
curl -s 'http://127.0.0.1:9032/history?limit=20'
```

To continue to older samples, use the returned `next_cursor`:

```bash
curl -s 'http://127.0.0.1:9032/history?limit=20&cursor=2026-03-14T20:05:19.794283Z'
```

To bound the window by timestamp:

```bash
curl -s 'http://127.0.0.1:9032/history?start=2026-03-14T20:00:00Z&end=2026-03-14T20:10:00Z&limit=20'
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

Do not paste massive history payloads back into the thread. Summarize the trend and include only the specific samples needed to support the conclusion.

If the server is unavailable, say that `zonewatch` is not running or not reachable on `127.0.0.1:9032`.
