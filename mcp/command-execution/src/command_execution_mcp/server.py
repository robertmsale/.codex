from __future__ import annotations

import re
import time
from pathlib import Path

from fastmcp import FastMCP

mcp = FastMCP("command-execution-mcp")
JOB_DIR = Path("/tmp/codex-command-jobs")


class CommandExecutionError(RuntimeError):
    pass


_JOB_ID_RE = re.compile(r"^[a-z0-9][a-z0-9-]{7,127}$")


def _marker_path(job_id: str) -> Path:
    return JOB_DIR / f"{job_id}.job"


@mcp.tool
def command_execution_wait(job_id: str) -> str:
    """Block until the launch-job marker file disappears."""
    jid = (job_id or "").strip()
    if not jid:
        raise CommandExecutionError("job_id is required")
    if not _JOB_ID_RE.fullmatch(jid):
        raise CommandExecutionError("invalid job_id format")

    marker = _marker_path(jid)
    if not marker.exists():
        return "all done"

    while marker.exists():
        time.sleep(2)

    return "all done"


def main() -> None:
    mcp.run(transport="stdio")


if __name__ == "__main__":
    main()
