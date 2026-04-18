import 'dart:async';
import 'dart:convert';
import 'dart:math' as math;
import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:http/http.dart' as http;
import 'package:shared_preferences/shared_preferences.dart';

import 'package:rinf/rinf.dart';

import '../bindings/bindings.dart';
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

class _RobdexWorkbenchState extends State<RobdexWorkbench>
    with SingleTickerProviderStateMixin {
  static const _hostPreferenceKey = 'bridge_host';
  static const _portPreferenceKey = 'bridge_port';

  late final WorkbenchController _controller;
  late final AppLifecycleListener _listener;
  late final AnimationController _spaceController;
  StreamSubscription<RustSignalPack<HookToastSignal>>? _hookToastSubscription;
  bool _didRequestConnect = false;
  late final TextEditingController _hostController;
  late final TextEditingController _portController;
  late final FocusNode _hostFocusNode;
  late final FocusNode _portFocusNode;

  @override
  void initState() {
    super.initState();
    _controller = WorkbenchController();
    _spaceController = AnimationController(
      vsync: this,
      duration: const Duration(days: 1),
    )..repeat();
    _hostController = TextEditingController(text: '127.0.0.1');
    _portController = TextEditingController(text: '42080');
    _hostFocusNode = FocusNode();
    _portFocusNode = FocusNode();
    _restoreBridgeSettings();
    _hookToastSubscription = HookToastSignal.rustSignalStream.listen((pack) {
      final signal = pack.message;
      if (!mounted) {
        return;
      }
      final messenger = ScaffoldMessenger.of(context);
      messenger.hideCurrentSnackBar();
      messenger.showSnackBar(
        SnackBar(
          content: Text(
            signal.detail.trim().isEmpty
                ? signal.message
                : '${signal.message}\n${signal.detail}',
          ),
          duration: Duration(milliseconds: signal.durationMs),
          action: SnackBarAction(
            label: 'Copy',
            onPressed: () {
              Clipboard.setData(ClipboardData(text: signal.copyText));
            },
          ),
        ),
      );
    });
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
    _spaceController.dispose();
    _hookToastSubscription?.cancel();
    _hostFocusNode.dispose();
    _portFocusNode.dispose();
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

  Uri get _bridgeBaseUri {
    final host = _hostController.text.trim().isEmpty
        ? '127.0.0.1'
        : _hostController.text.trim();
    final port = int.tryParse(_portController.text.trim()) ?? 42080;
    return Uri.parse('http://$host:$port');
  }

  Future<List<_HookLogEntry>> _fetchProjectHookLogs(String projectId) async {
    final response = await http.get(
      _bridgeBaseUri.resolve('/projects/$projectId/hook-logs'),
    );
    if (response.statusCode != 200) {
      throw StateError('Hook logs failed with ${response.statusCode}');
    }
    final payload = jsonDecode(response.body) as Map<String, dynamic>;
    final logs = payload['logs'];
    if (logs is! List) {
      return const <_HookLogEntry>[];
    }
    return logs
        .whereType<Map<String, dynamic>>()
        .map(_HookLogEntry.fromJson)
        .toList(growable: false);
  }

  Future<void> _clearProjectHookLogs(String projectId) async {
    final response = await http.delete(
      _bridgeBaseUri.resolve('/projects/$projectId/hook-logs'),
    );
    if (response.statusCode != 200) {
      throw StateError('Clear hook logs failed with ${response.statusCode}');
    }
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
        if (_controller.view == null) {
          final stage = _controller.error != null
              ? _ConnectionStage.error
              : _didRequestConnect
                  ? _ConnectionStage.connecting
                  : _ConnectionStage.idle;
          return _ConnectionScreen(
            animation: _spaceController,
            stage: stage,
            errorText: _controller.error?.toString(),
            hostController: _hostController,
            portController: _portController,
            hostFocusNode: _hostFocusNode,
            portFocusNode: _portFocusNode,
            onConnect: _attemptConnect,
            onReset: _returnToLogin,
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
          onCompactThread: _controller.compactThread,
          onTerminateCommandExecution: _controller.terminateCommandExecution,
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
            serviceTier: draft.serviceTier,
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
                        DropdownMenuItem(value: 'designer', child: Text('Designer')),
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
    String designerModelId = project.designerDefaultModel ?? '';
    String designerReasoningEffort = project.designerDefaultReasoningEffort ?? '';
    final orchestratorDeveloperInstructionsController = TextEditingController(
      text: project.orchestratorDeveloperInstructions ?? '',
    );
    final workerDeveloperInstructionsController = TextEditingController(
      text: project.workerDeveloperInstructions ?? '',
    );
    final qaDeveloperInstructionsController = TextEditingController(
      text: project.qaDeveloperInstructions ?? '',
    );
    final designerDeveloperInstructionsController = TextEditingController(
      text: project.designerDeveloperInstructions ?? '',
    );
    final operatorDeveloperInstructionsController = TextEditingController(
      text: project.operatorDeveloperInstructions ?? '',
    );
    final hiddenDeveloperInstructionsController = TextEditingController(
      text: project.hiddenDeveloperInstructions ?? '',
    );

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

        Widget developerInstructionsField(
          String label,
          TextEditingController controller,
        ) {
          return TextField(
            controller: controller,
            minLines: 2,
            maxLines: 5,
            decoration: InputDecoration(
              labelText: '$label Developer Instructions',
              alignLabelWithHint: true,
            ),
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
                      Align(
                        alignment: Alignment.centerLeft,
                        child: OutlinedButton.icon(
                          onPressed: () => _showProjectHookLogsSheet(project),
                          icon: const Icon(Icons.receipt_long_outlined),
                          label: const Text('Hook Logs'),
                        ),
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
                      const SizedBox(height: 8),
                      developerInstructionsField(
                        'Orchestrator',
                        orchestratorDeveloperInstructionsController,
                      ),
                      const SizedBox(height: 16),
                      Text('Worker Defaults',
                          style: Theme.of(context).textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.w700)),
                      const SizedBox(height: 8),
                      modelDropdown('Model', workerModelId,
                          (value) => setDialogState(() => workerModelId = value)),
                      const SizedBox(height: 8),
                      reasoningDropdown('Reasoning', workerReasoningEffort,
                          (value) => setDialogState(() => workerReasoningEffort = value)),
                      const SizedBox(height: 8),
                      developerInstructionsField(
                        'Worker',
                        workerDeveloperInstructionsController,
                      ),
                      const SizedBox(height: 16),
                      Text('QA Defaults',
                          style: Theme.of(context).textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.w700)),
                      const SizedBox(height: 8),
                      modelDropdown('Model', qaModelId,
                          (value) => setDialogState(() => qaModelId = value)),
                      const SizedBox(height: 8),
                      reasoningDropdown('Reasoning', qaReasoningEffort,
                          (value) => setDialogState(() => qaReasoningEffort = value)),
                      const SizedBox(height: 8),
                      developerInstructionsField(
                        'QA',
                        qaDeveloperInstructionsController,
                      ),
                      const SizedBox(height: 16),
                      Text('Designer Defaults',
                          style: Theme.of(context).textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.w700)),
                      const SizedBox(height: 8),
                      modelDropdown('Model', designerModelId,
                          (value) => setDialogState(() => designerModelId = value)),
                      const SizedBox(height: 8),
                      reasoningDropdown('Reasoning', designerReasoningEffort,
                          (value) => setDialogState(() => designerReasoningEffort = value)),
                      const SizedBox(height: 8),
                      developerInstructionsField(
                        'Designer',
                        designerDeveloperInstructionsController,
                      ),
                      const SizedBox(height: 16),
                      Text('Operator Defaults',
                          style: Theme.of(context).textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.w700)),
                      const SizedBox(height: 8),
                      developerInstructionsField(
                        'Operator',
                        operatorDeveloperInstructionsController,
                      ),
                      const SizedBox(height: 16),
                      Text('Hidden Defaults',
                          style: Theme.of(context).textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.w700)),
                      const SizedBox(height: 8),
                      developerInstructionsField(
                        'Hidden',
                        hiddenDeveloperInstructionsController,
                      ),
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
    orchestratorDeveloperInstructionsController.dispose();
    workerDeveloperInstructionsController.dispose();
    qaDeveloperInstructionsController.dispose();
    designerDeveloperInstructionsController.dispose();
    operatorDeveloperInstructionsController.dispose();
    hiddenDeveloperInstructionsController.dispose();

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
      designerModelId: designerModelId,
      designerReasoningEffort: designerReasoningEffort,
      orchestratorDeveloperInstructions:
          orchestratorDeveloperInstructionsController.text.trim(),
      workerDeveloperInstructions:
          workerDeveloperInstructionsController.text.trim(),
      qaDeveloperInstructions: qaDeveloperInstructionsController.text.trim(),
      designerDeveloperInstructions:
          designerDeveloperInstructionsController.text.trim(),
      operatorDeveloperInstructions:
          operatorDeveloperInstructionsController.text.trim(),
      hiddenDeveloperInstructions:
          hiddenDeveloperInstructionsController.text.trim(),
    );
  }

  Future<void> _showProjectHookLogsSheet(ProjectItem project) async {
    if (!mounted) {
      return;
    }
    await showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      builder: (context) {
        return FractionallySizedBox(
          heightFactor: 0.72,
          child: StatefulBuilder(
            builder: (context, setModalState) {
              Future<List<_HookLogEntry>> load() => _fetchProjectHookLogs(project.id);

              Future<void> clearLogs() async {
                await _clearProjectHookLogs(project.id);
                setModalState(() {});
              }

              return Padding(
                padding: const EdgeInsets.all(20),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      children: [
                        Expanded(
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text(
                                'Hook Logs',
                                style: Theme.of(context).textTheme.titleLarge,
                              ),
                              const SizedBox(height: 4),
                              Text(
                                project.name,
                                style: Theme.of(context).textTheme.bodySmall,
                              ),
                            ],
                          ),
                        ),
                        TextButton(
                          onPressed: () => Navigator.of(context).pop(),
                          child: const Text('Close'),
                        ),
                      ],
                    ),
                    const SizedBox(height: 16),
                    Expanded(
                      child: FutureBuilder<List<_HookLogEntry>>(
                        future: load(),
                        builder: (context, snapshot) {
                          if (snapshot.connectionState != ConnectionState.done) {
                            return const Center(child: CircularProgressIndicator());
                          }
                          if (snapshot.hasError) {
                            return Center(
                              child: Text(
                                'Failed to load hook logs: ${snapshot.error}',
                              ),
                            );
                          }
                          final logs = snapshot.data ?? const <_HookLogEntry>[];
                          return Column(
                            children: [
                              Align(
                                alignment: Alignment.centerRight,
                                child: TextButton.icon(
                                  onPressed: logs.isEmpty
                                      ? null
                                      : () async {
                                          await clearLogs();
                                        },
                                  icon: const Icon(Icons.delete_sweep_outlined),
                                  label: const Text('Clear Logs'),
                                ),
                              ),
                              const SizedBox(height: 8),
                              Expanded(
                                child: logs.isEmpty
                                    ? const Center(child: Text('No hook logs recorded.'))
                                    : ListView.separated(
                                        itemCount: logs.length,
                                        separatorBuilder: (_, _) =>
                                            const SizedBox(height: 10),
                                        itemBuilder: (context, index) {
                                          final log = logs[index];
                                          return Container(
                                            padding: const EdgeInsets.all(12),
                                            decoration: BoxDecoration(
                                              borderRadius: BorderRadius.circular(12),
                                              border: Border.all(
                                                color: Theme.of(context)
                                                    .colorScheme
                                                    .outlineVariant,
                                              ),
                                            ),
                                            child: Column(
                                              crossAxisAlignment:
                                                  CrossAxisAlignment.start,
                                              children: [
                                                Row(
                                                  children: [
                                                    Expanded(
                                                      child: Text(
                                                        log.event,
                                                        style: Theme.of(context)
                                                            .textTheme
                                                            .titleSmall,
                                                      ),
                                                    ),
                                                    Text(
                                                      log.status,
                                                      style: Theme.of(context)
                                                          .textTheme
                                                          .labelMedium,
                                                    ),
                                                  ],
                                                ),
                                                const SizedBox(height: 6),
                                                Text(
                                                  '${log.agentName} · ${log.role} · ${log.createdAtLabel}',
                                                  style: Theme.of(context)
                                                      .textTheme
                                                      .bodySmall,
                                                ),
                                                if (log.detail != null &&
                                                    log.detail!.trim().isNotEmpty) ...[
                                                  const SizedBox(height: 8),
                                                  SelectableText(
                                                    log.detail!,
                                                    style: Theme.of(context)
                                                        .textTheme
                                                        .bodySmall
                                                        ?.copyWith(
                                                          fontFamily: 'monospace',
                                                        ),
                                                  ),
                                                ],
                                              ],
                                            ),
                                          );
                                        },
                                      ),
                              ),
                            ],
                          );
                        },
                      ),
                    ),
                  ],
                ),
              );
            },
          ),
        );
      },
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

enum _ConnectionStage { idle, connecting, error }

class _ConnectionScreen extends StatelessWidget {
  const _ConnectionScreen({
    required this.animation,
    required this.stage,
    required this.errorText,
    required this.hostController,
    required this.portController,
    required this.hostFocusNode,
    required this.portFocusNode,
    required this.onConnect,
    required this.onReset,
  });

  final AnimationController animation;
  final _ConnectionStage stage;
  final String? errorText;
  final TextEditingController hostController;
  final TextEditingController portController;
  final FocusNode hostFocusNode;
  final FocusNode portFocusNode;
  final VoidCallback onConnect;
  final VoidCallback onReset;

  bool get _isBusy => stage == _ConnectionStage.connecting;
  bool get _isError => stage == _ConnectionStage.error;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final reduceMotion = MediaQuery.maybeOf(context)?.disableAnimations ??
        PlatformDispatcher.instance.accessibilityFeatures.disableAnimations;
    final panelBorder = _isError
        ? const Color(0xFFB86262)
        : _isBusy
            ? scheme.primary
            : const Color(0xFF5CA8FF);
    final panelGlow = _isError
        ? const Color(0xFFB86262)
        : _isBusy
            ? scheme.secondary
            : const Color(0xFF5B76FF);

    return Scaffold(
      body: Stack(
        fit: StackFit.expand,
        children: [
          RepaintBoundary(
            child: CustomPaint(
              painter: _StarfieldPainter(
                animation: animation,
                warp: _isBusy ? 1 : 0,
                reduceMotion: reduceMotion,
              ),
            ),
          ),
          IgnorePointer(
            child: DecoratedBox(
              decoration: BoxDecoration(
                gradient: RadialGradient(
                  center: const Alignment(0, -0.08),
                  radius: 0.72,
                  colors: [
                    panelGlow.withValues(alpha: _isBusy ? 0.14 : 0.08),
                    const Color(0xFF081018).withValues(alpha: 0.0),
                  ],
                ),
              ),
            ),
          ),
          SafeArea(
            child: Center(
              child: Padding(
                padding: const EdgeInsets.all(24),
                child: ConstrainedBox(
                  constraints: const BoxConstraints(maxWidth: 480),
                  child: TweenAnimationBuilder<double>(
                    duration: Duration(milliseconds: reduceMotion ? 0 : 650),
                    curve: Curves.easeOutCubic,
                    tween: Tween(begin: 0.92, end: 1),
                    builder: (context, scale, child) {
                      return Transform.scale(
                        scale: scale,
                        child: AnimatedOpacity(
                          duration:
                              Duration(milliseconds: reduceMotion ? 0 : 500),
                          curve: Curves.easeOut,
                          opacity: 1,
                          child: child,
                        ),
                      );
                    },
                    child: AnimatedContainer(
                      duration:
                          Duration(milliseconds: reduceMotion ? 0 : 320),
                      curve: Curves.easeOutCubic,
                      padding: const EdgeInsets.fromLTRB(22, 22, 22, 20),
                      decoration: BoxDecoration(
                        color: const Color(0xCC081019),
                        borderRadius: BorderRadius.circular(28),
                        border: Border.all(
                          color: panelBorder.withValues(
                            alpha: _isBusy ? 0.9 : 0.64,
                          ),
                        ),
                        boxShadow: [
                          BoxShadow(
                            color: panelGlow.withValues(
                              alpha: _isBusy ? 0.28 : 0.16,
                            ),
                            blurRadius: _isBusy ? 36 : 26,
                            spreadRadius: _isBusy ? 2 : 0,
                          ),
                          const BoxShadow(
                            color: Color(0x99000000),
                            blurRadius: 36,
                            offset: Offset(0, 20),
                          ),
                        ],
                        gradient: const LinearGradient(
                          begin: Alignment.topCenter,
                          end: Alignment.bottomCenter,
                          colors: [
                            Color(0xF0142030),
                            Color(0xEE09111A),
                          ],
                        ),
                      ),
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          Row(
                            children: [
                              _CoreBadge(
                                animation: animation,
                                isBusy: _isBusy,
                                isError: _isError,
                                reduceMotion: reduceMotion,
                              ),
                              const SizedBox(width: 14),
                              Expanded(
                                child: Column(
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: [
                                    Text(
                                      'Robdex',
                                      style: theme.textTheme.headlineMedium
                                          ?.copyWith(
                                        fontWeight: FontWeight.w800,
                                        letterSpacing: 0.6,
                                      ),
                                    ),
                                    const SizedBox(height: 4),
                                    Text(
                                      '${hostController.text.trim().isEmpty ? '127.0.0.1' : hostController.text.trim()}:${portController.text.trim().isEmpty ? '42080' : portController.text.trim()}',
                                      style: theme.textTheme.labelMedium
                                          ?.copyWith(
                                        color: scheme.secondary
                                            .withValues(alpha: 0.92),
                                        letterSpacing: 0.9,
                                      ),
                                    ),
                                  ],
                                ),
                              ),
                              if (_isError)
                                IconButton(
                                  onPressed: onReset,
                                  tooltip: 'Reset',
                                  icon: const Icon(Icons.arrow_back_rounded),
                                ),
                            ],
                          ),
                          const SizedBox(height: 22),
                          LayoutBuilder(
                            builder: (context, constraints) {
                              final horizontal = constraints.maxWidth >= 420;
                              if (!horizontal) {
                                return Column(
                                  children: [
                                    TextField(
                                      controller: hostController,
                                      focusNode: hostFocusNode,
                                      enabled: !_isBusy,
                                      textInputAction: TextInputAction.next,
                                      decoration:
                                          const InputDecoration(labelText: 'Host'),
                                      onSubmitted: (_) =>
                                          portFocusNode.requestFocus(),
                                    ),
                                    const SizedBox(height: 10),
                                    TextField(
                                      controller: portController,
                                      focusNode: portFocusNode,
                                      enabled: !_isBusy,
                                      keyboardType: TextInputType.number,
                                      textInputAction: TextInputAction.done,
                                      decoration:
                                          const InputDecoration(labelText: 'Port'),
                                      onSubmitted: (_) => onConnect(),
                                    ),
                                  ],
                                );
                              }
                              return Row(
                                children: [
                                  Expanded(
                                    flex: 3,
                                    child: TextField(
                                      controller: hostController,
                                      focusNode: hostFocusNode,
                                      enabled: !_isBusy,
                                      textInputAction: TextInputAction.next,
                                      decoration:
                                          const InputDecoration(labelText: 'Host'),
                                      onSubmitted: (_) =>
                                          portFocusNode.requestFocus(),
                                    ),
                                  ),
                                  const SizedBox(width: 10),
                                  Expanded(
                                    child: TextField(
                                      controller: portController,
                                      focusNode: portFocusNode,
                                      enabled: !_isBusy,
                                      keyboardType: TextInputType.number,
                                      textInputAction: TextInputAction.done,
                                      decoration:
                                          const InputDecoration(labelText: 'Port'),
                                      onSubmitted: (_) => onConnect(),
                                    ),
                                  ),
                                ],
                              );
                            },
                          ),
                          const SizedBox(height: 14),
                          AnimatedSwitcher(
                            duration:
                                Duration(milliseconds: reduceMotion ? 0 : 220),
                            child: _isError && errorText != null
                                ? Padding(
                                    key: const ValueKey('error'),
                                    padding: const EdgeInsets.only(bottom: 12),
                                    child: Row(
                                      crossAxisAlignment:
                                          CrossAxisAlignment.start,
                                      children: [
                                        const Padding(
                                          padding: EdgeInsets.only(top: 1),
                                          child: Icon(
                                            Icons.priority_high_rounded,
                                            size: 16,
                                            color: Color(0xFFFF8B7A),
                                          ),
                                        ),
                                        const SizedBox(width: 8),
                                        Expanded(
                                          child: Text(
                                            errorText!,
                                            style: theme.textTheme.bodySmall
                                                ?.copyWith(
                                              color: const Color(0xFFFFB0A6),
                                              height: 1.35,
                                            ),
                                          ),
                                        ),
                                      ],
                                    ),
                                  )
                                : const SizedBox.shrink(key: ValueKey('empty')),
                          ),
                          SizedBox(
                            height: 44,
                            child: FilledButton(
                              onPressed: _isBusy ? null : onConnect,
                              style: FilledButton.styleFrom(
                                backgroundColor: _isError
                                    ? const Color(0xFFB86262)
                                    : scheme.primary,
                                foregroundColor: Colors.black,
                              ),
                              child: Row(
                                mainAxisAlignment: MainAxisAlignment.center,
                                children: [
                                  AnimatedSwitcher(
                                    duration: Duration(
                                      milliseconds: reduceMotion ? 0 : 180,
                                    ),
                                    child: _isBusy
                                        ? SizedBox(
                                            key: const ValueKey('progress'),
                                            width: 16,
                                            height: 16,
                                            child: CircularProgressIndicator(
                                              strokeWidth: 2.1,
                                              valueColor:
                                                  const AlwaysStoppedAnimation(
                                                Colors.black,
                                              ),
                                            ),
                                          )
                                        : Icon(
                                            key: ValueKey(
                                              _isError ? 'retry' : 'connect',
                                            ),
                                            _isError
                                                ? Icons.refresh_rounded
                                                : Icons.north_east_rounded,
                                            size: 18,
                                          ),
                                  ),
                                  const SizedBox(width: 10),
                                  Text(_isError ? 'Retry' : 'Connect'),
                                ],
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _CoreBadge extends StatelessWidget {
  const _CoreBadge({
    required this.animation,
    required this.isBusy,
    required this.isError,
    required this.reduceMotion,
  });

  final AnimationController animation;
  final bool isBusy;
  final bool isError;
  final bool reduceMotion;

  @override
  Widget build(BuildContext context) {
    final color = isError
        ? const Color(0xFFFF8B7A)
        : isBusy
            ? Theme.of(context).colorScheme.primary
            : Theme.of(context).colorScheme.secondary;

    return SizedBox(
      width: 52,
      height: 52,
      child: AnimatedBuilder(
        animation: animation,
        builder: (context, child) {
          final turns = reduceMotion
              ? 0.0
              : (isBusy ? animation.value * 0.16 : animation.value * 0.05);
          return DecoratedBox(
            decoration: BoxDecoration(
              shape: BoxShape.circle,
              boxShadow: [
                BoxShadow(
                  color: color.withValues(alpha: isBusy ? 0.36 : 0.2),
                  blurRadius: isBusy ? 22 : 14,
                ),
              ],
              gradient: RadialGradient(
                colors: [
                  color.withValues(alpha: 0.34),
                  const Color(0xFF0B1725),
                ],
              ),
              border: Border.all(
                color: color.withValues(alpha: 0.76),
              ),
            ),
            child: Stack(
              alignment: Alignment.center,
              children: [
                Transform.rotate(
                  angle: turns * math.pi * 2,
                  child: Container(
                    width: 36,
                    height: 36,
                    decoration: BoxDecoration(
                      shape: BoxShape.circle,
                      border: Border.all(
                        color: color.withValues(alpha: 0.5),
                      ),
                    ),
                  ),
                ),
                Icon(
                  isError ? Icons.priority_high_rounded : Icons.auto_awesome,
                  size: 20,
                  color: color,
                ),
              ],
            ),
          );
        },
      ),
    );
  }
}

class _StarfieldPainter extends CustomPainter {
  const _StarfieldPainter({
    required this.animation,
    required this.warp,
    required this.reduceMotion,
  }) : super(repaint: animation);

  final AnimationController animation;
  final double warp;
  final bool reduceMotion;

  @override
  void paint(Canvas canvas, Size size) {
    final elapsedSeconds = reduceMotion
        ? 0.0
        : (animation.lastElapsedDuration?.inMilliseconds ?? 0) / 1000.0;
    final rect = Offset.zero & size;
    final background = Paint()
      ..shader = const LinearGradient(
        begin: Alignment.topCenter,
        end: Alignment.bottomCenter,
        colors: [
          Color(0xFF04070B),
          Color(0xFF08111B),
          Color(0xFF03060A),
        ],
      ).createShader(rect);
    canvas.drawRect(rect, background);

    final center = Offset(size.width / 2, size.height * 0.45);
    final glowPaint = Paint()
      ..shader = RadialGradient(
        colors: [
          const Color(0xFF305C9B).withValues(alpha: 0.14 + (0.05 * warp)),
          const Color(0xFF0A1320).withValues(alpha: 0.04),
          const Color(0xFF000000).withValues(alpha: 0),
        ],
      ).createShader(
        Rect.fromCircle(
          center: center,
          radius: math.max(size.width, size.height) * 0.52,
        ),
      );
    canvas.drawCircle(
      center,
      math.max(size.width, size.height) * 0.52,
      glowPaint,
    );

    _paintNebula(canvas, size, elapsedSeconds);

    _paintLayer(
      canvas,
      size,
      elapsedSeconds: elapsedSeconds,
      count: 70,
      cycleSeconds: 8.5 - (2.0 * warp),
      radiusScale: 0.55,
      color: const Color(0x55FFFFFF),
      trailScale: 10 + (22 * warp),
    );
    _paintLayer(
      canvas,
      size,
      elapsedSeconds: elapsedSeconds,
      count: 44,
      cycleSeconds: 6.5 - (1.6 * warp),
      radiusScale: 0.82,
      color: const Color(0x88A7D8FF),
      trailScale: 18 + (42 * warp),
    );
    _paintLayer(
      canvas,
      size,
      elapsedSeconds: elapsedSeconds,
      count: 22,
      cycleSeconds: 5.2 - (1.2 * warp),
      radiusScale: 1.1,
      color: const Color(0xCCFFFFFF),
      trailScale: 32 + (66 * warp),
    );
  }

  void _paintNebula(Canvas canvas, Size size, double elapsedSeconds) {
    final t = reduceMotion ? 0.0 : elapsedSeconds;
    final focus = Offset(size.width * 0.5, size.height * 0.46);
    final maxDimension = math.max(size.width, size.height);
    final layerMask = Paint()
      ..shader = RadialGradient(
        center: const Alignment(0, -0.04),
        radius: 1.05,
        colors: [
          const Color(0xFF000000).withValues(alpha: 0.0),
          const Color(0xFF000000).withValues(alpha: 0.0),
          const Color(0x4A000000),
          const Color(0xD2000000),
        ],
        stops: const [0.0, 0.18, 0.84, 1.0],
      ).createShader(Offset.zero & size);

    canvas.saveLayer(Offset.zero & size, Paint());

    for (var i = 0; i < 3; i++) {
      final phase = ((t / 18.0) + (i / 3)) % 1.0;
      final zoom = math.pow(2.45, phase).toDouble();
      final fade = math.sin(phase * math.pi);
      _paintNebulaZoomLayer(
        canvas,
        size,
        center: focus,
        scale: zoom,
        alpha: 0.16 + (fade * 0.18),
        primary: [
          const Color(0xFF5E7BFF),
          const Color(0xFF36C7FF),
          const Color(0xFFC061FF),
        ][i],
        secondary: [
          const Color(0xFF8E4DFF),
          const Color(0xFF7B3BFF),
          const Color(0xFFFF5E8E),
        ][i],
        tertiary: [
          const Color(0xFF24379B),
          const Color(0xFF1D4FA4),
          const Color(0xFF5B1F78),
        ][i],
        seed: 1400 + (i * 311),
        offset: Offset(
          (i - 1) * maxDimension * 0.085,
          (i == 2 ? 1 : -1) * maxDimension * 0.05,
        ),
      );
    }

    canvas.drawRect(
      Offset.zero & size,
      layerMask..blendMode = BlendMode.dstIn,
    );
    canvas.restore();
  }

  void _paintNebulaZoomLayer(
    Canvas canvas,
    Size size, {
    required Offset center,
    required double scale,
    required double alpha,
    required Color primary,
    required Color secondary,
    required Color tertiary,
    required int seed,
    required Offset offset,
  }) {
    final rootCenter = center + offset;
    final baseRadius = math.max(size.width, size.height) * 0.14 * scale;

    for (var depth = 0; depth < 4; depth++) {
      final localScale = math.pow(1.6, depth).toDouble();
      final radius = baseRadius * localScale;
      final orbit = radius * (0.22 + (0.05 * _hash(seed + (depth * 41))));
      final angle = (_hash(seed * 17 + depth * 13) * math.pi * 2) +
          (depth.isEven ? 0.5 : -0.35);
      final localCenter = rootCenter.translate(
        math.cos(angle) * orbit,
        math.sin(angle) * orbit * 0.8,
      );
      final path = _buildNebulaContour(
        localCenter,
        radius,
        seed + (depth * 97),
        yScale: 0.82 + (_hash(seed + depth * 9) * 0.18),
      );
      final shaderBounds = Rect.fromCircle(
        center: localCenter,
        radius: radius * 1.05,
      );
      final paint = Paint()
        ..blendMode = BlendMode.plus
        ..shader = RadialGradient(
          colors: [
            primary.withValues(alpha: alpha * (1.0 - (depth * 0.12))),
            secondary.withValues(alpha: alpha * (0.84 - (depth * 0.08))),
            tertiary.withValues(alpha: alpha * (0.54 - (depth * 0.06))),
            const Color(0x00000000),
          ],
          stops: const [0.0, 0.32, 0.72, 1.0],
        ).createShader(shaderBounds);
      canvas.drawPath(path, paint);

      final innerPath = _buildNebulaContour(
        localCenter.translate(radius * 0.08, -radius * 0.04),
        radius * 0.52,
        seed + (depth * 131),
        yScale: 0.76,
      );
      final innerPaint = Paint()
        ..blendMode = BlendMode.plus
        ..color = primary.withValues(
          alpha: alpha * 0.42 * (1 - (depth * 0.12)),
        );
      canvas.drawPath(innerPath, innerPaint);

      final innerAccentPath = _buildNebulaContour(
        localCenter.translate(-radius * 0.06, radius * 0.03),
        radius * 0.32,
        seed + (depth * 173),
        yScale: 0.7,
      );
      final innerAccentPaint = Paint()
        ..blendMode = BlendMode.plus
        ..color = secondary.withValues(
          alpha: alpha * 0.34 * (1 - (depth * 0.12)),
        );
      canvas.drawPath(innerAccentPath, innerAccentPaint);

      final contourPaint = Paint()
        ..blendMode = BlendMode.screen
        ..style = PaintingStyle.stroke
        ..strokeWidth = math.max(1.0, radius * 0.006)
        ..color = primary.withValues(
          alpha: alpha * 0.48 * (1 - (depth * 0.14)),
        );
      canvas.drawPath(path, contourPaint);

      final contourPaintSecondary = Paint()
        ..blendMode = BlendMode.screen
        ..style = PaintingStyle.stroke
        ..strokeWidth = math.max(0.7, radius * 0.0035)
        ..color = tertiary.withValues(
          alpha: alpha * 0.36 * (1 - (depth * 0.14)),
        );
      canvas.drawPath(innerPath, contourPaintSecondary);

      final contourPaintAccent = Paint()
        ..blendMode = BlendMode.screen
        ..style = PaintingStyle.stroke
        ..strokeWidth = math.max(0.55, radius * 0.0028)
        ..color = secondary.withValues(
          alpha: alpha * 0.26 * (1 - (depth * 0.16)),
        );
      canvas.drawPath(innerAccentPath, contourPaintAccent);
    }
  }

  Path _buildNebulaContour(
    Offset center,
    double radius,
    int seed, {
    double yScale = 1,
  }) {
    final path = Path();
    const segments = 120;
    for (var i = 0; i <= segments; i++) {
      final angle = (i / segments) * math.pi * 2;
      final wobbleA = math.sin((angle * 3) + (_hash(seed * 3) * math.pi * 2));
      final wobbleB =
          math.sin((angle * 5) - (_hash(seed * 5) * math.pi * 2)) * 0.42;
      final wobbleC =
          math.cos((angle * 8) + (_hash(seed * 7) * math.pi * 2)) * 0.18;
      final contour = 1 + (wobbleA * 0.18) + (wobbleB * 0.14) + (wobbleC * 0.1);
      final point = Offset(
        center.dx + (math.cos(angle) * radius * contour),
        center.dy + (math.sin(angle) * radius * contour * yScale),
      );
      if (i == 0) {
        path.moveTo(point.dx, point.dy);
      } else {
        path.lineTo(point.dx, point.dy);
      }
    }
    path.close();
    return path;
  }

  void _paintLayer(
    Canvas canvas,
    Size size, {
    required double elapsedSeconds,
    required int count,
    required double cycleSeconds,
    required double radiusScale,
    required Color color,
    required double trailScale,
  }) {
    final center = Offset(size.width / 2, size.height * 0.45);
    final maxRadius =
        math.sqrt(size.width * size.width + size.height * size.height) *
            radiusScale;
    final paint = Paint()
      ..strokeCap = StrokeCap.round
      ..color = color;
    final glowPaint = Paint()..blendMode = BlendMode.plus;
    final corePaint = Paint()..blendMode = BlendMode.plus;

    for (var i = 0; i < count; i++) {
      final seed = i + (radiusScale * 1000).round();
      final angle = _hash(seed * 37) * math.pi * 2;
      final spread = 0.58 + (_hash(seed * 17) * 0.72);
      final phase = _hash(seed * 53);
      final lifeProgress = (phase + (elapsedSeconds / cycleSeconds)) % 1.0;
      final eased = math.pow(lifeProgress, 2.35).toDouble();
      final radius = eased * maxRadius;
      final vector = Offset(
        math.cos(angle) * spread,
        math.sin(angle),
      );
      final normalized = vector / vector.distance;
      final point = center + (normalized * radius);

      if (!(-80 <= point.dx &&
          point.dx <= size.width + 80 &&
          -80 <= point.dy &&
          point.dy <= size.height + 80)) {
        continue;
      }

      final tail = normalized * (trailScale * lifeProgress);
      final brightness = 0.2 + (lifeProgress * 0.8);
      paint
        ..strokeWidth = 0.6 + (lifeProgress * 2.1)
        ..color = color.withValues(alpha: 0.1 + (lifeProgress * 0.82));
      canvas.drawLine(point - tail, point, paint);

      final glowRadius = (0.9 + (lifeProgress * 3.4)) * radiusScale;
      glowPaint.shader = RadialGradient(
        colors: [
          color.withValues(alpha: 0.34 * brightness),
          color.withValues(alpha: 0.12 * brightness),
          color.withValues(alpha: 0),
        ],
        stops: const [0.0, 0.42, 1.0],
      ).createShader(
        Rect.fromCircle(center: point, radius: glowRadius * 2.6),
      );
      canvas.drawCircle(point, glowRadius * 2.6, glowPaint);

      corePaint.color = Colors.white.withValues(
        alpha: 0.42 + (lifeProgress * 0.56),
      );
      canvas.drawCircle(
        point,
        (0.3 + (lifeProgress * 1.15)) * radiusScale,
        corePaint,
      );
    }
  }

  double _hash(int value) {
    final raw = math.sin(value * 12.9898) * 43758.5453;
    return raw - raw.floorToDouble();
  }

  @override
  bool shouldRepaint(covariant _StarfieldPainter oldDelegate) {
    return oldDelegate.warp != warp ||
        oldDelegate.reduceMotion != reduceMotion ||
        oldDelegate.animation != animation;
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

class _HookLogEntry {
  const _HookLogEntry({
    required this.createdAt,
    required this.agentName,
    required this.role,
    required this.event,
    required this.status,
    required this.detail,
  });

  final int createdAt;
  final String agentName;
  final String role;
  final String event;
  final String status;
  final String? detail;

  String get createdAtLabel {
    if (createdAt <= 0) {
      return 'now';
    }
    final dateTime = DateTime.fromMillisecondsSinceEpoch(createdAt * 1000);
    final month = dateTime.month.toString().padLeft(2, '0');
    final day = dateTime.day.toString().padLeft(2, '0');
    final hour = dateTime.hour.toString().padLeft(2, '0');
    final minute = dateTime.minute.toString().padLeft(2, '0');
    return '$month/$day $hour:$minute';
  }

  factory _HookLogEntry.fromJson(Map<String, dynamic> json) {
    final createdAtValue = json['createdAt'];
    final createdAt = switch (createdAtValue) {
      int value => value,
      double value => value.floor(),
      String value => int.tryParse(value) ?? 0,
      _ => 0,
    };
    return _HookLogEntry(
      createdAt: createdAt,
      agentName: (json['agentName'] as String?) ?? 'Unknown Agent',
      role: (json['role'] as String?) ?? 'unknown',
      event: (json['event'] as String?) ?? 'hook',
      status: (json['status'] as String?) ?? 'unknown',
      detail: json['detail'] as String?,
    );
  }
}
