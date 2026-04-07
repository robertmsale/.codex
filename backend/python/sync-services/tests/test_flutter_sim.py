from pathlib import Path
import socket
from tempfile import TemporaryDirectory
import unittest

from sync_services_http.bridge import BridgePaths
from sync_services_http.flutter_sim import expose_loopback_uri
from sync_services_http.flutter_sim import FlutterSimManager
from sync_services_http.flutter_sim import normalize_request_path
from sync_services_http.flutter_sim import parse_flutter_devices_output
from sync_services_http.flutter_sim import rewrite_loopback_uri


class FlutterSimTests(unittest.TestCase):
    def test_parse_flutter_devices_output_filters_ios_simulators(self) -> None:
        sample = """
Found 8 connected devices:
  iPad Pro 13-inch (M5) (mobile) • 717308AA-BBDF-422E-88E2-422A353FE864 • ios            • com.apple.CoreSimulator.SimRuntime.iOS-26-3 (simulator)
  iPad Pro 11-inch (M5) (mobile) • F8080594-26F7-4170-826A-452C15769215 • ios            • com.apple.CoreSimulator.SimRuntime.iOS-26-3 (simulator)
  macOS (desktop)                • macos                                • darwin-arm64   • macOS 26.3.1 25D2128 darwin-arm64
  Chrome (web)                   • chrome                               • web-javascript • Google Chrome 145.0.7632.160
"""
        devices = parse_flutter_devices_output(sample)
        self.assertEqual(len(devices), 2)
        self.assertEqual(devices[0]["device_id"], "717308AA-BBDF-422E-88E2-422A353FE864")
        self.assertEqual(devices[1]["name"], "iPad Pro 11-inch (M5) (mobile)")

    def test_normalize_request_path_maps_vm_path_to_host_path(self) -> None:
        with TemporaryDirectory() as temp_dir:
            host_home = Path(temp_dir) / "Users" / "robertsale"
            repo = host_home / "Code" / "robdex"
            repo.mkdir(parents=True)
            paths = BridgePaths(
                host_home=host_home,
                virtual_home=Path("/home/robertsale"),
                allowed_roots=(repo,),
            )
            normalized = normalize_request_path("/home/robertsale/Code/robdex", paths)
            self.assertEqual(Path(normalized).resolve(strict=False), repo.resolve(strict=False))

    def test_consume_machine_line_accepts_array_wrapped_events(self) -> None:
        manager = FlutterSimManager()

        class DummyProcess:
            pid = 123

            def poll(self):
                return None

        reservation = manager._launch_locked.__self__ if False else None
        from threading import Event
        from sync_services_http.flutter_sim import Reservation

        session = Reservation(
            device_id="dev",
            device_name="iPad",
            path="/tmp/example",
            launch_path="/tmp/example",
            target="lib/main.dart",
            process=DummyProcess(),
            created_at=0.0,
            ready_event=Event(),
        )
        manager._consume_machine_line(
            session,
            '[{"event":"app.debugPort","params":{"wsUri":"ws://app"}},{"event":"app.dtd","params":{"wsUri":"ws://dtd"}}]',
        )
        self.assertEqual(session.app_uri, "ws://app")
        self.assertEqual(session.dtd_uri, "ws://dtd")
        self.assertEqual(session.state, "ready")
        self.assertTrue(session.ready_event.is_set())

    def test_launch_plan_uses_device_scoped_ezra_lane_for_shared_repo_app(self) -> None:
        with TemporaryDirectory() as temp_dir:
            host_home = Path(temp_dir) / "Users" / "robertsale"
            shared_app = host_home / "Code" / "ezra" / "qa" / "repo" / "clients" / "app"
            master_app = host_home / "Code" / "ezra" / "ezra" / "clients" / "app"
            shared_app.mkdir(parents=True)
            master_app.mkdir(parents=True)
            paths = BridgePaths(
                host_home=host_home,
                virtual_home=Path("/home/robertsale"),
                allowed_roots=(host_home / "Code" / "ezra" / "ezra", host_home / "Code" / "ezra" / "qa" / "repo"),
            )
            manager = FlutterSimManager(paths=paths)
            plan = manager._launch_plan_for_device(
                path=str(shared_app),
                device={
                    "device_id": "F8080594-26F7-4170-826A-452C15769215",
                    "name": "iPad Pro 11-inch (M5)",
                },
            )
            self.assertEqual(plan["requested_path"], str(shared_app))
            self.assertEqual(
                plan["launch_path"],
                str(host_home / "Code" / "ezra" / "qa" / "F8080594-26F7-4170-826A-452C15769215" / "clients" / "app"),
            )
            self.assertEqual(plan["managed_sync_alpha"], str(host_home / "Code" / "ezra" / "ezra"))
            self.assertIsNotNone(plan["managed_sync_name"])

    def test_rewrite_loopback_uri_maps_to_host_internal(self) -> None:
        self.assertEqual(
            rewrite_loopback_uri("ws://127.0.0.1:58563/l5cZ7sZvUDc="),
            "ws://host.internal:58563/l5cZ7sZvUDc=",
        )
        self.assertEqual(
            rewrite_loopback_uri("ws://localhost:58564/path/ws"),
            "ws://host.internal:58564/path/ws",
        )
        self.assertEqual(
            rewrite_loopback_uri("ws://192.168.1.5:58564/path/ws"),
            "ws://192.168.1.5:58564/path/ws",
        )

    def test_expose_loopback_uri_creates_forwarder(self) -> None:
        server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        server.bind(("127.0.0.1", 0))
        server.listen()
        try:
            source_port = server.getsockname()[1]
            forwarders = {}
            exposed = expose_loopback_uri(
                f"ws://127.0.0.1:{source_port}/abc",
                forwarders=forwarders,
            )
            self.assertIsNotNone(exposed)
            self.assertEqual(len(forwarders), 1)
            self.assertIn("host.internal", exposed)
            exposed_port = int(exposed.split(":")[2].split("/")[0])
            probe = socket.create_connection(("127.0.0.1", exposed_port), timeout=1)
            accepted, _ = server.accept()
            probe.close()
            accepted.close()
            for forwarder in forwarders.values():
                forwarder.stop()
        finally:
            server.close()


if __name__ == "__main__":
    unittest.main()
