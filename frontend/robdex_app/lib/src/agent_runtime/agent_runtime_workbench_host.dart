import 'dart:async';

import 'package:flutter/material.dart';
import 'package:robdex_design_system/robdex_design_system.dart';

import 'agent_runtime_workbench_controller.dart';

class AgentRuntimeWorkbenchHost extends StatefulWidget {
  const AgentRuntimeWorkbenchHost({super.key});

  @override
  State<AgentRuntimeWorkbenchHost> createState() => _AgentRuntimeWorkbenchHostState();
}

class _AgentRuntimeWorkbenchHostState extends State<AgentRuntimeWorkbenchHost> {
  late final AgentRuntimeWorkbenchController _controller;
  late final TextEditingController _baseUrlController;

  @override
  void initState() {
    super.initState();
    _controller = AgentRuntimeWorkbenchController();
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
          return AgentRuntimeWorkbench(
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
          onCreateSession: () => _showCreateSessionModal(context, shell, data),
          onSendMessage: (submission) => _controller.sendMessage(shell.selectedSessionId ?? '', submission.text),
          onInterrupt: () {},
          onCloseSession: _controller.closeSession,
          onArchiveSession: _controller.archiveSession,
          onForkSession: _controller.forkSession,
          onProjectSelected: _controller.selectProject,
          onSettings: _controller.openSettings,
          showPermanentDetail: false,
          headerControls: _AgentRuntimeToolbar(
            surfaces: data.operationSurfaces,
            onOpenSurface: (surfaceId) => _showOperationsSurface(context, data, surfaceId, shell.selectedSessionId ?? ''),
          ),
        );
      },
    );
  }

  AgentRuntimeOperationsDetail _operationsDetail(AgentRuntimeWorkbenchData data, String? focusSurfaceId, String selectedSessionId) {
    return AgentRuntimeOperationsDetail(
      data: data,
      focusSurfaceId: focusSurfaceId,
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
      onCommandRegistryApprove: (action) => _controller.approveCommandRegistryRequest(action, selectedSessionId),
      onCommandRegistryApply: (action) => _controller.applyCommandRegistryRequest(action, selectedSessionId),
    );
  }

  Future<void> _showOperationsSurface(BuildContext context, AgentRuntimeWorkbenchData data, String surfaceId, String selectedSessionId) {
    return showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      backgroundColor: const Color(0xFF111820),
      builder: (context) => FractionallySizedBox(
        heightFactor: 0.86,
        child: _operationsDetail(data, surfaceId, selectedSessionId),
      ),
    );
  }

  Future<void> _showCreateSessionModal(BuildContext context, ConversationShellData shell, AgentRuntimeWorkbenchData data) {
    return showDialog<void>(
      context: context,
      builder: (context) => _CreateAgentRuntimeSessionDialog(
        shell: shell,
        data: data,
        onCreate: _controller.createSessionFromDraft,
      ),
    );
  }
}

bool _hasConnectedRuntime(AgentRuntimeWorkbenchData data) {
  return data.connectionState != 'disconnected' && data.connectionState != 'connecting' && data.connectionState != 'failed';
}

class _AgentRuntimeToolbar extends StatelessWidget {
  const _AgentRuntimeToolbar({required this.surfaces, required this.onOpenSurface});

  final List<AgentRuntimeOperationSurface> surfaces;
  final ValueChanged<String> onOpenSurface;

  @override
  Widget build(BuildContext context) {
    const primary = ['session', 'history', 'diagnostics', 'statistics', 'settings'];
    return Wrap(
      spacing: 6,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        for (final surface in surfaces.where((surface) => primary.contains(surface.surfaceId)))
          TextButton(
            key: ValueKey('agentRuntime.toolbar.${surface.surfaceId}'),
            onPressed: () => onOpenSurface(surface.surfaceId),
            child: Text(surface.title),
          ),
        PopupMenuButton<String>(
          key: const ValueKey('agentRuntime.toolbar.more'),
          tooltip: 'More runtime surfaces',
          onSelected: onOpenSurface,
          itemBuilder: (context) => [
            for (final surface in surfaces.where((surface) => !primary.contains(surface.surfaceId)))
              PopupMenuItem(value: surface.surfaceId, child: Text(surface.title)),
          ],
          child: const Padding(
            padding: EdgeInsets.symmetric(horizontal: 8, vertical: 6),
            child: Text('More'),
          ),
        ),
      ],
    );
  }
}

class _CreateAgentRuntimeSessionDialog extends StatefulWidget {
  const _CreateAgentRuntimeSessionDialog({required this.shell, required this.data, required this.onCreate});

  final ConversationShellData shell;
  final AgentRuntimeWorkbenchData data;
  final void Function({
    required String role,
    required String project,
    required String model,
    required String workdir,
    required String worktreeRoot,
    required String title,
    required String name,
  }) onCreate;

  @override
  State<_CreateAgentRuntimeSessionDialog> createState() => _CreateAgentRuntimeSessionDialogState();
}

class _CreateAgentRuntimeSessionDialogState extends State<_CreateAgentRuntimeSessionDialog> {
  late String _project;
  late String _role;
  late final TextEditingController _model;
  late final TextEditingController _title;
  late final TextEditingController _name;
  late final TextEditingController _workdir;
  late final TextEditingController _worktreeRoot;
  String? _error;

  @override
  void initState() {
    super.initState();
    _project = widget.shell.projects.isNotEmpty ? widget.shell.projects.first.id : '';
    _role = widget.data.roleAdmin.rows.isNotEmpty ? widget.data.roleAdmin.rows.first.id : '';
    _model = TextEditingController(text: widget.data.roleAdmin.selectedDetail?.model ?? '');
    _title = TextEditingController(text: 'New session');
    _name = TextEditingController();
    _workdir = TextEditingController();
    _worktreeRoot = TextEditingController();
  }

  @override
  void dispose() {
    _model.dispose();
    _title.dispose();
    _name.dispose();
    _workdir.dispose();
    _worktreeRoot.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final projects = widget.shell.projects.where((project) => project.id.isNotEmpty).toList(growable: false);
    final roles = widget.data.roleAdmin.rows.where((role) => role.id.isNotEmpty).toList(growable: false);
    return AlertDialog(
      title: const Text('Create session'),
      content: SizedBox(
        width: 520,
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              if (_error != null) ...[
                Text(_error!, style: const TextStyle(color: Color(0xFFFF9BA7))),
                const SizedBox(height: 10),
              ],
              DropdownButtonFormField<String>(
                initialValue: projects.any((project) => project.id == _project) ? _project : null,
                items: [for (final project in projects) DropdownMenuItem(value: project.id, child: Text(project.title))],
                onChanged: (value) => setState(() => _project = value ?? ''),
                decoration: const InputDecoration(labelText: 'Project'),
              ),
              DropdownButtonFormField<String>(
                initialValue: roles.any((role) => role.id == _role) ? _role : null,
                items: [for (final role in roles) DropdownMenuItem(value: role.id, child: Text(role.title))],
                onChanged: (value) => setState(() => _role = value ?? ''),
                decoration: const InputDecoration(labelText: 'Role'),
              ),
              TextField(controller: _model, decoration: const InputDecoration(labelText: 'Model')),
              TextField(controller: _title, decoration: const InputDecoration(labelText: 'Title')),
              TextField(controller: _name, decoration: const InputDecoration(labelText: 'Name')),
              TextField(controller: _workdir, decoration: const InputDecoration(labelText: 'Workdir')),
              TextField(controller: _worktreeRoot, decoration: const InputDecoration(labelText: 'Worktree root')),
            ],
          ),
        ),
      ),
      actions: [
        TextButton(onPressed: () => Navigator.of(context).pop(), child: const Text('Cancel')),
        FilledButton(onPressed: _submit, child: const Text('Create')),
      ],
    );
  }

  void _submit() {
    final missing = <String>[
      if (_project.trim().isEmpty) 'project',
      if (_role.trim().isEmpty) 'role',
      if (_model.text.trim().isEmpty) 'model',
      if (_title.text.trim().isEmpty) 'title',
      if (_name.text.trim().isEmpty) 'name',
      if (_workdir.text.trim().isEmpty) 'workdir',
      if (_worktreeRoot.text.trim().isEmpty) 'worktree root',
    ];
    if (missing.isNotEmpty) {
      setState(() {
        _error = 'Required: ${missing.join(', ')}';
      });
      return;
    }
    widget.onCreate(
      role: _role,
      project: _project,
      model: _model.text,
      workdir: _workdir.text,
      worktreeRoot: _worktreeRoot.text,
      title: _title.text,
      name: _name.text,
    );
    Navigator.of(context).pop();
  }
}
