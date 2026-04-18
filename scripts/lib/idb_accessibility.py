from __future__ import annotations

import json
import re
import sys
from typing import Any, Callable


class AccessibilityError(RuntimeError):
    pass


SWIPE_DURATION_MILLISECONDS_THRESHOLD = 10.0


def compact_idb_element(element: dict[str, Any]) -> str:
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
    if element.get("enabled") is False:
        parts.append("[disabled]")
    parts.append(f"[{left},{top}][{right},{bottom}]")
    return "- " + " ".join(parts)


def compact_idb_hierarchy_lines(elements: list[dict[str, Any]]) -> list[str]:
    lines: list[str] = []
    for element in elements:
        if (element.get("AXLabel") or "").strip() == "keyboard-frame":
            continue
        lines.append(compact_idb_element(element))
    return lines


def render_raw_hierarchy(raw: str) -> str:
    raw = (raw or "").strip()
    if not raw:
        return "No hierarchy output."
    try:
        payload = json.loads(raw)
    except json.JSONDecodeError:
        return raw
    if not isinstance(payload, list):
        return raw
    return "\n".join(compact_idb_hierarchy_lines([item for item in payload if isinstance(item, dict)]))


def normalized_swipe_duration(raw_duration: Any) -> float:
    try:
        duration = float(raw_duration)
    except Exception as error:
        raise AccessibilityError("swipe duration must be numeric.") from error
    if duration <= 0:
        raise AccessibilityError("swipe duration must be greater than zero.")
    if duration >= SWIPE_DURATION_MILLISECONDS_THRESHOLD:
        return duration / 1000.0
    return duration


def normalized_accessibility_strings(value: str) -> list[str]:
    raw = value.strip()
    if not raw:
        return []
    lines = [part.strip() for part in raw.splitlines() if part.strip()]
    variants: list[str] = []
    if raw:
        variants.append(raw)
    if lines:
        variants.append("\n".join(lines))
        deduped_lines: list[str] = []
        for line in lines:
            if deduped_lines and deduped_lines[-1] == line:
                continue
            deduped_lines.append(line)
        if deduped_lines:
            variants.append("\n".join(deduped_lines))
            variants.extend(deduped_lines)
    unique: list[str] = []
    seen: set[str] = set()
    for variant in variants:
        folded = variant.casefold()
        if folded in seen:
            continue
        seen.add(folded)
        unique.append(variant)
    return unique


def selector_candidates(selector: Any) -> list[tuple[str, str]]:
    if isinstance(selector, str):
        value = selector.strip()
        if not value:
            raise AccessibilityError("Selector string cannot be empty.")
        return [("AXLabel", value), ("AXUniqueId", value), ("AXValue", value)]
    if not isinstance(selector, dict):
        raise AccessibilityError("Selector must be a string or object.")
    candidates: list[tuple[str, str]] = []
    mapping = {
        "id": "AXUniqueId",
        "text": "AXLabel",
        "label": "AXLabel",
        "value": "AXValue",
    }
    for key, field in mapping.items():
        value = selector.get(key)
        if isinstance(value, str) and value.strip():
            candidates.append((field, value.strip()))
    if not candidates:
        raise AccessibilityError("Selector object must include id, text, label, or value.")
    return candidates


def element_frame_bounds(element: dict[str, Any]) -> tuple[float, float, float, float] | None:
    frame = element.get("frame")
    if not isinstance(frame, dict):
        return None
    try:
        left = float(frame.get("x", 0))
        top = float(frame.get("y", 0))
        width = float(frame.get("width", 0))
        height = float(frame.get("height", 0))
    except Exception:
        return None
    return left, top, left + width, top + height


def element_area(element: dict[str, Any]) -> float:
    bounds = element_frame_bounds(element)
    if bounds is None:
        return float("inf")
    left, top, right, bottom = bounds
    return max(0.0, right - left) * max(0.0, bottom - top)


def element_center(element: dict[str, Any]) -> tuple[int, int] | None:
    bounds = element_frame_bounds(element)
    if bounds is None:
        return None
    left, top, right, bottom = bounds
    return int(round((left + right) / 2)), int(round((top + bottom) / 2))


def element_contains_point(*, element: dict[str, Any], point: tuple[int, int]) -> bool:
    bounds = element_frame_bounds(element)
    if bounds is None:
        return False
    left, top, right, bottom = bounds
    x, y = point
    return left <= x <= right and top <= y <= bottom


def element_role_priority(element: dict[str, Any]) -> int:
    role = str(element.get("role_description") or element.get("type") or "").strip().casefold()
    if role in {"button", "text field", "switch", "tab", "link", "cell", "checkbox"}:
        return 0
    if role in {"image", "icon"}:
        return 1
    if role in {"text", "statictext", "group"}:
        return 3
    return 2


def matching_elements_for_selector(*, elements: list[dict[str, Any]], selector: Any) -> list[dict[str, Any]]:
    candidates = selector_candidates(selector)
    matches: list[dict[str, Any]] = []
    seen_ids: set[int] = set()
    for field, expected in candidates:
        expected_variants = normalized_accessibility_strings(expected)
        expected_folded = {variant.casefold() for variant in expected_variants}
        for element in elements:
            actual = element.get(field)
            if not isinstance(actual, str):
                continue
            actual_variants = normalized_accessibility_strings(actual)
            if any(variant.casefold() in expected_folded for variant in actual_variants):
                marker = id(element)
                if marker in seen_ids:
                    continue
                seen_ids.add(marker)
                matches.append(element)
    return matches


def best_matching_element(matches: list[dict[str, Any]]) -> dict[str, Any]:
    if not matches:
        raise AccessibilityError("No matching accessibility elements were found.")
    positive_area_matches = [element for element in matches if element_area(element) > 0]
    ranked_matches = positive_area_matches or matches
    return min(
        ranked_matches,
        key=lambda element: (
            element_role_priority(element),
            element_area(element),
        ),
    )


def find_idb_element(*, elements: list[dict[str, Any]], selector: Any) -> dict[str, Any]:
    matches = matching_elements_for_selector(elements=elements, selector=selector)
    if matches:
        return best_matching_element(matches)
    raise AccessibilityError(f"Could not find an accessibility element matching {selector!r}.")


def best_element_containing_point(*, elements: list[dict[str, Any]], point: tuple[int, int]) -> dict[str, Any] | None:
    matches = [element for element in elements if element_contains_point(element=element, point=point)]
    if not matches:
        return None
    return best_matching_element(matches)


def root_frame_size(*, elements: list[dict[str, Any]]) -> tuple[int, int]:
    root_frame = elements[0].get("frame") if elements else None
    if not isinstance(root_frame, dict):
        raise AccessibilityError("Accessibility root is missing frame data.")
    try:
        return int(round(float(root_frame["width"]))), int(round(float(root_frame["height"])))
    except Exception as error:
        raise AccessibilityError("Accessibility root frame is invalid.") from error


def ensure_nonzero_element_frame(*, element: dict[str, Any], selector: Any) -> None:
    frame = element.get("frame")
    if not isinstance(frame, dict):
        raise AccessibilityError(f"Element {selector!r} is missing frame data.")
    try:
        width = float(frame.get("width", 0))
        height = float(frame.get("height", 0))
    except Exception as error:
        raise AccessibilityError(f"Element {selector!r} has invalid frame data.") from error
    if width <= 0 or height <= 0:
        raise AccessibilityError(
            f"Element {selector!r} is exported with a zero-sized accessibility frame. "
            "This usually means the matched accessibility node is offscreen or virtualized; "
            "scroll or filter until the row is visible, then retry."
        )


def screen_dimensions_from_describe_output(raw: str) -> tuple[int, int]:
    match = re.search(r"width_points=(\d+), height_points=(\d+)", raw or "")
    if not match:
        raise AccessibilityError("Could not determine idb screen point dimensions.")
    return int(match.group(1)), int(match.group(2))


def point_candidates(
    *,
    portrait_width: int,
    portrait_height: int,
    point: tuple[int, int],
    root_width: int,
    root_height: int,
) -> list[tuple[str, tuple[int, int]]]:
    x, y = point
    if root_width == portrait_width and root_height == portrait_height:
        candidates = [
            ("portrait_0", (x, y)),
            ("portrait_180", (portrait_width - x, portrait_height - y)),
        ]
    elif root_width == portrait_height and root_height == portrait_width:
        candidates = [
            ("landscape_90", (portrait_width - y, x)),
            ("landscape_270", (y, portrait_height - x)),
        ]
    else:
        candidates = [("identity", (x, y))]
    unique: list[tuple[str, tuple[int, int]]] = []
    seen: set[tuple[int, int]] = set()
    for name, candidate in candidates:
        if candidate in seen:
            continue
        seen.add(candidate)
        unique.append((name, candidate))
    return unique


def probe_matches_element(*, probed: dict[str, Any] | None, element: dict[str, Any] | None) -> bool:
    if not probed:
        return False
    if element is None:
        return True
    for field in ("AXUniqueId", "AXLabel", "AXValue"):
        expected = element.get(field)
        actual = probed.get(field)
        if isinstance(expected, str) and expected and expected == actual:
            return True
    probed_frame = probed.get("frame")
    element_frame = element.get("frame")
    return isinstance(probed_frame, dict) and isinstance(element_frame, dict) and probed_frame == element_frame


def tap_coordinates_for_accessibility_point(
    *,
    portrait_width: int,
    portrait_height: int,
    root_width: int,
    root_height: int,
    point: tuple[int, int],
) -> tuple[int, int]:
    x, y = point
    if root_width == portrait_width and root_height == portrait_height:
        return x, y
    if root_width == portrait_height and root_height == portrait_width:
        return y, x
    raise AccessibilityError(
        "Unsupported simulator orientation for idb coordinate mapping. "
        "Use portrait or the supported landscape orientation."
    )


def resolve_tap_point(
    *,
    point: tuple[int, int],
    expected_element: dict[str, Any] | None,
    portrait_width: int,
    portrait_height: int,
    root_width: int,
    root_height: int,
    probe_point: Callable[[tuple[int, int]], dict[str, Any] | None],
) -> tuple[tuple[int, int], list[dict[str, Any]], dict[str, Any] | None, str | None]:
    probes: list[dict[str, Any]] = []
    for transform_name, candidate in point_candidates(
        portrait_width=portrait_width,
        portrait_height=portrait_height,
        point=point,
        root_width=root_width,
        root_height=root_height,
    ):
        probed = probe_point(candidate)
        matched = probe_matches_element(probed=probed, element=expected_element)
        probes.append({"transform": transform_name, "candidate": list(candidate), "matched": matched, "probed": probed})
        if matched:
            return candidate, probes, probed, transform_name
    fallback = tap_coordinates_for_accessibility_point(
        portrait_width=portrait_width,
        portrait_height=portrait_height,
        root_width=root_width,
        root_height=root_height,
        point=point,
    )
    return fallback, probes, None, None


def main() -> int:
    raw = sys.stdin.read()
    print(render_raw_hierarchy(raw))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
