from __future__ import annotations

import argparse
import importlib
import os
import threading
from typing import Any

import uvicorn
from fastapi import FastAPI
from pydantic import BaseModel
from starlette.concurrency import run_in_threadpool

from .flutter_drive_service import FlutterDriveService


class MaestroAppsRequest(BaseModel):
    device_id: str


class MaestroHierarchyRequest(BaseModel):
    device_id: str


class MaestroCommandRequest(BaseModel):
    device_id: str
    command: str
    input: Any | None = None
    label: str | None = None
    out_path: str | None = None


class MaestroFlowRequest(BaseModel):
    device_id: str
    commands: list[Any]
    label: str | None = None


SERVICE = FlutterDriveService(
    broker_base_url=os.environ.get("FLUTTER_SIM_BROKER_BASE_URL", "http://127.0.0.1:8767"),
)
_ROUTES_MODULE_NAME = "codex_services_http.flutter_drive_http_routes"
_ROUTES_LOCK = threading.Lock()


def load_routes_module():
    with _ROUTES_LOCK:
        module = importlib.import_module(_ROUTES_MODULE_NAME)
        importlib.invalidate_caches()
        return importlib.reload(module)


app = FastAPI(title="codex-flutter-drive-http", version="0.1.0")


@app.get("/healthz")
async def healthz() -> dict[str, Any]:
    return await run_in_threadpool(lambda: load_routes_module().healthz(service=SERVICE))


@app.get("/devices")
async def devices() -> dict[str, Any]:
    return await run_in_threadpool(lambda: load_routes_module().devices(service=SERVICE))


@app.post("/maestro/apps")
async def maestro_apps(request: MaestroAppsRequest) -> dict[str, Any]:
    return await run_in_threadpool(lambda: load_routes_module().maestro_apps(service=SERVICE, device_id=request.device_id))


@app.post("/maestro/hierarchy")
async def maestro_hierarchy(request: MaestroHierarchyRequest) -> dict[str, Any]:
    return await run_in_threadpool(
        lambda: load_routes_module().maestro_hierarchy(service=SERVICE, device_id=request.device_id)
    )


@app.post("/maestro/command")
async def maestro_command(request: MaestroCommandRequest) -> dict[str, Any]:
    return await run_in_threadpool(
        lambda: load_routes_module().maestro_command(
            service=SERVICE,
            device_id=request.device_id,
            command=request.command,
            input_payload=request.input,
            label=request.label,
            out_path=request.out_path,
        )
    )


@app.post("/maestro/flow")
async def maestro_flow(request: MaestroFlowRequest) -> dict[str, Any]:
    return await run_in_threadpool(
        lambda: load_routes_module().maestro_flow(
            service=SERVICE,
            device_id=request.device_id,
            commands=request.commands,
            label=request.label,
        )
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="Host-side Flutter drive command server")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8768)
    args = parser.parse_args()
    uvicorn.run(app, host=args.host, port=args.port)


if __name__ == "__main__":
    main()
