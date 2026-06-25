import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:robdex_app/src/agent_runtime/agent_runtime_workbench_controller.dart';
import 'package:robdex_app/src/agent_runtime/agent_runtime_workbench_host.dart';
import 'package:robdex_design_lab/main.dart';
import 'package:robdex_design_system/robdex_design_system.dart';

void main() {
  testWidgets('Design Lab fixture and production host use the same session control-plane widget', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1200, 1000));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(MaterialApp(home: buildAgentRuntimeSessionSettingsSurface()));
    await tester.pumpAndSettle();
    expect(find.byType(AgentRuntimeSessionControlPlane), findsOneWidget);
    expect(find.text('Session Settings'), findsOneWidget);
    expect(find.text('Started 09:14'), findsOneWidget);
    expect(find.textContaining('Requested 4m ago'), findsOneWidget);

    final controller = AgentRuntimeWorkbenchController(requestSink: (_, _) {});
    addTearDown(controller.dispose);
    controller.setViewDataForTest(mockAgentRuntimeConnected, shell: agentRuntimeConversationShellData(mockAgentRuntimeConnected));

    await tester.pumpWidget(MaterialApp(home: AgentRuntimeWorkbenchHost(controller: controller)));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('agentRuntime.toolbar.sessionSettings')));
    await tester.pumpAndSettle();

    expect(find.byType(AgentRuntimeSessionControlPlane), findsOneWidget);
    expect(find.byType(AlertDialog), findsNothing);
    expect(find.text('Session Settings'), findsOneWidget);
  });
}
