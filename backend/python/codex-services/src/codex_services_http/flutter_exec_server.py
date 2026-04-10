from __future__ import annotations

import argparse
import os
import pty
import selectors
import signal
import subprocess
import time
from pathlib import Path
from typing import Any

import uvicorn
from fastapi import FastAPI
from pydantic import BaseModel
from starlette.concurrency import run_in_threadpool

from .bridge import BridgeError
from .bridge import load_paths
from .bridge import require_allowed_path


class FlutterRunRequest(BaseModel):
    cwd: str
    argv: list[str]


def _flutter_wrapper_executable() -> str:
    configured = os.environ.get("FLUTTER_WRAPPER_BIN", "").strip()
    if configured:
        wrapper = Path(configured)
    else:
        wrapper = Path.home() / ".codex" / "scripts" / "flutterq"
    if wrapper.is_file() and os.access(wrapper, os.X_OK):
        return str(wrapper)
    raise BridgeError(f"flutter wrapper is not available: {wrapper}")


def _safe_cwd(raw_cwd: str) -> Path:
    paths = load_paths()
    return require_allowed_path(raw_cwd, paths)


def _run_flutter(request: FlutterRunRequest) -> dict[str, Any]:
    if not request.argv:
        raise BridgeError("argv must be a non-empty list.")
    cwd = _safe_cwd(request.cwd)
    master_fd, slave_fd = pty.openpty()
    os.set_blocking(master_fd, False)
    try:
        process = subprocess.Popen(
            [_flutter_wrapper_executable(), *request.argv],
            cwd=str(cwd),
            stdin=subprocess.DEVNULL,
            stdout=slave_fd,
            stderr=slave_fd,
            text=False,
            env=os.environ.copy(),
            start_new_session=True,
        )
    finally:
        os.close(slave_fd)
    output_chunks: list[bytes] = []
    try:
        selector = selectors.DefaultSelector()
        selector.register(master_fd, selectors.EVENT_READ)
        exit_seen_at: float | None = None
        while True:
            events = selector.select(timeout=0.2)
            saw_output = False
            for _key, _mask in events:
                try:
                    chunk = os.read(master_fd, 65536)
                except BlockingIOError:
                    continue
                except OSError:
                    chunk = b""
                if not chunk:
                    continue
                saw_output = True
                output_chunks.append(chunk)
            if process.poll() is not None:
                if saw_output:
                    exit_seen_at = time.monotonic()
                elif exit_seen_at is None:
                    exit_seen_at = time.monotonic()
                elif time.monotonic() - exit_seen_at >= 0.5:
                    break
        returncode = process.wait()
    finally:
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        except OSError:
            pass
        os.close(master_fd)
    output_text = b"".join(output_chunks).decode("utf-8", errors="replace")
    return {
        "ok": returncode == 0,
        "cwd": str(cwd),
        "argv": request.argv,
        "returncode": returncode,
        "stdout": output_text,
        "stderr": "",
    }


app = FastAPI(title="codex-flutter-http", version="0.1.0")


@app.get("/healthz")
async def healthz() -> dict[str, Any]:
    return {"ok": True, "flutter_wrapper_bin": _flutter_wrapper_executable()}


@app.post("/run")
async def run_flutter(request: FlutterRunRequest) -> dict[str, Any]:
    try:
        return await run_in_threadpool(_run_flutter, request)
    except BridgeError as error:
        return {"ok": False, "message": str(error), "returncode": 1, "stdout": "", "stderr": str(error)}


def main() -> None:
    parser = argparse.ArgumentParser(description="Host-side Flutter execution server")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8769)
    args = parser.parse_args()
    uvicorn.run(app, host=args.host, port=args.port)


if __name__ == "__main__":
    main()
