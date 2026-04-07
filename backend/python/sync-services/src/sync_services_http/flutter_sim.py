from __future__ import annotations

import base64
import json
import logging
import os
import re
import shutil
import signal
import socket
import subprocess
import tempfile
import threading
import time
import yaml
from urllib.parse import urlsplit
from urllib.parse import urlunsplit
from collections import deque
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Any
from urllib.request import urlopen

from .bridge import BridgeError
from .bridge import BridgePaths
from .bridge import load_paths
from .bridge import require_allowed_path


DEVICE_BULLET = "•"
FLUTTER_EXECUTABLE = "/usr/local/bin/flutter"
DOCKER_EXECUTABLE = shutil.which("docker") or "/usr/local/bin/docker"
BROKER_HOSTNAME = "host.internal"
EZRA_PILOT_LOGIN_EMAIL = "user@acme.com"
EZRA_PILOT_LOGIN_PASSWORD = "password123"
EZRA_IOS_APP_ID = "com.ezraworks.aierpios"
LOGGER = logging.getLogger(__name__)
BROKER_LAUNCH_LOG_DIR = os.environ.get("PARALLELS_SYNC_FLUTTER_SIM_LOG_DIR")
SCREENSHOT_ROOT = Path("/tmp/flutter-driver-screenshots")
IDB_EXECUTABLE = os.environ.get("IDB_BIN", str(Path.home() / ".local" / "bin" / "idb"))


def flutter_executable() -> str:
    return FLUTTER_EXECUTABLE


def idb_executable() -> str:
    configured = IDB_EXECUTABLE.strip()
    if configured:
        return configured
    return shutil.which("idb") or str(Path.home() / ".local" / "bin" / "idb")


def launch_env() -> dict[str, str]:
    env = os.environ.copy()
    current_path = env.get("PATH", "")
    segments = [segment for segment in current_path.split(os.pathsep) if segment]
    rbenv_shims = str(Path.home() / ".rbenv" / "shims")
    if rbenv_shims in segments:
        segments = [segment for segment in segments if segment != rbenv_shims]
    if "/opt/homebrew/bin" in segments:
        segments = [segment for segment in segments if segment != "/opt/homebrew/bin"]
    env["PATH"] = os.pathsep.join([rbenv_shims, "/opt/homebrew/bin", *segments])
    return env


def _timestamped_line(text: str) -> str:
    return f"{datetime.now().isoformat(timespec='seconds')} {text}"


def append_launch_log(relative_path: str, *lines: str) -> None:
    if not BROKER_LAUNCH_LOG_DIR:
        return
    path = Path(BROKER_LAUNCH_LOG_DIR) / relative_path
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as handle:
        for line in lines:
            text = str(line).rstrip()
            if not text:
                continue
            handle.write(_timestamped_line(text))
            handle.write("\n")


def run_flutter_clean(*, cwd: str, env: dict[str, str]) -> list[str]:
    result = subprocess.run(
        [flutter_executable(), "clean"],
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        stdin=subprocess.DEVNULL,
        text=True,
        env=env,
    )
    output = [line for line in result.stdout.splitlines() if line.strip()]
    if result.returncode != 0:
        raise BridgeError(
            "\n".join(
                [
                    f"flutter clean failed with code {result.returncode}",
                    *output[-40:],
                ]
            )
        )
    return output


def load_dotenv_value(path: Path, key: str) -> str | None:
    if not path.exists():
        return None
    for raw_line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        current_key, value = line.split("=", 1)
        if current_key.strip() != key:
            continue
        return value.strip().strip("'\"")
    return None


def parse_idb_list_targets_output(text: str) -> list[dict[str, str]]:
    devices: list[dict[str, str]] = []
    for raw_line in text.replace("\r", "").splitlines():
        line = raw_line.strip()
        if not line or "|" not in line:
            continue
        parts = [part.strip() for part in line.split("|")]
        if len(parts) < 7:
            continue
        name, device_id, state, target_type, os_version, architecture, companion = parts[:7]
        if state != "Booted" or target_type != "simulator" or not os_version.startswith("iOS "):
            continue
        devices.append(
            {
                "name": name,
                "device_id": normalize_device_id(device_id, platform="ios"),
                "platform": "ios",
                "details": f"{os_version} ({architecture})",
                "os_version": os_version,
                "architecture": architecture,
                "companion": companion if companion != "No Companion Connected" else "",
            }
        )
    return devices


def normalize_device_id(device_id: str, *, platform: str | None = None) -> str:
    normalized = device_id.strip()
    if platform == "ios" or re.fullmatch(r"[0-9a-fA-F-]{36}", normalized):
        return normalized.upper()
    return normalized


def default_app_id_for_platform(platform: str) -> str:
    return EZRA_IOS_APP_ID


def api_host_for_platform(platform: str) -> str:
    return "127.0.0.1"


def connection_domain_for_platform(platform: str, port: int | None) -> str | None:
    if not port:
        return None
    return f"{api_host_for_platform(platform)}:{port}"


def normalize_request_path(raw_path: str, paths: BridgePaths) -> str:
    path = require_allowed_path(raw_path, paths)
    return str(path)


def pick_free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as probe:
        probe.bind(("127.0.0.1", 0))
        return int(probe.getsockname()[1])


def generate_secret() -> str:
    return base64.urlsafe_b64encode(os.urandom(32)).decode().rstrip("=")


def rewrite_loopback_uri(raw_uri: str | None) -> str | None:
    if not raw_uri:
        return raw_uri
    parsed = urlsplit(raw_uri)
    if parsed.hostname not in {"127.0.0.1", "localhost"}:
        return raw_uri
    if parsed.port is None:
        netloc = BROKER_HOSTNAME
    else:
        netloc = f"{BROKER_HOSTNAME}:{parsed.port}"
    return urlunsplit((parsed.scheme, netloc, parsed.path, parsed.query, parsed.fragment))


class TcpForwarder:
    def __init__(self, *, target_host: str, target_port: int, listen_host: str = "0.0.0.0") -> None:
        self.target_host = target_host
        self.target_port = target_port
        self.listen_host = listen_host
        self.listen_port: int | None = None
        self._server: socket.socket | None = None
        self._thread: threading.Thread | None = None
        self._stop_event = threading.Event()

    def start(self) -> int:
        if self._server is not None:
            if self.listen_port is None:
                raise RuntimeError("forwarder started without a bound port")
            return self.listen_port
        server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        server.bind((self.listen_host, 0))
        server.listen()
        self.listen_port = int(server.getsockname()[1])
        self._server = server
        self._thread = threading.Thread(target=self._serve, daemon=True)
        self._thread.start()
        return self.listen_port

    def stop(self) -> None:
        self._stop_event.set()
        if self._server is not None:
            try:
                self._server.close()
            except Exception:
                pass
            self._server = None

    def _serve(self) -> None:
        server = self._server
        if server is None:
            return
        while not self._stop_event.is_set():
            try:
                client, _addr = server.accept()
            except OSError:
                return
            threading.Thread(target=self._handle_client, args=(client,), daemon=True).start()

    def _handle_client(self, client: socket.socket) -> None:
        try:
            upstream = socket.create_connection((self.target_host, self.target_port))
        except OSError:
            try:
                client.close()
            except Exception:
                pass
            return
        threads = [
            threading.Thread(target=self._pipe, args=(client, upstream), daemon=True),
            threading.Thread(target=self._pipe, args=(upstream, client), daemon=True),
        ]
        for thread in threads:
            thread.start()

    def _pipe(self, source: socket.socket, destination: socket.socket) -> None:
        try:
            while True:
                data = source.recv(65536)
                if not data:
                    break
                destination.sendall(data)
        except OSError:
            pass
        finally:
            try:
                source.close()
            except Exception:
                pass
            try:
                destination.close()
            except Exception:
                pass


def expose_loopback_uri(raw_uri: str | None, *, forwarders: dict[str, TcpForwarder]) -> str | None:
    if not raw_uri:
        return raw_uri
    parsed = urlsplit(raw_uri)
    if parsed.hostname not in {"127.0.0.1", "localhost"} or parsed.port is None:
        return raw_uri
    key = f"{parsed.hostname}:{parsed.port}"
    forwarder = forwarders.get(key)
    if forwarder is None:
        forwarder = TcpForwarder(target_host=parsed.hostname, target_port=parsed.port)
        forwarder.start()
        forwarders[key] = forwarder
    netloc = f"{BROKER_HOSTNAME}:{forwarder.listen_port}"
    return urlunsplit((parsed.scheme, netloc, parsed.path, parsed.query, parsed.fragment))


@dataclass
class Reservation:
    device_id: str
    device_name: str
    platform: str
    path: str
    launch_path: str
    target: str
    process: subprocess.Popen[str]
    created_at: float
    ready_event: threading.Event
    managed_sync_root: str | None = None
    managed_api_project: str | None = None
    managed_api_root: str | None = None
    managed_api_port: int | None = None
    managed_api_base_url: str | None = None
    managed_api_runtime_base_url: str | None = None
    state: str = "launching"
    app_id: str | None = None
    maestro_driver_port: int | None = None
    runtime_label: str | None = None
    artifacts_root: str | None = None
    flow_sequence: int = 0
    api_health_status: str | None = None
    api_health_message: str | None = None
    dtd_uri: str | None = None
    app_uri: str | None = None
    last_error: str | None = None
    recent_output: deque[str] | None = None
    dtd_forwarders: dict[str, TcpForwarder] | None = None
    app_forwarders: dict[str, TcpForwarder] | None = None

    def __post_init__(self) -> None:
        if self.recent_output is None:
            self.recent_output = deque(maxlen=200)
        if self.dtd_forwarders is None:
            self.dtd_forwarders = {}
        if self.app_forwarders is None:
            self.app_forwarders = {}

    def snapshot(self) -> dict[str, Any]:
        connection_domain = connection_domain_for_platform(self.platform, self.managed_api_port)
        return {
            "device_id": self.device_id,
            "device_name": self.device_name,
            "platform": self.platform,
            "path": self.path,
            "launch_path": self.launch_path,
            "lane_root": self.managed_sync_root,
            "target": self.target,
            "pid": self.process.pid,
            "state": self.state,
            "app_id": self.app_id,
            "maestro_driver_port": self.maestro_driver_port,
            "runtime_label": self.runtime_label,
            "dtd_uri": expose_loopback_uri(self.dtd_uri, forwarders=self.dtd_forwarders or {}),
            "app_uri": expose_loopback_uri(self.app_uri, forwarders=self.app_forwarders or {}),
            "last_error": self.last_error,
            "created_at": self.created_at,
            "recent_output": list(self.recent_output or ()),
            "api": {
                "compose_project": self.managed_api_project,
                "port": self.managed_api_port,
                "base_url": self.managed_api_base_url,
                "runtime_base_url": self.managed_api_runtime_base_url,
                "connection_domain": connection_domain,
                "status": self.api_health_status,
                "message": self.api_health_message,
                "login_email": EZRA_PILOT_LOGIN_EMAIL if self.managed_api_port else None,
                "login_password": EZRA_PILOT_LOGIN_PASSWORD if self.managed_api_port else None,
            },
        }


class FlutterSimManager:
    def __init__(
        self,
        *,
        paths: BridgePaths | None = None,
        poll_interval_seconds: int = 120,
        devices_timeout_seconds: int = 20,
    ) -> None:
        self.paths = paths or load_paths()
        self.poll_interval_seconds = poll_interval_seconds
        self.devices_timeout_seconds = devices_timeout_seconds
        self._lock = threading.RLock()
        self._launch_locks_by_path: dict[str, threading.Lock] = {}
        self._driver_locks_by_device: dict[str, threading.Lock] = {}
        self._inventory: dict[str, dict[str, str]] = {}
        self._reservations_by_device: dict[str, Reservation] = {}
        self._device_errors: dict[str, str] = {}
        self._bootstrap_logs_by_device: dict[str, deque[str]] = {}
        self._bootstrap_api_by_device: dict[str, dict[str, Any]] = {}
        self._failed_runtime_sessions_by_device: dict[str, dict[str, Any]] = {}
        self._bootstrap_state_by_device: dict[str, str] = {}
        self._bootstrap_threads_by_device: dict[str, threading.Thread] = {}
        self._bootstrap_events_by_device: dict[str, threading.Event] = {}
        self._refresh_lock = threading.Lock()
        self._last_refresh_error: str | None = None
        self._last_refresh_at: float | None = None
        self._captured_api_crash_container_ids: set[str] = set()
        self._stop_event = threading.Event()
        self._poll_thread: threading.Thread | None = None

    def _normalize_device_id(self, device_id: str) -> str:
        return normalize_device_id(device_id)

    def _ezra_master_repo_root(self) -> Path:
        return self.paths.host_home / "Code" / "ezra" / "ezra"

    def _ezra_shared_repo_root(self) -> Path:
        return self.paths.host_home / "Code" / "ezra" / "qa" / "repo"

    def _ezra_lane_root(self, *, device_id: str, device_name: str) -> Path:
        _ = device_name
        return self.paths.host_home / "Code" / "ezra" / "qa" / device_id

    def _api_project_name(self, *, device_id: str, device_name: str) -> str:
        slug = re.sub(r"[^a-z0-9]+", "-", device_name.lower()).strip("-") or "lane"
        return f"ezra-ai-{slug}-{device_id.lower()}"

    def _is_ezra_app_path(self, path: Path) -> bool:
        normalized = path.resolve(strict=False)
        return normalized == (self._ezra_master_repo_root() / "clients" / "app").resolve(strict=False) or normalized == (
            self._ezra_shared_repo_root() / "clients" / "app"
        ).resolve(strict=False)

    def _launch_plan_for_device(self, *, path: str, device: dict[str, str]) -> dict[str, str | None]:
        requested_path = Path(path).resolve(strict=False)
        if self._is_ezra_app_path(requested_path):
            lane_root = self._ezra_lane_root(device_id=device["device_id"], device_name=device["name"])
            repo_root = lane_root
            return {
                "platform": device["platform"],
                "requested_path": path,
                "launch_path": str(repo_root / "clients" / "app"),
                "managed_sync_root": str(lane_root),
                "managed_api_project": self._api_project_name(device_id=device["device_id"], device_name=device["name"]),
                "managed_api_root": str(repo_root),
                "managed_api_port": str(pick_free_port()),
                "managed_api_base_url": None,
                "managed_api_runtime_base_url": None,
            }
        return {
            "platform": device["platform"],
            "requested_path": path,
            "launch_path": path,
            "managed_sync_root": None,
            "managed_api_project": None,
            "managed_api_root": None,
            "managed_api_port": None,
            "managed_api_base_url": None,
            "managed_api_runtime_base_url": None,
        }

    def _compose_env(self, *, api_root: str, api_project: str, api_port: str) -> dict[str, str]:
        env = launch_env()
        cargo_target_dir = self._cargo_target_dir_on_host(api_project)
        cargo_target_dir.mkdir(parents=True, exist_ok=True)
        env["EZRA_DB_TEST_COMPOSE_PROJECT"] = api_project
        env["EZRA_DB_TEST_WORKTREE_ROOT"] = api_root
        env["EZRA_DB_TEST_TARGET_DIR_ON_HOST"] = str(cargo_target_dir)
        env["AI_INTEGRATION_API_PORT"] = api_port
        env["API_PORT"] = "8080"
        env["SQLX_OFFLINE"] = "false"
        env["RUN_DEV_SEED"] = "1"
        env["MCP_COMMIT_TOKEN_SECRET"] = env.get("MCP_COMMIT_TOKEN_SECRET") or generate_secret()
        return env

    def _cargo_target_dir_on_host(self, api_project: str) -> Path:
        return Path("/tmp") / f"{api_project}-cargo-target"

    def _reset_cargo_target_dir(self, api_project: str) -> Path:
        target_dir = self._cargo_target_dir_on_host(api_project)
        for _attempt in range(5):
            subprocess.run(
                ["rm", "-rf", str(target_dir)],
                capture_output=True,
                text=True,
                env=launch_env(),
            )
            if not target_dir.exists():
                break
            time.sleep(0.2)
        target_dir.mkdir(parents=True, exist_ok=True)
        return target_dir

    def _compose_command(self, *, api_root: str, api_project: str, subcommand: list[str]) -> list[str]:
        return [
            DOCKER_EXECUTABLE,
            "compose",
            "--project-name",
            api_project,
            "--project-directory",
            api_root,
            "-f",
            str(Path(api_root) / "deployments" / "local" / "docker-compose.db-tests.yml"),
            "-f",
            str(Path(api_root) / "deployments" / "local" / "docker-compose.ai-integration.yml"),
            *subcommand,
        ]

    def _prepare_ai_integration_compose(self, *, api_root: str) -> tuple[str, list[str]]:
        dotenv_path = Path(api_root) / ".env"
        container_port = load_dotenv_value(dotenv_path, "API_PORT") or "8080"
        compose_path = Path(api_root) / "deployments" / "local" / "docker-compose.ai-integration.yml"
        if not compose_path.exists():
            return container_port, []

        original_text = compose_path.read_text(encoding="utf-8")
        expected_mapping = '- "127.0.0.1:${AI_INTEGRATION_API_PORT:-18080}:8080"'
        desired_mapping = f'- "127.0.0.1:${{AI_INTEGRATION_API_PORT:-18080}}:{container_port}"'
        if expected_mapping not in original_text:
            if desired_mapping in original_text:
                return container_port, [f"broker: ai integration compose already maps host port to container:{container_port}"]
            return container_port, [f"broker: ai integration compose port mapping not rewritten automatically ({compose_path})"]

        compose_path.write_text(original_text.replace(expected_mapping, desired_mapping), encoding="utf-8")
        return container_port, [f"broker: rewrote ai integration compose port mapping to container:{container_port}"]

    def _run_compose(
        self,
        *,
        api_root: str,
        api_project: str,
        api_port: str,
        subcommand: list[str],
        timeout: int | None = None,
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            self._compose_command(api_root=api_root, api_project=api_project, subcommand=subcommand),
            cwd=api_root,
            capture_output=True,
            text=True,
            env=self._compose_env(api_root=api_root, api_project=api_project, api_port=api_port),
            timeout=timeout,
        )

    def _wait_for_api_ready(self, base_url: str) -> None:
        ready_url = f"{base_url}/healthz/ready"
        while True:
            try:
                with urlopen(ready_url, timeout=5) as response:
                    if response.status == 200:
                        return
            except Exception:
                pass
            time.sleep(1)

    def _api_is_ready(self, base_url: str, *, timeout_seconds: float = 1.5) -> bool:
        ready_url = f"{base_url}/healthz/ready"
        try:
            with urlopen(ready_url, timeout=timeout_seconds) as response:
                return response.status == 200
        except Exception:
            return False

    def _set_bootstrap_state(self, device_id: str, state: str, *, error: str | None = None) -> None:
        with self._lock:
            self._bootstrap_state_by_device[device_id] = state
            if error:
                self._device_errors[device_id] = error
            elif state in {"idle", "ready"}:
                self._device_errors.pop(device_id, None)

    def _bootstrap_log_for_device(self, device_id: str) -> deque[str]:
        with self._lock:
            lines = self._bootstrap_logs_by_device.get(device_id)
            if lines is None:
                lines = deque(maxlen=200)
                self._bootstrap_logs_by_device[device_id] = lines
            return lines

    def _append_bootstrap_log(self, device_id: str, *lines: str) -> None:
        log_lines = self._bootstrap_log_for_device(device_id)
        for line in lines:
            text = str(line).rstrip()
            if not text:
                continue
            log_lines.append(text)
            append_launch_log(f"devices/{device_id}.log", text)
            LOGGER.info("flutter-sim[%s] %s", device_id, text)

    def _append_api_log(self, device_id: str, *lines: str) -> None:
        append_launch_log(f"api/{device_id}.log", *lines)

    def _append_runtime_log(self, device_id: str, *lines: str) -> None:
        append_launch_log(f"runtime/{device_id}.log", *lines)

    def _set_bootstrap_api_details(
        self,
        device_id: str,
        *,
        platform: str,
        compose_project: str | None,
        port: int | None,
        base_url: str | None,
        runtime_base_url: str | None,
    ) -> None:
        with self._lock:
            self._bootstrap_api_by_device[device_id] = {
                "compose_project": compose_project,
                "port": port,
                "base_url": base_url,
                "runtime_base_url": runtime_base_url,
                "connection_domain": connection_domain_for_platform(platform, port),
                "login_email": EZRA_PILOT_LOGIN_EMAIL,
                "login_password": EZRA_PILOT_LOGIN_PASSWORD,
            }

    def _clear_bootstrap_diagnostics(self, device_id: str) -> None:
        with self._lock:
            self._bootstrap_logs_by_device.pop(device_id, None)
            self._bootstrap_api_by_device.pop(device_id, None)
            self._failed_runtime_sessions_by_device.pop(device_id, None)

    def _record_failed_runtime_session(self, reservation: Reservation) -> None:
        snapshot = reservation.snapshot()
        snapshot["pid"] = None
        snapshot["state"] = "failed"
        self._failed_runtime_sessions_by_device[reservation.device_id] = snapshot
        recent_output = snapshot.get("recent_output") or []
        LOGGER.error(
            "flutter-sim[%s] flutter run failed: %s",
            reservation.device_id,
            snapshot.get("last_error") or "unknown error",
        )
        for line in recent_output[-40:]:
            LOGGER.error("flutter-sim[%s] %s", reservation.device_id, line)

    def _inspect_api_container(
        self,
        *,
        api_project: str,
    ) -> dict[str, str] | None:
        result = subprocess.run(
            [
                DOCKER_EXECUTABLE,
                "ps",
                "-a",
                "--filter",
                f"label=com.docker.compose.project={api_project}",
                "--filter",
                "label=com.docker.compose.service=api_test",
                "--format",
                "{{.ID}}\t{{.Names}}\t{{.Status}}",
            ],
            capture_output=True,
            text=True,
            env=launch_env(),
        )
        if result.returncode != 0:
            return None
        lines = [line.rstrip() for line in (result.stdout or "").splitlines() if line.rstrip()]
        if not lines:
            return None
        container_id, name, status = (lines[-1].split("\t", 2) + ["", "", ""])[:3]
        return {"id": container_id, "name": name, "status": status}

    def _capture_api_crash_logs_once(self, *, device_id: str, container_id: str, container_name: str, status: str) -> None:
        with self._lock:
            if container_id in self._captured_api_crash_container_ids:
                return
            self._captured_api_crash_container_ids.add(container_id)
        result = subprocess.run(
            [DOCKER_EXECUTABLE, "logs", container_id],
            capture_output=True,
            text=True,
            env=launch_env(),
        )
        combined = "\n".join(part.rstrip() for part in ((result.stdout or ""), (result.stderr or "")) if part.rstrip())
        crash_lines = [line.rstrip() for line in combined.splitlines()[-200:] if line.rstrip()]
        header = [
            f"broker: captured api_test crash logs from {container_name or container_id}",
            f"broker: api_test container status {status}",
        ]
        if crash_lines:
            self._append_api_log(device_id, *header, *crash_lines)
        else:
            self._append_api_log(device_id, *header, "broker: docker logs returned no output")

    def _api_crash_detail(self, *, device_id: str, api_project: str) -> str:
        container = self._inspect_api_container(api_project=api_project)
        if container is None:
            return "Managed API stack is down: api_test container is not present."
        status = container.get("status", "").strip() or "unknown status"
        if status.startswith("Exited"):
            self._capture_api_crash_logs_once(
                device_id=device_id,
                container_id=container.get("id", ""),
                container_name=container.get("name", ""),
                status=status,
            )
        return f"Managed API stack failed before readiness: {status}."

    def _wait_for_api_ready_or_raise(self, *, device_id: str, api_project: str, base_url: str) -> None:
        ready_url = f"{base_url}/healthz/ready"
        while True:
            try:
                with urlopen(ready_url, timeout=5) as response:
                    if response.status == 200:
                        return
            except Exception:
                container = self._inspect_api_container(api_project=api_project)
                if container is None:
                    raise BridgeError("Managed API stack is down: api_test container is not present.")
                status = container.get("status", "").strip()
                if status.startswith("Exited"):
                    raise BridgeError(self._api_crash_detail(device_id=device_id, api_project=api_project))
            time.sleep(1)

    def _probe_managed_api(self, reservation: Reservation) -> tuple[str, str | None]:
        base_url = reservation.managed_api_base_url
        api_project = reservation.managed_api_project
        if not base_url or not api_project:
            return "unknown", None
        if self._api_is_ready(base_url):
            return "ready", None
        container = self._inspect_api_container(api_project=api_project)
        if container is None:
            return "api_down", "Managed API stack is down: api_test container is not present."
        status = container.get("status", "").strip()
        if status.startswith("Exited"):
            self._capture_api_crash_logs_once(
                device_id=reservation.device_id,
                container_id=container.get("id", ""),
                container_name=container.get("name", ""),
                status=status,
            )
            return "api_crashed", f"Managed API stack crashed: {status}."
        return "api_down", f"Managed API stack is not ready: {status or 'api_test unavailable'}."

    def _reconcile_reservation_api_health(self, reservation: Reservation) -> None:
        if reservation.process.poll() is not None:
            return
        status, message = self._probe_managed_api(reservation)
        with self._lock:
            reservation.api_health_status = status
            reservation.api_health_message = message
            if status == "ready":
                if reservation.state in {"api_down", "api_crashed"}:
                    reservation.state = "ready"
                    self._bootstrap_state_by_device[reservation.device_id] = "ready"
                    self._device_errors.pop(reservation.device_id, None)
                return
            if reservation.state == "ready":
                reservation.state = status
                self._bootstrap_state_by_device[reservation.device_id] = status
                reservation.last_error = message
                if message:
                    self._device_errors[reservation.device_id] = message

    def _bootstrap_state_for_device_locked(self, device_id: str, reservation: Reservation | None = None) -> str:
        if reservation is not None:
            return reservation.state
        return self._bootstrap_state_by_device.get(device_id, "idle")

    def _start_bootstrap_thread(self, *, device_id: str, restart: bool) -> tuple[str, threading.Event]:
        with self._lock:
            existing = self._bootstrap_threads_by_device.get(device_id)
            if existing is not None and existing.is_alive():
                event = self._bootstrap_events_by_device.get(device_id)
                if event is None:
                    event = threading.Event()
                    self._bootstrap_events_by_device[device_id] = event
                return self._bootstrap_state_by_device.get(device_id, "bootstrapping"), event

            state = "restarting" if restart else "bootstrapping"
            self._bootstrap_state_by_device[device_id] = state
            self._device_errors.pop(device_id, None)
            event = threading.Event()
            self._bootstrap_events_by_device[device_id] = event
            thread = threading.Thread(
                target=self._bootstrap_device_background,
                kwargs={"device_id": device_id, "restart": restart},
                daemon=True,
            )
            self._bootstrap_threads_by_device[device_id] = thread
            thread.start()
            return state, event

    def _schedule_lane_cleanup(self, path: str) -> None:
        cleanup_script = (
            "target=\"$1\"\n"
            "while [ -e \"$target\" ]; do\n"
            "  rm -rf \"$target\" 2>/dev/null || true\n"
            "  [ ! -e \"$target\" ] && exit 0\n"
            "  sleep 1\n"
            "done\n"
        )
        subprocess.Popen(
            ["bash", "-lc", cleanup_script, "--", path],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
            env=launch_env(),
        )

    def _archive_snapshot_lane(self, *, device_id: str, plan: dict[str, str | None]) -> None:
        lane_root_value = plan.get("managed_sync_root")
        if not lane_root_value:
            return
        lane_root = Path(lane_root_value)
        repo_root = self._ezra_master_repo_root()
        timestamp = datetime.now().strftime("%Y%m%d-%H%M%S")
        next_root = lane_root.with_name(f"{lane_root.name}.next-{timestamp}")
        previous_root = lane_root.with_name(f"{lane_root.name}.previous-{timestamp}")
        dotenv_bytes: bytes | None = None
        dotenv_path = lane_root / ".env"
        if dotenv_path.exists():
            dotenv_bytes = dotenv_path.read_bytes()

        lane_root.parent.mkdir(parents=True, exist_ok=True)
        if next_root.exists():
            shutil.rmtree(next_root, ignore_errors=True)
        next_root.mkdir(parents=True, exist_ok=True)

        self._append_bootstrap_log(
            device_id,
            f"broker: archiving tracked repo state from {repo_root}",
            f"broker: preparing fresh lane at {next_root}",
        )

        archive_cmd = ["git", "-C", str(repo_root), "archive", "--format=tar", "HEAD"]
        extract_cmd = ["tar", "-x", "-C", str(next_root)]
        archive = subprocess.Popen(
            archive_cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=False,
            env=launch_env(),
        )
        extract = subprocess.Popen(
            extract_cmd,
            stdin=archive.stdout,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=launch_env(),
        )
        if archive.stdout is not None:
            archive.stdout.close()
        extract_stdout, extract_stderr = extract.communicate()
        archive_stderr_bytes = b""
        if archive.stderr is not None:
            archive_stderr_bytes = archive.stderr.read()
        archive_return = archive.wait()
        archive_stderr = archive_stderr_bytes.decode("utf-8", errors="replace").strip()
        if archive_return != 0 or extract.returncode != 0:
            detail = "\n".join(
                part
                for part in (
                    archive_stderr,
                    (extract_stdout or "").strip(),
                    (extract_stderr or "").strip(),
                )
                if part
            ).strip() or "git archive failed"
            shutil.rmtree(next_root, ignore_errors=True)
            raise BridgeError(detail)

        if dotenv_bytes is not None:
            (next_root / ".env").write_bytes(dotenv_bytes)
            self._append_bootstrap_log(device_id, "broker: preserved lane-local .env")

        if lane_root.exists():
            lane_root.rename(previous_root)
            self._append_bootstrap_log(device_id, f"broker: parked previous lane at {previous_root}")
        next_root.rename(lane_root)
        self._append_bootstrap_log(device_id, f"broker: activated fresh lane at {lane_root}")

        if previous_root.exists():
            self._append_bootstrap_log(device_id, f"broker: scheduling best-effort cleanup for {previous_root}")
            self._schedule_lane_cleanup(str(previous_root))

    def _teardown_managed_sync(self, reservation: Reservation) -> None:
        _ = reservation

    def _ensure_managed_api(self, *, device_id: str, plan: dict[str, str | None]) -> list[str]:
        api_root = plan.get("managed_api_root")
        api_project = plan.get("managed_api_project")
        api_port = plan.get("managed_api_port")
        platform = str(plan.get("platform") or "ios")
        if not api_root or not api_project or not api_port:
            return []
        container_port, compose_notes = self._prepare_ai_integration_compose(api_root=api_root)
        docker_path = Path(DOCKER_EXECUTABLE)
        if not docker_path.exists():
            raise BridgeError(f"docker was not found at {DOCKER_EXECUTABLE}")

        docker_check = subprocess.run(
            [str(docker_path), "info"],
            capture_output=True,
            text=True,
            env=launch_env(),
        )
        if docker_check.returncode != 0:
            detail = (docker_check.stderr or docker_check.stdout).strip() or "docker info failed"
            raise BridgeError(f"docker is unavailable: {detail}")

        base_url = f"http://127.0.0.1:{api_port}"
        runtime_base_url = f"http://{api_host_for_platform(platform)}:{api_port}"
        plan["managed_api_base_url"] = base_url
        plan["managed_api_runtime_base_url"] = runtime_base_url
        self._set_bootstrap_api_details(
            device_id,
            platform=platform,
            compose_project=api_project,
            port=int(api_port),
            base_url=base_url,
            runtime_base_url=runtime_base_url,
        )
        lines = [
            f"broker: api compose project {api_project}",
            f"broker: api root {api_root}",
            f"broker: api base url {base_url}",
            f"broker: runtime api base url {runtime_base_url}",
            f"broker: api container port {container_port}",
            f"broker: login {EZRA_PILOT_LOGIN_EMAIL}:{EZRA_PILOT_LOGIN_PASSWORD}",
        ]
        lines.extend(compose_notes)
        self._append_bootstrap_log(device_id, *lines)
        self._append_api_log(device_id, *lines)

        steps = [
            ["up", "-d", "--build", "db_test", "test_runner"],
            ["exec", "-T", "test_runner", "cargo", "sqlx", "migrate", "run", "--source", "services/api/migrations"],
            ["up", "-d", "--build", "api_test"],
        ]
        for subcommand in steps:
            self._append_bootstrap_log(device_id, f"broker: docker compose {' '.join(subcommand)}")
            self._append_api_log(device_id, f"broker: docker compose {' '.join(subcommand)}")
            result = self._run_compose(
                api_root=api_root,
                api_project=api_project,
                api_port=api_port,
                subcommand=subcommand,
            )
            combined = "\n".join(part.strip() for part in ((result.stdout or ""), (result.stderr or "")) if part.strip())
            if combined:
                step_lines = [line.rstrip() for line in combined.splitlines()[-40:] if line.rstrip()]
                lines.extend(step_lines)
                self._append_bootstrap_log(device_id, *step_lines)
                self._append_api_log(device_id, *step_lines)
            if result.returncode != 0:
                if "--build" in subcommand:
                    target_dir = self._reset_cargo_target_dir(api_project)
                    retry_note = (
                        f"broker: compose build step failed; reset cargo target dir at {target_dir} and retrying once"
                    )
                    self._append_bootstrap_log(device_id, retry_note)
                    self._append_api_log(device_id, retry_note)
                    retry_result = self._run_compose(
                        api_root=api_root,
                        api_project=api_project,
                        api_port=api_port,
                        subcommand=subcommand,
                    )
                    retry_combined = "\n".join(
                        part.strip() for part in ((retry_result.stdout or ""), (retry_result.stderr or "")) if part.strip()
                    )
                    if retry_combined:
                        retry_lines = [line.rstrip() for line in retry_combined.splitlines()[-40:] if line.rstrip()]
                        lines.extend(retry_lines)
                        self._append_bootstrap_log(device_id, *retry_lines)
                        self._append_api_log(device_id, *retry_lines)
                    if retry_result.returncode == 0:
                        continue
                    original_detail = (
                        result.stderr or result.stdout or f"docker compose {' '.join(subcommand)} failed with exit code {result.returncode}"
                    ).strip()
                    retry_detail = (
                        retry_result.stderr
                        or retry_result.stdout
                        or f"docker compose retry {' '.join(subcommand)} failed with exit code {retry_result.returncode}"
                    ).strip()
                    detail = (
                        "Compose build failed, cargo target cache was reset, and retry still failed.\n"
                        f"first attempt: {original_detail}\n"
                        f"retry: {retry_detail}"
                    )
                    self._append_bootstrap_log(device_id, f"broker: compose retry failed: {detail}")
                    self._append_api_log(device_id, f"broker: compose retry failed: {detail}")
                    raise BridgeError(detail)
                detail = (result.stderr or result.stdout).strip() or f"docker compose {' '.join(subcommand)} failed with exit code {result.returncode}"
                self._append_bootstrap_log(device_id, f"broker: compose step failed: {detail}")
                self._append_api_log(device_id, f"broker: compose step failed: {detail}")
                raise BridgeError(detail)

        self._append_bootstrap_log(device_id, f"broker: waiting for API readiness at {base_url}/healthz/ready")
        self._append_api_log(device_id, f"broker: waiting for API readiness at {base_url}/healthz/ready")
        try:
            self._wait_for_api_ready_or_raise(device_id=device_id, api_project=api_project, base_url=base_url)
        except BridgeError as error:
            target_dir = self._reset_cargo_target_dir(api_project)
            retry_note = (
                f"broker: api_test failed before readiness; reset cargo target dir at {target_dir} and retrying api_test once"
            )
            self._append_bootstrap_log(device_id, retry_note)
            self._append_api_log(device_id, retry_note)
            retry_result = self._run_compose(
                api_root=api_root,
                api_project=api_project,
                api_port=api_port,
                subcommand=["up", "-d", "--build", "api_test"],
            )
            retry_combined = "\n".join(
                part.strip() for part in ((retry_result.stdout or ""), (retry_result.stderr or "")) if part.strip()
            )
            if retry_combined:
                retry_lines = [line.rstrip() for line in retry_combined.splitlines()[-40:] if line.rstrip()]
                lines.extend(retry_lines)
                self._append_bootstrap_log(device_id, *retry_lines)
                self._append_api_log(device_id, *retry_lines)
            if retry_result.returncode != 0:
                retry_detail = (
                    retry_result.stderr
                    or retry_result.stdout
                    or f"docker compose retry up -d --build api_test failed with exit code {retry_result.returncode}"
                ).strip()
                detail = (
                    "api_test failed before readiness, cargo target cache was reset, and retry build failed.\n"
                    f"first attempt: {error}\n"
                    f"retry: {retry_detail}"
                )
                self._append_bootstrap_log(device_id, f"broker: api_test retry failed: {detail}")
                self._append_api_log(device_id, f"broker: api_test retry failed: {detail}")
                raise BridgeError(detail) from error
            self._append_bootstrap_log(device_id, f"broker: waiting for API readiness at {base_url}/healthz/ready")
            self._append_api_log(device_id, f"broker: waiting for API readiness at {base_url}/healthz/ready")
            try:
                self._wait_for_api_ready_or_raise(device_id=device_id, api_project=api_project, base_url=base_url)
            except BridgeError as retry_error:
                detail = (
                    "api_test failed before readiness, cargo target cache was reset, and retry still failed.\n"
                    f"first attempt: {error}\n"
                    f"retry: {retry_error}"
                )
                self._append_bootstrap_log(device_id, f"broker: api readiness retry failed: {detail}")
                self._append_api_log(device_id, f"broker: api readiness retry failed: {detail}")
                raise BridgeError(detail) from retry_error
        self._append_bootstrap_log(device_id, "broker: API is ready")
        self._append_api_log(device_id, "broker: API is ready")
        return lines

    def _teardown_managed_api(self, reservation: Reservation) -> None:
        if not reservation.managed_api_root or not reservation.managed_api_project:
            return
        api_port = str(reservation.managed_api_port or 18080)
        self._run_compose(
            api_root=reservation.managed_api_root,
            api_project=reservation.managed_api_project,
            api_port=api_port,
            subcommand=["down", "-v", "--remove-orphans"],
        )

    def _teardown_plan_assets(self, plan: dict[str, str | None]) -> None:
        managed_api_root = plan.get("managed_api_root")
        managed_api_project = plan.get("managed_api_project")
        managed_api_port = plan.get("managed_api_port")
        if managed_api_root and managed_api_project:
            self._run_compose(
                api_root=managed_api_root,
                api_project=managed_api_project,
                api_port=str(managed_api_port or 18080),
                subcommand=["down", "-v", "--remove-orphans"],
            )
        _ = plan

    def start(self) -> None:
        if self._poll_thread is not None:
            return
        try:
            self.refresh_devices()
            self._reconcile_visible_devices_on_startup()
            self._bootstrap_visible_devices()
        except Exception:
            pass
        self._poll_thread = threading.Thread(target=self._poll_loop, daemon=True)
        self._poll_thread.start()

    def stop(self) -> None:
        self._stop_event.set()
        with self._lock:
            reservations = list(self._reservations_by_device.values())
        for reservation in reservations:
            self._stop_reservation(reservation)

    def _poll_loop(self) -> None:
        while not self._stop_event.is_set():
            try:
                self.refresh_devices()
            except Exception:
                pass
            if self._stop_event.wait(self.poll_interval_seconds):
                return

    def refresh_devices(self) -> list[dict[str, str]]:
        if not self._refresh_lock.acquire(blocking=False):
            with self._lock:
                return list(self._inventory.values())
        try:
            result = subprocess.run(
                [self._ensure_idb_available(), "list-targets"],
                capture_output=True,
                text=True,
                timeout=self.devices_timeout_seconds,
                env=launch_env(),
            )
        except subprocess.TimeoutExpired:
            message = "idb list-targets timed out"
            LOGGER.warning("flutter-sim device refresh failed: %s", message)
            with self._lock:
                self._last_refresh_error = message
                return list(self._inventory.values())
        finally:
            self._refresh_lock.release()
        output = (result.stdout or "").strip()
        error = (result.stderr or "").strip()
        if result.returncode != 0:
            message = error or output or "idb list-targets failed"
            LOGGER.warning("flutter-sim device refresh failed: %s", message)
            with self._lock:
                self._last_refresh_error = message
                return list(self._inventory.values())
        devices = parse_idb_list_targets_output(output)
        with self._lock:
            self._last_refresh_error = None
            self._last_refresh_at = time.time()
            seen = {device["device_id"] for device in devices}
            self._inventory = {device["device_id"]: device for device in devices}
            self._device_errors = {
                device_id: value
                for device_id, value in self._device_errors.items()
                if device_id in seen or device_id in self._reservations_by_device
            }
            self._bootstrap_state_by_device = {
                device_id: value
                for device_id, value in self._bootstrap_state_by_device.items()
                if device_id in seen or device_id in self._reservations_by_device
            }
            self._bootstrap_logs_by_device = {
                device_id: value
                for device_id, value in self._bootstrap_logs_by_device.items()
                if device_id in seen or device_id in self._reservations_by_device
            }
            self._bootstrap_api_by_device = {
                device_id: value
                for device_id, value in self._bootstrap_api_by_device.items()
                if device_id in seen or device_id in self._reservations_by_device
            }
            self._failed_runtime_sessions_by_device = {
                device_id: value
                for device_id, value in self._failed_runtime_sessions_by_device.items()
                if device_id in seen or device_id in self._reservations_by_device
            }
        return devices

    def devices(self) -> list[dict[str, Any]]:
        with self._lock:
            reservations = list(self._reservations_by_device.values())
        for reservation in reservations:
            self._reconcile_reservation_api_health(reservation)
        with self._lock:
            rows: list[dict[str, Any]] = []
            for device_id, device in sorted(self._inventory.items(), key=lambda item: item[1]["name"]):
                reservation = self._reservations_by_device.get(device_id)
                rows.append(
                    {
                        **device,
                        "available": reservation is None,
                        "reservation_path": reservation.path if reservation else None,
                        "state": self._bootstrap_state_for_device_locked(device_id, reservation),
                        "dtd_uri": reservation.dtd_uri if reservation else None,
                        "app_uri": reservation.app_uri if reservation else None,
                        "pid": reservation.process.pid if reservation else None,
                        "last_error": self._device_errors.get(device_id),
                        "api_status": reservation.api_health_status if reservation else None,
                        "api_message": reservation.api_health_message if reservation else None,
                        "recent_output": (
                            list(self._bootstrap_logs_by_device.get(device_id, ()))
                            if reservation is None and device_id not in self._failed_runtime_sessions_by_device
                            else (self._failed_runtime_sessions_by_device.get(device_id, {}).get("recent_output") if reservation is None else None)
                        ),
                    }
                )
            return rows

    def health(self) -> dict[str, Any]:
        with self._lock:
            return {
                "ok": True,
                "host_home": str(self.paths.host_home),
                "allowed_roots": [str(root) for root in self.paths.allowed_roots],
                "known_devices": len(self._inventory),
                "active_reservations": len(self._reservations_by_device),
                "bootstrapping_devices": sum(
                    1 for state in self._bootstrap_state_by_device.values() if state not in {"idle", "ready", "failed"}
                ),
                "last_refresh_at": self._last_refresh_at,
                "last_refresh_error": self._last_refresh_error,
            }

    def session_for_device(self, device_id: str) -> dict[str, Any] | None:
        device_id = self._normalize_device_id(device_id)
        with self._lock:
            reservation = self._reservations_by_device.get(device_id)
            device = self._inventory.get(device_id)
            if reservation is not None:
                return reservation.snapshot()
            failed_session = self._failed_runtime_sessions_by_device.get(device_id)
            if failed_session is not None:
                return failed_session
            if device is None:
                return None
            return {
                "device_id": device_id,
                "device_name": device["name"],
                "path": self._default_ezra_app_path(),
                "launch_path": str(self._ezra_lane_root(device_id=device_id, device_name=device["name"]) / "clients" / "app"),
                "lane_root": str(self._ezra_lane_root(device_id=device_id, device_name=device["name"])),
                "target": "default",
                "pid": None,
                "state": self._bootstrap_state_by_device.get(device_id, "idle"),
                "dtd_uri": None,
                "app_uri": None,
                "last_error": self._device_errors.get(device_id),
                "created_at": None,
                "recent_output": list(self._bootstrap_logs_by_device.get(device_id, ())),
                "api": self._bootstrap_api_by_device.get(
                    device_id,
                    {
                        "compose_project": self._api_project_name(device_id=device_id, device_name=device["name"]),
                        "port": None,
                        "base_url": None,
                        "connection_domain": None,
                        "login_email": EZRA_PILOT_LOGIN_EMAIL,
                        "login_password": EZRA_PILOT_LOGIN_PASSWORD,
                    },
                ),
            }

    def reserve(self, *, device_id: str) -> dict[str, Any]:
        device_id = self._normalize_device_id(device_id)
        try:
            self._device_for_id(device_id)
        except BridgeError as error:
            return {
                "ok": False,
                "message": str(error),
                "available_devices": self.devices(),
            }

        with self._lock:
            reservation = self._reservations_by_device.get(device_id)
        if reservation is not None and reservation.state == "ready":
            try:
                reservation = self._ready_reservation_for_device(device_id)
            except BridgeError as error:
                return {
                    "ok": False,
                    "message": str(error),
                    "available_devices": self.devices(),
                }
            return self._ready_runtime_response(reservation)

        state, event = self._start_bootstrap_thread(device_id=device_id, restart=False)
        event.wait()

        ready_event: threading.Event | None = None
        with self._lock:
            reservation = self._reservations_by_device.get(device_id)
            if reservation is not None and reservation.state != "ready":
                ready_event = reservation.ready_event
        if ready_event is not None:
            ready_event.wait()

        with self._lock:
            reservation = self._reservations_by_device.get(device_id)
            last_error = self._device_errors.get(device_id)

        if reservation is not None and reservation.state == "ready":
            return self._ready_runtime_response(reservation)

        return {
            "ok": False,
            "message": last_error or f"Runtime {state} failed before the runtime became ready.",
            "session": self.session_for_device(device_id),
            "available_devices": self.devices(),
        }

    def restart(self, *, device_id: str) -> dict[str, Any]:
        device_id = self._normalize_device_id(device_id)
        try:
            self._device_for_id(device_id)
        except BridgeError as error:
            return {
                "ok": False,
                "message": str(error),
                "available_devices": self.devices(),
            }

        state, event = self._start_bootstrap_thread(device_id=device_id, restart=True)
        event.wait()

        ready_event: threading.Event | None = None
        with self._lock:
            reservation = self._reservations_by_device.get(device_id)
            if reservation is not None and reservation.state != "ready":
                ready_event = reservation.ready_event
        if ready_event is not None:
            ready_event.wait()

        with self._lock:
            reservation = self._reservations_by_device.get(device_id)
            last_error = self._device_errors.get(device_id)

        if reservation is not None and reservation.state == "ready":
            return self._ready_runtime_response(reservation)

        return {
            "ok": False,
            "message": last_error or f"Runtime {state} failed before the runtime became ready.",
            "session": self.session_for_device(device_id),
            "available_devices": self.devices(),
        }

    def dump_logs(self, *, device_id: str) -> dict[str, Any]:
        device_id = self._normalize_device_id(device_id)
        timestamp = datetime.now().strftime("%Y%m%d-%H%M%S")
        output_dir = SCREENSHOT_ROOT / device_id / "logs"
        output_dir.mkdir(parents=True, exist_ok=True)

        copied_paths: list[str] = []

        def maybe_copy(source: Path, suffix: str) -> None:
            if not source.exists():
                return
            destination = output_dir / f"{timestamp}-{suffix}"
            shutil.copy2(source, destination)
            copied_paths.append(str(destination))

        if BROKER_LAUNCH_LOG_DIR:
            launch_dir = Path(BROKER_LAUNCH_LOG_DIR)
            maybe_copy(launch_dir / "devices" / f"{device_id}.log", "device.log")
            maybe_copy(launch_dir / "api" / f"{device_id}.log", "api.log")
            maybe_copy(launch_dir / "runtime" / f"{device_id}.log", "runtime.log")
            maybe_copy(launch_dir / "broker.stdout.log", "broker.stdout.log")
            maybe_copy(launch_dir / "broker.stderr.log", "broker.stderr.log")
        with self._lock:
            reservation = self._reservations_by_device.get(device_id)
            artifacts_root = reservation.artifacts_root if reservation is not None else None
        if artifacts_root:
            source_root = Path(artifacts_root)
            if source_root.exists():
                destination_root = output_dir / f"{timestamp}-driver"
                shutil.copytree(source_root, destination_root, dirs_exist_ok=True)
                copied_paths.extend(str(path) for path in sorted(destination_root.rglob("*")) if path.is_file())

        if not copied_paths:
            return {"ok": False, "message": f"No logs are available for device {device_id}."}

        return {
            "ok": True,
            "message": f"Log snapshot created for device {device_id}.",
            "paths": copied_paths,
        }

    def release(self, *, device_id: str) -> dict[str, Any]:
        device_id = self._normalize_device_id(device_id)
        with self._lock:
            existing = self._reservations_by_device.get(device_id)
            if existing is None:
                return {
                    "ok": False,
                    "message": f"No active reservation exists for device {device_id}.",
                    "available_devices": self._available_devices_locked(),
                }
            self._stop_reservation(existing, teardown_sync=True, teardown_api=True)
        return {
            "ok": True,
            "message": "Runtime released.",
            "available_devices": self.devices(),
        }

    def _default_ezra_app_path(self) -> str:
        return str((self._ezra_master_repo_root() / "clients" / "app").resolve(strict=False))

    def _ready_runtime_response(self, reservation: Reservation) -> dict[str, Any]:
        connection_domain = connection_domain_for_platform(reservation.platform, reservation.managed_api_port) or "unknown"
        login_email = EZRA_PILOT_LOGIN_EMAIL
        login_password = EZRA_PILOT_LOGIN_PASSWORD
        return {
            "ok": True,
            "message": (
                f"Device {reservation.device_id} is ready to be driven. "
                f"API: {connection_domain}. "
                f"Login: {login_email} / {login_password}."
            ),
        }

    def _reconcile_visible_devices_on_startup(self) -> None:
        with self._lock:
            devices = list(self._inventory.values())
        for device in devices:
            device_id = device["device_id"]
            try:
                plan = self._launch_plan_for_device(path=self._default_ezra_app_path(), device=device)
                api_root = plan.get("managed_api_root")
                api_project = plan.get("managed_api_project")
                api_port = plan.get("managed_api_port")
                if not api_root or not api_project:
                    continue
                self._append_bootstrap_log(
                    device_id,
                    "broker: reconciling managed runtime on startup",
                    f"broker: stopping stale compose project {api_project}",
                )
                self._run_compose(
                    api_root=api_root,
                    api_project=api_project,
                    api_port=str(api_port or 18080),
                    subcommand=["down", "-v", "--remove-orphans"],
                )
            except Exception as error:
                self._append_bootstrap_log(device_id, f"broker: startup reconcile skipped: {error}")

    def _bootstrap_visible_devices(self) -> None:
        with self._lock:
            device_ids = sorted(self._inventory.keys())
        for device_id in device_ids:
            threading.Thread(target=self._bootstrap_device_background, args=(device_id,), daemon=True).start()

    def _bootstrap_device_background(self, device_id: str, restart: bool = False) -> None:
        try:
            self._clear_bootstrap_diagnostics(device_id)
            if restart:
                self._restart_device_runtime(device_id=device_id, target="default")
            else:
                self._ensure_device_runtime(device_id=device_id, target="default")
        except Exception as error:
            self._append_bootstrap_log(device_id, f"broker: bootstrap failed: {error}")
            self._set_bootstrap_state(device_id, "failed", error=str(error))
        finally:
            with self._lock:
                thread = self._bootstrap_threads_by_device.get(device_id)
                if thread is threading.current_thread():
                    self._bootstrap_threads_by_device.pop(device_id, None)
                event = self._bootstrap_events_by_device.get(device_id)
                if event is not None:
                    event.set()

    def _device_for_id(self, device_id: str) -> dict[str, str]:
        device_id = self._normalize_device_id(device_id)
        with self._lock:
            device = self._inventory.get(device_id)
        if device is not None:
            return device
        self.refresh_devices()
        with self._lock:
            device = self._inventory.get(device_id)
        if device is None:
            raise BridgeError(f"Device {device_id} is not currently available.")
        return device

    def _ensure_device_runtime(self, *, device_id: str, target: str) -> Reservation:
        with self._lock:
            existing = self._reservations_by_device.get(device_id)
            if existing is not None:
                return existing
        device = self._device_for_id(device_id)
        plan = self._launch_plan_for_device(path=self._default_ezra_app_path(), device=device)
        self._append_bootstrap_log(
            device_id,
            f"broker: runtime requested for {device['name']}",
            f"broker: launch path {plan['launch_path']}",
        )
        with self._launch_lock_for_path(str(plan["launch_path"])):
            with self._lock:
                existing = self._reservations_by_device.get(device_id)
                if existing is not None:
                    return existing
            try:
                self._set_bootstrap_state(device_id, "syncing")
                self._archive_snapshot_lane(device_id=device_id, plan=plan)
                self._set_bootstrap_state(device_id, "starting_api")
                api_output = self._ensure_managed_api(device_id=device_id, plan=plan)
                with self._lock:
                    existing = self._reservations_by_device.get(device_id)
                    if existing is not None:
                        return existing
                    self._set_bootstrap_state(device_id, "launching")
                    return self._launch_locked(plan=plan, target=target, device=device, prelaunch_output=api_output)
            except Exception as error:
                self._append_bootstrap_log(device_id, "broker: preserving failed lane for inspection")
                if isinstance(error, BridgeError):
                    raise
                raise BridgeError(str(error)) from error

    def _restart_device_runtime(self, *, device_id: str, target: str) -> Reservation:
        device = self._device_for_id(device_id)
        plan = self._launch_plan_for_device(path=self._default_ezra_app_path(), device=device)
        self._append_bootstrap_log(
            device_id,
            f"broker: runtime restart requested for {device['name']}",
            f"broker: launch path {plan['launch_path']}",
        )
        with self._launch_lock_for_path(str(plan["launch_path"])):
            try:
                self._set_bootstrap_state(device_id, "stopping")
                with self._lock:
                    existing = self._reservations_by_device.get(device_id)
                if existing is not None:
                    self._stop_reservation(existing, teardown_sync=False, teardown_api=True)
                self._set_bootstrap_state(device_id, "syncing")
                self._archive_snapshot_lane(device_id=device_id, plan=plan)
                self._set_bootstrap_state(device_id, "starting_api")
                api_output = self._ensure_managed_api(device_id=device_id, plan=plan)
                with self._lock:
                    self._set_bootstrap_state(device_id, "launching")
                    return self._launch_locked(plan=plan, target=target, device=device, prelaunch_output=api_output)
            except Exception as error:
                self._append_bootstrap_log(device_id, "broker: preserving failed lane for inspection")
                if isinstance(error, BridgeError):
                    raise
                raise BridgeError(str(error)) from error

    def _available_devices_locked(self) -> list[dict[str, Any]]:
        rows = []
        for device_id, device in sorted(self._inventory.items(), key=lambda item: item[1]["name"]):
            if device_id in self._reservations_by_device:
                continue
            rows.append(
                {
                    **device,
                    "available": True,
                    "reservation_path": None,
                    "state": "idle",
                }
            )
        return rows

    def _launch_lock_for_path(self, path: str) -> threading.Lock:
        with self._lock:
            lock = self._launch_locks_by_path.get(path)
            if lock is None:
                lock = threading.Lock()
                self._launch_locks_by_path[path] = lock
            return lock

    def _driver_lock_for_device(self, device_id: str) -> threading.Lock:
        with self._lock:
            lock = self._driver_locks_by_device.get(device_id)
            if lock is None:
                lock = threading.Lock()
                self._driver_locks_by_device[device_id] = lock
            return lock

    def _driver_artifacts_root(self, reservation: Reservation) -> Path:
        if reservation.artifacts_root:
            root = Path(reservation.artifacts_root)
            root.mkdir(parents=True, exist_ok=True)
            return root
        if BROKER_LAUNCH_LOG_DIR:
            root = Path(BROKER_LAUNCH_LOG_DIR) / "driver" / reservation.device_id / (reservation.runtime_label or "runtime")
        else:
            root = SCREENSHOT_ROOT / reservation.device_id / "driver-runtime" / (reservation.runtime_label or "runtime")
        root.mkdir(parents=True, exist_ok=True)
        reservation.artifacts_root = str(root)
        return root

    def _next_driver_run_dir(self, reservation: Reservation, *, kind: str) -> Path:
        with self._lock:
            reservation.flow_sequence += 1
            sequence = reservation.flow_sequence
        root = self._driver_artifacts_root(reservation)
        run_dir = root / f"{sequence:04d}-{kind}"
        run_dir.mkdir(parents=True, exist_ok=True)
        return run_dir

    def _ensure_idb_available(self) -> str:
        idb_path = idb_executable()
        if idb_path and Path(idb_path).exists():
            return idb_path
        resolved = shutil.which("idb")
        if resolved:
            return resolved
        raise BridgeError("idb CLI is not installed or not available on PATH.")

    def _run_idb_cli(
        self,
        *,
        argv: list[str],
        cwd: Path,
    ) -> subprocess.CompletedProcess[str]:
        idb = self._ensure_idb_available()
        return subprocess.run(
            [idb, *argv],
            capture_output=True,
            text=True,
            cwd=str(cwd),
            env=launch_env(),
        )

    def _idb_screen_dimensions_points(self, *, reservation: Reservation) -> tuple[int, int]:
        result = self._run_idb_cli(
            argv=["describe", "--udid", reservation.device_id],
            cwd=Path(reservation.launch_path),
        )
        if result.returncode != 0:
            raise BridgeError((result.stderr or result.stdout or "idb describe failed").strip())
        match = re.search(r"width_points=(\d+), height_points=(\d+)", result.stdout or "")
        if not match:
            raise BridgeError("Could not determine idb screen point dimensions.")
        return int(match.group(1)), int(match.group(2))

    def _idb_element_frame_bounds(self, element: dict[str, Any]) -> tuple[int, int, int, int] | None:
        frame = element.get("frame")
        if not isinstance(frame, dict):
            return None
        try:
            x = int(round(float(frame["x"])))
            y = int(round(float(frame["y"])))
            width = int(round(float(frame["width"])))
            height = int(round(float(frame["height"])))
        except Exception:
            return None
        return x, y, x + width, y + height

    def _idb_element_center(self, element: dict[str, Any]) -> tuple[int, int] | None:
        bounds = self._idb_element_frame_bounds(element)
        if bounds is None:
            return None
        left, top, right, bottom = bounds
        return int((left + right) / 2), int((top + bottom) / 2)

    def _idb_tap_coordinates_for_accessibility_point(
        self,
        *,
        reservation: Reservation,
        elements: list[dict[str, Any]],
        point: tuple[int, int],
    ) -> tuple[int, int]:
        if not elements:
            raise BridgeError("Accessibility dump is empty.")
        root = elements[0]
        root_frame = root.get("frame")
        if not isinstance(root_frame, dict):
            raise BridgeError("Accessibility root is missing frame data.")
        try:
            root_width = int(round(float(root_frame["width"])))
            root_height = int(round(float(root_frame["height"])))
        except Exception as error:
            raise BridgeError("Accessibility root frame is invalid.") from error
        portrait_width, portrait_height = self._idb_screen_dimensions_points(reservation=reservation)
        x, y = point
        if root_width == portrait_width and root_height == portrait_height:
            return x, y
        if root_width == portrait_height and root_height == portrait_width:
            # Supported landscape orientation only. The operator agreed to keep
            # simulators in the non-reversed landscape orientation.
            return y, x
        raise BridgeError(
            "Unsupported simulator orientation for idb coordinate mapping. "
            "Use portrait or the supported landscape orientation."
        )

    def _idb_selector_candidates(self, selector: Any) -> list[tuple[str, str]]:
        if isinstance(selector, str):
            value = selector.strip()
            return [("AXLabel", value), ("AXUniqueId", value), ("AXValue", value)]
        if not isinstance(selector, dict):
            raise BridgeError("Selector must be a string or object.")
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
            raise BridgeError("Selector object must include id, text, label, or value.")
        return candidates

    def _find_idb_element(self, elements: list[dict[str, Any]], selector: Any) -> dict[str, Any]:
        candidates = self._idb_selector_candidates(selector)
        for field, expected in candidates:
            for element in elements:
                actual = element.get(field)
                if isinstance(actual, str) and actual == expected:
                    return element
        for field, expected in candidates:
            lowered = expected.casefold()
            for element in elements:
                actual = element.get(field)
                if isinstance(actual, str) and actual.casefold() == lowered:
                    return element
        raise BridgeError(f"Could not find an accessibility element matching {selector!r}.")

    def _idb_describe_all(self, *, reservation: Reservation) -> list[dict[str, Any]]:
        result = self._run_idb_cli(
            argv=["ui", "describe-all", "--json", "--udid", reservation.device_id],
            cwd=Path(reservation.launch_path),
        )
        if result.returncode != 0:
            raise BridgeError((result.stderr or result.stdout or "idb ui describe-all failed").strip())
        try:
            parsed = json.loads(result.stdout or "[]")
        except json.JSONDecodeError as error:
            raise BridgeError(f"idb ui describe-all returned invalid JSON: {error}") from error
        if not isinstance(parsed, list):
            raise BridgeError("idb ui describe-all did not return a JSON array.")
        return [element for element in parsed if isinstance(element, dict)]

    def _serialize_idb_hierarchy(self, elements: list[dict[str, Any]]) -> str:
        return json.dumps(elements, separators=(",", ":"))

    def _write_driver_result(
        self,
        *,
        run_dir: Path,
        kind: str,
        returncode: int,
        stdout: str,
        stderr: str,
        metadata: dict[str, Any],
    ) -> None:
        (run_dir / "stdout.log").write_text(stdout, encoding="utf-8")
        (run_dir / "stderr.log").write_text(stderr, encoding="utf-8")
        payload = {"kind": kind, "returncode": returncode, **metadata}
        (run_dir / "result.json").write_text(self._serialize_json_compact(payload), encoding="utf-8")

    def _idb_tap_selector(
        self,
        *,
        reservation: Reservation,
        selector: Any,
        run_dir: Path,
        duration: float | None = None,
    ) -> dict[str, Any]:
        elements = self._idb_describe_all(reservation=reservation)
        element = self._find_idb_element(elements, selector)
        center = self._idb_element_center(element)
        if center is None:
            raise BridgeError(f"Element {selector!r} does not have a usable frame.")
        tap_x, tap_y = self._idb_tap_coordinates_for_accessibility_point(
            reservation=reservation,
            elements=elements,
            point=center,
        )
        argv = ["ui", "tap"]
        if duration is not None:
            argv.extend(["--duration", str(duration)])
        argv.extend(["--udid", reservation.device_id, str(tap_x), str(tap_y)])
        result = self._run_idb_cli(argv=argv, cwd=Path(reservation.launch_path))
        self._write_driver_result(
            run_dir=run_dir,
            kind="tap",
            returncode=result.returncode,
            stdout=result.stdout or "",
            stderr=result.stderr or "",
            metadata={
                "device_id": reservation.device_id,
                "selector": selector,
                "center": list(center),
                "tap_point": [tap_x, tap_y],
            },
        )
        if result.returncode != 0:
            raise BridgeError((result.stderr or result.stdout or "idb ui tap failed").strip())
        return {"center": center, "tap_point": (tap_x, tap_y), "element": element}

    def _ready_reservation_for_device(self, device_id: str) -> Reservation:
        device_id = self._normalize_device_id(device_id)
        with self._lock:
            reservation = self._reservations_by_device.get(device_id)
        if reservation is None:
            raise BridgeError(f"Device {device_id} does not have an active reservation.")
        self._reconcile_reservation_api_health(reservation)
        if reservation.state != "ready" or reservation.process.poll() is not None:
            if reservation.api_health_status in {"api_down", "api_crashed"}:
                raise BridgeError(
                    reservation.api_health_message
                    or f"Managed API stack is down for device {device_id}. Run dump-logs or reboot."
                )
            raise BridgeError(f"Device {device_id} is not ready for commands.")
        return reservation

    def _serialize_json_compact(self, payload: Any) -> str:
        return json.dumps(payload, separators=(",", ":"))

    def maestro_hierarchy(self, *, device_id: str) -> dict[str, Any]:
        device_id = self._normalize_device_id(device_id)
        try:
            reservation = self._ready_reservation_for_device(device_id)
        except BridgeError as error:
            return {"ok": False, "message": str(error)}
        run_dir = self._next_driver_run_dir(reservation, kind="hierarchy")
        with self._driver_lock_for_device(device_id):
            try:
                elements = self._idb_describe_all(reservation=reservation)
            except BridgeError as error:
                self._write_driver_result(
                    run_dir=run_dir,
                    kind="hierarchy",
                    returncode=1,
                    stdout="",
                    stderr=str(error),
                    metadata={"device_id": device_id},
                )
                return {"ok": False, "message": str(error)}
        hierarchy = self._serialize_idb_hierarchy(elements)
        self._write_driver_result(
            run_dir=run_dir,
            kind="hierarchy",
            returncode=0,
            stdout=hierarchy,
            stderr="",
            metadata={"device_id": device_id, "count": len(elements)},
        )
        return {
            "ok": True,
            "device_id": device_id,
            "hierarchy": hierarchy,
            "artifacts": [str(run_dir / "stdout.log"), str(run_dir / "stderr.log"), str(run_dir / "result.json")],
        }

    def maestro_flow(
        self,
        *,
        device_id: str,
        commands: list[Any],
        label: str | None = None,
    ) -> dict[str, Any]:
        device_id = self._normalize_device_id(device_id)
        try:
            reservation = self._ready_reservation_for_device(device_id)
        except BridgeError as error:
            return {"ok": False, "message": str(error)}
        if not isinstance(commands, list) or not commands:
            return {"ok": False, "message": "Flow commands must be a non-empty list."}
        run_dir = self._next_driver_run_dir(reservation, kind="flow")
        command_log = run_dir / "flow.json"
        command_log.write_text(json.dumps(commands, indent=2), encoding="utf-8")
        stdout_lines: list[str] = []
        with self._driver_lock_for_device(device_id):
            screenshots: list[str] = []
            try:
                for index, command_entry in enumerate(commands, start=1):
                    response = self._execute_driver_command_entry(
                        reservation=reservation,
                        command_entry=command_entry,
                        run_dir=run_dir,
                        step_index=index,
                    )
                    message = response.get("message") or f"step {index} ok"
                    stdout_lines.append(message)
                    screenshot_path = response.get("screenshot")
                    if isinstance(screenshot_path, str) and screenshot_path:
                        screenshots.append(screenshot_path)
            except BridgeError as error:
                self._write_driver_result(
                    run_dir=run_dir,
                    kind="flow",
                    returncode=1,
                    stdout="\n".join(stdout_lines),
                    stderr=str(error),
                    metadata={"device_id": device_id, "label": label, "steps": len(commands)},
                )
                return {
                    "ok": False,
                    "message": str(error),
                    "stdout": "\n".join(stdout_lines),
                    "stderr": str(error),
                    "artifacts": [str(command_log), str(run_dir / "stdout.log"), str(run_dir / "stderr.log"), str(run_dir / "result.json")],
                }
        self._write_driver_result(
            run_dir=run_dir,
            kind="flow",
            returncode=0,
            stdout="\n".join(stdout_lines),
            stderr="",
            metadata={"device_id": device_id, "label": label, "steps": len(commands), "screenshots": screenshots},
        )
        return {
            "ok": True,
            "device_id": device_id,
            "message": f"Executed driver flow on device {device_id}.",
            "artifacts": [str(command_log), str(run_dir / "stdout.log"), str(run_dir / "stderr.log"), str(run_dir / "result.json"), *screenshots],
            "screenshots": screenshots,
        }

    def maestro_command(
        self,
        *,
        device_id: str,
        command: str,
        input_payload: Any | None = None,
        label: str | None = None,
        out_path: str | None = None,
    ) -> dict[str, Any]:
        device_id = self._normalize_device_id(device_id)
        command = command.strip()
        if not command:
            return {"ok": False, "message": "command is required."}
        if command == "clearField":
            if not isinstance(input_payload, dict) or not input_payload:
                return {"ok": False, "message": "clearField requires a selector object."}
            selector = dict(input_payload)
            fallback_erase = input_payload.get("fallbackErase", 100)
            try:
                fallback_count = int(fallback_erase)
            except Exception:
                return {"ok": False, "message": "clearField fallbackErase must be an integer."}
            return self.maestro_flow(
                device_id=device_id,
                commands=[
                    {"tapOn": selector},
                    {"eraseText": fallback_count},
                    {"forwardEraseText": fallback_count},
                ],
                label=label or command,
            )
        if command == "takeScreenshot":
            response = self.maestro_flow(
                device_id=device_id,
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
                "device_id": device_id,
                "path": screenshot_path,
                "content_type": "image/png",
                "data_base64": base64.b64encode(image_bytes).decode("ascii"),
                "artifacts": response.get("artifacts", []),
            }
        return self.maestro_flow(
            device_id=device_id,
            commands=[{command: input_payload} if input_payload is not None else command],
            label=label or command,
        )

    def _execute_driver_command_entry(
        self,
        *,
        reservation: Reservation,
        command_entry: Any,
        run_dir: Path,
        step_index: int,
    ) -> dict[str, Any]:
        step_run_dir = run_dir / f"step-{step_index:02d}"
        step_run_dir.mkdir(parents=True, exist_ok=True)
        if isinstance(command_entry, str):
            command_name = command_entry
            payload = None
        elif isinstance(command_entry, dict) and len(command_entry) == 1:
            command_name, payload = next(iter(command_entry.items()))
        else:
            raise BridgeError(f"Unsupported flow command format: {command_entry!r}")

        if command_name == "tapOn":
            tap = self._idb_tap_selector(reservation=reservation, selector=payload, run_dir=step_run_dir)
            return {"ok": True, "message": f"tapOn {payload!r} -> {tap['center']}"}
        if command_name == "longPressOn":
            tap = self._idb_tap_selector(reservation=reservation, selector=payload, run_dir=step_run_dir, duration=0.8)
            return {"ok": True, "message": f"longPressOn {payload!r} -> {tap['center']}"}
        if command_name == "inputText":
            if not isinstance(payload, str):
                raise BridgeError("inputText requires a string payload.")
            result = self._run_idb_cli(
                argv=["ui", "text", "--udid", reservation.device_id, payload],
                cwd=Path(reservation.launch_path),
            )
            self._write_driver_result(
                run_dir=step_run_dir,
                kind="text",
                returncode=result.returncode,
                stdout=result.stdout or "",
                stderr=result.stderr or "",
                metadata={"device_id": reservation.device_id, "length": len(payload)},
            )
            if result.returncode != 0:
                raise BridgeError((result.stderr or result.stdout or "idb ui text failed").strip())
            return {"ok": True, "message": f"inputText {payload!r}"}
        if command_name == "takeScreenshot":
            requested_name = payload if isinstance(payload, str) and payload.strip() else f"step-{step_index:02d}.png"
            destination = step_run_dir / Path(requested_name).name
            result = self._run_idb_cli(
                argv=["screenshot", "--udid", reservation.device_id, str(destination)],
                cwd=Path(reservation.launch_path),
            )
            self._write_driver_result(
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
                argv.extend(["--duration", str(duration)])
            elements = self._idb_describe_all(reservation=reservation)
            tap_x_start, tap_y_start = self._idb_tap_coordinates_for_accessibility_point(
                reservation=reservation,
                elements=elements,
                point=(x_start, y_start),
            )
            tap_x_end, tap_y_end = self._idb_tap_coordinates_for_accessibility_point(
                reservation=reservation,
                elements=elements,
                point=(x_end, y_end),
            )
            argv.extend(
                [
                    "--udid",
                    reservation.device_id,
                    str(tap_x_start),
                    str(tap_y_start),
                    str(tap_x_end),
                    str(tap_y_end),
                ]
            )
            result = self._run_idb_cli(argv=argv, cwd=Path(reservation.launch_path))
            self._write_driver_result(
                run_dir=step_run_dir,
                kind="swipe",
                returncode=result.returncode,
                stdout=result.stdout or "",
                stderr=result.stderr or "",
                metadata={
                    "device_id": reservation.device_id,
                    "start": [x_start, y_start],
                    "end": [x_end, y_end],
                    "swipe_start": [tap_x_start, tap_y_start],
                    "swipe_end": [tap_x_end, tap_y_end],
                },
            )
            if result.returncode != 0:
                raise BridgeError((result.stderr or result.stdout or "idb ui swipe failed").strip())
            return {"ok": True, "message": f"swipe [{x_start},{y_start}] -> [{x_end},{y_end}]"}
        if command_name == "eraseText":
            count = 1
            if payload is not None:
                try:
                    count = int(payload)
                except Exception as error:
                    raise BridgeError("eraseText payload must be an integer.") from error
            for key_index in range(count):
                result = self._run_idb_cli(
                    argv=["ui", "key", "--udid", reservation.device_id, "DELETE"],
                    cwd=Path(reservation.launch_path),
                )
                if result.returncode != 0:
                    self._write_driver_result(
                        run_dir=step_run_dir,
                        kind="eraseText",
                        returncode=result.returncode,
                        stdout=result.stdout or "",
                        stderr=result.stderr or "",
                        metadata={"device_id": reservation.device_id, "count": count, "attempt": key_index + 1},
                    )
                    raise BridgeError((result.stderr or result.stdout or "idb ui key DELETE failed").strip())
            self._write_driver_result(
                run_dir=step_run_dir,
                kind="eraseText",
                returncode=0,
                stdout="",
                stderr="",
                metadata={"device_id": reservation.device_id, "count": count},
            )
            return {"ok": True, "message": f"eraseText {count}"}
        if command_name == "forwardEraseText":
            count = 1
            if payload is not None:
                try:
                    count = int(payload)
                except Exception as error:
                    raise BridgeError("forwardEraseText payload must be an integer.") from error
            for key_index in range(count):
                result = self._run_idb_cli(
                    argv=["ui", "key", "--udid", reservation.device_id, "76"],
                    cwd=Path(reservation.launch_path),
                )
                if result.returncode != 0:
                    self._write_driver_result(
                        run_dir=step_run_dir,
                        kind="forwardEraseText",
                        returncode=result.returncode,
                        stdout=result.stdout or "",
                        stderr=result.stderr or "",
                        metadata={"device_id": reservation.device_id, "count": count, "attempt": key_index + 1},
                    )
                    raise BridgeError((result.stderr or result.stdout or "idb ui key 76 failed").strip())
            self._write_driver_result(
                run_dir=step_run_dir,
                kind="forwardEraseText",
                returncode=0,
                stdout="",
                stderr="",
                metadata={"device_id": reservation.device_id, "count": count},
            )
            return {"ok": True, "message": f"forwardEraseText {count}"}
        if command_name == "hideKeyboard":
            result = self._run_idb_cli(
                argv=["ui", "key", "--udid", reservation.device_id, "ESCAPE"],
                cwd=Path(reservation.launch_path),
            )
            self._write_driver_result(
                run_dir=step_run_dir,
                kind="hideKeyboard",
                returncode=result.returncode,
                stdout=result.stdout or "",
                stderr=result.stderr or "",
                metadata={"device_id": reservation.device_id},
            )
            if result.returncode != 0:
                raise BridgeError((result.stderr or result.stdout or "idb hide keyboard failed").strip())
            return {"ok": True, "message": "hideKeyboard"}
        raise BridgeError(f"Unsupported idb flow command: {command_name}")

    def driver_apps(self, *, device_id: str) -> dict[str, Any]:
        device_id = self._normalize_device_id(device_id)
        try:
            reservation = self._ready_reservation_for_device(device_id)
        except BridgeError as error:
            return {"ok": False, "message": str(error)}
        return {
            "ok": True,
            "device_id": device_id,
            "apps": [{"name": "Runner", "appId": reservation.app_id or EZRA_IOS_APP_ID}],
        }

    def driver_widget_tree(
        self,
        *,
        device_id: str,
        summary: bool = True,
        full: bool = False,
        stable_reads: int = 1,
        max_attempts: int = 1,
        settle_ms: int = 300,
    ) -> dict[str, Any]:
        _ = (summary, full, stable_reads, max_attempts, settle_ms)
        hierarchy = self.maestro_hierarchy(device_id=device_id)
        if not hierarchy.get("ok", False):
            return hierarchy
        return {"ok": True, "device_id": self._normalize_device_id(device_id), "tree": hierarchy.get("hierarchy")}

    def driver_command(
        self,
        *,
        device_id: str,
        command: str,
        args: list[str] | None = None,
        input_payload: dict[str, Any] | None = None,
        timeout_ms: int = 5000,
        out_path: str | None = None,
    ) -> dict[str, Any]:
        _ = (args, timeout_ms)
        maestro_command = {
            "tap": "tapOn",
            "enter_text": "inputText",
            "get_health": None,
            "screenshot": "takeScreenshot",
        }.get(command)
        if command == "get_health":
            try:
                self._ready_reservation_for_device(device_id)
            except BridgeError as error:
                return {"ok": False, "message": str(error)}
            return {"ok": True, "device_id": self._normalize_device_id(device_id), "result": {"status": "ok"}}
        if maestro_command is None:
            return {"ok": False, "message": f"{command} is not supported by the idb-backed broker."}
        return self.maestro_command(
            device_id=device_id,
            command=maestro_command,
            input_payload=input_payload,
            label=command,
            out_path=out_path,
        )

    def _launch_locked(
        self,
        *,
        plan: dict[str, str | None],
        target: str,
        device: dict[str, str],
        prelaunch_output: list[str] | None = None,
    ) -> Reservation:
        launch_path = str(plan["launch_path"])
        cwd = Path(launch_path)
        if not cwd.exists():
            raise BridgeError(f"Path does not exist: {launch_path}")
        launch_notes = list(prelaunch_output or ())
        env = launch_env()
        managed_api_base_url = str(plan["managed_api_base_url"]) if plan["managed_api_base_url"] else None
        managed_api_runtime_base_url = (
            str(plan["managed_api_runtime_base_url"]) if plan.get("managed_api_runtime_base_url") else managed_api_base_url
        )
        if managed_api_runtime_base_url:
            env["EZRA_API_BASE_URL"] = managed_api_runtime_base_url
        clean_output = run_flutter_clean(cwd=launch_path, env=env)
        self._append_bootstrap_log(
            device["device_id"],
            f"broker: running flutter clean in {launch_path}",
            *clean_output[-40:],
            f"broker: launching flutter on {device['name']}",
            f"broker: flutter cwd {launch_path}",
            f"broker: flutter target {target}",
            f"broker: flutter executable {flutter_executable()}",
        )
        process = subprocess.Popen(
            [
                flutter_executable(),
                "run",
                "-d",
                device["device_id"],
                f"--dart-define=EZRA_PILOT_LOGIN_EMAIL={EZRA_PILOT_LOGIN_EMAIL}",
                f"--dart-define=EZRA_PILOT_LOGIN_PASSWORD={EZRA_PILOT_LOGIN_PASSWORD}",
                "--dart-define=EZRA_DEV_CLEAR_LOCAL_CACHE_ON_STARTUP=true",
            ],
            cwd=launch_path,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            stdin=subprocess.DEVNULL,
            text=True,
            bufsize=1,
            start_new_session=True,
            env=env,
        )
        reservation = Reservation(
            device_id=device["device_id"],
            device_name=device["name"],
            platform=device["platform"],
            path=str(plan["requested_path"]),
            launch_path=launch_path,
            target=target,
            process=process,
            created_at=time.time(),
            ready_event=threading.Event(),
            managed_sync_root=str(plan["managed_sync_root"]) if plan["managed_sync_root"] else None,
            managed_api_project=str(plan["managed_api_project"]) if plan["managed_api_project"] else None,
            managed_api_root=str(plan["managed_api_root"]) if plan["managed_api_root"] else None,
            managed_api_port=int(str(plan["managed_api_port"])) if plan["managed_api_port"] else None,
            managed_api_base_url=managed_api_base_url,
            managed_api_runtime_base_url=managed_api_runtime_base_url,
            app_id=default_app_id_for_platform(device["platform"]),
            maestro_driver_port=None,
            runtime_label=datetime.now().strftime("%Y%m%d-%H%M%S"),
        )
        for line in launch_notes:
            reservation.recent_output.append(line)
        self._append_runtime_log(reservation.device_id, *launch_notes)
        self._clear_bootstrap_diagnostics(reservation.device_id)
        self._reservations_by_device[reservation.device_id] = reservation
        self._bootstrap_state_by_device[reservation.device_id] = "launching"
        self._device_errors.pop(reservation.device_id, None)
        threading.Thread(target=self._pump_output, args=(reservation,), daemon=True).start()
        threading.Thread(target=self._watch_process, args=(reservation,), daemon=True).start()
        return reservation

    def _stop_reservation(
        self,
        reservation: Reservation,
        *,
        teardown_sync: bool = True,
        teardown_api: bool = True,
    ) -> None:
        for forwarder in list((reservation.dtd_forwarders or {}).values()):
            forwarder.stop()
        for forwarder in list((reservation.app_forwarders or {}).values()):
            forwarder.stop()
        if reservation.process.poll() is None:
            try:
                os.killpg(reservation.process.pid, signal.SIGTERM)
                reservation.process.wait(timeout=10)
            except Exception:
                try:
                    os.killpg(reservation.process.pid, signal.SIGKILL)
                except Exception:
                    pass
        self._reservations_by_device.pop(reservation.device_id, None)
        self._bootstrap_state_by_device[reservation.device_id] = "idle"
        self._device_errors.pop(reservation.device_id, None)
        if teardown_api:
            self._teardown_managed_api(reservation)
        if teardown_sync:
            self._teardown_managed_sync(reservation)

    def _pump_output(self, reservation: Reservation) -> None:
        stream = reservation.process.stdout
        if stream is None:
            return
        for line in stream:
            text = line.rstrip()
            if text:
                reservation.recent_output.append(text)
                self._append_runtime_log(reservation.device_id, text)
                append_launch_log(f"devices/{reservation.device_id}.log", text)
                LOGGER.info("flutter-sim[%s] %s", reservation.device_id, text)
            self._consume_runtime_line(reservation, text)

    def _consume_runtime_line(self, reservation: Reservation, line: str) -> None:
        if reservation.state != "ready":
            ready_markers = (
                "Flutter run key commands.",
                "A Dart VM Service on ",
                "The Flutter DevTools debugger and profiler on ",
            )
            if any(marker in line for marker in ready_markers):
                reservation.state = "ready"
                self._set_bootstrap_state(reservation.device_id, "ready")
                reservation.ready_event.set()
        if not line.startswith("{"):
            if not line.startswith("["):
                return
        try:
            payload = json.loads(line)
        except json.JSONDecodeError:
            return
        items: list[dict[str, Any]] = []
        if isinstance(payload, dict):
            items = [payload]
        elif isinstance(payload, list):
            items = [item for item in payload if isinstance(item, dict)]
        else:
            return

        for item in items:
            event = item.get("event")
            params = item.get("params")
            if not isinstance(params, dict):
                params = {}
            if event == "app.debugPort":
                uri = params.get("wsUri") or params.get("uri")
                if isinstance(uri, str) and uri:
                    reservation.app_uri = uri
            if event == "app.dtd":
                uri = params.get("wsUri") or params.get("uri")
                if isinstance(uri, str) and uri:
                    reservation.dtd_uri = uri
                    reservation.state = "ready"
                    self._set_bootstrap_state(reservation.device_id, "ready")
                    reservation.ready_event.set()
            if event == "app.stop":
                reservation.last_error = "flutter run stopped before the runtime finished launching"

    def _watch_process(self, reservation: Reservation) -> None:
        code = reservation.process.wait()
        with self._lock:
            current = self._reservations_by_device.get(reservation.device_id)
            if current is not reservation:
                return
            if reservation.state == "ready":
                self._device_errors[reservation.device_id] = f"flutter run exited with code {code}"
            else:
                reservation.state = "failed"
                reservation.last_error = (
                    reservation.last_error or f"flutter run exited with code {code} before the runtime became ready"
                )
                reservation.ready_event.set()
                self._device_errors[reservation.device_id] = reservation.last_error
                self._record_failed_runtime_session(reservation)
            self._reservations_by_device.pop(reservation.device_id, None)
        self._teardown_managed_api(reservation)
        self._teardown_managed_sync(reservation)
