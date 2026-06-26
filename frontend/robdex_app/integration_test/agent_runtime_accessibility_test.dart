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
    await tester.binding.setSurfaceSize(const Size(1600, 1200));
    addTearDown(() async => tester.binding.setSurfaceSize(null));

    final controller = AgentRuntimeWorkbenchController(requestSink: (_, _) {});
    addTearDown(controller.dispose);
    controller.setViewDataForTest(mockAgentRuntimeConnected, shell: agentRuntimeConversationShellData(mockAgentRuntimeConnected));

    await tester.pumpWidget(MaterialApp(home: Scaffold(body: AgentRuntimeWorkbenchHost(controller: controller))));
    await tester.pumpAndSettle();
    await _pumpUntil(
      tester,
      condition: () => find.byKey(const ValueKey('agentRuntime.toolbar.runtimeOperations')).evaluate().isNotEmpty,
      reason: 'runtime operations toolbar did not render',
    );

    expect(find.bySemanticsLabel('Runtime operations'), findsWidgets);
    expect(find.bySemanticsLabel('Session settings'), findsWidgets);

    await tester.tap(find.byKey(const ValueKey('agentRuntime.toolbar.runtimeOperations')));
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
    await tester.pumpAndSettle();
    await _pumpUntil(
      tester,
      condition: () => find.byKey(const ValueKey('agentRuntime.operationsDetail.close')).evaluate().isEmpty,
      reason: 'runtime operations detail did not close',
    );

    expect(find.text('Runtime detail'), findsNothing);
    expect(find.bySemanticsLabel('Runtime operations'), findsWidgets);

    await tester.tap(find.byKey(const ValueKey('agentRuntime.toolbar.sessionSettings')));
    await tester.pumpAndSettle();
    expect(find.byType(AgentRuntimeSessionControlPlane), findsOneWidget);
    expect(find.text('Session Settings'), findsOneWidget);
    expect(find.text('Processes (2)'), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('agentRuntime.sessionControl.close')));
    await tester.pumpAndSettle();
    expect(find.byType(AgentRuntimeSessionControlPlane), findsNothing);

    await tester.tap(find.byTooltip('New session'));
    await tester.pump();
    expect(find.byType(AgentRuntimeCreateSessionDialog), findsOneWidget);

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

    expect(find.byWidgetPredicate((widget) => widget is Semantics && widget.properties.label == 'Agent Runtime workbench'), findsOneWidget);
    expect(find.byWidgetPredicate((widget) => widget is Semantics && widget.properties.label == 'Runtime URL'), findsOneWidget);
    expect(find.text('Connect to URL'), findsOneWidget);
  });

  testWidgets('Agent Runtime session settings opens with duplicate runtime model options', (tester) async {
    SharedPreferences.setMockInitialValues({});
    await tester.binding.setSurfaceSize(const Size(1600, 1200));
    addTearDown(() async => tester.binding.setSurfaceSize(null));

    final duplicateModelData = mockAgentRuntimeConnected.copyWith(
      selectedSessionControlPlane: mockAgentRuntimeConnected.selectedSessionControlPlane!.copyWith(
        activeModel: 'codex-live-model',
        modelOptions: const [
          AgentRuntimeModelOption(id: 'codex-live-model', displayLabel: 'Codex live model', source: 'runtime', isDefault: true),
          AgentRuntimeModelOption(id: 'codex-live-model', displayLabel: 'Codex live model duplicate', source: 'runtime', isDefault: false),
          AgentRuntimeModelOption(id: 'gpt-5.4-mini', displayLabel: 'GPT-5.4 Mini', source: 'runtime', isDefault: false),
        ],
      ),
    );
    final controller = AgentRuntimeWorkbenchController(requestSink: (_, _) {});
    addTearDown(controller.dispose);
    controller.setViewDataForTest(duplicateModelData, shell: agentRuntimeConversationShellData(duplicateModelData));

    await tester.pumpWidget(MaterialApp(home: Scaffold(body: AgentRuntimeWorkbenchHost(controller: controller))));
    await tester.pump();
    await _pumpUntil(
      tester,
      condition: () => find.byKey(const ValueKey('agentRuntime.toolbar.sessionSettings')).evaluate().isNotEmpty,
      reason: 'session settings toolbar did not render',
    );

    await tester.tap(find.byKey(const ValueKey('agentRuntime.toolbar.sessionSettings')));
    await tester.pump();
    await _pumpUntil(
      tester,
      condition: () => find.byType(AgentRuntimeSessionControlPlane).evaluate().isNotEmpty,
      reason: 'session settings surface did not open',
    );

    expect(find.text('Session Settings'), findsOneWidget);
    expect(find.byType(DropdownButtonFormField<String>), findsNWidgets(3));
    expect(tester.takeException(), isNull);
  });
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
