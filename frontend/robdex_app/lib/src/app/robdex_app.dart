import 'dart:async';
import 'dart:convert';
import 'dart:math' as math;
import 'dart:ui';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:rinf/rinf.dart';
import 'package:robdex_design_system/robdex_design_system.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../agent_runtime/agent_runtime_control_tower_host.dart';
import '../bindings/bindings.dart';
import '../core/state/workbench_controller.dart';
import '../ide_host_bridge/ide_host_bridge.dart' as ide_host_bridge;
import '../terminal/integrated_terminal.dart';
import '../web/dom_mirror/dom_mirror.dart';

const smartRadius = 2.0;

class RobdexApp extends StatelessWidget {
  const RobdexApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      debugShowCheckedModeBanner: false,
      title: 'Robdex',
      theme: buildRobdexTheme(),
      home: const RobdexHome(),
    );
  }
}

class RobdexHome extends StatefulWidget {
  const RobdexHome({super.key});

  @override
  State<RobdexHome> createState() => _RobdexHomeState();
}

class _RobdexHomeState extends State<RobdexHome> {
  int _index = 0;

  @override
  Widget build(BuildContext context) {
    return RobdexModeShell(
      selectedIndex: _index,
      onDestinationSelected: (index) => setState(() => _index = index),
      destinations: const [
        RobdexModeDestination(
          icon: Icons.forum_outlined,
          selectedIcon: Icons.forum,
          label: 'Workbench',
        ),
        RobdexModeDestination(
          icon: Icons.radar_outlined,
          selectedIcon: Icons.radar,
          label: 'Agent Runtime',
        ),
      ],
      children: const [
        RobdexWorkbench(),
        AgentRuntimeControlTowerHost(),
      ],
    );
  }
}

enum _ProjectSettingsTab {
  project,
  orchestrator,
  worker,
  qa,
  designer,
  planner,
  requirementsReviewer,
  hidden,
  operator,
}

enum _ProjectSettingsResult { cancel, save, delete }

class ProjectRoleModelSettingsPane extends StatelessWidget {
  const ProjectRoleModelSettingsPane({
    super.key,
    required this.roleKey,
    required this.availableModels,
    required this.modelId,
    required this.reasoningEffort,
    required this.onModelChanged,
    required this.onReasoningChanged,
  });

  final String roleKey;
  final List<ModelItem> availableModels;
  final String modelId;
  final String reasoningEffort;
  final ValueChanged<String> onModelChanged;
  final ValueChanged<String> onReasoningChanged;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Expanded(
          child: DropdownButtonFormField<String>(
            key: ValueKey('project.settings.$roleKey.model'),
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
                        model.name?.trim().isNotEmpty == true
                            ? model.name!
                            : model.id,
                      ),
                    ),
                  ),
            ],
            onChanged: (value) => onModelChanged(value ?? ''),
          ),
        ),
        const SizedBox(width: 14),
        Expanded(
          child: DropdownButtonFormField<String>(
            key: ValueKey('project.settings.$roleKey.reasoning'),
            initialValue: reasoningEffort,
            decoration: const InputDecoration(labelText: 'Reasoning'),
            items: [
              DropdownMenuItem(value: '', child: Text('Default')),
              DropdownMenuItem(value: 'low', child: Text('Low')),
              DropdownMenuItem(value: 'medium', child: Text('Medium')),
              DropdownMenuItem(value: 'high', child: Text('High')),
            ],
            onChanged: (value) => onReasoningChanged(value ?? ''),
          ),
        ),
      ],
    );
  }
}

class ProjectDefaultRuntimeSettingsPane extends StatelessWidget {
  const ProjectDefaultRuntimeSettingsPane({
    super.key,
    required this.sandboxMode,
    required this.approvalPolicy,
    required this.networkAccessMode,
    this.settingsKeyPrefix = 'project',
    this.inheritedSandboxMode,
    this.inheritedApprovalPolicy,
    this.inheritedNetworkAccess,
    required this.onSandboxModeChanged,
    required this.onApprovalPolicyChanged,
    required this.onNetworkAccessModeChanged,
  });

  final String sandboxMode;
  final String approvalPolicy;
  final String networkAccessMode;
  final String settingsKeyPrefix;
  final String? inheritedSandboxMode;
  final String? inheritedApprovalPolicy;
  final bool? inheritedNetworkAccess;
  final ValueChanged<String> onSandboxModeChanged;
  final ValueChanged<String> onApprovalPolicyChanged;
  final ValueChanged<String> onNetworkAccessModeChanged;

  @override
  Widget build(BuildContext context) {
    return Wrap(
      spacing: 12,
      runSpacing: 12,
      children: [
        SizedBox(
          width: 270,
          child: DropdownButtonFormField<String>(
            key: ValueKey('project.settings.$settingsKeyPrefix.sandbox'),
            isExpanded: true,
            initialValue: sandboxMode,
            decoration: const InputDecoration(labelText: 'Default sandbox'),
            items: [
              DropdownMenuItem(
                value: '',
                child: Text(_inheritedLabel('Default', inheritedSandboxMode)),
              ),
              DropdownMenuItem(value: 'read-only', child: Text('Read-only')),
              DropdownMenuItem(
                value: 'workspace-write',
                child: Text('Workspace'),
              ),
              DropdownMenuItem(
                value: 'danger-full-access',
                child: Text('Danger'),
              ),
              DropdownMenuItem(
                value: 'external-sandbox',
                child: Text('External'),
              ),
            ],
            onChanged: (value) => onSandboxModeChanged(value ?? ''),
          ),
        ),
        SizedBox(
          width: 270,
          child: DropdownButtonFormField<String>(
            key: ValueKey('project.settings.$settingsKeyPrefix.approval'),
            isExpanded: true,
            initialValue: approvalPolicy,
            decoration: const InputDecoration(labelText: 'Default approval'),
            items: [
              DropdownMenuItem(
                value: '',
                child: Text(
                  _inheritedLabel('Default', inheritedApprovalPolicy),
                ),
              ),
              DropdownMenuItem(value: 'untrusted', child: Text('untrusted')),
              DropdownMenuItem(value: 'on-failure', child: Text('on-failure')),
              DropdownMenuItem(value: 'on-request', child: Text('on-request')),
              DropdownMenuItem(value: 'never', child: Text('never')),
            ],
            onChanged: (value) => onApprovalPolicyChanged(value ?? ''),
          ),
        ),
        SizedBox(
          width: 270,
          child: DropdownButtonFormField<String>(
            key: ValueKey('project.settings.$settingsKeyPrefix.network'),
            isExpanded: true,
            initialValue: networkAccessMode,
            decoration: const InputDecoration(labelText: 'Default network'),
            items: [
              DropdownMenuItem(
                value: 'default',
                child: Text(
                  _inheritedLabel(
                    'Default',
                    _networkAccessLabel(inheritedNetworkAccess),
                  ),
                ),
              ),
              DropdownMenuItem(value: 'enabled', child: Text('Enabled')),
              DropdownMenuItem(value: 'disabled', child: Text('Disabled')),
            ],
            onChanged: (value) =>
                onNetworkAccessModeChanged(value ?? 'default'),
          ),
        ),
      ],
    );
  }
}

String _inheritedLabel(String fallback, String? inherited) {
  final value = inherited?.trim();
  if (value == null || value.isEmpty) {
    return fallback;
  }
  return '$fallback ($value)';
}

String? _networkAccessLabel(bool? value) {
  return value == null ? null : (value ? 'enabled' : 'disabled');
}

class ProjectRequirementComposable {
  const ProjectRequirementComposable({
    required this.id,
    required this.title,
    required this.description,
    required this.scope,
    required this.requirementCount,
    required this.requirements,
  });

  final String id;
  final String title;
  final String description;
  final String scope;
  final int requirementCount;
  final List<Map<String, dynamic>> requirements;

  factory ProjectRequirementComposable.fromJson(Map<String, dynamic> json) {
    final requirements = (json['requirements'] as List<dynamic>? ?? const [])
        .whereType<Map<String, dynamic>>()
        .map((item) => Map<String, dynamic>.from(item))
        .toList(growable: false);
    return ProjectRequirementComposable(
      id: json['id'] as String? ?? '',
      title: json['title'] as String? ?? '',
      description: json['description'] as String? ?? '',
      scope: json['scope'] as String? ?? '',
      requirementCount: json['requirementCount'] as int? ?? requirements.length,
      requirements: requirements,
    );
  }
}

class ProjectPermanentComposablesPane extends StatelessWidget {
  const ProjectPermanentComposablesPane({
    super.key,
    required this.composables,
    required this.selectedIds,
    required this.onChanged,
    this.error,
  });

  final List<ProjectRequirementComposable> composables;
  final List<String> selectedIds;
  final ValueChanged<List<String>> onChanged;
  final String? error;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final selected = selectedIds.toSet();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text('Permanent composables', style: theme.textTheme.titleSmall),
        const SizedBox(height: 6),
        Text(
          'Server-enforced Requirements composables that are always merged for this project.',
          style: theme.textTheme.bodySmall?.copyWith(
            color: theme.colorScheme.onSurface.withValues(alpha: 0.68),
          ),
        ),
        if (error != null && error!.isNotEmpty) ...[
          const SizedBox(height: 8),
          Text(
            error!,
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.error,
            ),
          ),
        ] else if (composables.isEmpty) ...[
          const SizedBox(height: 8),
          Text(
            'No composables detected for this project context.',
            style: theme.textTheme.bodySmall,
          ),
        ] else ...[
          const SizedBox(height: 8),
          for (final composable in composables)
            SwitchListTile(
              key: ValueKey('project.permanentComposable.${composable.id}'),
              contentPadding: EdgeInsets.zero,
              value: selected.contains(composable.id),
              title: Text(
                composable.title.isEmpty ? composable.id : composable.title,
              ),
              subtitle: Text(
                '${composable.id} | ${composable.scope.isEmpty ? 'unknown' : composable.scope} | ${composable.requirementCount} requirements'
                '${composable.description.isEmpty ? '' : '\n${composable.description}'}',
              ),
              secondary: Tooltip(
                message: 'Inspect composable',
                child: IconButton(
                  key: ValueKey(
                    'project.permanentComposable.inspect.${composable.id}',
                  ),
                  icon: const Icon(Icons.info_outline),
                  onPressed: () =>
                      _showProjectComposableDetails(context, composable),
                ),
              ),
              onChanged: (enabled) {
                final next = selectedIds.toList(growable: true);
                if (enabled) {
                  if (!next.contains(composable.id)) {
                    next.add(composable.id);
                  }
                } else {
                  next.removeWhere((id) => id == composable.id);
                }
                onChanged(List.unmodifiable(next));
              },
            ),
        ],
      ],
    );
  }

  void _showProjectComposableDetails(
    BuildContext context,
    ProjectRequirementComposable composable,
  ) {
    showDialog<void>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(
          composable.title.isEmpty ? composable.id : composable.title,
        ),
        content: SizedBox(
          width: 560,
          child: SingleChildScrollView(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  composable.description.isEmpty
                      ? composable.id
                      : composable.description,
                ),
                const SizedBox(height: 12),
                for (final requirement in composable.requirements) ...[
                  Text(
                    requirement['key'] as String? ?? '',
                    style: Theme.of(context).textTheme.labelLarge,
                  ),
                  Text(requirement['statement'] as String? ?? ''),
                  const SizedBox(height: 10),
                ],
              ],
            ),
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: const Text('Close'),
          ),
        ],
      ),
    );
  }
}

class ProjectManifestRunsPane extends StatelessWidget {
  const ProjectManifestRunsPane({super.key, required this.project});

  final ProjectItem project;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final runs = project.manifestRuns;
    return Column(
      key: const ValueKey('project.manifestRuns'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text('Manifest runs', style: theme.textTheme.titleSmall),
        const SizedBox(height: 6),
        Text(
          'Serial manifest lifecycle state. Use `robdex manifest activate/status/advance/cancel` for lifecycle actions.',
          style: theme.textTheme.bodySmall?.copyWith(
            color: theme.colorScheme.onSurface.withValues(alpha: 0.68),
          ),
        ),
        const SizedBox(height: 8),
        if (runs.isEmpty)
          Text(
            'No manifest runs for this project.',
            style: theme.textTheme.bodySmall,
          )
        else
          for (final run in runs) ...[
            DecoratedBox(
              decoration: BoxDecoration(
                border: Border(
                  left: BorderSide(
                    color: theme.colorScheme.primary.withValues(alpha: 0.5),
                    width: 3,
                  ),
                ),
              ),
              child: Padding(
                padding: const EdgeInsets.only(left: 10, top: 4, bottom: 8),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    Text(
                      '${run.title.isEmpty ? run.planId : run.title} | ${run.status}',
                      style: theme.textTheme.labelLarge?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    Text(
                      'run ${run.runId} | current ${run.currentPhaseId ?? '-'} | ${run.sourceHash}',
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: theme.colorScheme.onSurface.withValues(
                          alpha: 0.62,
                        ),
                      ),
                    ),
                    const SizedBox(height: 6),
                    for (final phase in run.phases)
                      Padding(
                        padding: const EdgeInsets.symmetric(vertical: 2),
                        child: Text(
                          '${phase.phaseId}: ${phase.status}'
                          ' | worker ${phase.workerThreadId ?? '-'}'
                          ' | handoff ${phase.hasHandoff ? 'yes' : 'no'}'
                          ' | blocker ${phase.hasBlocker ? 'yes' : 'no'}'
                          ' | waiver ${phase.hasWaiver ? 'yes' : 'no'}'
                          ' | resume ${phase.hasResumeDecision ? 'yes' : 'no'}'
                          ' | cleanup ${phase.archiveCleanupState.isEmpty ? '-' : phase.archiveCleanupState}',
                          style: theme.textTheme.bodySmall,
                        ),
                      ),
                  ],
                ),
              ),
            ),
            const SizedBox(height: 8),
          ],
      ],
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
  static const _graphicsEnabledPreferenceKey = 'graphics_enabled';

  late final WorkbenchController _controller;
  late final AppLifecycleListener _listener;
  late final AnimationController _spaceController;
  late final Future<FragmentProgram?> _nebulaProgramFuture;
  late final Future<FragmentProgram?> _peripheralProgramFuture;
  late final DomMirrorController _domMirrorController;
  late final IntegratedTerminalController _terminalController;
  StreamSubscription<RustSignalPack<HookToastSignal>>? _hookToastSubscription;
  bool _didRequestConnect = false;
  late final TextEditingController _hostController;
  late final TextEditingController _portController;
  late final FocusNode _hostFocusNode;
  late final FocusNode _portFocusNode;
  bool _graphicsEnabled = !kIsWeb;
  String? _lastShownControllerError;

  @override
  void initState() {
    super.initState();
    _controller = WorkbenchController();
    _spaceController = AnimationController(
      vsync: this,
      duration: const Duration(days: 1),
    )..repeat();
    _nebulaProgramFuture = _loadNebulaProgram();
    _peripheralProgramFuture = _loadPeripheralProgram();
    _domMirrorController = DomMirrorController();
    _terminalController = IntegratedTerminalController();
    _hostController = TextEditingController(text: '127.0.0.1');
    _portController = TextEditingController(text: '42080');
    _hostFocusNode = FocusNode();
    _portFocusNode = FocusNode();
    _controller.addListener(_showControllerErrorIfNeeded);
    if (kIsWeb) {
      _connectToSameOriginBridge();
    } else {
      _restoreBridgeSettings();
    }
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
        _terminalController.closeAll();
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
    _domMirrorController.dispose();
    _terminalController.dispose();
    _hookToastSubscription?.cancel();
    _controller.removeListener(_showControllerErrorIfNeeded);
    _hostFocusNode.dispose();
    _portFocusNode.dispose();
    _hostController.dispose();
    _portController.dispose();
    _controller.dispose();
    super.dispose();
  }

  void _showControllerErrorIfNeeded() {
    final error = _controller.error?.toString().trim();
    if (error == null || error.isEmpty || error == _lastShownControllerError) {
      return;
    }
    _lastShownControllerError = error;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) {
        return;
      }
      final messenger = ScaffoldMessenger.of(context);
      messenger.hideCurrentSnackBar();
      messenger.showSnackBar(SnackBar(content: Text(error)));
    });
  }

  Future<void> _restoreBridgeSettings() async {
    if (kIsWeb) {
      return;
    }
    final prefs = await SharedPreferences.getInstance();
    final host = prefs.getString(_hostPreferenceKey);
    final port = prefs.getInt(_portPreferenceKey);
    final graphicsEnabled = prefs.getBool(_graphicsEnabledPreferenceKey);
    if (!mounted) {
      return;
    }
    if ((host?.trim().isNotEmpty ?? false) ||
        port != null ||
        graphicsEnabled != null) {
      setState(() {
        if (host?.trim().isNotEmpty ?? false) {
          _hostController.text = host!.trim();
        }
        if (port != null && port > 0) {
          _portController.text = port.toString();
        }
        if (graphicsEnabled != null) {
          _graphicsEnabled = graphicsEnabled;
        }
      });
    }
  }

  Future<FragmentProgram?> _loadNebulaProgram() async {
    if (kIsWeb) {
      return null;
    }
    try {
      return await FragmentProgram.fromAsset('shaders/connection_nebula.frag');
    } catch (_) {
      return null;
    }
  }

  Future<FragmentProgram?> _loadPeripheralProgram() async {
    if (kIsWeb) {
      return null;
    }
    try {
      return await FragmentProgram.fromAsset(
        'shaders/peripheral_vision_filter.frag',
      );
    } catch (_) {
      return null;
    }
  }

  Future<void> _persistBridgeSettings() async {
    if (kIsWeb) {
      return;
    }
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
    await prefs.setBool(_graphicsEnabledPreferenceKey, _graphicsEnabled);
  }

  Future<void> _setGraphicsEnabled(bool value) async {
    if (kIsWeb) {
      return;
    }
    if (_graphicsEnabled == value) {
      return;
    }
    setState(() {
      _graphicsEnabled = value;
    });
    final prefs = await SharedPreferences.getInstance();
    await prefs.setBool(_graphicsEnabledPreferenceKey, value);
  }

  void _connectToSameOriginBridge() {
    final uri = _configuredWebBridgeBaseUri ?? Uri.base;
    final host = uri.host.isEmpty ? '127.0.0.1' : uri.host;
    final port = uri.hasPort
        ? uri.port
        : uri.scheme == 'https'
        ? 443
        : 80;
    _hostController.text = host;
    _portController.text = port.toString();
    _graphicsEnabled = false;
    _didRequestConnect = true;
    _controller.start(host: host, port: port);
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
    _controller.start(host: _hostController.text.trim(), port: port);
  }

  Uri? get _configuredWebBridgeBaseUri {
    if (!kIsWeb) {
      return null;
    }
    return ide_host_bridge.configuredBridgeBaseUri();
  }

  Future<List<_HookLogEntry>> _fetchProjectHookLogs(String projectId) async {
    final logs = await _controller.loadProjectHookLogs(projectId);
    return logs.map(_HookLogEntry.fromJson).toList(growable: false);
  }

  Future<void> _clearProjectHookLogs(String projectId) =>
      _controller.clearProjectHookLogs(projectId);

  void _returnToLogin() {
    _terminalController.closeAll();
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
          _domMirrorController.clear();
          if (kIsWeb) {
            return _WebConnectionScreen(
              errorText: _controller.error?.toString(),
              onRetry: _connectToSameOriginBridge,
            );
          }
          final stage = _controller.error != null
              ? _ConnectionStage.error
              : _didRequestConnect
              ? _ConnectionStage.connecting
              : _ConnectionStage.idle;
          return _ConnectionScreen(
            animation: _spaceController,
            nebulaProgramFuture: _nebulaProgramFuture,
            peripheralProgramFuture: _peripheralProgramFuture,
            graphicsEnabled: _graphicsEnabled,
            stage: stage,
            errorText: _controller.error?.toString(),
            hostController: _hostController,
            portController: _portController,
            hostFocusNode: _hostFocusNode,
            portFocusNode: _portFocusNode,
            onConnect: _attemptConnect,
            onReset: _returnToLogin,
            onGraphicsEnabledChanged: _setGraphicsEnabled,
          );
        }
        _domMirrorController.update(_controller.view);
        return RobdexShellScreen(
          enableGraphics: _graphicsEnabled,
          workbench: _controller.view!,
          onThreadSelected: _controller.selectThread,
          onProjectSelected: _controller.selectProject,
          onDisconnect: () {
            _returnToLogin();
          },
          onGlobalSettings: _showGlobalSettingsDialog,
          onCreateProject: _showCreateProjectDialog,
          onProjectSettings: _showProjectSettingsDialog,
          onCreateThread: _showCreateThreadDialog,
          onSpawnAgent: _showSpawnAgentDialog,
          onSendMessage: (submission) => _controller.sendMessage(
            submission.text,
            localImagePaths: submission.localImagePaths,
            requirementSetJson: submission.requirementSetJson,
          ),
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
          loadThreadStats: _controller.loadThreadStats,
          loadPeriodStats: _controller.loadPeriodStats,
          loadRequirementComposables: _controller.loadRequirementComposables,
          setThreadRequirements: (threadId, requirementSetJson) =>
              _controller.setThreadRequirements(
                recipientThreadId: threadId,
                projectPath: _controller.view?.selection.projectRootPath,
                requirementSetJson: requirementSetJson,
              ),
          uploadImageBytes: _controller.uploadImageBytes,
          loadFullSizeImage: _controller.loadFullSizeImage,
          onOpenLink: ide_host_bridge.openMentionedFile,
          chatBottomDrawer: IntegratedTerminalDrawer(
            controller: _terminalController,
            host: _bridgeHost,
          ),
          terminalAvailable: _terminalController.isAvailable,
          onTerminalPressed: _terminalController.toggleDrawer,
        );
      },
    );
  }

  String get _bridgeHost {
    final host = _hostController.text.trim();
    return host.isEmpty ? '127.0.0.1' : host;
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
    String requirementSetJson = '';
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
                        DropdownMenuItem(
                          value: 'worker',
                          child: Text('Worker'),
                        ),
                        DropdownMenuItem(
                          value: 'designer',
                          child: Text('Designer'),
                        ),
                        DropdownMenuItem(
                          value: 'planner',
                          child: Text('Planner'),
                        ),
                        DropdownMenuItem(value: 'qa', child: Text('QA')),
                        DropdownMenuItem(
                          value: 'operator',
                          child: Text('Operator'),
                        ),
                        DropdownMenuItem(
                          value: 'orchestrator',
                          child: Text('Orchestrator'),
                        ),
                        DropdownMenuItem(
                          value: 'hidden',
                          child: Text('Hidden'),
                        ),
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
                      decoration: const InputDecoration(
                        labelText: 'Initial prompt',
                      ),
                    ),
                    const SizedBox(height: 12),
                    OutlinedButton.icon(
                      onPressed: () async {
                        final next = await showRequirementSetFormDialog(
                          context,
                          initialJson: requirementSetJson,
                          title: 'Thread Requirements',
                          actionLabel: 'Attach',
                          helperText:
                              'These requirements are attached before the first turn starts.',
                          projectPath: project.rootPath,
                          loadComposableItems: _controller.loadRequirementComposables,
                          uploadImageBytes: _controller.uploadImageBytes,
                        );
                        if (next == null) {
                          return;
                        }
                        setDialogState(() {
                          requirementSetJson = next;
                        });
                      },
                      icon: Icon(
                        requirementSetJson.trim().isEmpty
                            ? Icons.rule_outlined
                            : Icons.rule_rounded,
                      ),
                      label: Text(
                        requirementSetJson.trim().isEmpty
                            ? 'Add requirements'
                            : 'Requirements attached',
                      ),
                    ),
                    const SizedBox(height: 12),
                    DropdownButtonFormField<String>(
                      initialValue: modelId,
                      decoration: const InputDecoration(labelText: 'Model'),
                      items: [
                        const DropdownMenuItem(
                          value: '',
                          child: Text('Default'),
                        ),
                        ...availableModels
                            .where((model) => !model.hidden)
                            .map(
                              (model) => DropdownMenuItem(
                                value: model.id,
                                child: Text(
                                  model.name?.trim().isNotEmpty == true
                                      ? model.name!
                                      : model.id,
                                ),
                              ),
                            ),
                      ],
                      onChanged: (value) =>
                          setDialogState(() => modelId = value ?? ''),
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
                            decoration: const InputDecoration(
                              labelText: 'Reasoning',
                            ),
                            items: const [
                              DropdownMenuItem(
                                value: '',
                                child: Text('Default'),
                              ),
                              DropdownMenuItem(
                                value: 'low',
                                child: Text('Low'),
                              ),
                              DropdownMenuItem(
                                value: 'medium',
                                child: Text('Medium'),
                              ),
                              DropdownMenuItem(
                                value: 'high',
                                child: Text('High'),
                              ),
                            ],
                            onChanged: (value) => setDialogState(
                              () => reasoningEffort = value ?? '',
                            ),
                          ),
                        ),
                        SizedBox(
                          width: 180,
                          child: DropdownButtonFormField<String>(
                            initialValue: sandboxMode,
                            decoration: const InputDecoration(
                              labelText: 'Sandbox',
                            ),
                            items: const [
                              DropdownMenuItem(
                                value: '',
                                child: Text('Default'),
                              ),
                              DropdownMenuItem(
                                value: 'workspace-write',
                                child: Text('Workspace'),
                              ),
                              DropdownMenuItem(
                                value: 'danger-full-access',
                                child: Text('Danger'),
                              ),
                            ],
                            onChanged: (value) =>
                                setDialogState(() => sandboxMode = value ?? ''),
                          ),
                        ),
                        SizedBox(
                          width: 180,
                          child: DropdownButtonFormField<String>(
                            initialValue: networkAccessMode,
                            decoration: const InputDecoration(
                              labelText: 'Network',
                            ),
                            items: const [
                              DropdownMenuItem(
                                value: 'default',
                                child: Text('Default'),
                              ),
                              DropdownMenuItem(
                                value: 'enabled',
                                child: Text('Enabled'),
                              ),
                              DropdownMenuItem(
                                value: 'disabled',
                                child: Text('Disabled'),
                              ),
                            ],
                            onChanged: (value) => setDialogState(
                              () => networkAccessMode = value ?? 'default',
                            ),
                          ),
                        ),
                        SizedBox(
                          width: 180,
                          child: DropdownButtonFormField<String>(
                            initialValue: approvalPolicy,
                            decoration: const InputDecoration(
                              labelText: 'Approval',
                            ),
                            items: const [
                              DropdownMenuItem(
                                value: '',
                                child: Text('Default'),
                              ),
                              DropdownMenuItem(
                                value: 'untrusted',
                                child: Text('untrusted'),
                              ),
                              DropdownMenuItem(
                                value: 'on-failure',
                                child: Text('on-failure'),
                              ),
                              DropdownMenuItem(
                                value: 'on-request',
                                child: Text('on-request'),
                              ),
                              DropdownMenuItem(
                                value: 'never',
                                child: Text('never'),
                              ),
                            ],
                            onChanged: (value) => setDialogState(
                              () => approvalPolicy = value ?? '',
                            ),
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
                        requirementSetJson: requirementSetJson,
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
      requirementSetJson: result.requirementSetJson,
    );
  }

  Future<void> _showGlobalSettingsDialog() async {
    final settings = _controller.view?.globalSettings;
    String approvalPolicy = settings?.approvalPolicy ?? '';
    String sandboxMode = settings?.sandboxMode ?? '';
    String networkAccessMode = settings?.networkAccess == null
        ? 'default'
        : (settings!.networkAccess! ? 'enabled' : 'disabled');
    final result = await showDialog<bool>(
      context: context,
      builder: (context) {
        return StatefulBuilder(
          builder: (context, setDialogState) {
            return AlertDialog(
              title: const Text('Global Settings'),
              content: SizedBox(
                width: 420,
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      'These defaults apply when a project or role does not override them.',
                      style: Theme.of(context).textTheme.bodySmall,
                    ),
                    const SizedBox(height: 16),
                    ProjectDefaultRuntimeSettingsPane(
                      settingsKeyPrefix: 'global',
                      sandboxMode: sandboxMode,
                      approvalPolicy: approvalPolicy,
                      networkAccessMode: networkAccessMode,
                      onSandboxModeChanged: (value) =>
                          setDialogState(() => sandboxMode = value),
                      onApprovalPolicyChanged: (value) =>
                          setDialogState(() => approvalPolicy = value),
                      onNetworkAccessModeChanged: (value) =>
                          setDialogState(() => networkAccessMode = value),
                    ),
                  ],
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
    if (result != true) {
      return;
    }
    _controller.updateGlobalSettings(
      approvalPolicy: approvalPolicy,
      sandboxMode: sandboxMode,
      networkAccessMode: networkAccessMode,
    );
  }

  Future<void> _showProjectSettingsDialog(ProjectItem project) async {
    final availableModels =
        _controller.view?.availableModels ?? const <ModelItem>[];
    var permanentRequirementComposables = project
        .permanentRequirementComposables
        .toList(growable: true);
    List<ProjectRequirementComposable> projectRequirementComposables =
        const <ProjectRequirementComposable>[];
    String? projectRequirementComposablesError;
    try {
      projectRequirementComposables = await _fetchProjectRequirementComposables(
        project,
      );
    } catch (error) {
      projectRequirementComposablesError = error.toString().replaceFirst(
        'Bad state: ',
        '',
      );
    }
    if (!mounted) {
      return;
    }
    final nameController = TextEditingController(text: project.name);
    final cwdController = TextEditingController(text: project.defaultCwd);
    _ProjectSettingsTab activeTab = _ProjectSettingsTab.project;
    bool autoRouteReplies = project.autoRouteReplies;
    bool routeApprovalRequests = project.routeApprovalRequests;
    String preferredModelProvider = project.preferredModelProvider ?? '';
    String defaultModelId = project.defaultModel ?? '';
    String defaultReasoningEffort = project.defaultReasoningEffort ?? '';
    String defaultSandboxMode = project.defaultSandboxMode ?? '';
    String defaultApprovalPolicy = project.defaultApprovalPolicy ?? '';
    String defaultNetworkAccessMode = project.defaultNetworkAccess == null
        ? 'default'
        : (project.defaultNetworkAccess! ? 'enabled' : 'disabled');
    final roleRuntimeDefaults = Map<String, RoleRuntimeDefaults>.from(
      project.roleRuntimeDefaults,
    );
    String orchestratorModelId = project.orchestratorDefaultModel ?? '';
    String orchestratorReasoningEffort =
        project.orchestratorDefaultReasoningEffort ?? '';
    String workerModelId = project.workerDefaultModel ?? '';
    String workerReasoningEffort = project.workerDefaultReasoningEffort ?? '';
    String qaModelId = project.qaDefaultModel ?? '';
    String qaReasoningEffort = project.qaDefaultReasoningEffort ?? '';
    String designerModelId = project.designerDefaultModel ?? '';
    String designerReasoningEffort =
        project.designerDefaultReasoningEffort ?? '';
    String plannerModelId = project.plannerDefaultModel ?? '';
    String plannerReasoningEffort = project.plannerDefaultReasoningEffort ?? '';
    String requirementsReviewerModelId =
        project.requirementsReviewerDefaultModel ?? '';
    String requirementsReviewerReasoningEffort =
        project.requirementsReviewerDefaultReasoningEffort ?? '';
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

    final result = await showDialog<_ProjectSettingsResult>(
      context: context,
      builder: (context) {
        final theme = Theme.of(context);

        Widget developerInstructionsField(TextEditingController controller) {
          return TextField(
            controller: controller,
            minLines: 8,
            maxLines: 14,
            decoration: const InputDecoration(
              labelText: 'Instructions',
              alignLabelWithHint: true,
            ),
          );
        }

        ({IconData icon, String tooltip, Color color}) tabVisuals(
          _ProjectSettingsTab tab,
        ) {
          return switch (tab) {
            _ProjectSettingsTab.project => (
              icon: Icons.workspaces_outlined,
              tooltip: 'Project',
              color: theme.colorScheme.primary,
            ),
            _ProjectSettingsTab.orchestrator => (
              icon: Icons.account_tree_outlined,
              tooltip: 'Orchestrator',
              color: theme.colorScheme.secondary,
            ),
            _ProjectSettingsTab.worker => (
              icon: Icons.build_circle_outlined,
              tooltip: 'Worker',
              color: theme.colorScheme.onSurface.withValues(alpha: 0.72),
            ),
            _ProjectSettingsTab.qa => (
              icon: Icons.fact_check_outlined,
              tooltip: 'QA',
              color: theme.colorScheme.tertiary,
            ),
            _ProjectSettingsTab.designer => (
              icon: Icons.palette_outlined,
              tooltip: 'Designer',
              color: Colors.amber.shade700,
            ),
            _ProjectSettingsTab.planner => (
              icon: Icons.psychology_alt_outlined,
              tooltip: 'Planner',
              color: theme.colorScheme.primary,
            ),
            _ProjectSettingsTab.requirementsReviewer => (
              icon: Icons.rule_folder_outlined,
              tooltip: 'Requirements Reviewer',
              color: theme.colorScheme.error,
            ),
            _ProjectSettingsTab.hidden => (
              icon: Icons.visibility_off_outlined,
              tooltip: 'Hidden',
              color: theme.colorScheme.outline,
            ),
            _ProjectSettingsTab.operator => (
              icon: Icons.verified_user_outlined,
              tooltip: 'Operator',
              color: theme.colorScheme.primary,
            ),
          };
        }

        Widget tabButton(
          _ProjectSettingsTab tab,
          void Function(VoidCallback fn) setDialogState,
        ) {
          final visuals = tabVisuals(tab);
          final selected = activeTab == tab;
          return Tooltip(
            message: visuals.tooltip,
            child: InkWell(
              borderRadius: BorderRadius.circular(smartRadius),
              onTap: () => setDialogState(() => activeTab = tab),
              child: AnimatedContainer(
                duration: const Duration(milliseconds: 180),
                curve: Curves.easeOutCubic,
                width: 44,
                height: 44,
                decoration: BoxDecoration(
                  borderRadius: BorderRadius.circular(smartRadius),
                  color: selected
                      ? visuals.color.withValues(alpha: 0.18)
                      : theme.colorScheme.surfaceContainerHighest.withValues(
                          alpha: 0.42,
                        ),
                  border: Border.all(
                    color: selected
                        ? visuals.color.withValues(alpha: 0.55)
                        : theme.colorScheme.outline.withValues(alpha: 0.18),
                  ),
                ),
                child: Icon(
                  visuals.icon,
                  size: 19,
                  color: selected
                      ? visuals.color
                      : theme.colorScheme.onSurface.withValues(alpha: 0.78),
                ),
              ),
            ),
          );
        }

        Widget paneShell({
          required Color accent,
          required List<Widget> children,
        }) {
          return AnimatedContainer(
            duration: const Duration(milliseconds: 220),
            curve: Curves.easeOutCubic,
            padding: const EdgeInsets.fromLTRB(18, 18, 18, 20),
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(smartRadius),
              color: theme.colorScheme.surface.withValues(alpha: 0.78),
              border: Border.all(color: accent.withValues(alpha: 0.28)),
              gradient: LinearGradient(
                begin: Alignment.topLeft,
                end: Alignment.bottomRight,
                colors: [
                  accent.withValues(alpha: 0.08),
                  theme.colorScheme.surface.withValues(alpha: 0.0),
                ],
              ),
              boxShadow: [
                BoxShadow(
                  color: Colors.black.withValues(alpha: 0.16),
                  blurRadius: 28,
                  offset: const Offset(0, 16),
                ),
              ],
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: children,
            ),
          );
        }

        Widget rootPathRow() {
          return Container(
            padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(smartRadius),
              color: theme.colorScheme.surfaceContainerHighest.withValues(
                alpha: 0.36,
              ),
              border: Border.all(
                color: theme.colorScheme.outline.withValues(alpha: 0.18),
              ),
            ),
            child: Row(
              children: [
                Icon(
                  Icons.folder_open_outlined,
                  size: 18,
                  color: theme.colorScheme.onSurface.withValues(alpha: 0.74),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: SelectableText(
                    project.rootPath,
                    maxLines: 2,
                    style: theme.textTheme.bodySmall?.copyWith(
                      fontFamily: 'monospace',
                      color: theme.colorScheme.onSurface.withValues(
                        alpha: 0.82,
                      ),
                    ),
                  ),
                ),
                const SizedBox(width: 8),
                Tooltip(
                  message: 'Copy root path',
                  child: IconButton(
                    onPressed: () {
                      Clipboard.setData(ClipboardData(text: project.rootPath));
                    },
                    visualDensity: VisualDensity.compact,
                    icon: const Icon(Icons.content_copy_outlined, size: 16),
                  ),
                ),
              ],
            ),
          );
        }

        Widget projectPane(void Function(VoidCallback fn) setDialogState) {
          final accent = tabVisuals(_ProjectSettingsTab.project).color;
          return paneShell(
            accent: accent,
            children: [
              Row(
                children: [
                  Expanded(
                    child: TextField(
                      controller: nameController,
                      decoration: const InputDecoration(labelText: 'Project'),
                    ),
                  ),
                  const SizedBox(width: 14),
                  Expanded(
                    child: TextField(
                      controller: cwdController,
                      decoration: const InputDecoration(
                        labelText: 'Default CWD',
                      ),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 14),
              rootPathRow(),
              const SizedBox(height: 14),
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Expanded(
                    child: DropdownButtonFormField<String>(
                      initialValue: preferredModelProvider,
                      decoration: const InputDecoration(labelText: 'Provider'),
                      items: const [
                        DropdownMenuItem(value: '', child: Text('Default')),
                        DropdownMenuItem(
                          value: 'openai',
                          child: Text('OpenAI'),
                        ),
                      ],
                      onChanged: (value) => setDialogState(
                        () => preferredModelProvider = value ?? '',
                      ),
                    ),
                  ),
                  const SizedBox(width: 12),
                  Tooltip(
                    message: 'Hook logs',
                    child: IconButton.filledTonal(
                      onPressed: () => _showProjectHookLogsSheet(project),
                      icon: const Icon(Icons.receipt_long_outlined),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 16),
              ProjectRoleModelSettingsPane(
                roleKey: 'project',
                availableModels: availableModels,
                modelId: defaultModelId,
                reasoningEffort: defaultReasoningEffort,
                onModelChanged: (value) =>
                    setDialogState(() => defaultModelId = value),
                onReasoningChanged: (value) =>
                    setDialogState(() => defaultReasoningEffort = value),
              ),
              const SizedBox(height: 14),
              ProjectDefaultRuntimeSettingsPane(
                settingsKeyPrefix: 'project',
                sandboxMode: defaultSandboxMode,
                approvalPolicy: defaultApprovalPolicy,
                networkAccessMode: defaultNetworkAccessMode,
                inheritedSandboxMode: project.globalDefaultSandboxMode,
                inheritedApprovalPolicy: project.globalDefaultApprovalPolicy,
                inheritedNetworkAccess: project.globalDefaultNetworkAccess,
                onSandboxModeChanged: (value) =>
                    setDialogState(() => defaultSandboxMode = value),
                onApprovalPolicyChanged: (value) =>
                    setDialogState(() => defaultApprovalPolicy = value),
                onNetworkAccessModeChanged: (value) =>
                    setDialogState(() => defaultNetworkAccessMode = value),
              ),
              const SizedBox(height: 16),
              SwitchListTile(
                value: autoRouteReplies,
                onChanged: (value) =>
                    setDialogState(() => autoRouteReplies = value),
                title: const Text('Auto-route replies'),
                contentPadding: EdgeInsets.zero,
              ),
              SwitchListTile(
                value: routeApprovalRequests,
                onChanged: (value) =>
                    setDialogState(() => routeApprovalRequests = value),
                title: const Text('Route approvals'),
                contentPadding: EdgeInsets.zero,
              ),
              const SizedBox(height: 16),
              ProjectPermanentComposablesPane(
                composables: projectRequirementComposables,
                selectedIds: permanentRequirementComposables,
                error: projectRequirementComposablesError,
                onChanged: (next) => setDialogState(
                  () => permanentRequirementComposables = next.toList(
                    growable: true,
                  ),
                ),
              ),
              const SizedBox(height: 16),
              ProjectManifestRunsPane(project: project),
            ],
          );
        }

        Widget rolePane({
          required _ProjectSettingsTab tab,
          required String roleKey,
          required String? modelId,
          required ValueChanged<String> onModelChanged,
          required String? reasoningEffort,
          required ValueChanged<String> onReasoningChanged,
          required TextEditingController? instructionsController,
          required bool supportsModelSettings,
          bool supportsDeveloperInstructions = true,
          bool supportsRuntimeSettings = true,
          required void Function(VoidCallback fn) setDialogState,
        }) {
          final accent = tabVisuals(tab).color;
          final runtimeDefaults =
              roleRuntimeDefaults[roleKey] ?? const RoleRuntimeDefaults();
          final networkAccessMode = runtimeDefaults.networkAccess == null
              ? 'default'
              : (runtimeDefaults.networkAccess! ? 'enabled' : 'disabled');
          void updateRuntimeDefaults({
            String? approvalPolicy,
            String? sandboxMode,
            String? networkAccessMode,
          }) {
            final nextApproval =
                approvalPolicy ?? runtimeDefaults.approvalPolicy ?? '';
            final nextSandbox =
                sandboxMode ?? runtimeDefaults.sandboxMode ?? '';
            final nextNetworkMode =
                networkAccessMode ??
                (runtimeDefaults.networkAccess == null
                    ? 'default'
                    : (runtimeDefaults.networkAccess!
                          ? 'enabled'
                          : 'disabled'));
            final nextNetwork = switch (nextNetworkMode) {
              'enabled' => true,
              'disabled' => false,
              _ => null,
            };
            if (nextApproval.isEmpty &&
                nextSandbox.isEmpty &&
                nextNetwork == null) {
              roleRuntimeDefaults.remove(roleKey);
            } else {
              roleRuntimeDefaults[roleKey] = RoleRuntimeDefaults(
                approvalPolicy: nextApproval.isEmpty ? null : nextApproval,
                sandboxMode: nextSandbox.isEmpty ? null : nextSandbox,
                networkAccess: nextNetwork,
              );
            }
          }

          return paneShell(
            accent: accent,
            children: [
              if (supportsModelSettings) ...[
                ProjectRoleModelSettingsPane(
                  roleKey: roleKey,
                  availableModels: availableModels,
                  modelId: modelId ?? '',
                  reasoningEffort: reasoningEffort ?? '',
                  onModelChanged: (value) =>
                      setDialogState(() => onModelChanged(value)),
                  onReasoningChanged: (value) =>
                      setDialogState(() => onReasoningChanged(value)),
                ),
                const SizedBox(height: 16),
              ],
              if (supportsRuntimeSettings) ...[
                ProjectDefaultRuntimeSettingsPane(
                  settingsKeyPrefix: roleKey,
                  sandboxMode: runtimeDefaults.sandboxMode ?? '',
                  approvalPolicy: runtimeDefaults.approvalPolicy ?? '',
                  networkAccessMode: networkAccessMode,
                  inheritedSandboxMode: defaultSandboxMode.trim().isNotEmpty
                      ? defaultSandboxMode
                      : project.globalDefaultSandboxMode,
                  inheritedApprovalPolicy:
                      defaultApprovalPolicy.trim().isNotEmpty
                      ? defaultApprovalPolicy
                      : project.globalDefaultApprovalPolicy,
                  inheritedNetworkAccess: defaultNetworkAccessMode == 'enabled'
                      ? true
                      : defaultNetworkAccessMode == 'disabled'
                      ? false
                      : project.globalDefaultNetworkAccess,
                  onSandboxModeChanged: (value) => setDialogState(
                    () => updateRuntimeDefaults(sandboxMode: value),
                  ),
                  onApprovalPolicyChanged: (value) => setDialogState(
                    () => updateRuntimeDefaults(approvalPolicy: value),
                  ),
                  onNetworkAccessModeChanged: (value) => setDialogState(
                    () => updateRuntimeDefaults(networkAccessMode: value),
                  ),
                ),
                const SizedBox(height: 16),
              ],
              if (supportsDeveloperInstructions &&
                  instructionsController != null)
                developerInstructionsField(instructionsController),
            ],
          );
        }

        return StatefulBuilder(
          builder: (context, setDialogState) {
            final tabs = _ProjectSettingsTab.values;
            final activeAccent = tabVisuals(activeTab).color;
            final activePane = switch (activeTab) {
              _ProjectSettingsTab.project => projectPane(setDialogState),
              _ProjectSettingsTab.orchestrator => rolePane(
                tab: activeTab,
                roleKey: 'orchestrator',
                modelId: orchestratorModelId,
                onModelChanged: (value) => orchestratorModelId = value,
                reasoningEffort: orchestratorReasoningEffort,
                onReasoningChanged: (value) =>
                    orchestratorReasoningEffort = value,
                instructionsController:
                    orchestratorDeveloperInstructionsController,
                supportsModelSettings: true,
                setDialogState: setDialogState,
              ),
              _ProjectSettingsTab.worker => rolePane(
                tab: activeTab,
                roleKey: 'worker',
                modelId: workerModelId,
                onModelChanged: (value) => workerModelId = value,
                reasoningEffort: workerReasoningEffort,
                onReasoningChanged: (value) => workerReasoningEffort = value,
                instructionsController: workerDeveloperInstructionsController,
                supportsModelSettings: true,
                setDialogState: setDialogState,
              ),
              _ProjectSettingsTab.qa => rolePane(
                tab: activeTab,
                roleKey: 'qa',
                modelId: qaModelId,
                onModelChanged: (value) => qaModelId = value,
                reasoningEffort: qaReasoningEffort,
                onReasoningChanged: (value) => qaReasoningEffort = value,
                instructionsController: qaDeveloperInstructionsController,
                supportsModelSettings: true,
                setDialogState: setDialogState,
              ),
              _ProjectSettingsTab.designer => rolePane(
                tab: activeTab,
                roleKey: 'designer',
                modelId: designerModelId,
                onModelChanged: (value) => designerModelId = value,
                reasoningEffort: designerReasoningEffort,
                onReasoningChanged: (value) => designerReasoningEffort = value,
                instructionsController: designerDeveloperInstructionsController,
                supportsModelSettings: true,
                setDialogState: setDialogState,
              ),
              _ProjectSettingsTab.planner => rolePane(
                tab: activeTab,
                roleKey: 'planner',
                modelId: plannerModelId,
                onModelChanged: (value) => plannerModelId = value,
                reasoningEffort: plannerReasoningEffort,
                onReasoningChanged: (value) => plannerReasoningEffort = value,
                instructionsController: null,
                supportsModelSettings: true,
                supportsDeveloperInstructions: false,
                setDialogState: setDialogState,
              ),
              _ProjectSettingsTab.requirementsReviewer => rolePane(
                tab: activeTab,
                roleKey: 'requirements-reviewer',
                modelId: requirementsReviewerModelId,
                onModelChanged: (value) => requirementsReviewerModelId = value,
                reasoningEffort: requirementsReviewerReasoningEffort,
                onReasoningChanged: (value) =>
                    requirementsReviewerReasoningEffort = value,
                instructionsController: null,
                supportsModelSettings: true,
                supportsDeveloperInstructions: false,
                setDialogState: setDialogState,
              ),
              _ProjectSettingsTab.hidden => rolePane(
                tab: activeTab,
                roleKey: 'hidden',
                modelId: null,
                onModelChanged: (_) {},
                reasoningEffort: null,
                onReasoningChanged: (_) {},
                instructionsController: hiddenDeveloperInstructionsController,
                supportsModelSettings: false,
                setDialogState: setDialogState,
              ),
              _ProjectSettingsTab.operator => rolePane(
                tab: activeTab,
                roleKey: 'operator',
                modelId: null,
                onModelChanged: (_) {},
                reasoningEffort: null,
                onReasoningChanged: (_) {},
                instructionsController: operatorDeveloperInstructionsController,
                supportsModelSettings: false,
                setDialogState: setDialogState,
              ),
            };

            return AlertDialog(
              insetPadding: const EdgeInsets.symmetric(
                horizontal: 28,
                vertical: 24,
              ),
              titlePadding: const EdgeInsets.fromLTRB(24, 22, 24, 0),
              contentPadding: const EdgeInsets.fromLTRB(24, 18, 24, 8),
              actionsPadding: const EdgeInsets.fromLTRB(24, 0, 24, 18),
              title: Row(
                children: [
                  Icon(
                    Icons.workspaces_outlined,
                    size: 20,
                    color: activeAccent.withValues(alpha: 0.88),
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Text(
                      project.name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                ],
              ),
              content: SizedBox(
                width: 700,
                height: 560,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    DecoratedBox(
                      decoration: BoxDecoration(
                        borderRadius: BorderRadius.circular(smartRadius),
                        color: theme.colorScheme.surfaceContainerHighest
                            .withValues(alpha: 0.22),
                        border: Border.all(
                          color: theme.colorScheme.outline.withValues(
                            alpha: 0.16,
                          ),
                        ),
                      ),
                      child: Padding(
                        padding: const EdgeInsets.all(8),
                        child: Row(
                          children: [
                            for (var i = 0; i < tabs.length; i++) ...[
                              if (i > 0) const SizedBox(width: 8),
                              tabButton(tabs[i], setDialogState),
                            ],
                          ],
                        ),
                      ),
                    ),
                    const SizedBox(height: 18),
                    Expanded(child: SingleChildScrollView(child: activePane)),
                  ],
                ),
              ),
              actions: [
                TextButton.icon(
                  onPressed: () =>
                      Navigator.of(context).pop(_ProjectSettingsResult.delete),
                  icon: const Icon(Icons.delete_outline),
                  label: const Text('Delete Project'),
                  style: TextButton.styleFrom(
                    foregroundColor: theme.colorScheme.error,
                  ),
                ),
                TextButton(
                  onPressed: () =>
                      Navigator.of(context).pop(_ProjectSettingsResult.cancel),
                  child: const Text('Cancel'),
                ),
                FilledButton(
                  onPressed: () =>
                      Navigator.of(context).pop(_ProjectSettingsResult.save),
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

    if (result == _ProjectSettingsResult.delete) {
      await _confirmAndDeleteProject(project);
      return;
    }

    if (result != _ProjectSettingsResult.save) {
      return;
    }

    _controller.updateProject(
      projectId: project.id,
      name: nameController.text.trim(),
      defaultCwd: cwdController.text.trim(),
      autoRouteReplies: autoRouteReplies,
      routeApprovalRequests: routeApprovalRequests,
      preferredModelProvider: preferredModelProvider,
      defaultModelId: defaultModelId,
      defaultReasoningEffort: defaultReasoningEffort,
      defaultSandboxMode: defaultSandboxMode,
      defaultApprovalPolicy: defaultApprovalPolicy,
      defaultNetworkAccessMode: defaultNetworkAccessMode,
      roleRuntimeDefaultsJson: jsonEncode(
        roleRuntimeDefaults.map((key, value) => MapEntry(key, value.toJson())),
      ),
      orchestratorModelId: orchestratorModelId,
      orchestratorReasoningEffort: orchestratorReasoningEffort,
      workerModelId: workerModelId,
      workerReasoningEffort: workerReasoningEffort,
      qaModelId: qaModelId,
      qaReasoningEffort: qaReasoningEffort,
      designerModelId: designerModelId,
      designerReasoningEffort: designerReasoningEffort,
      plannerModelId: plannerModelId,
      plannerReasoningEffort: plannerReasoningEffort,
      requirementsReviewerModelId: requirementsReviewerModelId,
      requirementsReviewerReasoningEffort: requirementsReviewerReasoningEffort,
      orchestratorDeveloperInstructions:
          orchestratorDeveloperInstructionsController.text.trim(),
      workerDeveloperInstructions: workerDeveloperInstructionsController.text
          .trim(),
      qaDeveloperInstructions: qaDeveloperInstructionsController.text.trim(),
      designerDeveloperInstructions: designerDeveloperInstructionsController
          .text
          .trim(),
      operatorDeveloperInstructions: operatorDeveloperInstructionsController
          .text
          .trim(),
      hiddenDeveloperInstructions: hiddenDeveloperInstructionsController.text
          .trim(),
      permanentRequirementComposables: permanentRequirementComposables,
    );
  }

  Future<void> _confirmAndDeleteProject(ProjectItem project) async {
    if (!mounted) {
      return;
    }
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Delete Project'),
        content: SizedBox(
          width: 460,
          child: Text(
            'Remove "${project.name}" from Robdex tracking? This deletes the project record, thread tracking, role settings, groups, and hook logs from Robdex state. Files on disk are not deleted.',
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            style: FilledButton.styleFrom(
              backgroundColor: Theme.of(context).colorScheme.error,
              foregroundColor: Theme.of(context).colorScheme.onError,
            ),
            child: const Text('Delete Project'),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted) {
      return;
    }
    _controller.deleteProject(project.id);
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(
          'Deleted project "${project.name}" from Robdex tracking.',
        ),
      ),
    );
  }

  Future<List<ProjectRequirementComposable>>
  _fetchProjectRequirementComposables(ProjectItem project) async {
    final items = await _controller.loadRequirementComposables(
      projectPath: project.rootPath,
    );
    return items
        .map(ProjectRequirementComposable.fromJson)
        .toList(growable: false);
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
              Future<List<_HookLogEntry>> load() =>
                  _fetchProjectHookLogs(project.id);

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
                          if (snapshot.connectionState !=
                              ConnectionState.done) {
                            return const Center(
                              child: CircularProgressIndicator(),
                            );
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
                                    ? const Center(
                                        child: Text('No hook logs recorded.'),
                                      )
                                    : ListView.separated(
                                        itemCount: logs.length,
                                        separatorBuilder: (_, _) =>
                                            const SizedBox(height: 10),
                                        itemBuilder: (context, index) {
                                          final log = logs[index];
                                          return Container(
                                            padding: const EdgeInsets.all(12),
                                            decoration: BoxDecoration(
                                              borderRadius:
                                                  BorderRadius.circular(
                                                    smartRadius,
                                                  ),
                                              border: Border.all(
                                                color: Theme.of(
                                                  context,
                                                ).colorScheme.outlineVariant,
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
                                                        style: Theme.of(
                                                          context,
                                                        ).textTheme.titleSmall,
                                                      ),
                                                    ),
                                                    Text(
                                                      log.status,
                                                      style: Theme.of(
                                                        context,
                                                      ).textTheme.labelMedium,
                                                    ),
                                                  ],
                                                ),
                                                const SizedBox(height: 6),
                                                Text(
                                                  '${log.agentName} · ${log.role} · ${log.createdAtLabel}',
                                                  style: Theme.of(
                                                    context,
                                                  ).textTheme.bodySmall,
                                                ),
                                                if (log.detail != null &&
                                                    log.detail!
                                                        .trim()
                                                        .isNotEmpty) ...[
                                                  const SizedBox(height: 8),
                                                  SelectableText(
                                                    log.detail!,
                                                    style: Theme.of(context)
                                                        .textTheme
                                                        .bodySmall
                                                        ?.copyWith(
                                                          fontFamily:
                                                              'monospace',
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
    String requirementSetJson = '';
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
                      decoration: const InputDecoration(
                        labelText: 'Agent name',
                      ),
                    ),
                    const SizedBox(height: 12),
                    DropdownButtonFormField<String>(
                      initialValue: role,
                      decoration: const InputDecoration(labelText: 'Role'),
                      items: const [
                        DropdownMenuItem(
                          value: 'worker',
                          child: Text('Worker'),
                        ),
                        DropdownMenuItem(value: 'qa', child: Text('QA')),
                        DropdownMenuItem(
                          value: 'operator',
                          child: Text('Operator'),
                        ),
                        DropdownMenuItem(
                          value: 'planner',
                          child: Text('Planner'),
                        ),
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
                      decoration: const InputDecoration(
                        labelText: 'Initial prompt',
                      ),
                    ),
                    const SizedBox(height: 12),
                    OutlinedButton.icon(
                      onPressed: () async {
                        final next = await showRequirementSetFormDialog(
                          context,
                          initialJson: requirementSetJson,
                          title: 'Agent Requirements',
                          actionLabel: 'Attach',
                          helperText:
                              'These requirements are attached before the spawned agent starts its first turn.',
                          projectPath:
                              _controller.view?.selection.projectRootPath,
                          loadComposableItems: _controller.loadRequirementComposables,
                          uploadImageBytes: _controller.uploadImageBytes,
                        );
                        if (next == null) {
                          return;
                        }
                        setDialogState(() {
                          requirementSetJson = next;
                        });
                      },
                      icon: Icon(
                        requirementSetJson.trim().isEmpty
                            ? Icons.rule_outlined
                            : Icons.rule_rounded,
                      ),
                      label: Text(
                        requirementSetJson.trim().isEmpty
                            ? 'Add requirements'
                            : 'Requirements attached',
                      ),
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
                        requirementSetJson: requirementSetJson,
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
      requirementSetJson: result.requirementSetJson,
    );
  }

  Future<String?> _promptGroupName(
    BuildContext context,
    String initialValue,
  ) async {
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

class _PeripheralDebugValues {
  const _PeripheralDebugValues({
    required this.start,
    required this.end,
    required this.blur,
    required this.chroma,
    required this.warp,
  });

  static const defaults = _PeripheralDebugValues(
    start: 0.28,
    end: 1.43,
    blur: 2.09,
    chroma: 1.73,
    warp: 0.011,
  );

  final double start;
  final double end;
  final double blur;
  final double chroma;
  final double warp;

  _PeripheralDebugValues copyWith({
    double? start,
    double? end,
    double? blur,
    double? chroma,
    double? warp,
  }) {
    return _PeripheralDebugValues(
      start: start ?? this.start,
      end: end ?? this.end,
      blur: blur ?? this.blur,
      chroma: chroma ?? this.chroma,
      warp: warp ?? this.warp,
    );
  }

  String toClipboardString() {
    return '''
start: ${start.toStringAsFixed(2)}
end: ${end.toStringAsFixed(2)}
blur: ${blur.toStringAsFixed(2)}
chroma: ${chroma.toStringAsFixed(2)}
warp: ${warp.toStringAsFixed(3)}''';
  }
}

class _WebConnectionScreen extends StatelessWidget {
  const _WebConnectionScreen({required this.errorText, required this.onRetry});

  final String? errorText;
  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Scaffold(
      backgroundColor: const Color(0xFF05090F),
      body: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 420),
          child: DecoratedBox(
            decoration: BoxDecoration(
              color: theme.colorScheme.surface.withValues(alpha: 0.86),
              borderRadius: BorderRadius.circular(smartRadius),
              border: Border.all(color: theme.colorScheme.outline),
            ),
            child: Padding(
              padding: const EdgeInsets.all(24),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    'Connecting to Robdex',
                    style: theme.textTheme.titleMedium,
                  ),
                  const SizedBox(height: 12),
                  Text(
                    errorText ?? 'Using the bridge that served this web app.',
                    style: theme.textTheme.bodySmall,
                  ),
                  const SizedBox(height: 18),
                  Align(
                    alignment: Alignment.centerRight,
                    child: FilledButton(
                      onPressed: onRetry,
                      child: const Text('Retry'),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _ConnectionScreen extends StatefulWidget {
  const _ConnectionScreen({
    required this.animation,
    required this.nebulaProgramFuture,
    required this.peripheralProgramFuture,
    required this.graphicsEnabled,
    required this.stage,
    required this.errorText,
    required this.hostController,
    required this.portController,
    required this.hostFocusNode,
    required this.portFocusNode,
    required this.onConnect,
    required this.onReset,
    required this.onGraphicsEnabledChanged,
  });

  final AnimationController animation;
  final Future<FragmentProgram?> nebulaProgramFuture;
  final Future<FragmentProgram?> peripheralProgramFuture;
  final bool graphicsEnabled;
  final _ConnectionStage stage;
  final String? errorText;
  final TextEditingController hostController;
  final TextEditingController portController;
  final FocusNode hostFocusNode;
  final FocusNode portFocusNode;
  final VoidCallback onConnect;
  final VoidCallback onReset;
  final ValueChanged<bool> onGraphicsEnabledChanged;

  @override
  State<_ConnectionScreen> createState() => _ConnectionScreenState();
}

class _ConnectionScreenState extends State<_ConnectionScreen> {
  bool _showDebugControls = false;
  _PeripheralDebugValues _debugValues = _PeripheralDebugValues.defaults;

  bool get _isBusy => widget.stage == _ConnectionStage.connecting;
  bool get _isError => widget.stage == _ConnectionStage.error;

  void _copyDebugValues() {
    Clipboard.setData(ClipboardData(text: _debugValues.toClipboardString()));
    ScaffoldMessenger.of(context).hideCurrentSnackBar();
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(const SnackBar(content: Text('Copied')));
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final isIOS = defaultTargetPlatform == TargetPlatform.iOS;
    final effectsEnabled = widget.graphicsEnabled;
    final reduceMotion =
        MediaQuery.maybeOf(context)?.disableAnimations ??
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
          if (!effectsEnabled)
            const DecoratedBox(
              decoration: BoxDecoration(
                gradient: LinearGradient(
                  begin: Alignment.topLeft,
                  end: Alignment.bottomRight,
                  colors: [
                    Color(0xFF05090F),
                    Color(0xFF081018),
                    Color(0xFF0A111A),
                  ],
                ),
              ),
            )
          else if (isIOS)
            RepaintBoundary(
              child: CustomPaint(
                painter: _StarfieldPainter(
                  animation: widget.animation,
                  warp: _isBusy ? 1 : 0,
                  reduceMotion: reduceMotion,
                ),
              ),
            )
          else
            Stack(
              fit: StackFit.expand,
              children: [
                _PeripheralVisionLayer(
                  programFuture: widget.peripheralProgramFuture,
                  animation: widget.animation,
                  warp: _isBusy ? 1 : 0,
                  reduceMotion: reduceMotion,
                  values: _debugValues,
                  child: Stack(
                    fit: StackFit.expand,
                    children: [
                      _NebulaShaderLayer(
                        programFuture: widget.nebulaProgramFuture,
                        animation: widget.animation,
                        warp: _isBusy ? 1 : 0,
                      ),
                      RepaintBoundary(
                        child: CustomPaint(
                          painter: _StarfieldPainter(
                            animation: widget.animation,
                            warp: _isBusy ? 1 : 0,
                            reduceMotion: reduceMotion,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
              ],
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
                          duration: Duration(
                            milliseconds: reduceMotion ? 0 : 500,
                          ),
                          curve: Curves.easeOut,
                          opacity: 1,
                          child: child,
                        ),
                      );
                    },
                    child: AnimatedContainer(
                      duration: Duration(milliseconds: reduceMotion ? 0 : 320),
                      curve: Curves.easeOutCubic,
                      padding: const EdgeInsets.fromLTRB(22, 22, 22, 20),
                      decoration: BoxDecoration(
                        color: const Color(0xCC081019),
                        borderRadius: BorderRadius.circular(smartRadius),
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
                          colors: [Color(0xF0142030), Color(0xEE09111A)],
                        ),
                      ),
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          Row(
                            children: [
                              _CoreBadge(
                                animation: widget.animation,
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
                                      '${widget.hostController.text.trim().isEmpty ? '127.0.0.1' : widget.hostController.text.trim()}:${widget.portController.text.trim().isEmpty ? '42080' : widget.portController.text.trim()}',
                                      style: theme.textTheme.labelMedium
                                          ?.copyWith(
                                            color: scheme.secondary.withValues(
                                              alpha: 0.92,
                                            ),
                                            letterSpacing: 0.9,
                                          ),
                                    ),
                                  ],
                                ),
                              ),
                              if (_isError)
                                IconButton(
                                  onPressed: widget.onReset,
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
                                      controller: widget.hostController,
                                      focusNode: widget.hostFocusNode,
                                      enabled: !_isBusy,
                                      textInputAction: TextInputAction.next,
                                      decoration: const InputDecoration(
                                        labelText: 'Host',
                                      ),
                                      onSubmitted: (_) =>
                                          widget.portFocusNode.requestFocus(),
                                    ),
                                    const SizedBox(height: 10),
                                    TextField(
                                      controller: widget.portController,
                                      focusNode: widget.portFocusNode,
                                      enabled: !_isBusy,
                                      keyboardType: TextInputType.number,
                                      textInputAction: TextInputAction.done,
                                      decoration: const InputDecoration(
                                        labelText: 'Port',
                                      ),
                                      onSubmitted: (_) => widget.onConnect(),
                                    ),
                                  ],
                                );
                              }
                              return Row(
                                children: [
                                  Expanded(
                                    flex: 3,
                                    child: TextField(
                                      controller: widget.hostController,
                                      focusNode: widget.hostFocusNode,
                                      enabled: !_isBusy,
                                      textInputAction: TextInputAction.next,
                                      decoration: const InputDecoration(
                                        labelText: 'Host',
                                      ),
                                      onSubmitted: (_) =>
                                          widget.portFocusNode.requestFocus(),
                                    ),
                                  ),
                                  const SizedBox(width: 10),
                                  Expanded(
                                    child: TextField(
                                      controller: widget.portController,
                                      focusNode: widget.portFocusNode,
                                      enabled: !_isBusy,
                                      keyboardType: TextInputType.number,
                                      textInputAction: TextInputAction.done,
                                      decoration: const InputDecoration(
                                        labelText: 'Port',
                                      ),
                                      onSubmitted: (_) => widget.onConnect(),
                                    ),
                                  ),
                                ],
                              );
                            },
                          ),
                          const SizedBox(height: 14),
                          AnimatedSwitcher(
                            duration: Duration(
                              milliseconds: reduceMotion ? 0 : 220,
                            ),
                            child: _isError && widget.errorText != null
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
                                            widget.errorText!,
                                            style: theme.textTheme.bodySmall
                                                ?.copyWith(
                                                  color: const Color(
                                                    0xFFFFB0A6,
                                                  ),
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
                              onPressed: _isBusy ? null : widget.onConnect,
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
                          const SizedBox(height: 12),
                          BootstrapEntryPanel(
                            host: widget.hostController.text.trim().isEmpty
                                ? '127.0.0.1'
                                : widget.hostController.text.trim(),
                            port: widget.portController.text.trim().isEmpty
                                ? '42080'
                                : widget.portController.text.trim(),
                            isBusy: _isBusy,
                            errorText: _isError ? widget.errorText : null,
                            onConnectExisting: widget.onConnect,
                            onBootstrapLocal: () {
                              showDialog<void>(
                                context: context,
                                builder: (context) =>
                                    const BootstrapHelpDialog(),
                              );
                            },
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
          SafeArea(
            child: Align(
              alignment: Alignment.bottomRight,
              child: Padding(
                padding: const EdgeInsets.all(16),
                child: _PeripheralDebugPanel(
                  expanded: _showDebugControls,
                  values: _debugValues,
                  graphicsEnabled: widget.graphicsEnabled,
                  onToggle: () {
                    setState(() {
                      _showDebugControls = !_showDebugControls;
                    });
                  },
                  onCopy: _copyDebugValues,
                  onChanged: (values) {
                    setState(() {
                      _debugValues = values;
                    });
                  },
                  onGraphicsEnabledChanged: widget.onGraphicsEnabledChanged,
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

@visibleForTesting
class BootstrapEntryPanel extends StatelessWidget {
  const BootstrapEntryPanel({
    super.key,
    required this.host,
    required this.port,
    required this.isBusy,
    this.errorText,
    required this.onConnectExisting,
    required this.onBootstrapLocal,
  });

  final String host;
  final String port;
  final bool isBusy;
  final String? errorText;
  final VoidCallback onConnectExisting;
  final VoidCallback onBootstrapLocal;

  String get _platformLabel {
    if (kIsWeb) {
      return 'Use the bridge that served this web app.';
    }
    switch (defaultTargetPlatform) {
      case TargetPlatform.macOS:
        return 'Bootstrap is available on macOS.';
      case TargetPlatform.linux:
        return 'Bootstrap is available on Linux.';
      case TargetPlatform.windows:
        return 'Windows bootstrap is WSL/future support.';
      case TargetPlatform.iOS:
      case TargetPlatform.android:
      case TargetPlatform.fuchsia:
        return 'Bootstrap is desktop-only.';
    }
  }

  bool get _canBootstrap {
    if (kIsWeb) {
      return false;
    }
    return defaultTargetPlatform == TargetPlatform.macOS ||
        defaultTargetPlatform == TargetPlatform.linux;
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return DecoratedBox(
      decoration: BoxDecoration(
        color: const Color(0x55111A26),
        borderRadius: BorderRadius.circular(smartRadius),
        border: Border.all(color: const Color(0x2636C7FF)),
      ),
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(
              'Bridge required',
              style: theme.textTheme.labelLarge?.copyWith(
                fontWeight: FontWeight.w700,
              ),
            ),
            const SizedBox(height: 4),
            Text(
              'Connect to $host:$port or bootstrap a local bridge-backed core setup.',
              style: theme.textTheme.bodySmall?.copyWith(height: 1.35),
            ),
            const SizedBox(height: 4),
            Text(
              _platformLabel,
              style: theme.textTheme.bodySmall?.copyWith(
                color: const Color(0xFF8FB6D9),
                height: 1.35,
              ),
            ),
            if (errorText != null && errorText!.trim().isNotEmpty) ...[
              const SizedBox(height: 8),
              Text(
                'Bridge health unavailable. Check the bridge, retry the connection, or bootstrap a local bridge.',
                style: theme.textTheme.bodySmall?.copyWith(
                  color: const Color(0xFFFFB0A6),
                  height: 1.35,
                ),
              ),
            ],
            const SizedBox(height: 10),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                OutlinedButton.icon(
                  onPressed: isBusy ? null : onConnectExisting,
                  icon: const Icon(Icons.link_rounded, size: 16),
                  label: const Text('Connect existing'),
                ),
                OutlinedButton.icon(
                  onPressed: !isBusy && _canBootstrap ? onBootstrapLocal : null,
                  icon: const Icon(Icons.construction_rounded, size: 16),
                  label: const Text('Bootstrap local'),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

@visibleForTesting
class BootstrapHelpDialog extends StatelessWidget {
  const BootstrapHelpDialog({super.key});

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('Bootstrap local Robdex'),
      content: const SelectableText(
        'Run the public helper outside Flutter:\n\n'
        'robdex bootstrap doctor\n'
        'robdex bootstrap plan --profile minimal\n'
        'robdex bootstrap apply --profile minimal\n\n'
        'The helper owns filesystem, service, and config operations. The GUI renders the status and connects to the bridge.',
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Close'),
        ),
      ],
    );
  }
}

class _PeripheralDebugPanel extends StatelessWidget {
  const _PeripheralDebugPanel({
    required this.expanded,
    required this.values,
    required this.graphicsEnabled,
    required this.onToggle,
    required this.onCopy,
    required this.onChanged,
    required this.onGraphicsEnabledChanged,
  });

  final bool expanded;
  final _PeripheralDebugValues values;
  final bool graphicsEnabled;
  final VoidCallback onToggle;
  final VoidCallback onCopy;
  final ValueChanged<_PeripheralDebugValues> onChanged;
  final ValueChanged<bool> onGraphicsEnabledChanged;

  @override
  Widget build(BuildContext context) {
    return AnimatedContainer(
      duration: const Duration(milliseconds: 180),
      curve: Curves.easeOutCubic,
      width: expanded ? 320 : 52,
      padding: EdgeInsets.all(expanded ? 14 : 0),
      decoration: BoxDecoration(
        color: const Color(0xCC081019),
        borderRadius: BorderRadius.circular(20),
        border: Border.all(color: const Color(0x3336C7FF)),
        boxShadow: const [
          BoxShadow(
            color: Color(0x77000000),
            blurRadius: 24,
            offset: Offset(0, 12),
          ),
        ],
      ),
      child: expanded
          ? Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                Row(
                  children: [
                    IconButton(
                      onPressed: onToggle,
                      tooltip: 'Hide',
                      icon: const Icon(Icons.tune_rounded),
                    ),
                    const Spacer(),
                    IconButton(
                      onPressed: onCopy,
                      tooltip: 'Copy',
                      icon: const Icon(Icons.content_copy_rounded),
                    ),
                  ],
                ),
                const SizedBox(height: 2),
                Row(
                  children: [
                    const Icon(Icons.bolt_outlined, size: 16),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        'Graphics',
                        style: Theme.of(context).textTheme.labelMedium,
                      ),
                    ),
                    Switch.adaptive(
                      value: graphicsEnabled,
                      onChanged: onGraphicsEnabledChanged,
                    ),
                  ],
                ),
                const SizedBox(height: 4),
                _DebugSlider(
                  label: 'Start',
                  value: values.start,
                  min: 0.2,
                  max: 1.2,
                  onChanged: (value) => onChanged(
                    values.copyWith(
                      start: value,
                      end: math.max(values.end, value + 0.05),
                    ),
                  ),
                ),
                _DebugSlider(
                  label: 'End',
                  value: values.end,
                  min: 0.8,
                  max: 2.0,
                  onChanged: (value) => onChanged(
                    values.copyWith(
                      end: value,
                      start: math.min(values.start, value - 0.05),
                    ),
                  ),
                ),
                _DebugSlider(
                  label: 'Blur',
                  value: values.blur,
                  min: 0,
                  max: 24,
                  onChanged: (value) => onChanged(values.copyWith(blur: value)),
                ),
                _DebugSlider(
                  label: 'Chroma',
                  value: values.chroma,
                  min: 0,
                  max: 4,
                  onChanged: (value) =>
                      onChanged(values.copyWith(chroma: value)),
                ),
                _DebugSlider(
                  label: 'Warp',
                  value: values.warp,
                  min: 0,
                  max: 0.12,
                  precision: 3,
                  onChanged: (value) => onChanged(values.copyWith(warp: value)),
                ),
              ],
            )
          : IconButton(
              onPressed: onToggle,
              tooltip: 'Tune',
              icon: const Icon(Icons.tune_rounded),
            ),
    );
  }
}

class _DebugSlider extends StatelessWidget {
  const _DebugSlider({
    required this.label,
    required this.value,
    required this.min,
    required this.max,
    required this.onChanged,
    this.precision = 2,
  });

  final String label;
  final double value;
  final double min;
  final double max;
  final int precision;
  final ValueChanged<double> onChanged;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Column(
        children: [
          Row(
            children: [
              Text(label, style: theme.textTheme.labelSmall),
              const Spacer(),
              Text(
                value.toStringAsFixed(precision),
                style: theme.textTheme.labelSmall?.copyWith(
                  fontFeatures: const [FontFeature.tabularFigures()],
                ),
              ),
            ],
          ),
          Slider(
            value: value.clamp(min, max),
            min: min,
            max: max,
            onChanged: onChanged,
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
              border: Border.all(color: color.withValues(alpha: 0.76)),
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
                      border: Border.all(color: color.withValues(alpha: 0.5)),
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

class _NebulaShaderLayer extends StatefulWidget {
  const _NebulaShaderLayer({
    required this.programFuture,
    required this.animation,
    required this.warp,
  });

  final Future<FragmentProgram?> programFuture;
  final AnimationController animation;
  final double warp;

  @override
  State<_NebulaShaderLayer> createState() => _NebulaShaderLayerState();
}

class _NebulaShaderLayerState extends State<_NebulaShaderLayer> {
  FragmentShader? _shader;

  @override
  void initState() {
    super.initState();
    _resolveProgram(widget.programFuture);
  }

  @override
  void didUpdateWidget(covariant _NebulaShaderLayer oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.programFuture != widget.programFuture) {
      _shader?.dispose();
      _shader = null;
      _resolveProgram(widget.programFuture);
    }
  }

  Future<void> _resolveProgram(Future<FragmentProgram?> future) async {
    final program = await future;
    if (!mounted || program == null) {
      return;
    }
    setState(() {
      _shader = program.fragmentShader();
    });
  }

  @override
  void dispose() {
    _shader?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final shader = _shader;
    if (shader == null) {
      return const SizedBox.expand();
    }
    return RepaintBoundary(
      child: CustomPaint(
        painter: _NebulaShaderPainter(
          animation: widget.animation,
          shader: shader,
          warp: widget.warp,
        ),
      ),
    );
  }
}

class _PeripheralVisionLayer extends StatefulWidget {
  const _PeripheralVisionLayer({
    required this.programFuture,
    required this.animation,
    required this.warp,
    required this.reduceMotion,
    required this.values,
    required this.child,
  });

  final Future<FragmentProgram?> programFuture;
  final AnimationController animation;
  final double warp;
  final bool reduceMotion;
  final _PeripheralDebugValues values;
  final Widget child;

  @override
  State<_PeripheralVisionLayer> createState() => _PeripheralVisionLayerState();
}

class _PeripheralVisionLayerState extends State<_PeripheralVisionLayer> {
  FragmentShader? _shader;

  @override
  void initState() {
    super.initState();
    _resolveProgram(widget.programFuture);
  }

  @override
  void didUpdateWidget(covariant _PeripheralVisionLayer oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.programFuture != widget.programFuture) {
      _shader?.dispose();
      _shader = null;
      _resolveProgram(widget.programFuture);
    }
  }

  Future<void> _resolveProgram(Future<FragmentProgram?> future) async {
    final program = await future;
    if (!mounted || program == null) {
      return;
    }
    setState(() {
      _shader = program.fragmentShader();
    });
  }

  @override
  void dispose() {
    _shader?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final shader = _shader;
    if (shader == null || !ImageFilter.isShaderFilterSupported) {
      return widget.child;
    }

    return AnimatedBuilder(
      animation: widget.animation,
      builder: (context, _) {
        final blurStrength = widget.reduceMotion
            ? widget.values.blur
            : (widget.warp > 0
                  ? widget.values.blur * 1.333
                  : widget.values.blur);
        final aberrationStrength = widget.reduceMotion
            ? widget.values.chroma
            : (widget.warp > 0
                  ? widget.values.chroma * 1.591
                  : widget.values.chroma);
        final warpStrength = widget.reduceMotion
            ? widget.values.warp
            : (widget.warp > 0
                  ? widget.values.warp * 1.5625
                  : widget.values.warp);

        shader.setFloat(2, 0.5);
        shader.setFloat(3, 0.46);
        shader.setFloat(4, widget.values.start);
        shader.setFloat(5, widget.values.end);
        shader.setFloat(6, blurStrength);
        shader.setFloat(7, aberrationStrength);
        shader.setFloat(8, warpStrength);

        return ClipRect(
          child: ImageFiltered(
            imageFilter: ImageFilter.shader(shader),
            child: widget.child,
          ),
        );
      },
    );
  }
}

class _NebulaShaderPainter extends CustomPainter {
  const _NebulaShaderPainter({
    required this.animation,
    required this.shader,
    required this.warp,
  }) : super(repaint: animation);

  final AnimationController animation;
  final FragmentShader shader;
  final double warp;

  @override
  void paint(Canvas canvas, Size size) {
    final elapsedSeconds =
        (animation.lastElapsedDuration?.inMilliseconds ?? 0) / 1000.0;
    shader.setFloat(0, size.width);
    shader.setFloat(1, size.height);
    shader.setFloat(2, elapsedSeconds.toDouble());
    shader.setFloat(3, warp);
    canvas.drawRect(Offset.zero & size, Paint()..shader = shader);
  }

  @override
  bool shouldRepaint(covariant _NebulaShaderPainter oldDelegate) {
    return oldDelegate.shader != shader ||
        oldDelegate.warp != warp ||
        oldDelegate.animation != animation;
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
      ..shader = LinearGradient(
        begin: Alignment.topCenter,
        end: Alignment.bottomCenter,
        colors: const [Color(0x1804070B), Color(0x2208111B), Color(0x1403060A)],
      ).createShader(rect);
    canvas.drawRect(rect, background);

    final center = Offset(size.width / 2, size.height * 0.45);
    final glowPaint = Paint()
      ..shader =
          RadialGradient(
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
      final vector = Offset(math.cos(angle) * spread, math.sin(angle));
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
      ).createShader(Rect.fromCircle(center: point, radius: glowRadius * 2.6));
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
  _HistoryMessageType _messageType = _HistoryMessageType.all;

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
          final historyEntries = widget.controller.threadHistoryEntries;
          final typeFilteredEntries = historyEntries
              .where((entry) => _messageType.matches(entry))
              .toList(growable: false);
          final filteredEntries = regex == null
              ? typeFilteredEntries
              : typeFilteredEntries
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
              const SizedBox(height: 8),
              Wrap(
                spacing: 8,
                runSpacing: 6,
                children: [
                  for (final type in _HistoryMessageType.values)
                    ChoiceChip(
                      label: Text(type.label),
                      selected: _messageType == type,
                      onSelected: (_) => setState(() => _messageType = type),
                    ),
                ],
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
                        '${filteredEntries.length} of ${historyEntries.length} results',
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

enum _HistoryMessageType {
  all('All'),
  user('User'),
  assistant('Assistant'),
  tool('Tool'),
  system('System'),
  other('Other');

  const _HistoryMessageType(this.label);

  final String label;

  bool matches(ChatEntry entry) {
    return switch (this) {
      _HistoryMessageType.all => true,
      _HistoryMessageType.user => _entryType(entry) == _HistoryMessageType.user,
      _HistoryMessageType.assistant =>
        _entryType(entry) == _HistoryMessageType.assistant,
      _HistoryMessageType.tool => _entryType(entry) == _HistoryMessageType.tool,
      _HistoryMessageType.system =>
        _entryType(entry) == _HistoryMessageType.system,
      _HistoryMessageType.other =>
        _entryType(entry) == _HistoryMessageType.other,
    };
  }
}

_HistoryMessageType _entryType(ChatEntry entry) {
  if (entry.isTool || entry.command != null || entry.output != null) {
    return _HistoryMessageType.tool;
  }

  final author = entry.author.trim().toLowerCase();
  final label = entry.displayLabel.trim().toLowerCase();
  if (author == 'user' ||
      author == 'operator' ||
      label == 'user' ||
      label == 'operator') {
    return _HistoryMessageType.user;
  }
  if (author == 'assistant' || label == 'assistant') {
    return _HistoryMessageType.assistant;
  }
  if (author == 'system' || label == 'system') {
    return _HistoryMessageType.system;
  }

  final kind = entry.kind?.trim().toLowerCase();
  if (kind == 'commandexecution' ||
      kind == 'filechange' ||
      kind == 'mcptoolcall' ||
      kind == 'tool') {
    return _HistoryMessageType.tool;
  }

  return _HistoryMessageType.other;
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
    required this.requirementSetJson,
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
  final String requirementSetJson;
}

class _AgentDraft {
  const _AgentDraft({
    required this.name,
    required this.role,
    required this.prompt,
    required this.requirementSetJson,
  });

  final String name;
  final String role;
  final String prompt;
  final String requirementSetJson;
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
    return formatLocalDateTimeLabel(createdAt);
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
