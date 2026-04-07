from __future__ import annotations

import json
import threading
import time
from pathlib import Path
from typing import Any
from urllib.error import HTTPError
from urllib.error import URLError
from urllib.request import Request
from urllib.request import urlopen

from .bridge import BridgeError
from .flutter_sim import Reservation
from .flutter_sim import FlutterSimManager
from .flutter_sim import default_app_id_for_platform


class _PlaceholderProcess:
    def __init__(self, pid: int | None) -> None:
        self.pid = int(pid or 0)

    def poll(self) -> None:
        return None


class FlutterDriveService:
    def __init__(self, *, broker_base_url: str) -> None:
        self.broker_base_url = broker_base_url.rstrip("/")
        self.manager = FlutterSimManager()
        self._repo_root = Path(__file__).resolve().parents[2]

    def _broker_request_json(
        self,
        *,
        method: str,
        path: str,
        payload: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        body = None
        headers: dict[str, str] = {}
        if payload is not None:
            body = json.dumps(payload).encode("utf-8")
            headers["Content-Type"] = "application/json"
        request = Request(f"{self.broker_base_url}{path}", method=method, data=body, headers=headers)
        try:
            with urlopen(request, timeout=10) as response:
                raw = response.read().decode("utf-8")
        except HTTPError as error:
            detail = error.read().decode("utf-8", errors="replace")
            raise BridgeError(f"Broker request failed: {detail or error.reason}") from error
        except URLError as error:
            raise BridgeError(f"Could not reach broker at {self.broker_base_url}: {error.reason}") from error
        try:
            parsed = json.loads(raw)
        except json.JSONDecodeError as error:
            raise BridgeError(f"Broker returned invalid JSON for {path}: {error}") from error
        if not isinstance(parsed, dict):
            raise BridgeError(f"Broker returned invalid payload for {path}.")
        return parsed

    def health(self) -> dict[str, Any]:
        broker = self._broker_request_json(method="GET", path="/healthz")
        return {"ok": True, "broker_base_url": self.broker_base_url, "broker": broker}

    def devices(self) -> list[dict[str, Any]]:
        payload = self._broker_request_json(method="GET", path="/devices")
        devices = payload.get("devices")
        if not isinstance(devices, list):
            raise BridgeError("Broker /devices response is missing devices.")
        return [device for device in devices if isinstance(device, dict)]

    def _ready_reservation(self, device_id: str) -> Reservation:
        normalized = self.manager._normalize_device_id(device_id)  # noqa: SLF001
        device_entry = next((row for row in self.devices() if row.get("device_id") == normalized), None)
        if device_entry is None:
            raise BridgeError(f"Device {normalized} is not known to the broker.")
        if device_entry.get("state") != "ready":
            raise BridgeError(f"Device {normalized} is not ready for commands.")
        launch_path = str(device_entry.get("reservation_path") or self._repo_root)
        platform = str(device_entry.get("platform") or "ios")
        return Reservation(
            device_id=normalized,
            device_name=str(device_entry.get("name") or normalized),
            platform=platform,
            path=launch_path,
            launch_path=launch_path,
            target=normalized,
            process=_PlaceholderProcess(device_entry.get("pid")),
            created_at=time.time(),
            ready_event=threading.Event(),
            state="ready",
            app_id=default_app_id_for_platform(platform),
            runtime_label="command-server",
        )

    def driver_apps(self, *, device_id: str) -> dict[str, Any]:
        reservation = self._ready_reservation(device_id)
        return {"ok": True, "device_id": reservation.device_id, "apps": [{"name": "Runner", "appId": reservation.app_id}]}
