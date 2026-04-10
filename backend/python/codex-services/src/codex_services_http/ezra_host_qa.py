from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path


def _slug(text: str) -> str:
    normalized = re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")
    return normalized or "lane"


@dataclass(frozen=True)
class EzraQaLane:
    device_id: str
    device_name: str
    lane_id: str
    qa_root: Path
    repo_root: Path
    app_path: Path
    api_project_name: str


@dataclass(frozen=True)
class EzraHostQaPlan:
    master_repo_root: Path
    qa_root: Path
    flutter_target: str
    api_launcher_script: Path
    api_project_prefix: str = "ezra-qa"
    repo_dir_name: str = "repo"

    @property
    def synced_repo_root(self) -> Path:
        return self.qa_root / self.repo_dir_name

    @property
    def app_path(self) -> Path:
        return self.synced_repo_root / "clients" / "app"

    def lane_for_device(self, *, device_id: str, device_name: str) -> EzraQaLane:
        lane_suffix = _slug(device_name)
        lane_id = f"{lane_suffix}-{device_id.lower()}"
        return EzraQaLane(
            device_id=device_id,
            device_name=device_name,
            lane_id=lane_id,
            qa_root=self.qa_root,
            repo_root=self.synced_repo_root,
            app_path=self.app_path,
            api_project_name=f"{self.api_project_prefix}-{lane_id}",
        )

    def lanes_for_devices(self, devices: list[dict[str, str]]) -> list[EzraQaLane]:
        lanes: list[EzraQaLane] = []
        for device in devices:
            device_id = (device.get("device_id") or "").strip()
            device_name = (device.get("name") or "").strip()
            if not device_id or not device_name:
                continue
            lanes.append(self.lane_for_device(device_id=device_id, device_name=device_name))
        return lanes
