from __future__ import annotations

import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Any

from idb_accessibility import element_area
from idb_accessibility import element_frame_bounds
from idb_accessibility import element_center
from idb_accessibility import element_role_priority
from idb_accessibility import find_idb_element
from idb_accessibility import orientation_metadata_from_elements
from idb_accessibility import root_frame_size
from idb_accessibility import screen_dimensions_from_describe_output
from idb_accessibility import ensure_nonzero_element_frame
from idb_accessibility import resolve_tap_point


class ScreenshotCropError(RuntimeError):
    pass


def resolve_magick() -> str:
    preferred = Path("/opt/homebrew/bin/magick")
    if preferred.exists():
        return str(preferred)
    resolved = shutil.which("magick")
    if resolved:
        return resolved
    raise ScreenshotCropError("magick is not installed or not available on PATH.")


def run_process(args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(args, capture_output=True, text=True)


def resolve_idb() -> str:
    preferred = Path.home() / ".local" / "bin" / "idb"
    if preferred.exists():
        return str(preferred)
    resolved = shutil.which("idb")
    if resolved:
        return resolved
    raise ScreenshotCropError("idb is not installed or not available on PATH.")


def image_dimensions(magick_bin: str, path: Path) -> tuple[int, int]:
    result = run_process([magick_bin, "identify", "-format", "%w %h", str(path)])
    if result.returncode != 0:
        raise ScreenshotCropError((result.stderr or result.stdout or "magick identify failed").strip())
    try:
        width_text, height_text = (result.stdout or "").strip().split()
        return int(width_text), int(height_text)
    except Exception as error:
        raise ScreenshotCropError(f"could not parse image dimensions for {path}") from error


def parse_selector(raw: str) -> object:
    raw = raw.strip()
    if not raw:
        raise ScreenshotCropError("selector cannot be empty.")
    if raw.startswith("{"):
        try:
            parsed = json.loads(raw)
        except json.JSONDecodeError as error:
            raise ScreenshotCropError(f"selector is not valid JSON: {error}") from error
        if not isinstance(parsed, dict):
            raise ScreenshotCropError("selector JSON must be an object.")
        return parsed
    return raw


def live_hierarchy_elements(device_id: str) -> list[dict[str, Any]]:
    result = run_process(["designer-drive", "--json", "hierarchy", "--device-id", device_id])
    if result.returncode != 0:
        raise ScreenshotCropError((result.stderr or result.stdout or "designer-drive hierarchy failed").strip())
    try:
        payload = json.loads(result.stdout or "{}")
    except json.JSONDecodeError as error:
        raise ScreenshotCropError(f"designer-drive hierarchy returned invalid JSON: {error}") from error
    raw_hierarchy = payload.get("hierarchy")
    if not isinstance(raw_hierarchy, str):
        raise ScreenshotCropError("designer-drive hierarchy JSON is missing the hierarchy payload.")
    try:
        parsed = json.loads(raw_hierarchy)
    except json.JSONDecodeError as error:
        raise ScreenshotCropError(f"hierarchy payload is not valid JSON: {error}") from error
    if not isinstance(parsed, list):
        raise ScreenshotCropError("hierarchy payload is not a JSON array.")
    return [item for item in parsed if isinstance(item, dict)]


def idb_describe_output(device_id: str) -> str:
    result = run_process([resolve_idb(), "describe", "--udid", device_id])
    if result.returncode != 0:
        raise ScreenshotCropError((result.stderr or result.stdout or "idb describe failed").strip())
    return result.stdout or ""


def describe_point(device_id: str, point: tuple[int, int]) -> dict[str, Any] | None:
    result = run_process(
        [resolve_idb(), "ui", "describe-point", "--json", "--udid", device_id, str(point[0]), str(point[1])]
    )
    if result.returncode != 0 or not (result.stdout or "").strip():
        return None
    try:
        payload = json.loads(result.stdout)
    except json.JSONDecodeError:
        return None
    return payload if isinstance(payload, dict) else None


def orientation_probe_elements(elements: list[dict[str, Any]]) -> list[dict[str, Any]]:
    candidates: list[dict[str, Any]] = []
    for index, element in enumerate(elements):
        if index == 0:
            continue
        if (element.get("AXLabel") or "").strip() == "keyboard-frame":
            continue
        frame = element.get("frame")
        if not isinstance(frame, dict):
            continue
        try:
            ensure_nonzero_element_frame(element=element, selector=element.get("AXUniqueId") or element.get("AXLabel") or "<probe>")
        except Exception:
            continue
        candidates.append(element)
    return sorted(
        candidates,
        key=lambda element: (
            element_role_priority(element),
            element_area(element),
        ),
    )


def infer_orientation_transform(
    *,
    device_id: str,
    elements: list[dict[str, Any]],
) -> str | None:
    describe_output = idb_describe_output(device_id)
    try:
        portrait_width, portrait_height = screen_dimensions_from_describe_output(describe_output)
        root_width, root_height = root_frame_size(elements=elements)
    except Exception as error:
        raise ScreenshotCropError(str(error)) from error
    for element in orientation_probe_elements(elements):
        center = element_center(element)
        if center is None:
            continue
        try:
            _tap_point, _probes, _probed, transform = resolve_tap_point(
                point=center,
                expected_element=element,
                portrait_width=portrait_width,
                portrait_height=portrait_height,
                root_width=root_width,
                root_height=root_height,
                probe_point=lambda candidate: describe_point(device_id, candidate),
            )
        except Exception:
            continue
        if transform:
            return transform
    return None


def rotate_image_in_place(*, image_path: Path, degrees: int) -> None:
    magick_bin = resolve_magick()
    suffix = image_path.suffix or ".png"
    with tempfile.NamedTemporaryFile(dir=str(image_path.parent), suffix=suffix, delete=False) as handle:
        temp_path = Path(handle.name)
    try:
        result = run_process([magick_bin, str(image_path), "-rotate", str(degrees), str(temp_path)])
        if result.returncode != 0:
            raise ScreenshotCropError((result.stderr or result.stdout or "magick rotate failed").strip())
        os.replace(temp_path, image_path)
    finally:
        if temp_path.exists():
            temp_path.unlink(missing_ok=True)


def normalize_fresh_screenshot_orientation(*, image_path: Path, device_id: str) -> dict[str, Any] | None:
    elements = live_hierarchy_elements(device_id)
    try:
        root_width, root_height = root_frame_size(elements=elements)
    except Exception as error:
        raise ScreenshotCropError(str(error)) from error
    magick_bin = resolve_magick()
    image_width, image_height = image_dimensions(magick_bin, image_path)
    orientation_metadata = orientation_metadata_from_elements(elements)
    transform = (
        str(orientation_metadata["transform"])
        if orientation_metadata is not None
        else infer_orientation_transform(device_id=device_id, elements=elements)
    )
    if transform is None:
        return None
    rotation: int | None = None
    if transform == "portrait_180":
        rotation = 180
    elif transform == "landscape_90" and image_width < image_height and root_width > root_height:
        rotation = 270
    elif transform == "landscape_270" and image_width < image_height and root_width > root_height:
        rotation = 90
    if rotation is None:
        metadata: dict[str, Any] = {
            "transform": transform,
            "rotated": False,
            "image_width": image_width,
            "image_height": image_height,
        }
        if orientation_metadata is not None:
            metadata["orientation"] = orientation_metadata
        return metadata
    rotate_image_in_place(image_path=image_path, degrees=rotation)
    normalized_width, normalized_height = image_dimensions(magick_bin, image_path)
    metadata = {
        "transform": transform,
        "rotation_degrees": rotation,
        "rotated": True,
        "image_width": normalized_width,
        "image_height": normalized_height,
    }
    if orientation_metadata is not None:
        metadata["orientation"] = orientation_metadata
    return metadata


def compute_exact_box(
    *,
    image_width: int,
    image_height: int,
    x: int,
    y: int,
    width: int,
    height: int,
) -> tuple[int, int, int, int]:
    if width <= 0 or height <= 0:
        raise ScreenshotCropError("width and height must be positive.")
    if x < 0 or y < 0:
        raise ScreenshotCropError("x and y must be non-negative.")
    if x >= image_width or y >= image_height:
        raise ScreenshotCropError("crop origin is outside the image bounds.")
    width = min(width, image_width - x)
    height = min(height, image_height - y)
    if width <= 0 or height <= 0:
        raise ScreenshotCropError("crop box is empty after clamping to image bounds.")
    return x, y, width, height


def scale_hierarchy_box_to_image(
    *,
    image_width: int,
    image_height: int,
    hierarchy_width: int,
    hierarchy_height: int,
    frame_left: float,
    frame_top: float,
    frame_right: float,
    frame_bottom: float,
) -> tuple[int, int, int, int]:
    if hierarchy_width <= 0 or hierarchy_height <= 0:
        raise ScreenshotCropError("hierarchy root frame has invalid size.")
    scale_x = image_width / hierarchy_width
    scale_y = image_height / hierarchy_height
    x = int(round(frame_left * scale_x))
    y = int(round(frame_top * scale_y))
    right = int(round(frame_right * scale_x))
    bottom = int(round(frame_bottom * scale_y))
    width = max(1, right - x)
    height = max(1, bottom - y)
    return compute_exact_box(
        image_width=image_width,
        image_height=image_height,
        x=x,
        y=y,
        width=width,
        height=height,
    )


def compute_selector_box(
    *,
    image_width: int,
    image_height: int,
    device_id: str,
    selector: object,
) -> tuple[int, int, int, int]:
    elements = live_hierarchy_elements(device_id)
    try:
        element = find_idb_element(elements=elements, selector=selector)
    except Exception as error:
        raise ScreenshotCropError(str(error)) from error
    bounds = element_frame_bounds(element)
    if bounds is None:
        raise ScreenshotCropError(f"element {selector!r} does not have a usable frame.")
    try:
        hierarchy_width, hierarchy_height = root_frame_size(elements=elements)
    except Exception as error:
        raise ScreenshotCropError(str(error)) from error
    return scale_hierarchy_box_to_image(
        image_width=image_width,
        image_height=image_height,
        hierarchy_width=hierarchy_width,
        hierarchy_height=hierarchy_height,
        frame_left=bounds[0],
        frame_top=bounds[1],
        frame_right=bounds[2],
        frame_bottom=bounds[3],
    )


def crop_image_to_box(*, image_path: Path, out_path: Path, x: int, y: int, width: int, height: int) -> None:
    magick_bin = resolve_magick()
    result = run_process(
        [
            magick_bin,
            str(image_path),
            "-crop",
            f"{width}x{height}+{x}+{y}",
            "+repage",
            str(out_path),
        ]
    )
    if result.returncode != 0:
        raise ScreenshotCropError((result.stderr or result.stdout or "magick crop failed").strip())


def crop_image_by_selector(*, image_path: Path, out_path: Path, device_id: str, selector: object) -> dict[str, int]:
    magick_bin = resolve_magick()
    image_width, image_height = image_dimensions(magick_bin, image_path)
    x, y, width, height = compute_selector_box(
        image_width=image_width,
        image_height=image_height,
        device_id=device_id,
        selector=selector,
    )
    crop_image_to_box(image_path=image_path, out_path=out_path, x=x, y=y, width=width, height=height)
    return {
        "x": x,
        "y": y,
        "width": width,
        "height": height,
        "image_width": image_width,
        "image_height": image_height,
    }
