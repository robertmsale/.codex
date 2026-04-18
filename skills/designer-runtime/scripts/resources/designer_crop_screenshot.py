#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path


ANCHORS: dict[str, tuple[float, float]] = {
    "top_left": (0.0, 0.0),
    "top_center": (0.5, 0.0),
    "top_right": (1.0, 0.0),
    "center_left": (0.0, 0.5),
    "center": (0.5, 0.5),
    "center_right": (1.0, 0.5),
    "bottom_left": (0.0, 1.0),
    "bottom_center": (0.5, 1.0),
    "bottom_right": (1.0, 1.0),
}

PRESETS: dict[str, dict[str, object]] = {
    "bottom_right": {"anchor": "bottom_right", "width_pct": 25.0, "height_pct": 25.0},
    "header": {"anchor": "top_center", "width_pct": 100.0, "height_pct": 14.0},
    "center": {"anchor": "center", "width_pct": 40.0, "height_pct": 40.0},
    "top_right": {"anchor": "top_right", "width_pct": 25.0, "height_pct": 25.0},
    "bottom_safe_area": {"anchor": "bottom_center", "width_pct": 100.0, "height_pct": 12.0},
}


class CropError(RuntimeError):
    pass


def resolve_magick() -> str:
    preferred = Path("/opt/homebrew/bin/magick")
    if preferred.exists():
        return str(preferred)
    resolved = shutil.which("magick")
    if resolved:
        return resolved
    raise CropError("magick is not installed or not available on PATH.")


def run_magick(args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(args, capture_output=True, text=True)


def image_dimensions(magick_bin: str, path: Path) -> tuple[int, int]:
    result = run_magick([magick_bin, "identify", "-format", "%w %h", str(path)])
    if result.returncode != 0:
        raise CropError((result.stderr or result.stdout or "magick identify failed").strip())
    try:
        width_text, height_text = (result.stdout or "").strip().split()
        return int(width_text), int(height_text)
    except Exception as error:
        raise CropError(f"could not parse image dimensions for {path}") from error


def clamp(value: int, minimum: int, maximum: int) -> int:
    return max(minimum, min(value, maximum))


def normalized_percent(value: float, field_name: str) -> float:
    if value <= 0 or value > 100:
        raise CropError(f"{field_name} must be greater than 0 and at most 100.")
    return value / 100.0


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
        raise CropError("width and height must be positive.")
    if x < 0 or y < 0:
        raise CropError("x and y must be non-negative.")
    if x >= image_width or y >= image_height:
        raise CropError("crop origin is outside the image bounds.")
    width = min(width, image_width - x)
    height = min(height, image_height - y)
    if width <= 0 or height <= 0:
        raise CropError("crop box is empty after clamping to image bounds.")
    return x, y, width, height


def compute_anchor_box(
    *,
    image_width: int,
    image_height: int,
    width_pct: float,
    height_pct: float,
    anchor: str,
    offset_x: int,
    offset_y: int,
) -> tuple[int, int, int, int]:
    width_ratio = normalized_percent(width_pct, "width_pct")
    height_ratio = normalized_percent(height_pct, "height_pct")
    if anchor not in ANCHORS:
        raise CropError(f"unknown anchor `{anchor}`.")

    crop_width = max(1, int(round(image_width * width_ratio)))
    crop_height = max(1, int(round(image_height * height_ratio)))
    crop_width = min(crop_width, image_width)
    crop_height = min(crop_height, image_height)

    anchor_x, anchor_y = ANCHORS[anchor]
    base_x = int(round((image_width - crop_width) * anchor_x))
    base_y = int(round((image_height - crop_height) * anchor_y))
    x = clamp(base_x + offset_x, 0, image_width - crop_width)
    y = clamp(base_y + offset_y, 0, image_height - crop_height)
    return x, y, crop_width, crop_height


def effective_relative_args(args: argparse.Namespace) -> tuple[str, float, float]:
    preset = PRESETS.get(args.preset) if args.preset else None
    anchor = str(args.anchor or (preset or {}).get("anchor") or "center")
    width_pct = args.width_pct if args.width_pct is not None else (preset or {}).get("width_pct")
    height_pct = args.height_pct if args.height_pct is not None else (preset or {}).get("height_pct")
    if width_pct is None or height_pct is None:
        raise CropError(
            "percentage mode requires --width-pct and --height-pct, or a preset that supplies them."
        )
    return anchor, float(width_pct), float(height_pct)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="designer-crop-screenshot")
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--input", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--preset", choices=sorted(PRESETS.keys()))
    parser.add_argument("--anchor", choices=sorted(ANCHORS.keys()))
    parser.add_argument("--width-pct", type=float)
    parser.add_argument("--height-pct", type=float)
    parser.add_argument("--offset-x", type=int, default=0)
    parser.add_argument("--offset-y", type=int, default=0)
    parser.add_argument("--x", type=int)
    parser.add_argument("--y", type=int)
    parser.add_argument("--width", type=int)
    parser.add_argument("--height", type=int)
    return parser


def render(payload: dict[str, object], as_json: bool) -> int:
    if as_json:
        print(json.dumps(payload, separators=(",", ":")))
        return 0 if payload.get("ok") else 1
    if not payload.get("ok"):
        print(str(payload.get("message") or "crop failed"), file=sys.stderr)
        return 1
    print(str(payload["out"]))
    print(
        f"crop: x={payload['x']} y={payload['y']} width={payload['width']} height={payload['height']} "
        f"from {payload['image_width']}x{payload['image_height']}"
    )
    return 0


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    input_path = Path(args.input).expanduser().resolve(strict=False)
    out_path = Path(args.out).expanduser().resolve(strict=False)

    try:
        if not input_path.exists():
            raise CropError(f"input image does not exist: {input_path}")
        magick_bin = resolve_magick()
        image_width, image_height = image_dimensions(magick_bin, input_path)

        exact_mode = all(value is not None for value in (args.x, args.y, args.width, args.height))
        if exact_mode:
            x, y, width, height = compute_exact_box(
                image_width=image_width,
                image_height=image_height,
                x=args.x,
                y=args.y,
                width=args.width,
                height=args.height,
            )
            mode = "exact"
        else:
            if any(value is not None for value in (args.x, args.y, args.width, args.height)):
                raise CropError("exact pixel mode requires --x, --y, --width, and --height together.")
            anchor, width_pct, height_pct = effective_relative_args(args)
            x, y, width, height = compute_anchor_box(
                image_width=image_width,
                image_height=image_height,
                width_pct=width_pct,
                height_pct=height_pct,
                anchor=anchor,
                offset_x=args.offset_x,
                offset_y=args.offset_y,
            )
            mode = "relative"

        out_path.parent.mkdir(parents=True, exist_ok=True)
        crop_result = run_magick(
            [
                magick_bin,
                str(input_path),
                "-crop",
                f"{width}x{height}+{x}+{y}",
                "+repage",
                str(out_path),
            ]
        )
        if crop_result.returncode != 0:
            raise CropError((crop_result.stderr or crop_result.stdout or "magick crop failed").strip())

        payload = {
            "ok": True,
            "input": str(input_path),
            "out": str(out_path),
            "mode": mode,
            "preset": args.preset,
            "anchor": args.anchor if mode == "exact" else (args.anchor or (PRESETS.get(args.preset or "", {}) or {}).get("anchor") or "center"),
            "x": x,
            "y": y,
            "width": width,
            "height": height,
            "image_width": image_width,
            "image_height": image_height,
        }
    except CropError as error:
        payload = {"ok": False, "message": str(error)}

    return render(payload, args.json)


if __name__ == "__main__":
    raise SystemExit(main())
