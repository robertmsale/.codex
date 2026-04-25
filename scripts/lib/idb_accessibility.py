from __future__ import annotations

import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CODEX_SERVICES_SRC = ROOT / "backend" / "python" / "codex-services" / "src"
if str(CODEX_SERVICES_SRC) not in sys.path:
    sys.path.insert(0, str(CODEX_SERVICES_SRC))

from codex_services_http.idb_accessibility import *  # noqa: F401,F403
