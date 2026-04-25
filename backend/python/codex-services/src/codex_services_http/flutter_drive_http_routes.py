from __future__ import annotations

import asyncio
import base64
import json
import logging
import time
from pathlib import Path
from typing import Any

from .bridge import BridgeError
from .flutter_drive_service import FlutterDriveService
from .idb_accessibility import AccessibilityError as SharedAccessibilityError
from .idb_accessibility import best_element_containing_point as shared_best_element_containing_point
from .idb_accessibility import best_matching_element as shared_best_matching_element
from .idb_accessibility import element_area as shared_element_area
from .idb_accessibility import element_contains_point as shared_element_contains_point
from .idb_accessibility import element_frame_bounds as shared_element_frame_bounds
from .idb_accessibility import element_role_priority as shared_element_role_priority
from .idb_accessibility import ensure_nonzero_element_frame as shared_ensure_nonzero_element_frame
from .idb_accessibility import find_idb_element as shared_find_idb_element
from .idb_accessibility import matching_elements_for_selector as shared_matching_elements_for_selector
from .idb_accessibility import normalized_accessibility_strings as shared_normalized_accessibility_strings
from .idb_accessibility import normalized_swipe_duration as shared_normalized_swipe_duration
from .idb_accessibility import orientation_metadata_from_elements as shared_orientation_metadata_from_elements
from .idb_accessibility import point_candidates as shared_point_candidates
from .idb_accessibility import probe_matches_element as shared_probe_matches_element
from .idb_accessibility import resolve_tap_point as shared_resolve_tap_point
from .idb_accessibility import root_frame_size as shared_root_frame_size
from .idb_accessibility import selector_candidates as shared_selector_candidates
from .idb_accessibility import transform_accessibility_point as shared_transform_accessibility_point

from idb.common.hid import _key_down_event
from idb.common.hid import _key_up_event
from idb.common.hid import key_press_to_events
from idb.common.hid import text_to_events
from idb.common.types import DomainSocketAddress
from idb.common.types import HIDDelay
from idb.grpc.client import Client


LEFT_COMMAND_KEYCODE = 227
A_KEYCODE = 4
BACKSPACE_KEYCODE = 42
ESCAPE_KEYCODE = 41
POST_TAP_HIERARCHY_DELAY_SECONDS = 1.0
SWIPE_DURATION_MILLISECONDS_THRESHOLD = 10.0


def healthz(*, service: FlutterDriveService) -> dict[str, Any]:
    return service.health()


def devices(*, service: FlutterDriveService) -> dict[str, Any]:
    return {"ok": True, "devices": service.devices()}


def _ready_reservation(service: FlutterDriveService, device_id: str):
    return service._ready_reservation(device_id)  # noqa: SLF001


def _run_dir(service: FlutterDriveService, reservation, *, kind: str) -> Path:
    return service.manager._next_driver_run_dir(reservation, kind=kind)  # noqa: SLF001


def _post_action_hierarchy(service: FlutterDriveService, *, reservation) -> str:
    time.sleep(POST_TAP_HIERARCHY_DELAY_SECONDS)
    elements = service.manager._idb_describe_all(reservation=reservation)  # noqa: SLF001
    return service.manager._serialize_idb_hierarchy(elements)  # noqa: SLF001


def _normalized_swipe_duration(raw_duration: Any) -> float:
    try:
        return shared_normalized_swipe_duration(raw_duration)
    except SharedAccessibilityError as error:
        raise BridgeError(str(error)) from error


def _idb_point_candidates(
    *,
    portrait_width: int,
    portrait_height: int,
    point: tuple[int, int],
    root_width: int,
    root_height: int,
) -> list[tuple[str, tuple[int, int]]]:
    return shared_point_candidates(
        portrait_width=portrait_width,
        portrait_height=portrait_height,
        point=point,
        root_width=root_width,
        root_height=root_height,
    )


def _idb_probe_point(service: FlutterDriveService, *, reservation, point: tuple[int, int]) -> dict[str, Any] | None:
    result = service.manager._run_idb_cli(  # noqa: SLF001
        argv=["ui", "describe-point", "--json", "--udid", reservation.device_id, str(point[0]), str(point[1])],
        cwd=Path(reservation.launch_path),
    )
    if result.returncode != 0 or not (result.stdout or "").strip():
        return None
    try:
        parsed = json.loads(result.stdout)
    except json.JSONDecodeError:
        return None
    return parsed if isinstance(parsed, dict) else None


def _idb_probe_matches_element(*, probed: dict[str, Any] | None, element: dict[str, Any]) -> bool:
    return shared_probe_matches_element(probed=probed, element=element)


def _element_role_priority(element: dict[str, Any]) -> int:
    return shared_element_role_priority(element)


def _element_frame_bounds(element: dict[str, Any]) -> tuple[float, float, float, float] | None:
    return shared_element_frame_bounds(element)


def _element_area(element: dict[str, Any]) -> float:
    return shared_element_area(element)


def _matching_elements_for_selector(*, elements: list[dict[str, Any]], selector: Any) -> list[dict[str, Any]]:
    return shared_matching_elements_for_selector(elements=elements, selector=selector)


def _best_matching_element(matches: list[dict[str, Any]]) -> dict[str, Any]:
    try:
        return shared_best_matching_element(matches)
    except SharedAccessibilityError as error:
        raise BridgeError(str(error)) from error


def _element_contains_point(*, element: dict[str, Any], point: tuple[int, int]) -> bool:
    return shared_element_contains_point(element=element, point=point)


def _best_element_containing_point(*, elements: list[dict[str, Any]], point: tuple[int, int]) -> dict[str, Any] | None:
    return shared_best_element_containing_point(elements=elements, point=point)


def _normalized_accessibility_strings(value: str) -> list[str]:
    return shared_normalized_accessibility_strings(value)


def _route_selector_candidates(selector: Any) -> list[tuple[str, str]]:
    try:
        return shared_selector_candidates(selector)
    except SharedAccessibilityError as error:
        raise BridgeError(str(error)) from error


def _route_find_idb_element(*, elements: list[dict[str, Any]], selector: Any) -> dict[str, Any]:
    try:
        return shared_find_idb_element(elements=elements, selector=selector)
    except SharedAccessibilityError as error:
        raise BridgeError(str(error)) from error


def _idb_root_frame_size(*, elements: list[dict[str, Any]]) -> tuple[int, int]:
    try:
        return shared_root_frame_size(elements=elements)
    except SharedAccessibilityError as error:
        raise BridgeError(str(error)) from error


def _ensure_nonzero_element_frame(*, element: dict[str, Any], selector: Any) -> None:
    try:
        shared_ensure_nonzero_element_frame(element=element, selector=selector)
    except SharedAccessibilityError as error:
        raise BridgeError(str(error)) from error


def _idb_resolve_tap_point(
    *,
    service: FlutterDriveService,
    reservation,
    elements: list[dict[str, Any]],
    point: tuple[int, int],
    expected_element: dict[str, Any] | None = None,
) -> tuple[tuple[int, int], list[dict[str, Any]], dict[str, Any] | None, str | None]:
    manager = service.manager
    portrait_width, portrait_height = manager._idb_screen_dimensions_points(reservation=reservation)  # noqa: SLF001
    root_width, root_height = _idb_root_frame_size(elements=elements)
    try:
        orientation_metadata = shared_orientation_metadata_from_elements(elements)
        if orientation_metadata is not None:
            tap_point = shared_transform_accessibility_point(
                point=point,
                portrait_width=portrait_width,
                portrait_height=portrait_height,
                transform=str(orientation_metadata["transform"]),
            )
            return (
                tap_point,
                [
                    {
                        "transform": orientation_metadata["transform"],
                        "candidate": list(tap_point),
                        "matched": True,
                        "source": "orientationBeacon",
                    }
                ],
                None,
                str(orientation_metadata["transform"]),
            )
        return shared_resolve_tap_point(
            point=point,
            expected_element=expected_element,
            portrait_width=portrait_width,
            portrait_height=portrait_height,
            root_width=root_width,
            root_height=root_height,
            probe_point=lambda candidate: _idb_probe_point(service, reservation=reservation, point=candidate),
        )
    except SharedAccessibilityError as error:
        raise BridgeError(str(error)) from error


def _apply_transform(
    *,
    transform_name: str | None,
    point: tuple[int, int],
    portrait_width: int,
    portrait_height: int,
) -> tuple[int, int]:
    x, y = point
    if transform_name == "portrait_0":
        return x, y
    if transform_name == "portrait_180":
        return portrait_width - x, portrait_height - y
    if transform_name == "landscape_90":
        return portrait_width - y, x
    if transform_name == "landscape_270":
        return y, portrait_height - x
    return x, y


def _idb_reference_element(elements: list[dict[str, Any]], manager) -> dict[str, Any] | None:
    for element in elements:
        center = manager._idb_element_center(element)  # noqa: SLF001
        bounds = manager._idb_element_frame_bounds(element)  # noqa: SLF001
        if center is None or bounds is None:
            continue
        left, top, right, bottom = bounds
        if right <= left or bottom <= top:
            continue
        return element
    return None


def _idb_tap_selector(
    *,
    service: FlutterDriveService,
    reservation,
    selector: Any,
    run_dir: Path,
    duration: float | None = None,
) -> dict[str, Any]:
    manager = service.manager
    elements = manager._idb_describe_all(reservation=reservation)  # noqa: SLF001
    element = _route_find_idb_element(elements=elements, selector=selector)
    _ensure_nonzero_element_frame(element=element, selector=selector)
    center = manager._idb_element_center(element)  # noqa: SLF001
    if center is None:
        raise BridgeError(f"Element {selector!r} does not have a usable frame.")
    tap_point, probes, _, _ = _idb_resolve_tap_point(
        service=service,
        reservation=reservation,
        elements=elements,
        point=center,
        expected_element=element,
    )
    argv = ["ui", "tap"]
    if duration is not None:
        argv.extend(["--duration", str(duration)])
    argv.extend(["--udid", reservation.device_id, str(tap_point[0]), str(tap_point[1])])
    result = manager._run_idb_cli(argv=argv, cwd=Path(reservation.launch_path))  # noqa: SLF001
    manager._write_driver_result(  # noqa: SLF001
        run_dir=run_dir,
        kind="tap",
        returncode=result.returncode,
        stdout=result.stdout or "",
        stderr=result.stderr or "",
        metadata={
            "device_id": reservation.device_id,
            "selector": selector,
            "center": list(center),
            "tap_point": list(tap_point),
            "probes": probes,
        },
    )
    if result.returncode != 0:
        raise BridgeError((result.stderr or result.stdout or "idb ui tap failed").strip())
    return {"center": center, "tap_point": tap_point, "element": element}


def _idb_tap_point(
    *,
    service: FlutterDriveService,
    reservation,
    point: tuple[int, int],
    run_dir: Path,
    duration: float | None = None,
) -> dict[str, Any]:
    manager = service.manager
    elements = manager._idb_describe_all(reservation=reservation)  # noqa: SLF001
    expected_element = _best_element_containing_point(elements=elements, point=point)
    tap_point, probes, probed, _ = _idb_resolve_tap_point(
        service=service,
        reservation=reservation,
        elements=elements,
        point=point,
        expected_element=expected_element,
    )
    argv = ["ui", "tap"]
    if duration is not None:
        argv.extend(["--duration", str(duration)])
    argv.extend(["--udid", reservation.device_id, str(tap_point[0]), str(tap_point[1])])
    result = manager._run_idb_cli(argv=argv, cwd=Path(reservation.launch_path))  # noqa: SLF001
    manager._write_driver_result(  # noqa: SLF001
        run_dir=run_dir,
        kind="tap-point",
        returncode=result.returncode,
        stdout=result.stdout or "",
        stderr=result.stderr or "",
        metadata={
            "device_id": reservation.device_id,
            "point": list(point),
            "tap_point": list(tap_point),
            "probes": probes,
        },
    )
    if result.returncode != 0:
        raise BridgeError((result.stderr or result.stdout or "idb ui tap failed").strip())
    return {"point": point, "tap_point": tap_point, "element": probed or expected_element}


def _compact_element_summary(element: dict[str, Any]) -> str:
    label = (element.get("AXLabel") or "").strip()
    value = element.get("AXValue")
    unique_id = (element.get("AXUniqueId") or "").strip()
    role = (element.get("role_description") or element.get("type") or "").strip()
    frame = element.get("frame") or {}
    try:
        left = int(round(float(frame.get("x", 0))))
        top = int(round(float(frame.get("y", 0))))
        width = int(round(float(frame.get("width", 0))))
        height = int(round(float(frame.get("height", 0))))
    except Exception:
        left = top = width = height = 0
    right = left + width
    bottom = top + height
    primary = label or (value.strip() if isinstance(value, str) and value.strip() else "") or unique_id or role or "<node>"
    parts = [primary]
    if unique_id:
        parts.append(f"id=`{unique_id}`")
    if isinstance(value, str) and value.strip() and value.strip() != primary:
        parts.append(f"value={json.dumps(value)}")
    if role and role.casefold() not in primary.casefold():
        parts.append(f"role={json.dumps(role)}")
    parts.append(f"[{left},{top}][{right},{bottom}]")
    return " ".join(parts)


async def _idb_cmd_a_delete_async(*, device_id: str) -> None:
    companion_path = Path("/tmp/idb") / f"{device_id}_companion.sock"
    if not companion_path.exists():
        raise BridgeError(f"idb companion socket not found for device {device_id}: {companion_path}")

    logger = logging.getLogger("flutter-drive-http.idb-hid")
    async with Client.build(
        address=DomainSocketAddress(path=str(companion_path)),
        logger=logger,
    ) as client:
        await client.set_hardware_keyboard(True)
        await client.send_events(
            [
                _key_down_event(LEFT_COMMAND_KEYCODE),
                _key_down_event(A_KEYCODE),
                _key_up_event(A_KEYCODE),
                _key_up_event(LEFT_COMMAND_KEYCODE),
                HIDDelay(duration=0.1),
                *key_press_to_events(BACKSPACE_KEYCODE),
            ]
        )


def _idb_cmd_a_delete(*, device_id: str) -> None:
    asyncio.run(_idb_cmd_a_delete_async(device_id=device_id))


async def _idb_key_async(*, device_id: str, keycode: int) -> None:
    companion_path = Path("/tmp/idb") / f"{device_id}_companion.sock"
    if not companion_path.exists():
        raise BridgeError(f"idb companion socket not found for device {device_id}: {companion_path}")

    logger = logging.getLogger("flutter-drive-http.idb-hid")
    async with Client.build(
        address=DomainSocketAddress(path=str(companion_path)),
        logger=logger,
    ) as client:
        await client.set_hardware_keyboard(True)
        await client.key(keycode)


def _idb_key(*, device_id: str, keycode: int) -> None:
    asyncio.run(_idb_key_async(device_id=device_id, keycode=keycode))


async def _idb_text_async(*, device_id: str, text: str) -> None:
    companion_path = Path("/tmp/idb") / f"{device_id}_companion.sock"
    if not companion_path.exists():
        raise BridgeError(f"idb companion socket not found for device {device_id}: {companion_path}")

    logger = logging.getLogger("flutter-drive-http.idb-hid")
    async with Client.build(
        address=DomainSocketAddress(path=str(companion_path)),
        logger=logger,
    ) as client:
        await client.text(text)


def _idb_text(*, device_id: str, text: str) -> None:
    asyncio.run(_idb_text_async(device_id=device_id, text=text))


async def _idb_set_hardware_keyboard_async(*, device_id: str, enabled: bool) -> None:
    companion_path = Path("/tmp/idb") / f"{device_id}_companion.sock"
    if not companion_path.exists():
        raise BridgeError(f"idb companion socket not found for device {device_id}: {companion_path}")

    logger = logging.getLogger("flutter-drive-http.idb-hid")
    async with Client.build(
        address=DomainSocketAddress(path=str(companion_path)),
        logger=logger,
    ) as client:
        await client.set_hardware_keyboard(enabled)


def _idb_set_hardware_keyboard(*, device_id: str, enabled: bool) -> None:
    asyncio.run(_idb_set_hardware_keyboard_async(device_id=device_id, enabled=enabled))


def maestro_apps(*, service: FlutterDriveService, device_id: str) -> dict[str, Any]:
    return service.driver_apps(device_id=device_id)


def maestro_hierarchy(*, service: FlutterDriveService, device_id: str) -> dict[str, Any]:
    try:
        reservation = _ready_reservation(service, device_id)
    except BridgeError as error:
        return {"ok": False, "message": str(error)}
    run_dir = _run_dir(service, reservation, kind="hierarchy")
    with service.manager._driver_lock_for_device(reservation.device_id):  # noqa: SLF001
        try:
            elements = service.manager._idb_describe_all(reservation=reservation)  # noqa: SLF001
        except BridgeError as error:
            service.manager._write_driver_result(  # noqa: SLF001
                run_dir=run_dir,
                kind="hierarchy",
                returncode=1,
                stdout="",
                stderr=str(error),
                metadata={"device_id": reservation.device_id},
            )
            return {"ok": False, "message": str(error)}
    hierarchy = service.manager._serialize_idb_hierarchy(elements)  # noqa: SLF001
    service.manager._write_driver_result(  # noqa: SLF001
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
    service: FlutterDriveService,
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

    manager = service.manager

    if command_name == "tapOn":
        tap = _idb_tap_selector(service=service, reservation=reservation, selector=payload, run_dir=step_run_dir)
        post_hierarchy = _post_action_hierarchy(service, reservation=reservation)
        return {
            "ok": True,
            "message": f"tapOn {payload!r} -> {tap['center']}",
            "tapped_description": _compact_element_summary(tap["element"]),
            "post_hierarchy": post_hierarchy,
        }
    if command_name == "tapPoint":
        if not isinstance(payload, dict):
            raise BridgeError("tapPoint requires an object payload.")
        try:
            x = int(payload["x"])
            y = int(payload["y"])
        except Exception as error:
            raise BridgeError("tapPoint requires integer x and y.") from error
        tap = _idb_tap_point(
            service=service,
            reservation=reservation,
            point=(x, y),
            run_dir=step_run_dir,
        )
        post_hierarchy = _post_action_hierarchy(service, reservation=reservation)
        return {
            "ok": True,
            "message": f"tapPoint [{x},{y}] -> {tap['tap_point']}",
            "tapped_description": _compact_element_summary(tap["element"]) if isinstance(tap.get("element"), dict) else None,
            "post_hierarchy": post_hierarchy,
        }
    if command_name == "longPressOn":
        tap = _idb_tap_selector(
            service=service,
            reservation=reservation,
            selector=payload,
            run_dir=step_run_dir,
            duration=0.8,
        )
        post_hierarchy = _post_action_hierarchy(service, reservation=reservation)
        return {
            "ok": True,
            "message": f"longPressOn {payload!r} -> {tap['center']}",
            "tapped_description": _compact_element_summary(tap["element"]),
            "post_hierarchy": post_hierarchy,
        }
    if command_name == "inputText":
        if not isinstance(payload, str):
            raise BridgeError("inputText requires a string payload.")
        try:
            _idb_text(device_id=reservation.device_id, text=payload)
            result_returncode = 0
            result_stdout = ""
            result_stderr = ""
        except Exception as error:
            result_returncode = 1
            result_stdout = ""
            result_stderr = str(error)
        manager._write_driver_result(  # noqa: SLF001
            run_dir=step_run_dir,
            kind="text",
            returncode=result_returncode,
            stdout=result_stdout,
            stderr=result_stderr,
            metadata={"device_id": reservation.device_id, "length": len(payload)},
        )
        if result_returncode != 0:
            raise BridgeError((result_stderr or result_stdout or "idb text failed").strip())
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
            argv.extend(["--duration", str(_normalized_swipe_duration(duration))])
        elements = manager._idb_describe_all(reservation=reservation)  # noqa: SLF001
        portrait_width, portrait_height = manager._idb_screen_dimensions_points(reservation=reservation)  # noqa: SLF001
        root_width, root_height = _idb_root_frame_size(elements=elements)
        reference_element = _idb_reference_element(elements, manager)
        reference_probes: list[dict[str, Any]] = []
        transform_name: str | None = None
        if reference_element is not None:
            reference_center = manager._idb_element_center(reference_element)  # noqa: SLF001
            if reference_center is not None:
                _, reference_probes, _, transform_name = _idb_resolve_tap_point(
                    service=service,
                    reservation=reservation,
                    elements=elements,
                    point=reference_center,
                    expected_element=reference_element,
                )
        if transform_name is not None:
            swipe_start = _apply_transform(
                transform_name=transform_name,
                point=(x_start, y_start),
                portrait_width=portrait_width,
                portrait_height=portrait_height,
            )
            swipe_end = _apply_transform(
                transform_name=transform_name,
                point=(x_end, y_end),
                portrait_width=portrait_width,
                portrait_height=portrait_height,
            )
        else:
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
                "root_size": [root_width, root_height],
                "portrait_size": [portrait_width, portrait_height],
                "transform": transform_name,
                "probes": reference_probes,
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
        try:
            _idb_key(device_id=reservation.device_id, keycode=ESCAPE_KEYCODE)
        except Exception as error:
            manager._write_driver_result(  # noqa: SLF001
                run_dir=step_run_dir,
                kind="hideKeyboard",
                returncode=1,
                stdout="",
                stderr=str(error),
                metadata={"device_id": reservation.device_id, "keycode": ESCAPE_KEYCODE},
            )
            if isinstance(error, BridgeError):
                raise
            raise BridgeError(str(error)) from error
        manager._write_driver_result(  # noqa: SLF001
            run_dir=step_run_dir,
            kind="hideKeyboard",
            returncode=0,
            stdout="",
            stderr="",
            metadata={"device_id": reservation.device_id, "keycode": ESCAPE_KEYCODE},
        )
        return {"ok": True, "message": "hideKeyboard"}
    if command_name == "clearField":
        if not isinstance(payload, dict) or not payload:
            raise BridgeError("clearField requires a selector object.")
        selector = dict(payload)
        tap = _idb_tap_selector(service=service, reservation=reservation, selector=selector, run_dir=step_run_dir)
        try:
            _idb_cmd_a_delete(device_id=reservation.device_id)
        except Exception as error:
            manager._write_driver_result(  # noqa: SLF001
                run_dir=step_run_dir,
                kind="clearField",
                returncode=1,
                stdout="",
                stderr=str(error),
                metadata={"device_id": reservation.device_id, "selector": selector, "center": list(tap["center"])},
            )
            if isinstance(error, BridgeError):
                raise
            raise BridgeError(str(error)) from error
        manager._write_driver_result(  # noqa: SLF001
            run_dir=step_run_dir,
            kind="clearField",
            returncode=0,
            stdout="",
            stderr="",
            metadata={"device_id": reservation.device_id, "selector": selector, "center": list(tap["center"])},
        )
        return {"ok": True, "message": f"clearField {selector!r} -> {tap['center']}"}
    raise BridgeError(f"Unsupported idb flow command: {command_name}")


def _run_driver_flow(
    *,
    service: FlutterDriveService,
    reservation,
    commands: list[Any],
    label: str | None,
) -> dict[str, Any]:
    if not isinstance(commands, list) or not commands:
        return {"ok": False, "message": "Flow commands must be a non-empty list."}
    manager = service.manager
    run_dir = _run_dir(service, reservation, kind="flow")
    command_log = run_dir / "flow.json"
    command_log.write_text(json.dumps(commands, indent=2), encoding="utf-8")
    stdout_lines: list[str] = []
    screenshots: list[str] = []
    last_tapped_description: str | None = None
    last_post_hierarchy: str | None = None
    with manager._driver_lock_for_device(reservation.device_id):  # noqa: SLF001
        try:
            for index, command_entry in enumerate(commands, start=1):
                response = _execute_driver_command_entry(
                    service=service,
                    reservation=reservation,
                    command_entry=command_entry,
                    run_dir=run_dir,
                    step_index=index,
                )
                stdout_lines.append(response.get("message") or f"step {index} ok")
                screenshot_path = response.get("screenshot")
                if isinstance(screenshot_path, str) and screenshot_path:
                    screenshots.append(screenshot_path)
                if isinstance(response.get("tapped_description"), str):
                    last_tapped_description = response["tapped_description"]
                if isinstance(response.get("post_hierarchy"), str):
                    last_post_hierarchy = response["post_hierarchy"]
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
        "tapped_description": last_tapped_description,
        "post_hierarchy": last_post_hierarchy,
    }


def maestro_command(
    *,
    service: FlutterDriveService,
    device_id: str,
    command: str,
    input_payload: Any | None,
    label: str | None,
    out_path: str | None,
) -> dict[str, Any]:
    normalized = service.manager._normalize_device_id(device_id)  # noqa: SLF001
    try:
        reservation = _ready_reservation(service, normalized)
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
        return _run_driver_flow(
            service=service,
            reservation=reservation,
            commands=[{"clearField": dict(input_payload)}],
            label=label or command,
        )
    if command == "takeScreenshot":
        response = _run_driver_flow(
            service=service,
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
        service=service,
        reservation=reservation,
        commands=[{command: input_payload} if input_payload is not None else command],
        label=label or command,
    )


def maestro_flow(
    *,
    service: FlutterDriveService,
    device_id: str,
    commands: list[Any],
    label: str | None,
) -> dict[str, Any]:
    normalized = service.manager._normalize_device_id(device_id)  # noqa: SLF001
    try:
        reservation = _ready_reservation(service, normalized)
    except BridgeError as error:
        return {"ok": False, "message": str(error)}
    return _run_driver_flow(
        service=service,
        reservation=reservation,
        commands=commands,
        label=label,
    )
