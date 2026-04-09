import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'package:rinf/rinf.dart';

import '../core/state/workbench_controller.dart';
import '../core/models/workbench_models.dart';
import '../features/chat/chat_timeline.dart';
import '../features/shell/robdex_shell_screen.dart';
import '../theme/robdex_theme.dart';

class RobdexApp extends StatelessWidget {
  const RobdexApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      debugShowCheckedModeBanner: false,
      title: 'Robdex',
      theme: buildRobdexTheme(),
      home: const RobdexWorkbench(),
    );
  }
}

class RobdexWorkbench extends StatefulWidget {
  const RobdexWorkbench({super.key});

  @override
  State<RobdexWorkbench> createState() => _RobdexWorkbenchState();
}

class _RobdexWorkbenchState extends State<RobdexWorkbench> {
  static const _hostPreferenceKey = 'bridge_host';
  static const _portPreferenceKey = 'bridge_port';

  late final WorkbenchController _controller;
  late final AppLifecycleListener _listener;
  bool _didRequestConnect = false;
  late final TextEditingController _hostController;
  late final TextEditingController _portController;

  @override
  void initState() {
    super.initState();
    _controller = WorkbenchController();
    _hostController = TextEditingController(text: '127.0.0.1');
    _portController = TextEditingController(text: '42080');
    _restoreBridgeSettings();
    _listener = AppLifecycleListener(
      onExitRequested: () async {
        finalizeRust();
        return AppExitResponse.exit;
      },
    );
  }

  @override
  void dispose() {
    _persistBridgeSettings();
    _listener.dispose();
    _hostController.dispose();
    _portController.dispose();
    _controller.dispose();
    super.dispose();
  }

  Future<void> _restoreBridgeSettings() async {
    final prefs = await SharedPreferences.getInstance();
    final host = prefs.getString(_hostPreferenceKey);
    final port = prefs.getInt(_portPreferenceKey);
    if (!mounted) {
      return;
    }
    if ((host?.trim().isNotEmpty ?? false) || port != null) {
      setState(() {
        if (host?.trim().isNotEmpty ?? false) {
          _hostController.text = host!.trim();
        }
        if (port != null && port > 0) {
          _portController.text = port.toString();
        }
      });
    }
  }

  Future<void> _persistBridgeSettings() async {
    final prefs = await SharedPreferences.getInstance();
    final host = _hostController.text.trim();
    final port = int.tryParse(_portController.text.trim());
    await prefs.setString(
      _hostPreferenceKey,
      host.isEmpty ? '127.0.0.1' : host,
    );
    if (port != null && port > 0) {
      await prefs.setInt(_portPreferenceKey, port);
    } else {
      await prefs.setInt(_portPreferenceKey, 42080);
    }
  }

  Future<void> _attemptConnect() async {
    final port = int.tryParse(_portController.text.trim());
    if (port == null) {
      return;
    }
    await _persistBridgeSettings();
    if (mounted) {
      setState(() {
        _didRequestConnect = true;
      });
    }
    _controller.start(
      host: _hostController.text.trim(),
      port: port,
    );
  }

  void _returnToLogin() {
    _controller.disconnect();
    if (mounted) {
      setState(() {
        _didRequestConnect = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: _controller,
      builder: (context, _) {
        if (_controller.error != null && _controller.view == null) {
          return Scaffold(
            body: Center(
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 420),
                child: Padding(
                  padding: const EdgeInsets.all(24),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        'Connection Failed',
                        style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                              fontWeight: FontWeight.w700,
                            ),
                      ),
                      const SizedBox(height: 10),
                      Text(
                        '${_controller.error}',
                        style: Theme.of(context).textTheme.bodySmall,
                      ),
                      const SizedBox(height: 16),
                      Row(
                        children: [
                          TextButton(
                            onPressed: _returnToLogin,
                            child: const Text('Back'),
                          ),
                          const SizedBox(width: 8),
                          FilledButton(
                            onPressed: _attemptConnect,
                            child: const Text('Retry'),
                          ),
                        ],
                      ),
                    ],
                  ),
                ),
              ),
            ),
          );
        }
        if (!_didRequestConnect) {
          return Scaffold(
            body: Center(
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 420),
                child: Padding(
                  padding: const EdgeInsets.all(24),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        'Robdex',
                        style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                              fontWeight: FontWeight.w700,
                            ),
                      ),
                      const SizedBox(height: 10),
                      Text(
                        'Connect to the bridge when you are ready. No thread state is loaded before login.',
                        style: Theme.of(context).textTheme.bodySmall,
                      ),
                      const SizedBox(height: 16),
                      TextField(
                        controller: _hostController,
                        decoration: const InputDecoration(labelText: 'Host'),
                      ),
                      const SizedBox(height: 8),
                      TextField(
                        controller: _portController,
                        decoration: const InputDecoration(labelText: 'Port'),
                        keyboardType: TextInputType.number,
                      ),
                      const SizedBox(height: 16),
                      Row(
                        children: [
                          Expanded(
                            child: Text(
                              'Bridge endpoint',
                              style: Theme.of(context).textTheme.labelSmall,
                            ),
                          ),
                          FilledButton(
                            onPressed: _attemptConnect,
                            child: const Text('Connect'),
                          ),
                        ],
                      ),
                    ],
                  ),
                ),
              ),
            ),
          );
        }
        if (_controller.view == null) {
          return const Scaffold(
            body: Center(child: CircularProgressIndicator()),
          );
        }
        return RobdexShellScreen(
          workbench: _controller.view!,
          onThreadSelected: _controller.selectThread,
          onProjectSelected: _controller.selectProject,
          onDisconnect: () {
            _returnToLogin();
          },
          onCreateProject: _showCreateProjectDialog,
          onProjectSettings: _showProjectSettingsDialog,
          onCreateThread: _showCreateThreadDialog,
          onSpawnAgent: _showSpawnAgentDialog,
          onSendMessage: _controller.sendMessage,
          onOpenHistory: _showHistorySheet,
          onInterruptThread: _controller.interruptThread,
          onApprovalDecision: (approval, decision, message) async {
            _controller.decideApproval(
              approvalId: approval.id,
              decision: decision,
              message: message,
            );
          },
          onSettingsChanged: (draft) => _controller.updateThreadSettings(
            role: draft.role,
            approvalPolicy: draft.approvalPolicy,
            sandboxMode: draft.sandboxMode,
            networkAccessMode: draft.networkAccessMode,
            modelId: draft.modelId,
            reasoningEffort: draft.reasoningEffort,
          ),
          onRunningStateChanged: _controller.setThreadRunningState,
          onRenameThread: _controller.renameThread,
          onArchiveThread: _controller.archiveThread,
          onWarmHandoff: _controller.warmHandoff,
          onSetProjectOrchestrator: () {
            final view = _controller.view;
            final selection = view?.selection;
            final projectId = selection?.projectId;
            final projectPath = selection?.projectRootPath;
            final threadId = selection?.threadId;
            if (projectId == null || projectPath == null || threadId == null) {
              return;
            }
            _controller.setProjectOrchestrator(
              projectId: projectId,
              projectPath: projectPath,
              threadId: threadId,
            );
          },
          onCreateThreadGroup: _controller.createThreadGroup,
          onRenameThreadGroup: (group) async {
            final renamed = await _promptGroupName(context, group.title);
            if (renamed != null && renamed.trim().isNotEmpty) {
              _controller.renameThreadGroup(groupId: group.id, title: renamed);
            }
          },
          onDeleteThreadGroup: _controller.deleteThreadGroup,
          onArchiveThreadGroup: _controller.archiveThreadGroup,
          onMoveSelectedThreadToGroup: _controller.moveSelectedThreadToGroup,
          onUpdateWorkerMetadata: (draft) => _controller.updateWorkerMetadata(
            issueNumber: draft.issueNumber,
            pullRequestNumber: draft.pullRequestNumber,
            blockedReason: draft.blockedReason,
            unblockWhen: draft.unblockWhen,
            clearBlocked: draft.clearBlocked,
          ),
        );
      },
    );
  }

  Future<void> _showCreateProjectDialog() async {
    final nameController = TextEditingController();
    final rootController = TextEditingController();
    final cwdController = TextEditingController();
    final result = await showDialog<_ProjectDraft>(
      context: context,
      builder: (context) {
        return AlertDialog(
          title: const Text('Create Project'),
          content: SizedBox(
            width: 420,
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                TextField(
                  controller: nameController,
                  decoration: const InputDecoration(labelText: 'Name'),
                ),
                const SizedBox(height: 12),
                TextField(
                  controller: rootController,
                  decoration: const InputDecoration(labelText: 'Root Path'),
                ),
                const SizedBox(height: 12),
                TextField(
                  controller: cwdController,
                  decoration: const InputDecoration(labelText: 'Default CWD'),
                ),
              ],
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(context).pop(),
              child: const Text('Cancel'),
            ),
            FilledButton(
              onPressed: () {
                Navigator.of(context).pop(
                  _ProjectDraft(
                    name: nameController.text,
                    rootPath: rootController.text,
                    defaultCwd: cwdController.text,
                  ),
                );
              },
              child: const Text('Create'),
            ),
          ],
        );
      },
    );
    nameController.dispose();
    rootController.dispose();
    cwdController.dispose();

    if (result == null) {
      return;
    }
    _controller.createProject(
      name: result.name,
      rootPath: result.rootPath,
      defaultCwd: result.defaultCwd,
    );
  }

  Future<void> _showHistorySheet() async {
    final view = _controller.view;
    final threadId = view?.selection.threadId;
    if (threadId == null) {
      return;
    }
    _controller.fetchThreadHistory();
    await showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      builder: (context) {
        return SafeArea(
          child: FractionallySizedBox(
            heightFactor: 0.92,
            child: _ThreadHistorySheet(
              controller: _controller,
              threadId: threadId,
              threadName: _controller.view?.selection.threadName ?? 'History',
              contextWindowRemainingPercent:
                  _controller.view?.contextWindowRemainingPercent,
            ),
          ),
        );
      },
    );
  }

  Future<void> _showCreateThreadDialog(ProjectItem project) async {
    final view = _controller.view;
    final availableModels = view?.availableModels ?? const <ModelItem>[];
    final titleController = TextEditingController();
    final promptController = TextEditingController();
    String role = 'worker';
    String approvalPolicy = '';
    String sandboxMode = '';
    String networkAccessMode = 'default';
    String modelId = '';
    String reasoningEffort = '';
    final result = await showDialog<_ThreadDraft>(
      context: context,
      builder: (context) {
        return StatefulBuilder(
          builder: (context, setDialogState) {
            return AlertDialog(
              title: const Text('Create Thread'),
              content: SizedBox(
                width: 420,
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    TextField(
                      controller: titleController,
                      decoration: const InputDecoration(labelText: 'Title'),
                    ),
                    const SizedBox(height: 12),
                    Align(
                      alignment: Alignment.centerLeft,
                      child: Text(
                        '${project.name}  ${project.defaultCwd}',
                        style: Theme.of(context).textTheme.labelSmall?.copyWith(
                              fontFamily: 'monospace',
                            ),
                      ),
                    ),
                    const SizedBox(height: 12),
                    DropdownButtonFormField<String>(
                      initialValue: role,
                      decoration: const InputDecoration(labelText: 'Role'),
                      items: const [
                        DropdownMenuItem(value: 'worker', child: Text('Worker')),
                        DropdownMenuItem(value: 'qa', child: Text('QA')),
                        DropdownMenuItem(value: 'operator', child: Text('Operator')),
                        DropdownMenuItem(value: 'orchestrator', child: Text('Orchestrator')),
                        DropdownMenuItem(value: 'hidden', child: Text('Hidden')),
                      ],
                      onChanged: (value) {
                        if (value == null) return;
                        setDialogState(() => role = value);
                      },
                    ),
                    const SizedBox(height: 12),
                    TextField(
                      controller: promptController,
                      minLines: 3,
                      maxLines: 8,
                      decoration: const InputDecoration(labelText: 'Initial prompt'),
                    ),
                    const SizedBox(height: 12),
                    DropdownButtonFormField<String>(
                      initialValue: modelId,
                      decoration: const InputDecoration(labelText: 'Model'),
                      items: [
                        const DropdownMenuItem(value: '', child: Text('Default')),
                        ...availableModels
                            .where((model) => !model.hidden)
                            .map(
                              (model) => DropdownMenuItem(
                                value: model.id,
                                child: Text(
                                  model.name?.trim().isNotEmpty == true ? model.name! : model.id,
                                ),
                              ),
                            ),
                      ],
                      onChanged: (value) => setDialogState(() => modelId = value ?? ''),
                    ),
                    const SizedBox(height: 12),
                    Wrap(
                      spacing: 12,
                      runSpacing: 12,
                      children: [
                        SizedBox(
                          width: 180,
                          child: DropdownButtonFormField<String>(
                            initialValue: reasoningEffort,
                            decoration: const InputDecoration(labelText: 'Reasoning'),
                            items: const [
                              DropdownMenuItem(value: '', child: Text('Default')),
                              DropdownMenuItem(value: 'low', child: Text('Low')),
                              DropdownMenuItem(value: 'medium', child: Text('Medium')),
                              DropdownMenuItem(value: 'high', child: Text('High')),
                            ],
                            onChanged: (value) => setDialogState(() => reasoningEffort = value ?? ''),
                          ),
                        ),
                        SizedBox(
                          width: 180,
                          child: DropdownButtonFormField<String>(
                            initialValue: sandboxMode,
                            decoration: const InputDecoration(labelText: 'Sandbox'),
                            items: const [
                              DropdownMenuItem(value: '', child: Text('Default')),
                              DropdownMenuItem(value: 'workspace-write', child: Text('Workspace')),
                              DropdownMenuItem(value: 'danger-full-access', child: Text('Danger')),
                            ],
                            onChanged: (value) => setDialogState(() => sandboxMode = value ?? ''),
                          ),
                        ),
                        SizedBox(
                          width: 180,
                          child: DropdownButtonFormField<String>(
                            initialValue: networkAccessMode,
                            decoration: const InputDecoration(labelText: 'Network'),
                            items: const [
                              DropdownMenuItem(value: 'default', child: Text('Default')),
                              DropdownMenuItem(value: 'enabled', child: Text('Enabled')),
                              DropdownMenuItem(value: 'disabled', child: Text('Disabled')),
                            ],
                            onChanged: (value) => setDialogState(() => networkAccessMode = value ?? 'default'),
                          ),
                        ),
                        SizedBox(
                          width: 180,
                          child: DropdownButtonFormField<String>(
                            initialValue: approvalPolicy,
                            decoration: const InputDecoration(labelText: 'Approval'),
                            items: const [
                              DropdownMenuItem(value: '', child: Text('Default')),
                              DropdownMenuItem(value: 'untrusted', child: Text('untrusted')),
                              DropdownMenuItem(value: 'on-failure', child: Text('on-failure')),
                              DropdownMenuItem(value: 'on-request', child: Text('on-request')),
                              DropdownMenuItem(value: 'never', child: Text('never')),
                            ],
                            onChanged: (value) => setDialogState(() => approvalPolicy = value ?? ''),
                          ),
                        ),
                      ],
                    ),
                  ],
                ),
              ),
              actions: [
                TextButton(
                  onPressed: () => Navigator.of(context).pop(),
                  child: const Text('Cancel'),
                ),
                FilledButton(
                  onPressed: () {
                    Navigator.of(context).pop(
                      _ThreadDraft(
                        projectId: project.id,
                        title: titleController.text,
                        initialPrompt: promptController.text,
                        role: role,
                        approvalPolicy: approvalPolicy,
                        sandboxMode: sandboxMode,
                        networkAccessMode: networkAccessMode,
                        modelId: modelId,
                        reasoningEffort: reasoningEffort,
                      ),
                    );
                  },
                  child: const Text('Create'),
                ),
              ],
            );
          },
        );
      },
    );
    titleController.dispose();
    promptController.dispose();

    if (result == null) {
      return;
    }
    _controller.createThread(
      projectId: result.projectId,
      title: result.title,
      initialPrompt: result.initialPrompt,
      role: result.role,
      approvalPolicy: result.approvalPolicy,
      sandboxMode: result.sandboxMode,
      networkAccessMode: result.networkAccessMode,
      modelId: result.modelId,
      reasoningEffort: result.reasoningEffort,
    );
  }

  Future<void> _showProjectSettingsDialog(ProjectItem project) async {
    final availableModels = _controller.view?.availableModels ?? const <ModelItem>[];
    final nameController = TextEditingController(text: project.name);
    final cwdController = TextEditingController(text: project.defaultCwd);
    bool autoRouteReplies = project.autoRouteReplies;
    bool routeApprovalRequests = project.routeApprovalRequests;
    String preferredModelProvider = project.preferredModelProvider ?? '';
    String orchestratorModelId = project.orchestratorDefaultModel ?? '';
    String orchestratorReasoningEffort = project.orchestratorDefaultReasoningEffort ?? '';
    String workerModelId = project.workerDefaultModel ?? '';
    String workerReasoningEffort = project.workerDefaultReasoningEffort ?? '';
    String qaModelId = project.qaDefaultModel ?? '';
    String qaReasoningEffort = project.qaDefaultReasoningEffort ?? '';

    final result = await showDialog<bool>(
      context: context,
      builder: (context) {
        Widget modelDropdown(
          String label,
          String current,
          ValueChanged<String> onChanged,
        ) {
          return DropdownButtonFormField<String>(
            initialValue: current,
            decoration: InputDecoration(labelText: label),
            items: [
              const DropdownMenuItem(value: '', child: Text('Default')),
              ...availableModels
                  .where((model) => !model.hidden)
                  .map(
                    (model) => DropdownMenuItem(
                      value: model.id,
                      child: Text(model.name?.trim().isNotEmpty == true ? model.name! : model.id),
                    ),
                  ),
            ],
            onChanged: (value) => onChanged(value ?? ''),
          );
        }

        Widget reasoningDropdown(
          String label,
          String current,
          ValueChanged<String> onChanged,
        ) {
          return DropdownButtonFormField<String>(
            initialValue: current,
            decoration: InputDecoration(labelText: label),
            items: const [
              DropdownMenuItem(value: '', child: Text('Default')),
              DropdownMenuItem(value: 'low', child: Text('Low')),
              DropdownMenuItem(value: 'medium', child: Text('Medium')),
              DropdownMenuItem(value: 'high', child: Text('High')),
            ],
            onChanged: (value) => onChanged(value ?? ''),
          );
        }

        return StatefulBuilder(
          builder: (context, setDialogState) {
            return AlertDialog(
              title: const Text('Project Settings'),
              content: SizedBox(
                width: 540,
                child: SingleChildScrollView(
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      TextField(
                        controller: nameController,
                        decoration: const InputDecoration(labelText: 'Project name'),
                      ),
                      const SizedBox(height: 12),
                      Text(
                        project.rootPath,
                        style: Theme.of(context).textTheme.labelSmall?.copyWith(
                              fontFamily: 'monospace',
                            ),
                      ),
                      const SizedBox(height: 12),
                      TextField(
                        controller: cwdController,
                        decoration: const InputDecoration(labelText: 'Default CWD'),
                      ),
                      const SizedBox(height: 12),
                      SwitchListTile(
                        value: autoRouteReplies,
                        onChanged: (value) => setDialogState(() => autoRouteReplies = value),
                        title: const Text('Auto-route replies'),
                        contentPadding: EdgeInsets.zero,
                      ),
                      SwitchListTile(
                        value: routeApprovalRequests,
                        onChanged: (value) => setDialogState(() => routeApprovalRequests = value),
                        title: const Text('Route approvals'),
                        contentPadding: EdgeInsets.zero,
                      ),
                      const SizedBox(height: 12),
                      DropdownButtonFormField<String>(
                        initialValue: preferredModelProvider,
                        decoration: const InputDecoration(labelText: 'Preferred model provider'),
                        items: const [
                          DropdownMenuItem(value: '', child: Text('Default')),
                          DropdownMenuItem(value: 'openai', child: Text('OpenAI')),
                        ],
                        onChanged: (value) =>
                            setDialogState(() => preferredModelProvider = value ?? ''),
                      ),
                      const SizedBox(height: 16),
                      Text('Orchestrator Defaults',
                          style: Theme.of(context).textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.w700)),
                      const SizedBox(height: 8),
                      modelDropdown('Model', orchestratorModelId,
                          (value) => setDialogState(() => orchestratorModelId = value)),
                      const SizedBox(height: 8),
                      reasoningDropdown('Reasoning', orchestratorReasoningEffort,
                          (value) => setDialogState(() => orchestratorReasoningEffort = value)),
                      const SizedBox(height: 16),
                      Text('Worker Defaults',
                          style: Theme.of(context).textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.w700)),
                      const SizedBox(height: 8),
                      modelDropdown('Model', workerModelId,
                          (value) => setDialogState(() => workerModelId = value)),
                      const SizedBox(height: 8),
                      reasoningDropdown('Reasoning', workerReasoningEffort,
                          (value) => setDialogState(() => workerReasoningEffort = value)),
                      const SizedBox(height: 16),
                      Text('QA Defaults',
                          style: Theme.of(context).textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.w700)),
                      const SizedBox(height: 8),
                      modelDropdown('Model', qaModelId,
                          (value) => setDialogState(() => qaModelId = value)),
                      const SizedBox(height: 8),
                      reasoningDropdown('Reasoning', qaReasoningEffort,
                          (value) => setDialogState(() => qaReasoningEffort = value)),
                    ],
                  ),
                ),
              ),
              actions: [
                TextButton(
                  onPressed: () => Navigator.of(context).pop(false),
                  child: const Text('Cancel'),
                ),
                FilledButton(
                  onPressed: () => Navigator.of(context).pop(true),
                  child: const Text('Save'),
                ),
              ],
            );
          },
        );
      },
    );

    nameController.dispose();
    cwdController.dispose();

    if (result != true) {
      return;
    }

    _controller.updateProject(
      projectId: project.id,
      name: nameController.text.trim(),
      defaultCwd: cwdController.text.trim(),
      autoRouteReplies: autoRouteReplies,
      routeApprovalRequests: routeApprovalRequests,
      preferredModelProvider: preferredModelProvider,
      orchestratorModelId: orchestratorModelId,
      orchestratorReasoningEffort: orchestratorReasoningEffort,
      workerModelId: workerModelId,
      workerReasoningEffort: workerReasoningEffort,
      qaModelId: qaModelId,
      qaReasoningEffort: qaReasoningEffort,
    );
  }

  Future<void> _showSpawnAgentDialog() async {
    final nameController = TextEditingController();
    final promptController = TextEditingController();
    String role = 'worker';
    final result = await showDialog<_AgentDraft>(
      context: context,
      builder: (context) {
        return StatefulBuilder(
          builder: (context, setDialogState) {
            return AlertDialog(
              title: const Text('Spawn Agent'),
              content: SizedBox(
                width: 440,
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    TextField(
                      controller: nameController,
                      decoration: const InputDecoration(labelText: 'Agent name'),
                    ),
                    const SizedBox(height: 12),
                    DropdownButtonFormField<String>(
                      initialValue: role,
                      decoration: const InputDecoration(labelText: 'Role'),
                      items: const [
                        DropdownMenuItem(value: 'worker', child: Text('Worker')),
                        DropdownMenuItem(value: 'qa', child: Text('QA')),
                        DropdownMenuItem(value: 'operator', child: Text('Operator')),
                      ],
                      onChanged: (value) {
                        if (value == null) return;
                        setDialogState(() {
                          role = value;
                        });
                      },
                    ),
                    const SizedBox(height: 12),
                    TextField(
                      controller: promptController,
                      minLines: 3,
                      maxLines: 6,
                      decoration: const InputDecoration(labelText: 'Initial prompt'),
                    ),
                  ],
                ),
              ),
              actions: [
                TextButton(
                  onPressed: () => Navigator.of(context).pop(),
                  child: const Text('Cancel'),
                ),
                FilledButton(
                  onPressed: () {
                    Navigator.of(context).pop(
                      _AgentDraft(
                        name: nameController.text,
                        role: role,
                        prompt: promptController.text,
                      ),
                    );
                  },
                  child: const Text('Spawn'),
                ),
              ],
            );
          },
        );
      },
    );
    nameController.dispose();
    promptController.dispose();

    if (result == null) {
      return;
    }
    _controller.spawnAgent(
      name: result.name,
      role: result.role,
      prompt: result.prompt,
    );
  }

  Future<String?> _promptGroupName(BuildContext context, String initialValue) async {
    final controller = TextEditingController(text: initialValue);
    final result = await showDialog<String>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Thread Group'),
        content: TextField(
          controller: controller,
          decoration: const InputDecoration(labelText: 'Title'),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(controller.text),
            child: const Text('Save'),
          ),
        ],
      ),
    );
    controller.dispose();
    return result;
  }
}

class _ThreadHistorySheet extends StatefulWidget {
  const _ThreadHistorySheet({
    required this.controller,
    required this.threadId,
    required this.threadName,
    required this.contextWindowRemainingPercent,
  });

  final WorkbenchController controller;
  final String threadId;
  final String threadName;
  final int? contextWindowRemainingPercent;

  @override
  State<_ThreadHistorySheet> createState() => _ThreadHistorySheetState();
}

class _ThreadHistorySheetState extends State<_ThreadHistorySheet> {
  late final TextEditingController _searchController;
  String _pattern = '';

  @override
  void initState() {
    super.initState();
    _searchController = TextEditingController();
  }

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(12, 12, 12, 8),
      child: AnimatedBuilder(
        animation: widget.controller,
        builder: (context, _) {
          final error = widget.controller.threadHistoryError;
          final regex = _buildRegex(_pattern);
          final filteredEntries = regex == null
              ? widget.controller.threadHistoryEntries
              : widget.controller.threadHistoryEntries
                  .where((entry) => regex.hasMatch(_searchableText(entry)))
                  .toList(growable: false);

          return Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  Text(
                    'History',
                    style: Theme.of(context).textTheme.titleMedium?.copyWith(
                          fontWeight: FontWeight.w700,
                        ),
                  ),
                  const Spacer(),
                  IconButton(
                    onPressed: widget.controller.fetchThreadHistory,
                    icon: const Icon(Icons.refresh),
                    tooltip: 'Reload history',
                  ),
                  IconButton(
                    onPressed: () => Navigator.of(context).pop(),
                    icon: const Icon(Icons.close),
                    tooltip: 'Close',
                  ),
                ],
              ),
              const SizedBox(height: 8),
              TextField(
                controller: _searchController,
                onChanged: (value) => setState(() => _pattern = value),
                decoration: InputDecoration(
                  labelText: 'Search history (regular expression)',
                  prefixIcon: const Icon(Icons.search),
                  suffixIcon: _pattern.isEmpty
                      ? null
                      : IconButton(
                          onPressed: () {
                            _searchController.clear();
                            setState(() => _pattern = '');
                          },
                          icon: const Icon(Icons.close),
                          tooltip: 'Clear',
                        ),
                ),
              ),
              if (_pattern.isNotEmpty && regex == null) ...[
                const SizedBox(height: 6),
                Text(
                  'Invalid regular expression',
                  style: Theme.of(context).textTheme.bodySmall?.copyWith(
                        color: Theme.of(context).colorScheme.error,
                      ),
                ),
              ],
              if (error != null) ...[
                const SizedBox(height: 8),
                Text(
                  '$error',
                  style: Theme.of(context).textTheme.bodySmall?.copyWith(
                        color: Theme.of(context).colorScheme.error,
                      ),
                ),
              ],
              const SizedBox(height: 8),
              Expanded(
                child: ChatTimeline(
                  threadId: widget.threadId,
                  entries: filteredEntries,
                  title: widget.threadName,
                  contextWindowRemainingPercent:
                      widget.contextWindowRemainingPercent,
                  onSend: (_) {},
                  onInterrupt: () {},
                  composerEnabled: false,
                  isRunning: false,
                  showComposer: false,
                  headerControls: Row(
                    children: [
                      if (widget.controller.isThreadHistoryLoading)
                        const SizedBox(
                          width: 16,
                          height: 16,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        ),
                      if (widget.controller.isThreadHistoryLoading)
                        const SizedBox(width: 8),
                      Text(
                        '${filteredEntries.length} results',
                        style: Theme.of(context).textTheme.labelSmall,
                      ),
                    ],
                  ),
                ),
              ),
            ],
          );
        },
      ),
    );
  }
}

RegExp? _buildRegex(String pattern) {
  if (pattern.trim().isEmpty) {
    return null;
  }
  try {
    return RegExp(pattern, caseSensitive: false, multiLine: true);
  } catch (_) {
    return null;
  }
}

String _searchableText(ChatEntry entry) {
  return [
    entry.author,
    entry.displayLabel,
    entry.subtitle ?? '',
    entry.body,
    entry.command ?? '',
    entry.output ?? '',
    entry.status ?? '',
    entry.kind ?? '',
  ].join('\n');
}

class _ProjectDraft {
  const _ProjectDraft({
    required this.name,
    required this.rootPath,
    required this.defaultCwd,
  });

  final String name;
  final String rootPath;
  final String defaultCwd;
}

class _ThreadDraft {
  const _ThreadDraft({
    required this.projectId,
    required this.title,
    required this.initialPrompt,
    required this.role,
    required this.approvalPolicy,
    required this.sandboxMode,
    required this.networkAccessMode,
    required this.modelId,
    required this.reasoningEffort,
  });

  final String projectId;
  final String title;
  final String initialPrompt;
  final String role;
  final String approvalPolicy;
  final String sandboxMode;
  final String networkAccessMode;
  final String modelId;
  final String reasoningEffort;
}

class _AgentDraft {
  const _AgentDraft({
    required this.name,
    required this.role,
    required this.prompt,
  });

  final String name;
  final String role;
  final String prompt;
}
