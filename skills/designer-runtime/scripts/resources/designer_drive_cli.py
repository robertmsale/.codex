#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[4]
SHARED_SCRIPT_LIB = ROOT / "scripts" / "lib"
if str(SHARED_SCRIPT_LIB) not in sys.path:
    sys.path.insert(0, str(SHARED_SCRIPT_LIB))

from direct_idb_driver import add_common_subcommands
from direct_idb_driver import default_app_id_for_platform
from direct_idb_driver import run_cli


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="designer-drive")
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--app-id")
    add_common_subcommands(parser, include_devices=True, include_driver_alias=False)
    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    launch_path = Path(os.getcwd()).expanduser().resolve(strict=False)
    app_id = args.app_id or default_app_id_for_platform("ios")
    return run_cli(args=args, launch_path=launch_path, app_id=app_id, allow_devices=True)


if __name__ == "__main__":
    raise SystemExit(main())
