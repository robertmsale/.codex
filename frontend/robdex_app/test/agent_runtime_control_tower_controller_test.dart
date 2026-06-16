import 'package:flutter_test/flutter_test.dart';
import 'package:robdex_app/src/agent_runtime/agent_runtime_control_tower_controller.dart';

void main() {
  test('role activate operation maps role and version ids for Rust transport', () {
    final operation = agentRuntimeRoleActivateOperationForTest('runtime-allow', 'role-version-0');

    expect(operation['operation'], 'activateRoleVersion');
    expect(operation['request'], {
      'roleId': 'runtime-allow',
      'versionId': 'role-version-0',
    });
  });
}
