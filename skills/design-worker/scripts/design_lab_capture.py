#!/usr/bin/env python3
from __future__ import annotations

import argparse
import contextlib
import functools
import http.client
import http.server
import os
import struct
import shlex
import socket
import subprocess
import sys
import threading
import time
import zlib
from pathlib import Path


class CaptureError(RuntimeError):
    def __init__(self, phase: str, message: str) -> None:
        super().__init__(message)
        self.phase = phase


def parse_args() -> tuple[argparse.Namespace, list[str]]:
    parser = argparse.ArgumentParser(
        description="Build a Flutter Design Lab web artifact, serve it ephemerally, and capture a Bun/WebView screenshot.",
    )
    parser.add_argument("--workdir", required=True, help="Project/worktree root. Defaults lab dir to <workdir>/clients/design_lab.")
    parser.add_argument("--lab-dir", help="Design Lab directory. Relative paths resolve under --workdir.")
    parser.add_argument("--out", required=True, help="Screenshot output path.")
    parser.add_argument("--session", help="Logical capture name. Defaults to design-lab-capture-<epoch>-<pid>.")
    parser.add_argument("--story", help="DESIGN_LAB_STORY dart define.")
    parser.add_argument("--shell", dest="shell_name", help="DESIGN_LAB_SHELL dart define.")
    parser.add_argument("--fixture", help="DESIGN_LAB_FIXTURE dart define.")
    parser.add_argument("--viewport", help="DESIGN_LAB_VIEWPORT dart define.")
    parser.add_argument("--theme", help="DESIGN_LAB_THEME dart define.")
    parser.add_argument("--inspector", help="DESIGN_LAB_INSPECTOR dart define.")
    parser.add_argument("--dart-define", action="append", default=[], metavar="K=V", help="Extra dart define. May be repeated.")
    parser.add_argument("--width", help="Passed to npm run bun:shot.")
    parser.add_argument("--height", help="Passed to npm run bun:shot.")
    parser.add_argument("--build-timeout", type=float, default=180.0)
    parser.add_argument("--serve-timeout", type=float, default=20.0)
    parser.add_argument("--shot-timeout", type=float, default=90.0)
    parser.add_argument("--log", help="Log path. Defaults to /tmp/design-lab/<session>.log.")
    parser.add_argument("--build-mode", choices=("release", "profile"), default="release")
    parser.add_argument("--skip-npm-install", action="store_true", help="Do not run npm install when node_modules/.bin/bun is missing.")
    parser.add_argument("shot_args", nargs=argparse.REMAINDER, help="Extra args after -- are passed to npm run bun:shot.")
    args = parser.parse_args()
    shot_args = list(args.shot_args)
    if shot_args and shot_args[0] == "--":
        shot_args = shot_args[1:]
    validate_shot_args(parser, shot_args)
    return args, shot_args


def validate_shot_args(parser: argparse.ArgumentParser, shot_args: list[str]) -> None:
    reserved_options = {
        "--port": "design-lab-capture owns the ephemeral localhost port",
        "--out": "design-lab-capture owns the screenshot output path",
    }
    readiness_bypass_options = {
        "--skipReady": "visual proof must wait for the Design Lab readiness signal",
        "--skip-ready": "visual proof must wait for the Design Lab readiness signal",
    }
    backend_override_options = {
        "--backend": (
            "Chrome backend is disabled. Use only the built-in Bun WebView Webkit backend "
            "by omitting --backend (default backend)."
        ),
    }
    for arg in shot_args:
        option = arg.split("=", 1)[0]
        if option in reserved_options:
            parser.error(f"{option} is reserved: {reserved_options[option]}")
        if option in readiness_bypass_options:
            parser.error(f"{option} is forbidden: {readiness_bypass_options[option]}")
        if option in backend_override_options:
            parser.error(f"{option} is forbidden: {backend_override_options[option]}")


def resolve_lab_dir(workdir: Path, raw_lab_dir: str | None) -> Path:
    if raw_lab_dir:
        candidate = Path(raw_lab_dir).expanduser()
        if not candidate.is_absolute():
            candidate = workdir / candidate
    else:
        candidate = workdir / "clients" / "design_lab"
    return candidate.resolve()


def append_log(log_path: Path, text: str) -> None:
    with log_path.open("a", encoding="utf-8") as handle:
        handle.write(text)
        if text and not text.endswith("\n"):
            handle.write("\n")


def shell_join(argv: list[str]) -> str:
    return " ".join(shlex.quote(part) for part in argv)


def run_phase(phase: str, argv: list[str], cwd: Path, timeout: float, log_path: Path, env: dict[str, str] | None = None) -> None:
    append_log(log_path, f"\n[{phase}] cwd={cwd}")
    append_log(log_path, f"[{phase}] command={shell_join(argv)}")
    try:
        result = subprocess.run(
            argv,
            cwd=str(cwd),
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired as exc:
        if exc.stdout:
            append_log(log_path, exc.stdout)
        raise CaptureError(phase, f"timed out after {timeout:g}s") from exc
    append_log(log_path, result.stdout)
    if result.returncode != 0:
        raise CaptureError(phase, f"command exited {result.returncode}")


def run_capture_with_recovery(args: argparse.Namespace, url: str, shot_args: list[str], lab_dir: Path, log_path: Path, out_path: Path) -> None:
    attempts: list[tuple[str, list[str]]] = [("default-backend", [])]
    last_error: CaptureError | None = None
    for attempt_name, recovery_args in attempts:
        if out_path.exists():
            out_path.unlink()
        try:
            append_log(log_path, f"\n[capture] attempt={attempt_name}")
            run_phase("capture", shot_command(args, url, [*shot_args, *recovery_args]), lab_dir, args.shot_timeout, log_path)
            if not out_path.is_file():
                raise CaptureError("capture", f"screenshot output was not created: {out_path}")
            if png_is_blank_white(out_path):
                raise CaptureError(
                    "capture",
                    f"screenshot output is blank white: {out_path}. Design Lab did not render usable visual proof.",
                )
            return
        except CaptureError as exc:
            last_error = exc
            append_log(log_path, f"[capture] attempt={attempt_name} failed: {exc}")
    raise last_error or CaptureError("capture", "capture failed")


def choose_port() -> int:
    with contextlib.closing(socket.socket(socket.AF_INET, socket.SOCK_STREAM)) as sock:
        sock.bind(("127.0.0.1", 0))
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        return int(sock.getsockname()[1])


def wait_for_http(port: int, timeout: float) -> None:
    deadline = time.monotonic() + timeout
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        try:
            conn = http.client.HTTPConnection("127.0.0.1", port, timeout=1.0)
            conn.request("GET", "/")
            response = conn.getresponse()
            response.read()
            conn.close()
            if 200 <= response.status < 500:
                return
        except Exception as exc:  # noqa: BLE001 - preserved in final error.
            last_error = exc
        time.sleep(0.1)
    detail = f": {last_error}" if last_error else ""
    raise CaptureError("serve", f"static server did not respond within {timeout:g}s{detail}")


def serve_build(build_dir: Path, port: int, log_path: Path) -> tuple[http.server.ThreadingHTTPServer, threading.Thread]:
    class QuietHandler(http.server.SimpleHTTPRequestHandler):
        def log_message(self, format: str, *args: object) -> None:
            append_log(log_path, f"[serve] {self.address_string()} - {format % args}")

    handler = functools.partial(QuietHandler, directory=str(build_dir))
    try:
        server = http.server.ThreadingHTTPServer(("127.0.0.1", port), handler)
    except OSError as exc:
        raise CaptureError("serve", f"failed to bind 127.0.0.1:{port}: {exc}") from exc
    thread = threading.Thread(target=server.serve_forever, name="design-lab-static-server", daemon=True)
    thread.start()
    append_log(log_path, f"\n[serve] root={build_dir}")
    append_log(log_path, f"[serve] listening=http://127.0.0.1:{port}/")
    return server, thread


def validate_inputs(args: argparse.Namespace, lab_dir: Path) -> None:
    if not lab_dir.is_dir():
        raise CaptureError("input", f"Design Lab directory does not exist: {lab_dir}")
    if not (lab_dir / "pubspec.yaml").is_file():
        raise CaptureError("input", f"pubspec.yaml not found in Design Lab directory: {lab_dir}")
    package_json = lab_dir / "package.json"
    if not package_json.is_file():
        raise CaptureError("input", f"package.json with npm script `bun:shot` not found in Design Lab directory: {lab_dir}")


def build_command(args: argparse.Namespace) -> list[str]:
    cmd = ["flutter", "build", "web", f"--{args.build_mode}", "--no-wasm-dry-run"]
    defines = {
        "DESIGN_LAB_SESSION": args.session,
        "DESIGN_LAB_STORY": args.story,
        "DESIGN_LAB_SHELL": args.shell_name,
        "DESIGN_LAB_FIXTURE": args.fixture,
        "DESIGN_LAB_VIEWPORT": args.viewport,
        "DESIGN_LAB_THEME": args.theme,
        "DESIGN_LAB_INSPECTOR": args.inspector,
    }
    for key, value in defines.items():
        if value:
            cmd.extend(["--dart-define", f"{key}={value}"])
    for define in args.dart_define:
        if define:
            cmd.extend(["--dart-define", define])
    return cmd


def shot_command(args: argparse.Namespace, url: str, shot_args: list[str]) -> list[str]:
    cmd = ["npm", "run", "bun:shot", "--", "--url", url, "--out", args.out]
    if args.width:
        cmd.extend(["--width", args.width])
    if args.height:
        cmd.extend(["--height", args.height])
    if args.build_mode == "release" and "--noForceRunMain" not in shot_args:
        cmd.append("--noForceRunMain")
    cmd.extend(shot_args)
    return cmd


def png_is_blank_white(path: Path) -> bool:
    """Return true when a PNG screenshot is all near-white pixels.

    This catches Design Lab readiness timeout fallbacks that produce a valid
    white PNG. A valid screenshot must not pass visual proof with no rendered
    content.
    """
    try:
        width, height, color_type, raw = read_png_pixels(path)
    except Exception:  # noqa: BLE001 - invalid or unsupported images should be handled elsewhere.
        return False
    if width < 16 or height < 16:
        return False
    if color_type == 6:
        stride = 4
        rgb_offsets = (0, 1, 2)
    elif color_type == 2:
        stride = 3
        rgb_offsets = (0, 1, 2)
    elif color_type == 0:
        stride = 1
        rgb_offsets = (0, 0, 0)
    else:
        return False
    if not raw:
        return False
    min_channel = 255
    max_channel = 0
    sample_step = max(stride, (len(raw) // 50000 // stride) * stride)
    for index in range(0, len(raw), sample_step):
        if index + stride > len(raw):
            break
        for offset in rgb_offsets:
            value = raw[index + offset]
            min_channel = min(min_channel, value)
            max_channel = max(max_channel, value)
        if min_channel < 248 or max_channel - min_channel > 3:
            return False
    return min_channel >= 248


def read_png_pixels(path: Path) -> tuple[int, int, int, bytes]:
    data = path.read_bytes()
    if not data.startswith(b"\x89PNG\r\n\x1a\n"):
        raise ValueError("not a png")
    pos = 8
    width = height = bit_depth = color_type = None
    compressed = bytearray()
    while pos + 8 <= len(data):
        length = struct.unpack(">I", data[pos:pos + 4])[0]
        chunk_type = data[pos + 4:pos + 8]
        chunk_data = data[pos + 8:pos + 8 + length]
        pos += 12 + length
        if chunk_type == b"IHDR":
            width, height, bit_depth, color_type = struct.unpack(">IIBB", chunk_data[:10])
        elif chunk_type == b"IDAT":
            compressed.extend(chunk_data)
        elif chunk_type == b"IEND":
            break
    if width is None or height is None or bit_depth != 8 or color_type not in (0, 2, 6):
        raise ValueError("unsupported png")
    channels = {0: 1, 2: 3, 6: 4}[color_type]
    row_bytes = width * channels
    decompressed = zlib.decompress(bytes(compressed))
    rows = []
    prev = bytearray(row_bytes)
    offset = 0
    for _row in range(height):
        filter_type = decompressed[offset]
        offset += 1
        current = bytearray(decompressed[offset:offset + row_bytes])
        offset += row_bytes
        unfilter_scanline(current, prev, channels, filter_type)
        rows.append(bytes(current))
        prev = current
    return width, height, color_type, b"".join(rows)


def unfilter_scanline(current: bytearray, previous: bytearray, bpp: int, filter_type: int) -> None:
    if filter_type == 0:
        return
    for index in range(len(current)):
        left = current[index - bpp] if index >= bpp else 0
        up = previous[index]
        up_left = previous[index - bpp] if index >= bpp else 0
        if filter_type == 1:
            current[index] = (current[index] + left) & 0xFF
        elif filter_type == 2:
            current[index] = (current[index] + up) & 0xFF
        elif filter_type == 3:
            current[index] = (current[index] + ((left + up) // 2)) & 0xFF
        elif filter_type == 4:
            current[index] = (current[index] + paeth(left, up, up_left)) & 0xFF
        else:
            raise ValueError(f"unsupported png filter {filter_type}")


def paeth(left: int, up: int, up_left: int) -> int:
    estimate = left + up - up_left
    left_delta = abs(estimate - left)
    up_delta = abs(estimate - up)
    up_left_delta = abs(estimate - up_left)
    if left_delta <= up_delta and left_delta <= up_left_delta:
        return left
    if up_delta <= up_left_delta:
        return up
    return up_left


def main() -> int:
    args, extra_shot_args = parse_args()
    workdir = Path(args.workdir).expanduser().resolve()
    lab_dir = resolve_lab_dir(workdir, args.lab_dir)
    args.session = args.session or f"design-lab-capture-{int(time.time())}-{os.getpid()}"
    log_path = Path(args.log or f"/tmp/design-lab/{args.session}.log").expanduser()
    log_path.parent.mkdir(parents=True, exist_ok=True)
    log_path.write_text("", encoding="utf-8")
    out_path = Path(args.out).expanduser()
    out_path.parent.mkdir(parents=True, exist_ok=True)

    server: http.server.ThreadingHTTPServer | None = None
    try:
        validate_inputs(args, lab_dir)
        append_log(log_path, f"[input] workdir={workdir}")
        append_log(log_path, f"[input] lab_dir={lab_dir}")
        append_log(log_path, f"[input] out={out_path}")

        if not args.skip_npm_install and not (lab_dir / "node_modules" / ".bin" / "bun").is_file():
            run_phase("dependencies", ["npm", "install"], lab_dir, args.build_timeout, log_path)

        run_phase("build", build_command(args), lab_dir, args.build_timeout, log_path)
        build_dir = lab_dir / "build" / "web"
        if not (build_dir / "index.html").is_file():
            raise CaptureError("build", f"build output missing index.html: {build_dir}")

        port = choose_port()
        server, _thread = serve_build(build_dir, port, log_path)
        wait_for_http(port, args.serve_timeout)

        url = f"http://127.0.0.1:{port}/"
        run_capture_with_recovery(args, url, extra_shot_args, lab_dir, log_path, out_path)

        print("build: ok")
        print(f"screenshot: {out_path}")
        print(f"log: {log_path}")
        return 0
    except CaptureError as exc:
        print(f"phase: {exc.phase}", file=sys.stderr)
        print(f"error: {exc}", file=sys.stderr)
        print(f"log: {log_path}", file=sys.stderr)
        return 1
    finally:
        if server is not None:
            append_log(log_path, "\n[cleanup] stopping ephemeral static server")
            server.shutdown()
            server.server_close()


if __name__ == "__main__":
    raise SystemExit(main())
