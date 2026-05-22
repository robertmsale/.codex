#!/usr/bin/env python3
import json
import sys


def main() -> int:
    status = int(sys.argv[1])
    raw = sys.stdin.read()
    lines = [line for line in raw.splitlines() if line.strip()]

    events = []
    non_json = []
    for line in lines:
        try:
            parsed = json.loads(line)
        except json.JSONDecodeError:
            if not line.startswith('[{"event":"test.startedProcess"'):
                non_json.append(line)
            continue
        if isinstance(parsed, dict):
            events.append(parsed)
        elif (
            isinstance(parsed, list)
            and len(parsed) == 1
            and isinstance(parsed[0], dict)
            and parsed[0].get("event") == "test.startedProcess"
        ):
            continue
        else:
            non_json.append(line)

    if not events:
        print(raw.rstrip())
        return status

    tests = {}
    prints = {}
    failures = []
    global_errors = []

    for event in events:
        event_type = event.get("type")
        if event_type == "testStart":
            test = event.get("test") or {}
            tests[test.get("id")] = test
        elif event_type == "print":
            test_id = event.get("testID")
            if test_id is not None:
                prints.setdefault(test_id, []).append(event.get("message", ""))
        elif event_type == "error":
            if event.get("testID") is not None:
                failures.append(event)
            else:
                global_errors.append(event)

    if status == 0:
        print("tests passed")
        return 0

    if non_json:
        for line in non_json[:12]:
            print(line)

    seen_ids = set()
    for failure in failures:
        test_id = failure.get("testID")
        if test_id in seen_ids:
            continue
        seen_ids.add(test_id)
        test = tests.get(test_id, {})
        name = test.get("name") or f"test {test_id}"
        url = test.get("url") or ""
        print(f"FAIL: {name}")
        if url:
            print(f"  file: {url}")

        error = (failure.get("error") or "").rstrip()
        if error:
            for line in error.splitlines()[:6]:
                print(f"  {line}")

        stack = (failure.get("stackTrace") or "").splitlines()
        if stack:
            print(f"  stack: {stack[0]}")
            if len(stack) > 1:
                print(f"  at: {stack[1]}")

        test_prints = prints.get(test_id) or []
        if test_prints:
            print("  stdout:")
            head = test_prints[:4]
            tail = test_prints[-2:] if len(test_prints) > 4 else []
            for line in head:
                print(f"    {line}")
            omitted = len(test_prints) - len(head) - len(tail)
            if omitted > 0:
                print(f"    ... {omitted} more line(s) omitted ...")
            for line in tail:
                print(f"    {line}")

    for error_event in global_errors:
        error = (error_event.get("error") or "").rstrip()
        stack = (error_event.get("stackTrace") or "").splitlines()
        print("ERROR: flutter test")
        if error:
            for line in error.splitlines()[:12]:
                print(f"  {line}")
        if stack:
            print(f"  stack: {stack[0]}")
            if len(stack) > 1:
                print(f"  at: {stack[1]}")

    if not failures and not global_errors:
        print("flutter test exited nonzero without structured failure events.")
        print(f"tests started: {len(tests)}")
        event_tail = events[-8:]
        if event_tail:
            print("machine event tail:")
            for event in event_tail:
                summary = {"type": event.get("type")}
                for key in ("success", "skipped", "hidden", "testID", "suiteID", "count", "time"):
                    if key in event:
                        summary[key] = event.get(key)
                test = event.get("test")
                if isinstance(test, dict):
                    summary["test"] = test.get("name")
                print(f"  {json.dumps(summary, separators=(',', ':'))}")
        if non_json:
            printed_non_json = min(len(non_json), 12)
            tail = non_json[printed_non_json:][-20:]
            if tail:
                print("raw output tail:")
                for line in tail:
                    print(f"  {line}")
        else:
            raw_tail = lines[-24:]
            if raw_tail:
                print("raw output tail:")
                for line in raw_tail:
                    print(f"  {line}")

    print(f"{len(failures)} test failure event(s), {len(global_errors)} global error event(s)")
    return status


if __name__ == "__main__":
    raise SystemExit(main())
