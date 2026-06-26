import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:robdex_app/src/agent_runtime/agent_runtime_workbench_controller.dart';
import 'package:robdex_app/src/agent_runtime/agent_runtime_workbench_host.dart';
import 'package:robdex_design_system/robdex_design_system.dart';
import 'package:shared_preferences/shared_preferences.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('Agent Runtime modal dismissal and semantics survive shell rebuilds', (tester) async {
    SharedPreferences.setMockInitialValues({});
    await tester.binding.setSurfaceSize(const Size(1200, 1000));
    addTearDown(() async => tester.binding.setSurfaceSize(null));

    final controller = AgentRuntimeWorkbenchController(requestSink: (_, _) {});
    addTearDown(controller.dispose);
    controller.setViewDataForTest(mockAgentRuntimeConnected, shell: agentRuntimeConversationShellData(mockAgentRuntimeConnected));

    await tester.pumpWidget(MaterialApp(home: AgentRuntimeWorkbenchHost(controller: controller)));
    await tester.pump();
    await _pumpUntil(
      tester,
      condition: () => find.byKey(const ValueKey('agentRuntime.toolbar.runtimeOperations')).evaluate().isNotEmpty,
      reason: 'runtime operations toolbar did not render',
    );

    expect(find.bySemanticsLabel('Runtime operations'), findsWidgets);
    expect(find.bySemanticsLabel('Session settings'), findsWidgets);

    await tester.pumpWidget(const MaterialApp(home: _RuntimeOperationsHarness()));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('agentRuntime.integration.openOperations')));
    await tester.pump();
    await _pumpUntil(
      tester,
      condition: () => find.byKey(const ValueKey('agentRuntime.operationsDetail.close')).evaluate().isNotEmpty,
      reason: 'runtime operations detail did not open',
    );
    await tester.pump(const Duration(milliseconds: 500));

    expect(find.text('Runtime detail'), findsOneWidget);
    expect(find.text('Process Manager'), findsWidgets);
    expect(find.bySemanticsLabel('Close runtime operations detail'), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('agentRuntime.operationsDetail.close')));
    await tester.pump();
    await _pumpUntil(
      tester,
      condition: () => find.byKey(const ValueKey('agentRuntime.operationsDetail.close')).evaluate().isEmpty,
      reason: 'runtime operations detail did not close',
    );

    expect(find.text('Runtime detail'), findsNothing);
    expect(find.bySemanticsLabel('Runtime operations'), findsWidgets);

    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: AgentRuntimeWorkbench(
          data: mockAgentRuntimeEmpty.copyWith(
            connectionState: 'disconnected',
            connectionTone: 'muted',
            statusLabel: 'Not connected',
            discovery: const AgentRuntimeDiscoveryInfo(
              state: 'notLoaded',
              tone: 'muted',
              title: 'Discovery not loaded',
              message: 'Refresh discovery to check the local Agent Runtime service.',
              discoveryPath: '',
              connectable: false,
            ),
          ),
          baseUrlController: TextEditingController(text: 'http://127.0.0.1:42080'),
          onConnect: () {},
          onRefreshDiscovery: () {},
          onConnectDiscovered: () {},
          onRefreshIcloudRemoteDiscovery: () {},
          onConnectIcloudRemote: () {},
          onImportRemoteProfile: () {},
          onRefreshImportedRemoteProfile: () {},
          onConnectImportedRemoteProfile: () {},
          onDisconnect: () {},
        ),
      ),
    ));
    await tester.pump();

    expect(find.bySemanticsLabel('Agent Runtime workbench'), findsOneWidget);
    expect(find.bySemanticsLabel('Runtime URL'), findsWidgets);
    expect(find.bySemanticsLabel('Connect to URL'), findsOneWidget);
  });
}

class _RuntimeOperationsHarness extends StatelessWidget {
  const _RuntimeOperationsHarness();

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: const Color(0xFF05090F),
      body: Center(
        child: FilledButton(
          key: const ValueKey('agentRuntime.integration.openOperations'),
          onPressed: () {
            showModalBottomSheet<void>(
              context: context,
              isScrollControlled: true,
              isDismissible: true,
              enableDrag: true,
              useSafeArea: true,
              showDragHandle: true,
              backgroundColor: const Color(0xFF111820),
              builder: (sheetContext) => FractionallySizedBox(
                heightFactor: 0.86,
                child: AgentRuntimeOperationsDetail(
                  data: mockAgentRuntimeConnected,
                  focusSurfaceId: 'processManager',
                  onClose: () => Navigator.of(sheetContext).pop(),
                ),
              ),
            );
          },
          child: const Text('Runtime operations'),
        ),
      ),
    );
  }
}

Future<void> _pumpUntil(
  WidgetTester tester, {
  required bool Function() condition,
  required String reason,
  Duration timeout = const Duration(seconds: 3),
}) async {
  final end = DateTime.now().add(timeout);
  while (DateTime.now().isBefore(end)) {
    if (condition()) {
      return;
    }
    await tester.pump(const Duration(milliseconds: 50));
  }
  fail(reason);
}
