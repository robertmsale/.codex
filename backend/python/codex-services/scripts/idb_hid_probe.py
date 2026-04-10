#!/usr/bin/env python3

import argparse
import asyncio
import logging
from pathlib import Path

from idb.common.hid import _key_down_event, _key_up_event, key_press_to_events
from idb.common.types import DomainSocketAddress, HIDDelay
from idb.grpc.client import Client


LEFT_COMMAND_KEYCODE = 227
A_KEYCODE = 4
BACKSPACE_KEYCODE = 42


async def run(udid: str, enable_hardware_keyboard: bool) -> None:
    companion_path = Path("/tmp/idb") / f"{udid}_companion.sock"
    if not companion_path.exists():
        raise SystemExit(f"companion socket not found: {companion_path}")

    logger = logging.getLogger("idb-hid-probe")
    logger.addHandler(logging.StreamHandler())
    logger.setLevel(logging.INFO)

    async with Client.build(
        address=DomainSocketAddress(path=str(companion_path)),
        logger=logger,
    ) as client:
        if enable_hardware_keyboard:
            await client.set_hardware_keyboard(True)

        events = [
            _key_down_event(LEFT_COMMAND_KEYCODE),
            _key_down_event(A_KEYCODE),
            _key_up_event(A_KEYCODE),
            _key_up_event(LEFT_COMMAND_KEYCODE),
            HIDDelay(duration=0.1),
            *key_press_to_events(BACKSPACE_KEYCODE),
        ]
        await client.send_events(events)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--udid", required=True)
    parser.add_argument("--enable-hardware-keyboard", action="store_true")
    args = parser.parse_args()
    asyncio.run(run(args.udid, args.enable_hardware_keyboard))


if __name__ == "__main__":
    main()
