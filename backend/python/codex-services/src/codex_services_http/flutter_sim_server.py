from __future__ import annotations

import argparse
import importlib
import threading
from contextlib import asynccontextmanager
from typing import Any

import uvicorn
from fastapi import FastAPI
from pydantic import BaseModel
from starlette.concurrency import run_in_threadpool

from .flutter_sim import FlutterSimManager


class ReserveRequest(BaseModel):
    device_id: str


class RebootRequest(BaseModel):
    device_id: str


class DumpLogsRequest(BaseModel):
    device_id: str


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


MANAGER = FlutterSimManager()
_ROUTES_MODULE_NAME = "codex_services_http.flutter_sim_http_routes"
_ROUTES_LOCK = threading.Lock()


def load_routes_module():
    with _ROUTES_LOCK:
        module = importlib.import_module(_ROUTES_MODULE_NAME)
        importlib.invalidate_caches()
        return importlib.reload(module)


@asynccontextmanager
async def lifespan(_app: FastAPI):
    MANAGER.start()
    try:
        yield
    finally:
        MANAGER.stop()


app = FastAPI(title="codex-flutter-sim-http", version="0.1.0", lifespan=lifespan)


@app.get("/healthz")
async def healthz() -> dict[str, Any]:
    return await run_in_threadpool(lambda: load_routes_module().healthz(manager=MANAGER))


@app.get("/devices")
async def devices() -> dict[str, Any]:
    return await run_in_threadpool(lambda: load_routes_module().devices(manager=MANAGER))


@app.post("/reserve")
async def reserve(request: ReserveRequest) -> dict[str, Any]:
    return await run_in_threadpool(lambda: load_routes_module().reserve(manager=MANAGER, device_id=request.device_id))


@app.post("/reboot")
async def reboot(request: RebootRequest) -> dict[str, Any]:
    return await run_in_threadpool(lambda: load_routes_module().reboot(manager=MANAGER, device_id=request.device_id))


@app.post("/dump-logs")
async def dump_logs(request: DumpLogsRequest) -> dict[str, Any]:
    return await run_in_threadpool(lambda: load_routes_module().dump_logs(manager=MANAGER, device_id=request.device_id))


@app.post("/maestro/apps")
async def maestro_apps(request: MaestroAppsRequest) -> dict[str, Any]:
    return await run_in_threadpool(lambda: load_routes_module().maestro_apps(manager=MANAGER, device_id=request.device_id))


@app.post("/maestro/hierarchy")
async def maestro_hierarchy(request: MaestroHierarchyRequest) -> dict[str, Any]:
    return await run_in_threadpool(
        lambda: load_routes_module().maestro_hierarchy(manager=MANAGER, device_id=request.device_id)
    )


@app.post("/maestro/command")
async def maestro_command(request: MaestroCommandRequest) -> dict[str, Any]:
    return await run_in_threadpool(
        lambda: load_routes_module().maestro_command(
            manager=MANAGER,
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
            manager=MANAGER,
            device_id=request.device_id,
            commands=request.commands,
            label=request.label,
        )
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="Host-side Flutter simulator reservation server")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8767)
    args = parser.parse_args()
    uvicorn.run(app, host=args.host, port=args.port)


if __name__ == "__main__":
    main()
