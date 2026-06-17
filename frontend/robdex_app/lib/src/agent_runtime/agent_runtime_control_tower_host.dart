import 'package:flutter/material.dart';
import 'package:robdex_design_system/robdex_design_system.dart';

import 'agent_runtime_control_tower_controller.dart';

class AgentRuntimeControlTowerHost extends StatefulWidget {
  const AgentRuntimeControlTowerHost({super.key});

  @override
  State<AgentRuntimeControlTowerHost> createState() => _AgentRuntimeControlTowerHostState();
}

class _AgentRuntimeControlTowerHostState extends State<AgentRuntimeControlTowerHost> {
  late final AgentRuntimeControlTowerController _controller;
  late final TextEditingController _baseUrlController;

  @override
  void initState() {
    super.initState();
    _controller = AgentRuntimeControlTowerController();
    _baseUrlController = TextEditingController(text: 'http://127.0.0.1:8765');
    _controller.addListener(_syncBaseUrl);
  }

  @override
  void dispose() {
    _controller.removeListener(_syncBaseUrl);
    _controller.dispose();
    _baseUrlController.dispose();
    super.dispose();
  }

  void _syncBaseUrl() {
    final next = _controller.data.baseUrl;
    if (_baseUrlController.text != next) {
      _baseUrlController.text = next;
    }
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: _controller,
      builder: (context, _) {
        final data = _controller.data;
        if (!_hasConnectedRuntime(data)) {
          return AgentRuntimeControlTower(
            data: data,
            baseUrlController: _baseUrlController,
            onConnect: () => _controller.connect(_baseUrlController.text),
            onRefreshDiscovery: _controller.refreshDiscovery,
            onConnectDiscovered: _controller.connectDiscoveredRuntime,
            onRefreshIcloudRemoteDiscovery: _controller.refreshIcloudRemoteDiscovery,
            onConnectIcloudRemote: _controller.connectIcloudRemoteRuntime,
            onImportRemoteProfile: _controller.importRemoteProfileDocument,
            onRefreshImportedRemoteProfile: _controller.refreshImportedRemoteProfile,
            onConnectImportedRemoteProfile: _controller.connectImportedRemoteRuntime,
            onPollStream: _controller.pollStreamOnce,
            onDisconnect: _controller.disconnect,
            onRoleValidate: _controller.validateRoleDraft,
            onRoleCreate: _controller.createRoleFromDraft,
            onRoleUpdate: _controller.updateRoleFromDraft,
            onRoleExport: _controller.exportRole,
            onRoleArchive: _controller.archiveRole,
            onRoleUnarchive: _controller.unarchiveRole,
            onRoleActivate: _controller.activateRoleVersion,
            onWorkflowMemorySelect: _controller.selectWorkflowMemory,
            onWorkflowMemoryAttempted: _controller.markWorkflowMemoryAttempted,
            onWorkflowMemoryHelpful: _controller.markWorkflowMemoryHelpful,
            onWorkflowMemoryNotHelpful: _controller.markWorkflowMemoryNotHelpful,
          );
        }
        final shell = _controller.shellData;
        if (shell == null) {
          return const ColoredBox(
            color: Color(0xFF05090F),
            child: Center(child: Text('Loading runtime shell…')),
          );
        }
        return ConversationShellScreen(
          data: shell,
          onSessionSelected: _controller.selectSession,
          onCreateSession: _controller.createSession,
          onSendMessage: (submission) => _controller.sendMessage(shell.selectedSessionId ?? '', submission.text),
          onInterrupt: () {},
          onCloseSession: _controller.closeSession,
          onArchiveSession: _controller.archiveSession,
          onForkSession: _controller.forkSession,
          onProjectSelected: _controller.selectProject,
          onSettings: _controller.openSettings,
          detailContent: AgentRuntimeOperationsDetail(
            data: data,
            onRoleValidate: _controller.validateRoleDraft,
            onRoleCreate: _controller.createRoleFromDraft,
            onRoleUpdate: _controller.updateRoleFromDraft,
            onRoleExport: _controller.exportRole,
            onRoleArchive: _controller.archiveRole,
            onRoleUnarchive: _controller.unarchiveRole,
            onRoleActivate: _controller.activateRoleVersion,
            onWorkflowMemorySelect: _controller.selectWorkflowMemory,
            onWorkflowMemoryAttempted: _controller.markWorkflowMemoryAttempted,
            onWorkflowMemoryHelpful: _controller.markWorkflowMemoryHelpful,
            onWorkflowMemoryNotHelpful: _controller.markWorkflowMemoryNotHelpful,
            onApprovalApprove: _controller.approveAction,
            onApprovalResume: _controller.resumeApproval,
            onCommandRegistryApprove: (action) => _controller.approveCommandRegistryRequest(action, shell.selectedSessionId ?? ''),
            onCommandRegistryApply: (action) => _controller.applyCommandRegistryRequest(action, shell.selectedSessionId ?? ''),
          ),
        );
      },
    );
  }
}

bool _hasConnectedRuntime(AgentRuntimeControlTowerData data) {
  return data.connectionState != 'disconnected' && data.connectionState != 'connecting' && data.connectionState != 'failed';
}
