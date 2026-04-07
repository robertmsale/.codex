from __future__ import annotations

import base64
import json
from pathlib import Path
from typing import Any

from .bridge import BridgeError
from .flutter_sim import FlutterSimManager


def healthz(*, manager: FlutterSimManager) -> dict[str, Any]:
    return manager.health()


def devices(*, manager: FlutterSimManager) -> dict[str, Any]:
    return {"ok": True, "devices": manager.devices()}


def reserve(*, manager: FlutterSimManager, device_id: str) -> dict[str, Any]:
    return manager.reserve(device_id=device_id)


def reboot(*, manager: FlutterSimManager, device_id: str) -> dict[str, Any]:
    return manager.restart(device_id=device_id)


def dump_logs(*, manager: FlutterSimManager, device_id: str) -> dict[str, Any]:
    return manager.dump_logs(device_id=device_id)


def _ready_reservation(manager: FlutterSimManager, device_id: str):
    return manager._ready_reservation_for_device(device_id)  # noqa: SLF001


def _run_dir(manager: FlutterSimManager, reservation, *, kind: str) -> Path:
    return manager._next_driver_run_dir(reservation, kind=kind)  # noqa: SLF001


def maestro_apps(*, manager: FlutterSimManager, device_id: str) -> dict[str, Any]:
    return manager.driver_apps(device_id=device_id)


def maestro_hierarchy(*, manager: FlutterSimManager, device_id: str) -> dict[str, Any]:
    try:
        reservation = _ready_reservation(manager, device_id)
    except BridgeError as error:
        return {"ok": False, "message": str(error)}
    run_dir = _run_dir(manager, reservation, kind="hierarchy")
    with manager._driver_lock_for_device(reservation.device_id):  # noqa: SLF001
        try:
            elements = manager._idb_describe_all(reservation=reservation)  # noqa: SLF001
        except BridgeError as error:
            manager._write_driver_result(  # noqa: SLF001
                run_dir=run_dir,
                kind="hierarchy",
                returncode=1,
                stdout="",
                stderr=str(error),
                metadata={"device_id": reservation.device_id},
            )
            return {"ok": False, "message": str(error)}
    hierarchy = manager._serialize_idb_hierarchy(elements)  # noqa: SLF001
    manager._write_driver_result(  # noqa: SLF001
        run_dir=run_dir,
        kind="hierarchy",
        returncode=0,
        stdout=hierarchy,
        stderr="",
        metadata={"device_id": reservation.device_id, "count": len(elements)},
    )
    return {
        "ok": True,
        "device_id": reservation.device_id,
        "hierarchy": hierarchy,
        "artifacts": [str(run_dir / "stdout.log"), str(run_dir / "stderr.log"), str(run_dir / "result.json")],
    }


def _execute_driver_command_entry(
    *,
    manager: FlutterSimManager,
    reservation,
    command_entry: Any,
    run_dir: Path,
    step_index: int,
) -> dict[str, Any]:
    step_run_dir = run_dir / f"step-{step_index:02d}"
    step_run_dir.mkdir(parents=True, exist_ok=True)
    if isinstance(command_entry, str):
        command_name = command_entry
        payload = None
    elif (
        isinstance(command_entry, dict)
        and set(command_entry.keys()).issuperset({"command"})
        and isinstance(command_entry.get("command"), str)
    ):
        command_name = str(command_entry["command"])
        payload = command_entry.get("input")
    elif isinstance(command_entry, dict) and len(command_entry) == 1:
        command_name, payload = next(iter(command_entry.items()))
    else:
        raise BridgeError(f"Unsupported flow command format: {command_entry!r}")

    if command_name == "tapOn":
        tap = manager._idb_tap_selector(reservation=reservation, selector=payload, run_dir=step_run_dir)  # noqa: SLF001
        return {"ok": True, "message": f"tapOn {payload!r} -> {tap['center']}"}
    if command_name == "longPressOn":
        tap = manager._idb_tap_selector(  # noqa: SLF001
            reservation=reservation,
            selector=payload,
            run_dir=step_run_dir,
            duration=0.8,
        )
        return {"ok": True, "message": f"longPressOn {payload!r} -> {tap['center']}"}
    if command_name == "inputText":
        if not isinstance(payload, str):
            raise BridgeError("inputText requires a string payload.")
        result = manager._run_idb_cli(  # noqa: SLF001
            argv=["ui", "text", "--udid", reservation.device_id, payload],
            cwd=Path(reservation.launch_path),
        )
        manager._write_driver_result(  # noqa: SLF001
            run_dir=step_run_dir,
            kind="text",
            returncode=result.returncode,
            stdout=result.stdout or "",
            stderr=result.stderr or "",
            metadata={"device_id": reservation.device_id, "length": len(payload)},
        )
        if result.returncode != 0:
            raise BridgeError((result.stderr or result.stdout or "idb ui text failed").strip())
        return {"ok": True, "message": f"inputText {payload!r}"}
    if command_name == "takeScreenshot":
        requested_name = payload if isinstance(payload, str) and payload.strip() else f"step-{step_index:02d}.png"
        destination = step_run_dir / Path(requested_name).name
        result = manager._run_idb_cli(  # noqa: SLF001
            argv=["screenshot", "--udid", reservation.device_id, str(destination)],
            cwd=Path(reservation.launch_path),
        )
        manager._write_driver_result(  # noqa: SLF001
            run_dir=step_run_dir,
            kind="screenshot",
            returncode=result.returncode,
            stdout=result.stdout or "",
            stderr=result.stderr or "",
            metadata={"device_id": reservation.device_id, "path": str(destination)},
        )
        if result.returncode != 0:
            raise BridgeError((result.stderr or result.stdout or "idb screenshot failed").strip())
        return {"ok": True, "message": f"takeScreenshot {destination}", "screenshot": str(destination)}
    if command_name == "swipe":
        if not isinstance(payload, dict):
            raise BridgeError("swipe requires an object payload.")
        try:
            x_start = int(payload["x_start"])
            y_start = int(payload["y_start"])
            x_end = int(payload["x_end"])
            y_end = int(payload["y_end"])
        except Exception as error:
            raise BridgeError("swipe requires integer x_start, y_start, x_end, and y_end.") from error
        argv = ["ui", "swipe"]
        duration = payload.get("duration")
        if duration is not None:
            argv.extend(["--duration", str(duration)])
        elements = manager._idb_describe_all(reservation=reservation)  # noqa: SLF001
        swipe_start = manager._idb_tap_coordinates_for_accessibility_point(  # noqa: SLF001
            reservation=reservation,
            elements=elements,
            point=(x_start, y_start),
        )
        swipe_end = manager._idb_tap_coordinates_for_accessibility_point(  # noqa: SLF001
            reservation=reservation,
            elements=elements,
            point=(x_end, y_end),
        )
        argv.extend(
            [
                "--udid",
                reservation.device_id,
                str(swipe_start[0]),
                str(swipe_start[1]),
                str(swipe_end[0]),
                str(swipe_end[1]),
            ]
        )
        result = manager._run_idb_cli(argv=argv, cwd=Path(reservation.launch_path))  # noqa: SLF001
        manager._write_driver_result(  # noqa: SLF001
            run_dir=step_run_dir,
            kind="swipe",
            returncode=result.returncode,
            stdout=result.stdout or "",
            stderr=result.stderr or "",
            metadata={
                "device_id": reservation.device_id,
                "start": [x_start, y_start],
                "end": [x_end, y_end],
                "swipe_start": list(swipe_start),
                "swipe_end": list(swipe_end),
            },
        )
        if result.returncode != 0:
            raise BridgeError((result.stderr or result.stdout or "idb ui swipe failed").strip())
        return {"ok": True, "message": f"swipe [{x_start},{y_start}] -> [{x_end},{y_end}]"}
    if command_name in {"eraseText", "forwardEraseText"}:
        count = 1
        if payload is not None:
            try:
                count = int(payload)
            except Exception as error:
                raise BridgeError(f"{command_name} payload must be an integer.") from error
        keycode = "42" if command_name == "eraseText" else "76"
        for key_index in range(count):
            result = manager._run_idb_cli(  # noqa: SLF001
                argv=["ui", "key", "--udid", reservation.device_id, keycode],
                cwd=Path(reservation.launch_path),
            )
            if result.returncode != 0:
                manager._write_driver_result(  # noqa: SLF001
                    run_dir=step_run_dir,
                    kind=command_name,
                    returncode=result.returncode,
                    stdout=result.stdout or "",
                    stderr=result.stderr or "",
                    metadata={"device_id": reservation.device_id, "count": count, "attempt": key_index + 1},
                )
                raise BridgeError((result.stderr or result.stdout or f"idb ui key {keycode} failed").strip())
        manager._write_driver_result(  # noqa: SLF001
            run_dir=step_run_dir,
            kind=command_name,
            returncode=0,
            stdout="",
            stderr="",
            metadata={"device_id": reservation.device_id, "count": count},
        )
        return {"ok": True, "message": f"{command_name} {count}"}
    if command_name == "hideKeyboard":
        result = manager._run_idb_cli(  # noqa: SLF001
            argv=["ui", "key", "--udid", reservation.device_id, "ESCAPE"],
            cwd=Path(reservation.launch_path),
        )
        manager._write_driver_result(  # noqa: SLF001
            run_dir=step_run_dir,
            kind="hideKeyboard",
            returncode=result.returncode,
            stdout=result.stdout or "",
            stderr=result.stderr or "",
            metadata={"device_id": reservation.device_id},
        )
        if result.returncode != 0:
            raise BridgeError((result.stderr or result.stdout or "idb hide keyboard failed").strip())
        return {"ok": True, "message": "hideKeyboard"}
    if command_name == "clearField":
        if not isinstance(payload, dict) or not payload:
            raise BridgeError("clearField requires a selector object.")
        fallback_erase = payload.get("fallbackErase", 100)
        try:
            fallback_count = int(fallback_erase)
        except Exception as error:
            raise BridgeError("clearField fallbackErase must be an integer.") from error
        selector = {key: value for key, value in payload.items() if key != "fallbackErase"}
        steps = [
            {"tapOn": selector},
            {"eraseText": fallback_count},
            {"forwardEraseText": fallback_count},
        ]
        messages: list[str] = []
        for nested_index, nested_command in enumerate(steps, start=1):
            nested = _execute_driver_command_entry(
                manager=manager,
                reservation=reservation,
                command_entry=nested_command,
                run_dir=step_run_dir,
                step_index=nested_index,
            )
            messages.append(nested.get("message") or f"nested step {nested_index} ok")
        return {"ok": True, "message": "; ".join(messages)}
    raise BridgeError(f"Unsupported idb flow command: {command_name}")


def _run_driver_flow(
    *,
    manager: FlutterSimManager,
    reservation,
    commands: list[Any],
    label: str | None,
) -> dict[str, Any]:
    if not isinstance(commands, list) or not commands:
        return {"ok": False, "message": "Flow commands must be a non-empty list."}
    run_dir = _run_dir(manager, reservation, kind="flow")
    command_log = run_dir / "flow.json"
    command_log.write_text(json.dumps(commands, indent=2), encoding="utf-8")
    stdout_lines: list[str] = []
    screenshots: list[str] = []
    with manager._driver_lock_for_device(reservation.device_id):  # noqa: SLF001
        try:
            for index, command_entry in enumerate(commands, start=1):
                response = _execute_driver_command_entry(
                    manager=manager,
                    reservation=reservation,
                    command_entry=command_entry,
                    run_dir=run_dir,
                    step_index=index,
                )
                stdout_lines.append(response.get("message") or f"step {index} ok")
                screenshot_path = response.get("screenshot")
                if isinstance(screenshot_path, str) and screenshot_path:
                    screenshots.append(screenshot_path)
        except BridgeError as error:
            manager._write_driver_result(  # noqa: SLF001
                run_dir=run_dir,
                kind="flow",
                returncode=1,
                stdout="\n".join(stdout_lines),
                stderr=str(error),
                metadata={"device_id": reservation.device_id, "label": label, "steps": len(commands)},
            )
            return {
                "ok": False,
                "message": str(error),
                "stdout": "\n".join(stdout_lines),
                "stderr": str(error),
                "artifacts": [str(command_log), str(run_dir / "stdout.log"), str(run_dir / "stderr.log"), str(run_dir / "result.json")],
            }
    manager._write_driver_result(  # noqa: SLF001
        run_dir=run_dir,
        kind="flow",
        returncode=0,
        stdout="\n".join(stdout_lines),
        stderr="",
        metadata={"device_id": reservation.device_id, "label": label, "steps": len(commands), "screenshots": screenshots},
    )
    return {
        "ok": True,
        "device_id": reservation.device_id,
        "message": f"Executed driver flow on device {reservation.device_id}.",
        "artifacts": [str(command_log), str(run_dir / "stdout.log"), str(run_dir / "stderr.log"), str(run_dir / "result.json"), *screenshots],
        "screenshots": screenshots,
    }


def maestro_command(
    *,
    manager: FlutterSimManager,
    device_id: str,
    command: str,
    input_payload: Any | None,
    label: str | None,
    out_path: str | None,
) -> dict[str, Any]:
    normalized = manager._normalize_device_id(device_id)  # noqa: SLF001
    try:
        reservation = _ready_reservation(manager, normalized)
    except BridgeError as error:
        return {"ok": False, "message": str(error)}

    command = command.strip()
    if not command:
        return {"ok": False, "message": "command is required."}
    if command == "get_health":
        return {"ok": True, "device_id": normalized, "result": {"status": "ok"}}
    if command == "clearField":
        if not isinstance(input_payload, dict) or not input_payload:
            return {"ok": False, "message": "clearField requires a selector object."}
        selector = dict(input_payload)
        fallback_erase = input_payload.get("fallbackErase", 100)
        try:
            fallback_count = int(fallback_erase)
        except Exception:
            return {"ok": False, "message": "clearField fallbackErase must be an integer."}
        return _run_driver_flow(
            manager=manager,
            reservation=reservation,
            commands=[{"tapOn": selector}, {"eraseText": fallback_count}, {"forwardEraseText": fallback_count}],
            label=label or command,
        )
    if command == "takeScreenshot":
        response = _run_driver_flow(
            manager=manager,
            reservation=reservation,
            commands=[{"takeScreenshot": out_path or "screenshot.png"}],
            label=label or command,
        )
        if not response.get("ok", False):
            return response
        screenshots = response.get("screenshots") or []
        screenshot_path = screenshots[0] if screenshots else None
        if not screenshot_path:
            return {"ok": False, "message": "idb did not produce a screenshot artifact."}
        image_bytes = Path(screenshot_path).read_bytes()
        return {
            "ok": True,
            "device_id": normalized,
            "path": screenshot_path,
            "content_type": "image/png",
            "data_base64": base64.b64encode(image_bytes).decode("ascii"),
            "artifacts": response.get("artifacts", []),
        }
    return _run_driver_flow(
        manager=manager,
        reservation=reservation,
        commands=[{command: input_payload} if input_payload is not None else command],
        label=label or command,
    )


def maestro_flow(
    *,
    manager: FlutterSimManager,
    device_id: str,
    commands: list[Any],
    label: str | None,
) -> dict[str, Any]:
    normalized = manager._normalize_device_id(device_id)  # noqa: SLF001
    try:
        reservation = _ready_reservation(manager, normalized)
    except BridgeError as error:
        return {"ok": False, "message": str(error)}
    return _run_driver_flow(
        manager=manager,
        reservation=reservation,
        commands=commands,
        label=label,
    )
