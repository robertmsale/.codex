#!/usr/bin/env python3
from __future__ import annotations

import argparse
import contextlib
import functools
import http.client
import http.server
import os
import shlex
import socket
import subprocess
import sys
import threading
import time
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
        "--backend": "design-lab-capture uses the project screenshot backend default",
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
        run_phase("capture", shot_command(args, url, extra_shot_args), lab_dir, args.shot_timeout, log_path)
        if not out_path.is_file():
            raise CaptureError("capture", f"screenshot output was not created: {out_path}")

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
