from pathlib import Path
import unittest

from codex_services_http.ezra_host_qa import EzraHostQaPlan


class EzraHostQaPlanTests(unittest.TestCase):
    def test_lane_for_device_uses_shared_synced_repo_and_stable_api_project(self) -> None:
        plan = EzraHostQaPlan(
            master_repo_root=Path("/Users/robertsale/Code/ezra/ezra"),
            qa_root=Path("/Users/robertsale/Code/ezra/qa"),
            flutter_target="lib/flutter_driver_pilot_main.dart",
            api_launcher_script=Path("/Users/robertsale/Code/ezra/ezra/scripts/ai_integration.sh"),
        )

        lane = plan.lane_for_device(
            device_id="717308AA-BBDF-422E-88E2-422A353FE864",
            device_name="iPad Pro 13-inch (M5) (mobile)",
        )

        self.assertEqual(lane.lane_id, "ipad-pro-13-inch-m5-mobile-717308aa-bbdf-422e-88e2-422a353fe864")
        self.assertEqual(lane.repo_root, Path("/Users/robertsale/Code/ezra/qa/repo"))
        self.assertEqual(lane.app_path, Path("/Users/robertsale/Code/ezra/qa/repo/clients/app"))
        self.assertEqual(lane.api_project_name, "ezra-qa-ipad-pro-13-inch-m5-mobile-717308aa-bbdf-422e-88e2-422a353fe864")

    def test_lanes_for_devices_skips_incomplete_rows(self) -> None:
        plan = EzraHostQaPlan(
            master_repo_root=Path("/Users/robertsale/Code/ezra/ezra"),
            qa_root=Path("/Users/robertsale/Code/ezra/qa"),
            flutter_target="lib/flutter_driver_pilot_main.dart",
            api_launcher_script=Path("/Users/robertsale/Code/ezra/ezra/scripts/ai_integration.sh"),
        )

        lanes = plan.lanes_for_devices(
            [
                {"device_id": "dev-1", "name": "iPad A"},
                {"device_id": "", "name": "missing"},
                {"device_id": "dev-2", "name": ""},
            ]
        )

        self.assertEqual(len(lanes), 1)
        self.assertEqual(lanes[0].device_id, "dev-1")


if __name__ == "__main__":
    unittest.main()
