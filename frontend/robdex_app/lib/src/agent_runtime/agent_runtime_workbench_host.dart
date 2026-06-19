import 'dart:async';

import 'package:flutter/material.dart';
import 'package:robdex_design_system/robdex_design_system.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'agent_runtime_workbench_controller.dart';

class AgentRuntimeWorkbenchHost extends StatefulWidget {
  const AgentRuntimeWorkbenchHost({super.key});

  @override
  State<AgentRuntimeWorkbenchHost> createState() => _AgentRuntimeWorkbenchHostState();
}

class _AgentRuntimeWorkbenchHostState extends State<AgentRuntimeWorkbenchHost> {
  static const String _leftRailWidthPreferenceKey = 'agentRuntime.leftRailWidth';
  late final AgentRuntimeWorkbenchController _controller;
  late final TextEditingController _baseUrlController;
  double _leftRailWidth = 288;

  @override
  void initState() {
    super.initState();
    _controller = AgentRuntimeWorkbenchController();
    _baseUrlController = TextEditingController(text: 'http://127.0.0.1:8765');
    _controller.addListener(_syncBaseUrl);
    unawaited(_restoreLeftRailWidth());
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
          onCreateProject: () => _showCreateProjectModal(context, shell, data),
          onEditProject: (projectId) => _showProjectSettingsModal(context, projectId),
          onNewSessionInProject: (projectId) => _showCreateSessionModal(context, shell, data, initialProjectId: projectId),
          onArchiveProject: _controller.archiveProject,
          onSettings: () => _showGlobalSettingsModal(context, data),
          showPermanentDetail: false,
          leftRailWidth: _leftRailWidth,
          onLeftRailWidthChanged: _updateLeftRailWidth,
          headerControls: _AgentRuntimeToolbar(
            surfaces: data.operationSurfaces,
            onDisconnect: _controller.disconnect,
            onOpenSessionSettings: () => _showSessionSettingsModal(context, shell, data),
            onOpenSurface: (surfaceId) => _showOperationsSurface(context, data, surfaceId, shell.selectedSessionId ?? ''),
          ),
        );
      },
    );
  }

  Future<void> _restoreLeftRailWidth() async {
    final preferences = await SharedPreferences.getInstance();
    final restored = preferences.getDouble(_leftRailWidthPreferenceKey);
    if (restored == null || !mounted) {
      return;
    }
    setState(() {
      _leftRailWidth = restored.clamp(ConversationShellScreen.minLeftRailWidth, ConversationShellScreen.maxLeftRailWidth).toDouble();
    });
  }

  void _updateLeftRailWidth(double width) {
    final next = width.clamp(ConversationShellScreen.minLeftRailWidth, ConversationShellScreen.maxLeftRailWidth).toDouble();
    if ((_leftRailWidth - next).abs() < 0.5) {
      return;
    }
    setState(() {
      _leftRailWidth = next;
    });
    unawaited(_persistLeftRailWidth(next));
  }

  Future<void> _persistLeftRailWidth(double width) async {
    final preferences = await SharedPreferences.getInstance();
    await preferences.setDouble(_leftRailWidthPreferenceKey, width);
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
      onApprovalDeny: _controller.denyAction,
      onApprovalResume: _controller.resumeApproval,
      onCommandRegistryApprove: (action) => _controller.approveCommandRegistryRequest(action, selectedSessionId),
      onCommandRegistryDeny: (action) => _controller.denyCommandRegistryRequest(action, selectedSessionId),
      onCommandRegistryPreview: (action) => _controller.previewCommandRegistryRequest(action, selectedSessionId),
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

  Future<void> _showCreateSessionModal(BuildContext context, ConversationShellData shell, AgentRuntimeWorkbenchData data, {String? initialProjectId}) {
    return showDialog<void>(
      context: context,
      builder: (context) => AgentRuntimeCreateSessionDialog(
        shell: shell,
        data: data,
        initialProjectId: initialProjectId,
        onCreate: _controller.createSessionFromDraft,
      ),
    );
  }

  Future<void> _showCreateProjectModal(BuildContext context, ConversationShellData shell, AgentRuntimeWorkbenchData data) {
    return showDialog<void>(
      context: context,
      builder: (context) => AgentRuntimeCreateProjectDialog(
        data: data,
        existingProjectKeys: shell.projects
            .where((project) => project.id != '__all__' && project.id != '__unassigned__')
            .map((project) => project.id)
            .toList(growable: false),
        onCreate: _controller.createProject,
      ),
    );
  }

  Future<void> _showGlobalSettingsModal(BuildContext context, AgentRuntimeWorkbenchData data) {
    return showDialog<void>(
      context: context,
      builder: (context) => AgentRuntimeGlobalSettingsDialog(
        data: data,
        onConnectManual: (baseUrl) => _controller.connect(baseUrl),
        onRefreshDiscovery: _controller.refreshDiscovery,
        onConnectDiscovery: _controller.connectDiscoveredRuntime,
        onRefreshIcloud: _controller.refreshIcloudRemoteDiscovery,
        onConnectIcloud: _controller.connectIcloudRemoteRuntime,
        onImportProfile: _controller.importRemoteProfileDocument,
        onRefreshImportedProfile: _controller.refreshImportedRemoteProfile,
        onConnectImportedProfile: _controller.connectImportedRemoteRuntime,
        onDisconnect: _controller.disconnect,
      ),
    );
  }

  Future<void> _showProjectSettingsModal(BuildContext context, String projectId) {
    final data = _controller.data;
    return showDialog<void>(
      context: context,
      builder: (context) => AgentRuntimeProjectSettingsDialog(
        data: data,
        projectId: projectId,
        onSave: _controller.updateProject,
        onArchive: _controller.archiveProject,
        onUnarchive: _controller.unarchiveProject,
      ),
    );
  }

  Future<void> _showSessionSettingsModal(BuildContext context, ConversationShellData shell, AgentRuntimeWorkbenchData data) {
    return showDialog<void>(
      context: context,
      builder: (context) => AgentRuntimeSessionSettingsDialog(
        shell: shell,
        data: data,
        onSave: _controller.updateSessionSettings,
        onClose: _controller.closeSession,
        onArchive: _controller.archiveSession,
        onFork: _controller.forkSession,
      ),
    );
  }
}

bool _hasConnectedRuntime(AgentRuntimeWorkbenchData data) {
  return data.connectionState != 'disconnected' && data.connectionState != 'connecting' && data.connectionState != 'failed';
}

class _AgentRuntimeToolbar extends StatelessWidget {
  const _AgentRuntimeToolbar({required this.surfaces, required this.onOpenSurface, required this.onOpenSessionSettings, required this.onDisconnect});

  final List<AgentRuntimeOperationSurface> surfaces;
  final ValueChanged<String> onOpenSurface;
  final VoidCallback onOpenSessionSettings;
  final VoidCallback onDisconnect;

  @override
  Widget build(BuildContext context) {
    final hasSessionSurface = surfaces.any((surface) => surface.surfaceId == 'session');
    return Wrap(
      spacing: 6,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        IconButton(
          key: const ValueKey('agentRuntime.toolbar.runtimeOperations'),
          tooltip: 'Runtime operations',
          onPressed: () => onOpenSurface('history'),
          icon: const Icon(Icons.manage_history_rounded, size: 18),
        ),
        IconButton(
          key: const ValueKey('agentRuntime.toolbar.sessionSettings'),
          tooltip: 'Session settings',
          onPressed: hasSessionSurface ? onOpenSessionSettings : null,
          icon: const Icon(Icons.tune_rounded, size: 18),
        ),
        IconButton(
          key: const ValueKey('agentRuntime.toolbar.disconnect'),
          tooltip: 'Disconnect',
          onPressed: onDisconnect,
          icon: const Icon(Icons.link_off_rounded, size: 18),
        ),
        PopupMenuButton<String>(
          key: const ValueKey('agentRuntime.toolbar.sections'),
          tooltip: 'Runtime operation sections',
          onSelected: onOpenSurface,
          itemBuilder: (context) => [for (final surface in surfaces) PopupMenuItem(value: surface.surfaceId, child: Text(surface.title))],
          icon: const Icon(Icons.more_horiz_rounded, size: 18),
        ),
      ],
    );
  }
}

class AgentRuntimeCreateSessionDialog extends StatefulWidget {
  const AgentRuntimeCreateSessionDialog({super.key, required this.shell, required this.data, required this.onCreate, this.initialProjectId});

  final ConversationShellData shell;
  final AgentRuntimeWorkbenchData data;
  final String? initialProjectId;
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
  State<AgentRuntimeCreateSessionDialog> createState() => _CreateAgentRuntimeSessionDialogState();
}

class _CreateAgentRuntimeSessionDialogState extends State<AgentRuntimeCreateSessionDialog> {
  late String _project;
  late String _role;
  late String _model;
  late final TextEditingController _title;
  late final TextEditingController _name;
  late final TextEditingController _workdir;
  late final TextEditingController _worktreeRoot;
  String? _error;

  @override
  void initState() {
    super.initState();
    final projectChoices = widget.shell.projects.where((project) => project.id != '__all__').toList(growable: false);
    _project = widget.initialProjectId != null && projectChoices.any((project) => project.id == widget.initialProjectId)
        ? widget.initialProjectId!
        : projectChoices.isNotEmpty
            ? projectChoices.first.id
            : '__unassigned__';
    _role = widget.data.roleAdmin.rows.isNotEmpty ? widget.data.roleAdmin.rows.first.id : '';
    final models = _modelOptions();
    _model = models.isNotEmpty ? models.first : '';
    _title = TextEditingController(text: 'New session');
    _name = TextEditingController(text: _sessionNameFromTitle('New session'));
    _title.addListener(_syncGeneratedName);
    _workdir = TextEditingController(text: '.');
    _worktreeRoot = TextEditingController(text: '.');
  }

  @override
  void dispose() {
    _title.removeListener(_syncGeneratedName);
    _title.dispose();
    _name.dispose();
    _workdir.dispose();
    _worktreeRoot.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final projects = widget.shell.projects.where((project) => project.id.isNotEmpty && project.id != '__all__').toList(growable: false);
    final roles = widget.data.roleAdmin.rows.where((role) => role.id.isNotEmpty).toList(growable: false);
    final models = _modelOptions();
    return AlertDialog(
      title: const Text('Create session'),
      contentPadding: const EdgeInsets.fromLTRB(24, 12, 24, 10),
      content: ConstrainedBox(
        constraints: BoxConstraints(maxWidth: 680, maxHeight: MediaQuery.of(context).size.height * 0.78),
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(
                'Choose the project, runtime role, model, and workspace before the session is created.',
                style: Theme.of(context).textTheme.bodyMedium?.copyWith(color: Theme.of(context).colorScheme.onSurfaceVariant),
              ),
              if (_error != null) ...[
                const SizedBox(height: 14),
                _RuntimeFormNotice(message: _error!, tone: _RuntimeFormNoticeTone.error),
              ],
              const SizedBox(height: 18),
              _RuntimeFormSection(
                title: 'Session scope',
                description: 'These values are persisted with the session and sent to the runtime.',
                children: [
                  _RuntimeFormGrid(children: [
                    _RuntimeLabeledField(
                      label: 'Project',
                      helper: 'Use Unassigned only when the session is intentionally projectless.',
                      child: DropdownButtonFormField<String>(
                        key: const ValueKey('agentRuntime.createSession.project'),
                        initialValue: projects.any((project) => project.id == _project) ? _project : null,
                        items: [for (final project in projects) DropdownMenuItem(value: project.id, child: Text(project.title, overflow: TextOverflow.ellipsis))],
                        onChanged: (value) => setState(() => _project = value ?? ''),
                        isExpanded: true,
                        decoration: _runtimeInputDecoration(hintText: 'Choose project'),
                      ),
                    ),
                    _RuntimeLabeledField(
                      label: 'Role',
                      helper: 'Runtime role used for this session.',
                      child: DropdownButtonFormField<String>(
                        key: const ValueKey('agentRuntime.createSession.role'),
                        initialValue: roles.any((role) => role.id == _role) ? _role : null,
                        items: [for (final role in roles) DropdownMenuItem(value: role.id, child: Text(role.title, overflow: TextOverflow.ellipsis))],
                        onChanged: (value) => setState(() => _role = value ?? ''),
                        isExpanded: true,
                        decoration: _runtimeInputDecoration(hintText: 'Choose role'),
                      ),
                    ),
                    _RuntimeLabeledField(
                      label: 'Model',
                      helper: models.isEmpty ? 'No model options were provided by the runtime projection.' : 'Model option supplied by Rust-owned runtime data.',
                      child: models.isEmpty
                          ? TextField(
                              key: const ValueKey('agentRuntime.createSession.noModel'),
                              enabled: false,
                              decoration: _runtimeInputDecoration(hintText: 'No model options available'),
                            )
                          : DropdownButtonFormField<String>(
                              key: const ValueKey('agentRuntime.createSession.model'),
                              initialValue: models.contains(_model) ? _model : models.first,
                              items: [for (final model in models) DropdownMenuItem(value: model, child: Text(model, overflow: TextOverflow.ellipsis))],
                              onChanged: (value) => setState(() => _model = value ?? models.first),
                              isExpanded: true,
                              decoration: _runtimeInputDecoration(hintText: 'Choose model'),
                            ),
                    ),
                  ]),
                ],
              ),
              const SizedBox(height: 16),
              _RuntimeFormSection(
                title: 'Session identity',
                description: 'The user-facing title creates a stable session name before submission.',
                children: [
                  _RuntimeFormGrid(children: [
                    _RuntimeLabeledField(
                      label: 'Title',
                      helper: 'Shown in the sessions list.',
                      child: TextField(
                        key: const ValueKey('agentRuntime.createSession.title'),
                        controller: _title,
                        decoration: _runtimeInputDecoration(hintText: 'New session'),
                      ),
                    ),
                    _RuntimeLabeledField(
                      label: 'Generated session name',
                      helper: 'Read-only slug generated from the title.',
                      child: TextField(
                        key: const ValueKey('agentRuntime.createSession.name'),
                        controller: _name,
                        readOnly: true,
                        decoration: _runtimeInputDecoration(hintText: 'new-session'),
                      ),
                    ),
                  ]),
                ],
              ),
              const SizedBox(height: 16),
              _RuntimeFormSection(
                title: 'Workspace',
                description: 'The runtime uses these paths for command execution and worktree context.',
                children: [
                  _RuntimeFormGrid(children: [
                    _RuntimeLabeledField(
                      label: 'Workdir',
                      helper: 'Initial working directory for runtime commands.',
                      child: TextField(
                        key: const ValueKey('agentRuntime.createSession.workdir'),
                        controller: _workdir,
                        decoration: _runtimeInputDecoration(hintText: '/path/to/workdir'),
                      ),
                    ),
                    _RuntimeLabeledField(
                      label: 'Worktree root',
                      helper: 'Repository or worktree root for this session.',
                      child: TextField(
                        key: const ValueKey('agentRuntime.createSession.worktreeRoot'),
                        controller: _worktreeRoot,
                        decoration: _runtimeInputDecoration(hintText: '/path/to/worktree'),
                      ),
                    ),
                  ]),
                ],
              ),
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
      if (_model.trim().isEmpty) 'model',
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
      model: _model,
      workdir: _workdir.text,
      worktreeRoot: _worktreeRoot.text,
      title: _title.text,
      name: _name.text,
    );
    Navigator.of(context).pop();
  }

  List<String> _modelOptions() {
    final models = <String>{
      if ((widget.data.roleAdmin.selectedDetail?.model ?? '').trim().isNotEmpty) widget.data.roleAdmin.selectedDetail!.model.trim(),
      if ((widget.data.roleAdmin.editorDraft?.model ?? '').trim().isNotEmpty) widget.data.roleAdmin.editorDraft!.model.trim(),
    }.toList(growable: false);
    return models;
  }

  void _syncGeneratedName() {
    final next = _sessionNameFromTitle(_title.text);
    if (_name.text != next) {
      _name.text = next;
    }
  }

  String _sessionNameFromTitle(String title) {
    final normalized = title
        .trim()
        .toLowerCase()
        .replaceAll(RegExp(r'[^a-z0-9]+'), '-')
        .replaceAll(RegExp(r'^-+|-+$'), '');
    return normalized.isEmpty ? 'new-session' : normalized;
  }
}

InputDecoration _runtimeInputDecoration({required String hintText}) {
  return InputDecoration(
    hintText: hintText,
    isDense: false,
    filled: true,
    floatingLabelBehavior: FloatingLabelBehavior.never,
    contentPadding: const EdgeInsets.symmetric(horizontal: 14, vertical: 14),
    border: OutlineInputBorder(borderRadius: BorderRadius.circular(12)),
    enabledBorder: OutlineInputBorder(
      borderRadius: BorderRadius.circular(12),
      borderSide: const BorderSide(color: Color(0xFF2D3946)),
    ),
    focusedBorder: OutlineInputBorder(
      borderRadius: BorderRadius.circular(12),
      borderSide: const BorderSide(color: Color(0xFF8AB4FF), width: 1.4),
    ),
  );
}

class _RuntimeFormSection extends StatelessWidget {
  const _RuntimeFormSection({required this.title, required this.description, required this.children});

  final String title;
  final String description;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return DecoratedBox(
      decoration: BoxDecoration(
        color: theme.colorScheme.surfaceContainerHighest.withValues(alpha: 0.32),
        borderRadius: BorderRadius.circular(18),
        border: Border.all(color: theme.colorScheme.outlineVariant.withValues(alpha: 0.48)),
      ),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(title, style: theme.textTheme.titleSmall?.copyWith(fontWeight: FontWeight.w700)),
            const SizedBox(height: 4),
            Text(description, style: theme.textTheme.bodySmall?.copyWith(color: theme.colorScheme.onSurfaceVariant)),
            const SizedBox(height: 14),
            ...children,
          ],
        ),
      ),
    );
  }
}

class _RuntimeFormGrid extends StatelessWidget {
  const _RuntimeFormGrid({required this.children});

  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    return Wrap(
      spacing: 14,
      runSpacing: 14,
      children: [
        for (final child in children)
          SizedBox(
            width: 292,
            child: child,
          ),
      ],
    );
  }
}

class _RuntimeLabeledField extends StatelessWidget {
  const _RuntimeLabeledField({required this.label, required this.child, this.helper});

  final String label;
  final String? helper;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(label, style: theme.textTheme.labelLarge?.copyWith(fontWeight: FontWeight.w700)),
        const SizedBox(height: 7),
        child,
        if (helper != null && helper!.trim().isNotEmpty) ...[
          const SizedBox(height: 6),
          Text(helper!, style: theme.textTheme.bodySmall?.copyWith(color: theme.colorScheme.onSurfaceVariant)),
        ],
      ],
    );
  }
}

enum _RuntimeFormNoticeTone { error }

class _RuntimeFormNotice extends StatelessWidget {
  const _RuntimeFormNotice({required this.message, required this.tone});

  final String message;
  final _RuntimeFormNoticeTone tone;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    return DecoratedBox(
      decoration: BoxDecoration(
        color: const Color(0xFF3A1D26),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: const Color(0xFFFF9BA7).withValues(alpha: 0.42)),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
        child: Text(message, style: Theme.of(context).textTheme.bodyMedium?.copyWith(color: colors.onErrorContainer)),
      ),
    );
  }
}

class AgentRuntimeCreateProjectDialog extends StatefulWidget {
  const AgentRuntimeCreateProjectDialog({super.key, required this.data, required this.onCreate, this.existingProjectKeys = const []});

  final AgentRuntimeWorkbenchData data;
  final List<String> existingProjectKeys;
  final void Function({
    required String projectKey,
    required String displayName,
    required String defaultWorkdir,
    required String defaultWorktreeRoot,
    required String defaultRoleId,
    required String defaultModel,
    required bool tracked,
    required bool listed,
  }) onCreate;

  @override
  State<AgentRuntimeCreateProjectDialog> createState() => _CreateProjectDialogState();
}

class _CreateProjectDialogState extends State<AgentRuntimeCreateProjectDialog> {
  late final TextEditingController _key;
  late final TextEditingController _displayName;
  late final TextEditingController _workdir;
  late final TextEditingController _worktreeRoot;
  late String _role;
  late final TextEditingController _model;
  bool _tracked = true;
  bool _listed = true;
  String? _error;

  @override
  void initState() {
    super.initState();
    _key = TextEditingController();
    _displayName = TextEditingController();
    _workdir = TextEditingController(text: '.');
    _worktreeRoot = TextEditingController(text: '.');
    _role = widget.data.roleAdmin.rows.isNotEmpty ? widget.data.roleAdmin.rows.first.id : '';
    _model = TextEditingController(text: widget.data.roleAdmin.selectedDetail?.model ?? '');
  }

  @override
  void dispose() {
    _key.dispose();
    _displayName.dispose();
    _workdir.dispose();
    _worktreeRoot.dispose();
    _model.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final roles = widget.data.roleAdmin.rows.where((role) => role.id.isNotEmpty).toList(growable: false);
    return AlertDialog(
      title: const Text('Create project'),
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
              TextField(controller: _key, decoration: const InputDecoration(labelText: 'Project key')),
              TextField(controller: _displayName, decoration: const InputDecoration(labelText: 'Display name')),
              TextField(controller: _workdir, decoration: const InputDecoration(labelText: 'Default workdir')),
              TextField(controller: _worktreeRoot, decoration: const InputDecoration(labelText: 'Default worktree root')),
              DropdownButtonFormField<String>(
                initialValue: roles.any((role) => role.id == _role) ? _role : null,
                items: [for (final role in roles) DropdownMenuItem(value: role.id, child: Text(role.title))],
                onChanged: (value) => setState(() => _role = value ?? ''),
                decoration: const InputDecoration(labelText: 'Default role'),
              ),
              TextField(controller: _model, decoration: const InputDecoration(labelText: 'Default model')),
              SwitchListTile(value: _tracked, onChanged: (value) => setState(() => _tracked = value), title: const Text('Tracked')),
              SwitchListTile(value: _listed, onChanged: (value) => setState(() => _listed = value), title: const Text('Listed')),
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
    final projectKey = _key.text.trim();
    final missing = <String>[
      if (projectKey.isEmpty) 'project key',
      if (_displayName.text.trim().isEmpty) 'display name',
      if (_workdir.text.trim().isEmpty) 'default workdir',
      if (_worktreeRoot.text.trim().isEmpty) 'default worktree root',
      if (_role.trim().isEmpty) 'default role',
      if (_model.text.trim().isEmpty) 'default model',
    ];
    if (missing.isNotEmpty) {
      setState(() => _error = 'Required: ${missing.join(', ')}');
      return;
    }
    if (!RegExp(r'^[A-Za-z0-9][A-Za-z0-9._-]{1,127}$').hasMatch(projectKey)) {
      setState(() => _error = 'Project key must use letters, numbers, dot, dash, or underscore.');
      return;
    }
    if (widget.existingProjectKeys.any((key) => key.toLowerCase() == projectKey.toLowerCase())) {
      setState(() => _error = 'Project key already exists.');
      return;
    }
    widget.onCreate(
      projectKey: projectKey,
      displayName: _displayName.text,
      defaultWorkdir: _workdir.text,
      defaultWorktreeRoot: _worktreeRoot.text,
      defaultRoleId: _role,
      defaultModel: _model.text,
      tracked: _tracked,
      listed: _listed,
    );
    Navigator.of(context).pop();
  }
}

class AgentRuntimeGlobalSettingsDialog extends StatefulWidget {
  const AgentRuntimeGlobalSettingsDialog({
    super.key,
    required this.data,
    required this.onConnectManual,
    required this.onRefreshDiscovery,
    required this.onConnectDiscovery,
    required this.onRefreshIcloud,
    required this.onConnectIcloud,
    required this.onImportProfile,
    required this.onRefreshImportedProfile,
    required this.onConnectImportedProfile,
    required this.onDisconnect,
  });

  final AgentRuntimeWorkbenchData data;
  final ValueChanged<String> onConnectManual;
  final VoidCallback onRefreshDiscovery;
  final VoidCallback onConnectDiscovery;
  final VoidCallback onRefreshIcloud;
  final VoidCallback onConnectIcloud;
  final VoidCallback onImportProfile;
  final VoidCallback onRefreshImportedProfile;
  final VoidCallback onConnectImportedProfile;
  final VoidCallback onDisconnect;

  @override
  State<AgentRuntimeGlobalSettingsDialog> createState() => _GlobalSettingsDialogState();
}

class _GlobalSettingsDialogState extends State<AgentRuntimeGlobalSettingsDialog> {
  late final TextEditingController _baseUrl;

  @override
  void initState() {
    super.initState();
    _baseUrl = TextEditingController(text: widget.data.baseUrl);
  }

  @override
  void dispose() {
    _baseUrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final diagnostics = <(String, String)>[
      ('Runtime identity', widget.data.discovery.runtimeIdentity ?? 'Unavailable'),
      ('Health URL', widget.data.discovery.healthUrl ?? 'Unavailable'),
      ('WebSocket URL', widget.data.discovery.webSocketUrl ?? 'Unavailable'),
      ('Discovery path', widget.data.discovery.discoveryPath),
      ('iCloud profile path', widget.data.remoteDiscovery.discoveryPath),
      ('Imported profile path', widget.data.importedRemoteDiscovery.discoveryPath),
      ('Connection state', widget.data.connectionState),
    ];
    return AlertDialog(
      title: const Text('Global settings'),
      content: SizedBox(
        width: 620,
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              if (widget.data.errorMessage != null) ...[
                Text(widget.data.errorMessage!, style: const TextStyle(color: Color(0xFFFF9BA7))),
                const SizedBox(height: 10),
              ],
              TextField(controller: _baseUrl, decoration: const InputDecoration(labelText: 'Base URL')),
              const SizedBox(height: 12),
              Wrap(
                spacing: 8,
                runSpacing: 8,
                children: [
                  FilledButton(onPressed: () => widget.onConnectManual(_baseUrl.text), child: const Text('Connect manual URL')),
                  OutlinedButton(onPressed: widget.onRefreshDiscovery, child: const Text('Refresh local discovery')),
                  OutlinedButton(onPressed: widget.onConnectDiscovery, child: const Text('Connect local discovery')),
                  OutlinedButton(onPressed: widget.onRefreshIcloud, child: const Text('Refresh iCloud profile')),
                  OutlinedButton(onPressed: widget.onConnectIcloud, child: const Text('Connect iCloud profile')),
                  OutlinedButton(onPressed: widget.onImportProfile, child: const Text('Import remote profile document')),
                  OutlinedButton(onPressed: widget.onRefreshImportedProfile, child: const Text('Refresh imported profile')),
                  OutlinedButton(onPressed: widget.onConnectImportedProfile, child: const Text('Connect imported profile')),
                  OutlinedButton(onPressed: widget.onDisconnect, child: const Text('Disconnect')),
                ],
              ),
              const SizedBox(height: 16),
              const Text('Diagnostics'),
              const SizedBox(height: 8),
              for (final row in diagnostics)
                Padding(
                  padding: const EdgeInsets.only(bottom: 6),
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      SizedBox(width: 170, child: Text(row.$1, style: const TextStyle(color: Color(0xFFAAB6C4)))),
                      Expanded(child: Text(row.$2.isEmpty ? 'Unavailable' : row.$2)),
                    ],
                  ),
                ),
            ],
          ),
        ),
      ),
      actions: [
        TextButton(onPressed: () => Navigator.of(context).pop(), child: const Text('Close')),
      ],
    );
  }
}

class AgentRuntimeProjectSettingsDialog extends StatefulWidget {
  const AgentRuntimeProjectSettingsDialog({super.key, required this.data, required this.projectId, required this.onSave, required this.onArchive, required this.onUnarchive});

  final AgentRuntimeWorkbenchData data;
  final String projectId;
  final void Function({
    required String projectKey,
    required String displayName,
    required String defaultWorkdir,
    required String defaultWorktreeRoot,
    required String defaultRoleId,
    required String defaultModel,
    required bool tracked,
    required bool listed,
  }) onSave;
  final ValueChanged<String> onArchive;
  final ValueChanged<String> onUnarchive;

  @override
  State<AgentRuntimeProjectSettingsDialog> createState() => _ProjectSettingsDialogState();
}

class _ProjectSettingsDialogState extends State<AgentRuntimeProjectSettingsDialog> {
  late final TextEditingController _displayName;
  late final TextEditingController _workdir;
  late final TextEditingController _worktreeRoot;
  late String _role;
  late final TextEditingController _model;
  bool _tracked = true;
  bool _listed = true;
  String? _error;

  @override
  void initState() {
    super.initState();
    _displayName = TextEditingController(text: widget.projectId);
    _workdir = TextEditingController(text: '.');
    _worktreeRoot = TextEditingController(text: '.');
    _role = widget.data.roleAdmin.rows.isNotEmpty ? widget.data.roleAdmin.rows.first.id : '';
    _model = TextEditingController(text: widget.data.roleAdmin.selectedDetail?.model ?? '');
  }

  @override
  void dispose() {
    _displayName.dispose();
    _workdir.dispose();
    _worktreeRoot.dispose();
    _model.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final roles = widget.data.roleAdmin.rows.where((role) => role.id.isNotEmpty).toList(growable: false);
    return AlertDialog(
      title: const Text('Project settings'),
      content: SizedBox(
        width: 520,
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              TextFormField(initialValue: widget.projectId, readOnly: true, decoration: const InputDecoration(labelText: 'Project key')),
              if (_error != null) ...[
                const SizedBox(height: 8),
                Text(_error!, style: const TextStyle(color: Color(0xFFFF9BA7))),
              ],
              TextField(controller: _displayName, decoration: const InputDecoration(labelText: 'Display name')),
              TextField(controller: _workdir, decoration: const InputDecoration(labelText: 'Default workdir')),
              TextField(controller: _worktreeRoot, decoration: const InputDecoration(labelText: 'Default worktree root')),
              DropdownButtonFormField<String>(
                initialValue: roles.any((role) => role.id == _role) ? _role : null,
                items: [for (final role in roles) DropdownMenuItem(value: role.id, child: Text(role.title))],
                onChanged: (value) => setState(() => _role = value ?? ''),
                decoration: const InputDecoration(labelText: 'Default role'),
              ),
              TextField(controller: _model, decoration: const InputDecoration(labelText: 'Default model')),
              SwitchListTile(value: _tracked, onChanged: (value) => setState(() => _tracked = value), title: const Text('Tracked')),
              SwitchListTile(value: _listed, onChanged: (value) => setState(() => _listed = value), title: const Text('Listed')),
            ],
          ),
        ),
      ),
      actions: [
        TextButton(onPressed: () => Navigator.of(context).pop(), child: const Text('Cancel')),
        TextButton(
          onPressed: () {
            widget.onArchive(widget.projectId);
            _popDialogIfPresent(context);
          },
          child: const Text('Archive'),
        ),
        TextButton(
          onPressed: () {
            widget.onUnarchive(widget.projectId);
            _popDialogIfPresent(context);
          },
          child: const Text('Unarchive'),
        ),
        FilledButton(onPressed: _submit, child: const Text('Save')),
      ],
    );
  }

  void _submit() {
    final missing = <String>[
      if (_displayName.text.trim().isEmpty) 'display name',
      if (_workdir.text.trim().isEmpty) 'default workdir',
      if (_worktreeRoot.text.trim().isEmpty) 'default worktree root',
      if (_model.text.trim().isEmpty) 'default model',
    ];
    if (missing.isNotEmpty) {
      setState(() => _error = 'Required: ${missing.join(', ')}');
      return;
    }
    widget.onSave(
      projectKey: widget.projectId,
      displayName: _displayName.text,
      defaultWorkdir: _workdir.text,
      defaultWorktreeRoot: _worktreeRoot.text,
      defaultRoleId: _role,
      defaultModel: _model.text,
      tracked: _tracked,
      listed: _listed,
    );
    _popDialogIfPresent(context);
  }
}

class AgentRuntimeSessionSettingsDialog extends StatefulWidget {
  const AgentRuntimeSessionSettingsDialog({
    super.key,
    required this.shell,
    required this.data,
    required this.onSave,
    required this.onClose,
    required this.onArchive,
    required this.onFork,
  });

  final ConversationShellData shell;
  final AgentRuntimeWorkbenchData data;
  final void Function({
    required String sessionId,
    required String project,
    required String role,
    required String model,
    required String workdir,
    required String worktreeRoot,
    required String title,
    required String name,
    required bool tracked,
  }) onSave;
  final ValueChanged<String> onClose;
  final ValueChanged<String> onArchive;
  final ValueChanged<String> onFork;

  @override
  State<AgentRuntimeSessionSettingsDialog> createState() => _AgentRuntimeSessionSettingsDialogState();
}

class _AgentRuntimeSessionSettingsDialogState extends State<AgentRuntimeSessionSettingsDialog> {
  late final ConversationSession? _session;
  late String _project;
  late String _role;
  late final TextEditingController _model;
  late final TextEditingController _title;
  late final TextEditingController _name;
  late final TextEditingController _workdir;
  late final TextEditingController _worktreeRoot;
  bool _tracked = true;
  String? _error;

  @override
  void initState() {
    super.initState();
    ConversationSession? selected;
    for (final session in widget.shell.sessions) {
      if (session.id == widget.shell.selectedSessionId) {
        selected = session;
        break;
      }
    }
    _session = selected;
    final projectChoices = widget.shell.projects.where((project) => project.id != '__all__').toList(growable: false);
    _project = projectChoices.isNotEmpty ? projectChoices.first.id : '__unassigned__';
    _role = _session?.rolePresentation.roleId ?? (widget.data.roleAdmin.rows.isNotEmpty ? widget.data.roleAdmin.rows.first.id : '');
    _model = TextEditingController(text: widget.data.roleAdmin.selectedDetail?.model ?? '');
    _title = TextEditingController(text: _session?.title ?? '');
    _name = TextEditingController(text: (_session?.title ?? '').toLowerCase().replaceAll(RegExp(r'[^a-z0-9]+'), '-').replaceAll(RegExp(r'^-|-$'), ''));
    _workdir = TextEditingController(text: '.');
    _worktreeRoot = TextEditingController(text: '.');
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
    final sessionId = widget.shell.selectedSessionId ?? '';
    final projects = widget.shell.projects.where((project) => project.id.isNotEmpty && project.id != '__all__').toList(growable: false);
    final roles = widget.data.roleAdmin.rows.where((role) => role.id.isNotEmpty).toList(growable: false);
    return AlertDialog(
      title: const Text('Session settings'),
      content: SizedBox(
        width: 560,
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              TextFormField(initialValue: sessionId.isEmpty ? 'No selected session' : sessionId, readOnly: true, decoration: const InputDecoration(labelText: 'Session')),
              if (_error != null) ...[
                const SizedBox(height: 8),
                Text(_error!, style: const TextStyle(color: Color(0xFFFF9BA7))),
              ],
              TextField(controller: _title, decoration: const InputDecoration(labelText: 'Title')),
              TextField(controller: _name, decoration: const InputDecoration(labelText: 'Name')),
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
              TextField(controller: _workdir, decoration: const InputDecoration(labelText: 'Workdir')),
              TextField(controller: _worktreeRoot, decoration: const InputDecoration(labelText: 'Worktree root')),
              SwitchListTile(value: _tracked, onChanged: (value) => setState(() => _tracked = value), title: const Text('Tracked')),
            ],
          ),
        ),
      ),
      actions: [
        TextButton(onPressed: () => Navigator.of(context).pop(), child: const Text('Cancel')),
        TextButton(onPressed: sessionId.isEmpty ? null : () => _lifecycle(widget.onClose), child: const Text('Close session')),
        TextButton(onPressed: sessionId.isEmpty ? null : () => _lifecycle(widget.onArchive), child: const Text('Archive session')),
        TextButton(onPressed: sessionId.isEmpty ? null : () => _lifecycle(widget.onFork), child: const Text('Fork session')),
        FilledButton(onPressed: sessionId.isEmpty ? null : _submit, child: const Text('Save')),
      ],
    );
  }

  void _lifecycle(ValueChanged<String> action) {
    final sessionId = widget.shell.selectedSessionId;
    if (sessionId == null || sessionId.isEmpty) {
      return;
    }
    action(sessionId);
    _popDialogIfPresent(context);
  }

  void _submit() {
    final sessionId = widget.shell.selectedSessionId;
    final missing = <String>[
      if (sessionId == null || sessionId.isEmpty) 'session',
      if (_project.trim().isEmpty) 'project',
      if (_role.trim().isEmpty) 'role',
      if (_model.text.trim().isEmpty) 'model',
      if (_title.text.trim().isEmpty) 'title',
      if (_name.text.trim().isEmpty) 'name',
      if (_workdir.text.trim().isEmpty) 'workdir',
      if (_worktreeRoot.text.trim().isEmpty) 'worktree root',
    ];
    if (missing.isNotEmpty) {
      setState(() => _error = 'Required: ${missing.join(', ')}');
      return;
    }
    widget.onSave(
      sessionId: sessionId!,
      project: _project,
      role: _role,
      model: _model.text,
      workdir: _workdir.text,
      worktreeRoot: _worktreeRoot.text,
      title: _title.text,
      name: _name.text,
      tracked: _tracked,
    );
    _popDialogIfPresent(context);
  }
}

void _popDialogIfPresent(BuildContext context) {
  final route = ModalRoute.of(context);
  if (route != null && !route.isFirst) {
    Navigator.of(context).pop();
  }
}
