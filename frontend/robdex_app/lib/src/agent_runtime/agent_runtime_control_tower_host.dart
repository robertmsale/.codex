import 'dart:async';
import 'dart:convert';
import 'dart:developer' as developer;

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
    _registerLiveSmokeExtension();
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

  void _registerLiveSmokeExtension() {
    assert(() {
      developer.registerExtension('ext.robdex.agentRuntimeLiveSmoke', (method, parameters) async {
        final message = parameters['message'] ?? 'GUI composer live smoke';
        final baseUrl = parameters['baseUrl'];
        final result = await _runLiveSmoke(message, baseUrl: baseUrl);
        return developer.ServiceExtensionResponse.result(jsonEncode(result));
      });
      return true;
    }());
  }

  Future<Map<String, Object?>> _runLiveSmoke(String message, {String? baseUrl}) async {
    final trimmed = message.trim();
    if (trimmed.isEmpty) {
      return {'ok': false, 'error': 'empty smoke message rejected'};
    }
    if (baseUrl != null && baseUrl.trim().isNotEmpty) {
      _baseUrlController.text = baseUrl.trim();
    }
    _controller.connect(_baseUrlController.text);
    await _waitFor(() => _hasConnectedRuntime(_controller.data), const Duration(seconds: 12));
    if (!_hasConnectedRuntime(_controller.data)) {
      return {'ok': false, 'error': 'runtime did not connect', 'state': _controller.data.connectionState};
    }
    await _waitFor(() => _controller.data.sessions.isNotEmpty, const Duration(seconds: 12));
    final beforeSessions = _controller.data.sessions.map((session) => session.id).toSet();
    _controller.createLiveSmokeSession();
    await _waitFor(() => _controller.data.sessions.any((session) => !beforeSessions.contains(session.id)), const Duration(seconds: 12));
    await Future<void>.delayed(const Duration(seconds: 1));
    _controller.connect(_baseUrlController.text);
    await _waitFor(() => _hasConnectedRuntime(_controller.data) && _controller.data.sessions.isNotEmpty, const Duration(seconds: 12));
    String? selected;
    for (final session in _controller.data.sessions) {
      if (!beforeSessions.contains(session.id) || session.title == 'GUI composer live smoke') {
        selected = session.id;
        break;
      }
    }
    selected ??= _controller.shellData?.selectedSessionId ?? (_controller.data.sessions.isNotEmpty ? _controller.data.sessions.first.id : null);
    if (selected != null) {
      _controller.selectSession(selected);
      await _waitFor(() => _controller.shellData?.selectedSessionId == selected, const Duration(seconds: 12));
    }
    selected = _controller.shellData?.selectedSessionId ?? selected;
    if (selected == null || selected.isEmpty) {
      return {'ok': false, 'error': 'no session selected'};
    }
    final smokePrompt = 'Use execute_code with exactly this Starlark source: output({"smoke": "ok", "source": "gui-composer"})';
    _controller.sendMessage(selected, smokePrompt);
    await _waitFor(() {
      final entries = _controller.shellData?.entries ?? const <ChatEntry>[];
      return entries.any((entry) => entry.author == 'owner' && _isSmokePromptBubble(entry.body)) &&
          entries.any((entry) => entry.author == 'assistant' && entry.body.trim().isNotEmpty && !entry.isStreaming);
    }, const Duration(seconds: 90));
    final shell = _controller.shellData;
    final entries = shell?.entries ?? const <ChatEntry>[];
    final userBubble = entries.any((entry) => entry.author == 'owner' && _isSmokePromptBubble(entry.body));
    ChatEntry? assistant;
    for (final entry in entries) {
      if (entry.author == 'assistant' && entry.body.trim().isNotEmpty) {
        assistant = entry;
      }
    }
    final rawEventInChat = entries.any((entry) {
      final visible = '${entry.displayLabel} ${entry.subtitle ?? ''} ${entry.body}';
      return visible.contains('role.imported') || visible.contains('turn.started') || visible.contains('model.final_response');
    });
    return {
      'ok': userBubble && assistant != null && !rawEventInChat,
      'selectedSessionId': selected,
      'entryCount': entries.length,
      'userBubble': userBubble,
      'assistantBubble': assistant?.body,
      'rawEventInChat': rawEventInChat,
      'connectionState': _controller.data.connectionState,
      'entries': entries
          .map((entry) => {
                'author': entry.author,
                'label': entry.displayLabel,
                'body': entry.body,
                'status': entry.status,
              })
          .toList(growable: false),
    };
  }

  bool _isSmokePromptBubble(String body) {
    return body.contains('output({"smoke": "ok", "source": "gui-composer"})');
  }

  Future<void> _waitFor(bool Function() condition, Duration timeout) async {
    final deadline = DateTime.now().add(timeout);
    while (DateTime.now().isBefore(deadline)) {
      if (condition()) return;
      await Future<void>.delayed(const Duration(milliseconds: 200));
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
            onSessionClose: _controller.closeSession,
            onSessionArchive: _controller.archiveSession,
            onSessionFork: _controller.forkSession,
            onProcessTerminate: _controller.terminateProcess,
            onProcessInput: (handle) => _controller.inputProcess(handle, '\n'),
            onProcessFlush: _controller.flushProcess,
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
            onSessionClose: _controller.closeSession,
            onSessionArchive: _controller.archiveSession,
            onSessionFork: _controller.forkSession,
            onProcessTerminate: _controller.terminateProcess,
            onProcessInput: (handle) => _controller.inputProcess(handle, '\n'),
            onProcessFlush: _controller.flushProcess,
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
