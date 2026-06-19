import 'package:flutter/material.dart';

import '../../core/models/agent_runtime_workbench_models.dart';
import 'agent_runtime_code_editor.dart';

typedef AgentRuntimeRoleVersionAction = void Function(String roleId, String versionId);

class AgentRuntimeWorkbench extends StatelessWidget {
  const AgentRuntimeWorkbench({
    super.key,
    required this.data,
    required this.baseUrlController,
    required this.onConnect,
    required this.onRefreshDiscovery,
    required this.onConnectDiscovered,
    required this.onRefreshIcloudRemoteDiscovery,
    required this.onConnectIcloudRemote,
    required this.onImportRemoteProfile,
    required this.onRefreshImportedRemoteProfile,
    required this.onConnectImportedRemoteProfile,
    required this.onDisconnect,
    this.onRoleValidate,
    this.onRoleCreate,
    this.onRoleUpdate,
    this.onRoleExport,
    this.onRoleArchive,
    this.onRoleUnarchive,
    this.onRoleActivate,
    this.onWorkflowMemorySelect,
    this.onWorkflowMemoryAttempted,
    this.onWorkflowMemoryHelpful,
    this.onWorkflowMemoryNotHelpful,
    this.onSessionClose,
    this.onSessionArchive,
    this.onSessionFork,
    this.onProcessTerminate,
    this.onProcessInput,
    this.onProcessFlush,
    this.onApprovalApprove,
    this.onApprovalDeny,
    this.onApprovalResume,
    this.onCommandRegistryApprove,
    this.onCommandRegistryDeny,
    this.onCommandRegistryPreview,
    this.onCommandRegistryApply,
  });

  final AgentRuntimeWorkbenchData data;
  final TextEditingController baseUrlController;
  final VoidCallback onConnect;
  final VoidCallback onRefreshDiscovery;
  final VoidCallback onConnectDiscovered;
  final VoidCallback onRefreshIcloudRemoteDiscovery;
  final VoidCallback onConnectIcloudRemote;
  final VoidCallback onImportRemoteProfile;
  final VoidCallback onRefreshImportedRemoteProfile;
  final VoidCallback onConnectImportedRemoteProfile;
  final VoidCallback onDisconnect;
  final ValueChanged<AgentRuntimeRoleEditorDraft>? onRoleValidate;
  final ValueChanged<AgentRuntimeRoleEditorDraft>? onRoleCreate;
  final ValueChanged<AgentRuntimeRoleEditorDraft>? onRoleUpdate;
  final ValueChanged<String>? onRoleExport;
  final ValueChanged<String>? onRoleArchive;
  final ValueChanged<String>? onRoleUnarchive;
  final AgentRuntimeRoleVersionAction? onRoleActivate;
  final ValueChanged<AgentRuntimeWorkflowMemoryRow>? onWorkflowMemorySelect;
  final ValueChanged<AgentRuntimeWorkflowMemoryDetail>? onWorkflowMemoryAttempted;
  final ValueChanged<AgentRuntimeWorkflowMemoryDetail>? onWorkflowMemoryHelpful;
  final ValueChanged<AgentRuntimeWorkflowMemoryDetail>? onWorkflowMemoryNotHelpful;
  final ValueChanged<String>? onSessionClose;
  final ValueChanged<String>? onSessionArchive;
  final ValueChanged<String>? onSessionFork;
  final ValueChanged<String>? onProcessTerminate;
  final ValueChanged<String>? onProcessInput;
  final ValueChanged<String>? onProcessFlush;
  final ValueChanged<AgentRuntimeActionItem>? onApprovalApprove;
  final ValueChanged<AgentRuntimeActionItem>? onApprovalDeny;
  final ValueChanged<AgentRuntimeActionItem>? onApprovalResume;
  final ValueChanged<AgentRuntimeActionItem>? onCommandRegistryApprove;
  final ValueChanged<AgentRuntimeActionItem>? onCommandRegistryDeny;
  final ValueChanged<AgentRuntimeActionItem>? onCommandRegistryPreview;
  final ValueChanged<AgentRuntimeActionItem>? onCommandRegistryApply;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: const BoxDecoration(color: Color(0xFF05090F)),
      child: SafeArea(
        child: LayoutBuilder(
          builder: (context, constraints) {
            final narrow = constraints.maxWidth < 720;
            final connected = _isConnected(data);
            if (!connected) {
              return _RuntimeLoginScreen(
                data: data,
                baseUrlController: baseUrlController,
                onConnect: onConnect,
                onRefreshDiscovery: onRefreshDiscovery,
                onConnectDiscovered: onConnectDiscovered,
                onRefreshIcloudRemoteDiscovery: onRefreshIcloudRemoteDiscovery,
                onConnectIcloudRemote: onConnectIcloudRemote,
                onImportRemoteProfile: onImportRemoteProfile,
                onRefreshImportedRemoteProfile: onRefreshImportedRemoteProfile,
                onConnectImportedRemoteProfile: onConnectImportedRemoteProfile,
                errorMessage: data.errorMessage == null ? null : _friendlyError(data.errorMessage!),
              );
            }
            return Column(
              children: [
                _RuntimeTopBar(
                  data: data,
                  onDisconnect: onDisconnect,
                ),
                if (data.errorMessage case final error?)
                  _InlineError(message: _friendlyError(error)),
                Expanded(
                  child: Padding(
                    padding: EdgeInsets.fromLTRB(narrow ? 10 : 14, 0, narrow ? 10 : 14, 12),
                    child: narrow
                        ? ListView(
                            children: [
                              SizedBox(height: 220, child: _SessionsPanel(data)),
                              const SizedBox(height: 10),
                              SizedBox(height: 260, child: _TimelinePanel(data)),
                              SizedBox(height: 210, child: _ActionsPanel(data)),
                              const SizedBox(height: 10),
                              SizedBox(height: 360, child: _DetailsPanel(data,
                                  onRoleValidate: onRoleValidate,
                                  onRoleCreate: onRoleCreate,
                                  onRoleUpdate: onRoleUpdate,
                                  onRoleExport: onRoleExport,
                                  onRoleArchive: onRoleArchive,
                                  onRoleUnarchive: onRoleUnarchive,
                                  onRoleActivate: onRoleActivate,
                                  onWorkflowMemorySelect: onWorkflowMemorySelect,
                                  onWorkflowMemoryAttempted: onWorkflowMemoryAttempted,
                                  onWorkflowMemoryHelpful: onWorkflowMemoryHelpful,
                                  onWorkflowMemoryNotHelpful: onWorkflowMemoryNotHelpful,
                                  onSessionClose: onSessionClose,
                                  onSessionArchive: onSessionArchive,
                                  onSessionFork: onSessionFork,
                                  onProcessTerminate: onProcessTerminate,
                                  onProcessInput: onProcessInput,
                                  onProcessFlush: onProcessFlush,
                                  onApprovalApprove: onApprovalApprove,
                                  onApprovalDeny: onApprovalDeny,
                                  onApprovalResume: onApprovalResume,
                                  onCommandRegistryApprove: onCommandRegistryApprove,
                                  onCommandRegistryDeny: onCommandRegistryDeny,
                                  onCommandRegistryPreview: onCommandRegistryPreview,
                                  onCommandRegistryApply: onCommandRegistryApply)),
                            ],
                          )
                        : Row(
                            crossAxisAlignment: CrossAxisAlignment.stretch,
                            children: [
                              SizedBox(width: 260, child: _SessionsPanel(data)),
                              const SizedBox(width: 10),
                              Expanded(child: _TimelinePanel(data)),
                              const SizedBox(width: 10),
                              SizedBox(
                                width: 312,
                                child: Column(
                                  children: [
                                    SizedBox(height: 230, child: _ActionsPanel(data)),
                                    const SizedBox(height: 10),
                                    Expanded(
                                      child: _DetailsPanel(data,
                                          onRoleValidate: onRoleValidate,
                                          onRoleCreate: onRoleCreate,
                                          onRoleUpdate: onRoleUpdate,
                                          onRoleExport: onRoleExport,
                                          onRoleArchive: onRoleArchive,
                                          onRoleUnarchive: onRoleUnarchive,
                                          onRoleActivate: onRoleActivate,
                                          onWorkflowMemorySelect: onWorkflowMemorySelect,
                                          onWorkflowMemoryAttempted: onWorkflowMemoryAttempted,
                                          onWorkflowMemoryHelpful: onWorkflowMemoryHelpful,
                                          onWorkflowMemoryNotHelpful: onWorkflowMemoryNotHelpful,
                                          onSessionClose: onSessionClose,
                                          onSessionArchive: onSessionArchive,
                                          onSessionFork: onSessionFork,
                                          onProcessTerminate: onProcessTerminate,
                                          onProcessInput: onProcessInput,
                                          onProcessFlush: onProcessFlush,
                                          onApprovalApprove: onApprovalApprove,
                                          onApprovalDeny: onApprovalDeny,
                                          onApprovalResume: onApprovalResume,
                                          onCommandRegistryApprove: onCommandRegistryApprove,
                                          onCommandRegistryDeny: onCommandRegistryDeny,
                                          onCommandRegistryPreview: onCommandRegistryPreview,
                                          onCommandRegistryApply: onCommandRegistryApply),
                                    ),
                                  ],
                                ),
                              ),
                            ],
                          ),
                  ),
                ),
              ],
            );
          },
        ),
      ),
    );
  }
}

class _RuntimeLoginScreen extends StatelessWidget {
  const _RuntimeLoginScreen({
    required this.data,
    required this.baseUrlController,
    required this.onConnect,
    required this.onRefreshDiscovery,
    required this.onConnectDiscovered,
    required this.onRefreshIcloudRemoteDiscovery,
    required this.onConnectIcloudRemote,
    required this.onImportRemoteProfile,
    required this.onRefreshImportedRemoteProfile,
    required this.onConnectImportedRemoteProfile,
    this.errorMessage,
  });

  final AgentRuntimeWorkbenchData data;
  final TextEditingController baseUrlController;
  final VoidCallback onConnect;
  final VoidCallback onRefreshDiscovery;
  final VoidCallback onConnectDiscovered;
  final VoidCallback onRefreshIcloudRemoteDiscovery;
  final VoidCallback onConnectIcloudRemote;
  final VoidCallback onImportRemoteProfile;
  final VoidCallback onRefreshImportedRemoteProfile;
  final VoidCallback onConnectImportedRemoteProfile;
  final String? errorMessage;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return LayoutBuilder(
      builder: (context, constraints) {
        final narrow = constraints.maxWidth < 640;
        final primary = data.discovery.connectable
            ? _HeaderAction(label: 'Connect local runtime', onPressed: onConnectDiscovered, filled: true)
            : data.remoteDiscovery.connectable
                ? _HeaderAction(label: 'Connect iCloud profile', onPressed: onConnectIcloudRemote, filled: true)
                : data.importedRemoteDiscovery.connectable
                    ? _HeaderAction(label: 'Connect imported profile', onPressed: onConnectImportedRemoteProfile, filled: true)
                    : _HeaderAction(label: 'Connect to URL', onPressed: onConnect, filled: true);
        return Center(
          child: SingleChildScrollView(
            padding: EdgeInsets.symmetric(horizontal: narrow ? 18 : 36, vertical: 24),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 720),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Text(
                    'Agent Runtime',
                    textAlign: TextAlign.center,
                    style: theme.textTheme.headlineSmall?.copyWith(color: Colors.white, fontWeight: FontWeight.w800),
                  ),
                  const SizedBox(height: 8),
                  Text(
                    'Connect to a local service or imported remote profile.',
                    textAlign: TextAlign.center,
                    style: theme.textTheme.bodyMedium?.copyWith(color: const Color(0xFFA6B6CA)),
                  ),
                  const SizedBox(height: 22),
                  DecoratedBox(
                    decoration: BoxDecoration(
                      color: const Color(0xFF0B111A),
                      border: Border.all(color: const Color(0xFF243045)),
                      borderRadius: BorderRadius.circular(8),
                    ),
                    child: Padding(
                      padding: EdgeInsets.all(narrow ? 14 : 18),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Row(
                            children: [
                              Expanded(
                                child: Text(
                                  'Runtime setup',
                                  style: theme.textTheme.titleSmall?.copyWith(color: Colors.white, fontWeight: FontWeight.w700),
                                ),
                              ),
                              _StatusDot(label: _loginStatusLabel(data), tone: data.connectionTone),
                            ],
                          ),
                          if (errorMessage case final message?) ...[
                            const SizedBox(height: 12),
                            _InlineNotice(message: message, tone: 'danger'),
                          ],
                          const SizedBox(height: 14),
                          _LoginTargetField(baseUrlController: baseUrlController, primary: primary),
                          const SizedBox(height: 16),
                          _DiscoveryControls(
                            discovery: data.discovery,
                            remoteDiscovery: data.remoteDiscovery,
                            importedDiscovery: data.importedRemoteDiscovery,
                            onRefreshDiscovery: onRefreshDiscovery,
                            onConnectDiscovered: onConnectDiscovered,
                            onRefreshIcloudRemoteDiscovery: onRefreshIcloudRemoteDiscovery,
                            onConnectIcloudRemote: onConnectIcloudRemote,
                            onImportRemoteProfile: onImportRemoteProfile,
                            onRefreshImportedRemoteProfile: onRefreshImportedRemoteProfile,
                            onConnectImportedRemoteProfile: onConnectImportedRemoteProfile,
                          ),
                        ],
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        );
      },
    );
  }
}

class _RuntimeTopBar extends StatelessWidget {
  const _RuntimeTopBar({
    required this.data,
    required this.onDisconnect,
  });

  final AgentRuntimeWorkbenchData data;
  final VoidCallback onDisconnect;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.fromLTRB(14, 12, 14, 10),
      child: Row(
        children: [
          Expanded(
            child: Wrap(
              spacing: 10,
              runSpacing: 6,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                Text(
                  'Agent Runtime',
                  style: theme.textTheme.titleMedium?.copyWith(color: Colors.white, fontWeight: FontWeight.w700),
                ),
                _StatusDot(label: data.statusLabel, tone: data.connectionTone),
                if (data.pendingRequestCount > 0) _MutedText('${data.pendingRequestCount} pending'),
              ],
            ),
          ),
          const SizedBox(width: 8),
          OutlinedButton(onPressed: onDisconnect, child: const Text('Disconnect')),
        ],
      ),
    );
  }
}

class _LoginTargetField extends StatelessWidget {
  const _LoginTargetField({required this.baseUrlController, required this.primary});

  final TextEditingController baseUrlController;
  final _HeaderAction primary;

  @override
  Widget build(BuildContext context) {
    final compact = MediaQuery.sizeOf(context).width < 560;
    final field = TextField(
      controller: baseUrlController,
      style: const TextStyle(color: Colors.white),
      decoration: const InputDecoration(labelText: 'Runtime URL', helperText: 'Manual fallback when discovery is unavailable'),
    );
    if (compact) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          field,
          const SizedBox(height: 10),
          primary,
        ],
      );
    }
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Expanded(child: field),
        const SizedBox(width: 10),
        Padding(padding: const EdgeInsets.only(top: 2), child: primary),
      ],
    );
  }
}

class _DiscoveryControls extends StatelessWidget {
  const _DiscoveryControls({
    required this.discovery,
    required this.remoteDiscovery,
    required this.importedDiscovery,
    required this.onRefreshDiscovery,
    required this.onConnectDiscovered,
    required this.onRefreshIcloudRemoteDiscovery,
    required this.onConnectIcloudRemote,
    required this.onImportRemoteProfile,
    required this.onRefreshImportedRemoteProfile,
    required this.onConnectImportedRemoteProfile,
  });

  final AgentRuntimeDiscoveryInfo discovery;
  final AgentRuntimeDiscoveryInfo remoteDiscovery;
  final AgentRuntimeDiscoveryInfo importedDiscovery;
  final VoidCallback onRefreshDiscovery;
  final VoidCallback onConnectDiscovered;
  final VoidCallback onRefreshIcloudRemoteDiscovery;
  final VoidCallback onConnectIcloudRemote;
  final VoidCallback onImportRemoteProfile;
  final VoidCallback onRefreshImportedRemoteProfile;
  final VoidCallback onConnectImportedRemoteProfile;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text('Discovery', style: theme.textTheme.labelMedium?.copyWith(color: const Color(0xFF9FB2CB))),
        const SizedBox(height: 6),
        Wrap(
          spacing: 18,
          runSpacing: 12,
          children: [
            _DiscoveryButton(label: 'Local', info: discovery, onRefresh: onRefreshDiscovery, onConnect: onConnectDiscovered),
            _DiscoveryButton(label: 'iCloud', info: remoteDiscovery, onRefresh: onRefreshIcloudRemoteDiscovery, onConnect: onConnectIcloudRemote),
            _DiscoveryButton(
              label: 'Imported',
              info: importedDiscovery,
              onRefresh: onRefreshImportedRemoteProfile,
              onConnect: onConnectImportedRemoteProfile,
              leadingAction: OutlinedButton(onPressed: onImportRemoteProfile, child: const Text('Import')),
            ),
          ],
        ),
      ],
    );
  }
}

class _DiscoveryButton extends StatelessWidget {
  const _DiscoveryButton({required this.label, required this.info, required this.onRefresh, required this.onConnect, this.leadingAction});

  final String label;
  final AgentRuntimeDiscoveryInfo info;
  final VoidCallback onRefresh;
  final VoidCallback onConnect;
  final Widget? leadingAction;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return SizedBox(
      width: 220,
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 2),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(label, style: theme.textTheme.labelMedium?.copyWith(color: Colors.white, fontWeight: FontWeight.w700)),
            const SizedBox(height: 2),
            Text(
              _discoveryStateText(info),
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: theme.textTheme.bodySmall?.copyWith(color: _toneColor(info.tone)),
            ),
            const SizedBox(height: 6),
            Wrap(
              spacing: 6,
              runSpacing: 6,
              children: [
                ?leadingAction,
                OutlinedButton(onPressed: onRefresh, child: const Text('Refresh')),
                FilledButton(onPressed: info.connectable ? onConnect : null, child: const Text('Connect')),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

class _SessionsPanel extends StatelessWidget {
  const _SessionsPanel(this.data);
  final AgentRuntimeWorkbenchData data;
  @override
  Widget build(BuildContext context) => _Panel(
        title: data.sessionsTitle,
        subtitle: _cleanSectionCopy(data.sessionsSubtitle),
        child: data.sessions.isEmpty
            ? _EmptyState(title: data.sessionsEmptyTitle, body: _cleanSectionCopy(data.sessionsEmptyText))
            : ListView.separated(
                itemCount: data.sessions.length,
                separatorBuilder: (_, _) => const SizedBox(height: 8),
                itemBuilder: (context, index) => _SessionTile(data.sessions[index]),
              ),
      );
}

class _TimelinePanel extends StatelessWidget {
  const _TimelinePanel(this.data);
  final AgentRuntimeWorkbenchData data;
  @override
  Widget build(BuildContext context) => _Panel(
        title: data.timelineTitle,
        subtitle: _cleanSectionCopy(data.timelineSubtitle),
        child: data.timeline.isEmpty
            ? _EmptyState(title: data.timelineEmptyTitle, body: _cleanSectionCopy(data.timelineEmptyText))
            : ListView.separated(
                itemCount: data.timeline.length,
                separatorBuilder: (_, _) => const Divider(height: 16),
                itemBuilder: (context, index) => _TimelineTile(data.timeline[index]),
              ),
      );
}

class _ActionsPanel extends StatelessWidget {
  const _ActionsPanel(this.data);
  final AgentRuntimeWorkbenchData data;
  @override
  Widget build(BuildContext context) => _Panel(
        title: data.actionsTitle,
        subtitle: _cleanSectionCopy(data.actionsSubtitle),
        child: data.actions.isEmpty
            ? _EmptyState(title: data.actionsEmptyTitle, body: _cleanSectionCopy(data.actionsEmptyText))
            : ListView.separated(
                itemCount: data.actions.length,
                separatorBuilder: (_, _) => const SizedBox(height: 8),
                itemBuilder: (context, index) => _ActionTile(data.actions[index]),
              ),
      );
}

class _DetailsPanel extends StatelessWidget {
  const _DetailsPanel(
    this.data, {
    this.focusSurfaceId,
    this.onRoleValidate,
    this.onRoleCreate,
    this.onRoleUpdate,
    this.onRoleExport,
    this.onRoleArchive,
    this.onRoleUnarchive,
    this.onRoleActivate,
    this.onWorkflowMemorySelect,
    this.onWorkflowMemoryAttempted,
    this.onWorkflowMemoryHelpful,
    this.onWorkflowMemoryNotHelpful,
    this.onSessionClose,
    this.onSessionArchive,
    this.onSessionFork,
    this.onProcessTerminate,
    this.onProcessInput,
    this.onProcessFlush,
    this.onApprovalApprove,
    this.onApprovalDeny,
    this.onApprovalResume,
    this.onCommandRegistryApprove,
    this.onCommandRegistryDeny,
    this.onCommandRegistryPreview,
    this.onCommandRegistryApply,
  });

  final AgentRuntimeWorkbenchData data;
  final String? focusSurfaceId;
  final ValueChanged<AgentRuntimeRoleEditorDraft>? onRoleValidate;
  final ValueChanged<AgentRuntimeRoleEditorDraft>? onRoleCreate;
  final ValueChanged<AgentRuntimeRoleEditorDraft>? onRoleUpdate;
  final ValueChanged<String>? onRoleExport;
  final ValueChanged<String>? onRoleArchive;
  final ValueChanged<String>? onRoleUnarchive;
  final AgentRuntimeRoleVersionAction? onRoleActivate;
  final ValueChanged<AgentRuntimeWorkflowMemoryRow>? onWorkflowMemorySelect;
  final ValueChanged<AgentRuntimeWorkflowMemoryDetail>? onWorkflowMemoryAttempted;
  final ValueChanged<AgentRuntimeWorkflowMemoryDetail>? onWorkflowMemoryHelpful;
  final ValueChanged<AgentRuntimeWorkflowMemoryDetail>? onWorkflowMemoryNotHelpful;
  final ValueChanged<String>? onSessionClose;
  final ValueChanged<String>? onSessionArchive;
  final ValueChanged<String>? onSessionFork;
  final ValueChanged<String>? onProcessTerminate;
  final ValueChanged<String>? onProcessInput;
  final ValueChanged<String>? onProcessFlush;
  final ValueChanged<AgentRuntimeActionItem>? onApprovalApprove;
  final ValueChanged<AgentRuntimeActionItem>? onApprovalDeny;
  final ValueChanged<AgentRuntimeActionItem>? onApprovalResume;
  final ValueChanged<AgentRuntimeActionItem>? onCommandRegistryApprove;
  final ValueChanged<AgentRuntimeActionItem>? onCommandRegistryDeny;
  final ValueChanged<AgentRuntimeActionItem>? onCommandRegistryPreview;
  final ValueChanged<AgentRuntimeActionItem>? onCommandRegistryApply;

  @override
  Widget build(BuildContext context) {
    final surfaces = focusSurfaceId == null
        ? data.operationSurfaces
        : data.operationSurfaces.where((surface) => surface.surfaceId == focusSurfaceId).toList(growable: false);
    final operationSections = surfaces.isEmpty
        ? const <Widget>[]
        : surfaces
            .map((surface) => _OperationsSection(
                  title: surface.title,
                  rows: surface.rows.map((row) => _OperationsRow(row.label, row.value)).toList(growable: false),
                  actions: surface.actions,
                  onActionPressed: _operationActionHandler(surface.surfaceId),
                ))
            .toList(growable: false);
    return _Panel(
      title: _cleanSectionCopy(data.detailTitle),
      subtitle: _cleanSectionCopy(data.detailSubtitle),
      child: ListView(
        children: [
          if (focusSurfaceId == 'roleAdmin') ...[
            _RoleAdminPanel(
              data.roleAdmin,
              onValidate: onRoleValidate,
              onCreate: onRoleCreate,
              onUpdate: onRoleUpdate,
              onExport: onRoleExport,
              onArchive: onRoleArchive,
              onUnarchive: onRoleUnarchive,
              onActivate: onRoleActivate,
            ),
          ] else if (focusSurfaceId == 'workflowMemory') ...[
            _WorkflowMemoryPanel(
              data.workflowMemory,
              onSelect: onWorkflowMemorySelect,
              onAttempted: onWorkflowMemoryAttempted,
              onHelpful: onWorkflowMemoryHelpful,
              onNotHelpful: onWorkflowMemoryNotHelpful,
            ),
          ] else if (operationSections.isEmpty)
            _OperationsSection(
              title: 'Session',
              rows: [
                _OperationsRow('Selected', data.selectedSessionLabel),
                _OperationsRow('Connection', data.statusLabel),
              ],
            )
          else
            ...operationSections,
          if (focusSurfaceId == null && data.actions.isNotEmpty) ...[
            Text(data.actionsTitle, style: Theme.of(context).textTheme.labelMedium?.copyWith(color: Colors.white70)),
            const SizedBox(height: 6),
            ...data.actions.map(
              (action) => _ActionTile(
                action,
                onApprovalApprove: onApprovalApprove == null ? null : () => onApprovalApprove!(action),
                onApprovalDeny: onApprovalDeny == null ? null : () => onApprovalDeny!(action),
                onApprovalResume: onApprovalResume == null ? null : () => onApprovalResume!(action),
                onCommandRegistryApprove: onCommandRegistryApprove == null ? null : () => onCommandRegistryApprove!(action),
                onCommandRegistryDeny: onCommandRegistryDeny == null ? null : () => onCommandRegistryDeny!(action),
                onCommandRegistryPreview: onCommandRegistryPreview == null ? null : () => onCommandRegistryPreview!(action),
                onCommandRegistryApply: onCommandRegistryApply == null ? null : () => onCommandRegistryApply!(action),
              ),
            ),
            const SizedBox(height: 10),
          ],
          if (focusSurfaceId == null) ...[
            _RoleAdminPanel(
              data.roleAdmin,
              onValidate: onRoleValidate,
              onCreate: onRoleCreate,
              onUpdate: onRoleUpdate,
              onExport: onRoleExport,
              onArchive: onRoleArchive,
              onUnarchive: onRoleUnarchive,
              onActivate: onRoleActivate,
            ),
            const SizedBox(height: 10),
            _WorkflowMemoryPanel(
              data.workflowMemory,
              onSelect: onWorkflowMemorySelect,
              onAttempted: onWorkflowMemoryAttempted,
              onHelpful: onWorkflowMemoryHelpful,
              onNotHelpful: onWorkflowMemoryNotHelpful,
            ),
            const SizedBox(height: 10),
            ...data.controllerFacts.where(_isUserFacingFact).map(_FactRow.new),
          ],
        ],
      ),
    );
  }

  VoidCallback? Function(AgentRuntimeActionItem) _operationActionHandler(String surfaceId) {
    return (action) {
      switch (action.kind) {
        case 'closeSession':
          return onSessionClose == null ? null : () => onSessionClose!(action.id);
        case 'archiveSession':
          return onSessionArchive == null ? null : () => onSessionArchive!(action.id);
        case 'forkSession':
          return onSessionFork == null ? null : () => onSessionFork!(action.id);
        case 'processTerminate':
          return onProcessTerminate == null ? null : () => onProcessTerminate!(action.id);
        case 'processInput':
          return onProcessInput == null ? null : () => onProcessInput!(action.id);
        case 'processFlush':
          return onProcessFlush == null ? null : () => onProcessFlush!(action.id);
        case 'approval':
          return onApprovalApprove == null ? null : () => onApprovalApprove!(action);
        case 'approvalDeny':
          return onApprovalDeny == null ? null : () => onApprovalDeny!(action);
        case 'approvalResume':
          return onApprovalResume == null ? null : () => onApprovalResume!(action);
        case 'commandRegistryPreview':
          return onCommandRegistryPreview == null ? null : () => onCommandRegistryPreview!(action);
        case 'commandRegistryRequest':
          return onCommandRegistryApprove == null ? null : () => onCommandRegistryApprove!(action);
        case 'commandRegistryDeny':
          return onCommandRegistryDeny == null ? null : () => onCommandRegistryDeny!(action);
        case 'commandRegistryApply':
          return onCommandRegistryApply == null ? null : () => onCommandRegistryApply!(action);
        default:
          return null;
      }
    };
  }
}

class AgentRuntimeOperationsDetail extends StatelessWidget {
  const AgentRuntimeOperationsDetail({
    super.key,
    required this.data,
    this.focusSurfaceId,
    this.onRoleValidate,
    this.onRoleCreate,
    this.onRoleUpdate,
    this.onRoleExport,
    this.onRoleArchive,
    this.onRoleUnarchive,
    this.onRoleActivate,
    this.onWorkflowMemorySelect,
    this.onWorkflowMemoryAttempted,
    this.onWorkflowMemoryHelpful,
    this.onWorkflowMemoryNotHelpful,
    this.onSessionClose,
    this.onSessionArchive,
    this.onSessionFork,
    this.onProcessTerminate,
    this.onProcessInput,
    this.onProcessFlush,
    this.onApprovalApprove,
    this.onApprovalDeny,
    this.onApprovalResume,
    this.onCommandRegistryApprove,
    this.onCommandRegistryDeny,
    this.onCommandRegistryPreview,
    this.onCommandRegistryApply,
  });

  final AgentRuntimeWorkbenchData data;
  final String? focusSurfaceId;
  final ValueChanged<AgentRuntimeRoleEditorDraft>? onRoleValidate;
  final ValueChanged<AgentRuntimeRoleEditorDraft>? onRoleCreate;
  final ValueChanged<AgentRuntimeRoleEditorDraft>? onRoleUpdate;
  final ValueChanged<String>? onRoleExport;
  final ValueChanged<String>? onRoleArchive;
  final ValueChanged<String>? onRoleUnarchive;
  final AgentRuntimeRoleVersionAction? onRoleActivate;
  final ValueChanged<AgentRuntimeWorkflowMemoryRow>? onWorkflowMemorySelect;
  final ValueChanged<AgentRuntimeWorkflowMemoryDetail>? onWorkflowMemoryAttempted;
  final ValueChanged<AgentRuntimeWorkflowMemoryDetail>? onWorkflowMemoryHelpful;
  final ValueChanged<AgentRuntimeWorkflowMemoryDetail>? onWorkflowMemoryNotHelpful;
  final ValueChanged<String>? onSessionClose;
  final ValueChanged<String>? onSessionArchive;
  final ValueChanged<String>? onSessionFork;
  final ValueChanged<String>? onProcessTerminate;
  final ValueChanged<String>? onProcessInput;
  final ValueChanged<String>? onProcessFlush;
  final ValueChanged<AgentRuntimeActionItem>? onApprovalApprove;
  final ValueChanged<AgentRuntimeActionItem>? onApprovalDeny;
  final ValueChanged<AgentRuntimeActionItem>? onApprovalResume;
  final ValueChanged<AgentRuntimeActionItem>? onCommandRegistryApprove;
  final ValueChanged<AgentRuntimeActionItem>? onCommandRegistryDeny;
  final ValueChanged<AgentRuntimeActionItem>? onCommandRegistryPreview;
  final ValueChanged<AgentRuntimeActionItem>? onCommandRegistryApply;

  @override
  Widget build(BuildContext context) {
    return _DetailsPanel(
      data,
      focusSurfaceId: focusSurfaceId,
      onRoleValidate: onRoleValidate,
      onRoleCreate: onRoleCreate,
      onRoleUpdate: onRoleUpdate,
      onRoleExport: onRoleExport,
      onRoleArchive: onRoleArchive,
      onRoleUnarchive: onRoleUnarchive,
      onRoleActivate: onRoleActivate,
      onWorkflowMemorySelect: onWorkflowMemorySelect,
      onWorkflowMemoryAttempted: onWorkflowMemoryAttempted,
      onWorkflowMemoryHelpful: onWorkflowMemoryHelpful,
      onWorkflowMemoryNotHelpful: onWorkflowMemoryNotHelpful,
      onSessionClose: onSessionClose,
      onSessionArchive: onSessionArchive,
      onSessionFork: onSessionFork,
      onProcessTerminate: onProcessTerminate,
      onProcessInput: onProcessInput,
      onProcessFlush: onProcessFlush,
      onApprovalApprove: onApprovalApprove,
      onApprovalDeny: onApprovalDeny,
      onApprovalResume: onApprovalResume,
      onCommandRegistryApprove: onCommandRegistryApprove,
      onCommandRegistryDeny: onCommandRegistryDeny,
      onCommandRegistryPreview: onCommandRegistryPreview,
      onCommandRegistryApply: onCommandRegistryApply,
    );
  }
}

class _OperationsRow {
  const _OperationsRow(this.label, this.value);
  final String label;
  final String value;
}

class _OperationsSection extends StatelessWidget {
  const _OperationsSection({required this.title, required this.rows, this.actions = const [], this.onActionPressed});

  final String title;
  final List<_OperationsRow> rows;
  final List<AgentRuntimeActionItem> actions;
  final VoidCallback? Function(AgentRuntimeActionItem action)? onActionPressed;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: 14),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(title, style: theme.textTheme.labelLarge?.copyWith(color: Colors.white70, fontWeight: FontWeight.w800)),
          const SizedBox(height: 6),
          if (rows.isEmpty)
            const Text('No items', style: TextStyle(color: Color(0xFF8FA1B8), fontSize: 12))
          else
            for (final row in rows)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 3),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    SizedBox(width: 110, child: Text(_cleanSectionCopy(row.label), style: const TextStyle(color: Color(0xFF8FA1B8), fontSize: 12))),
                    Expanded(child: Text(_cleanSectionCopy(row.value), style: const TextStyle(fontSize: 12))),
                  ],
                ),
              ),
          if (actions.isNotEmpty) ...[
            const SizedBox(height: 8),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                for (final action in actions)
                  OutlinedButton(
                    onPressed: onActionPressed?.call(action),
                    child: Text(_cleanSectionCopy(action.title)),
                  ),
              ],
            ),
          ],
        ],
      ),
    );
  }
}

class _HeaderAction extends StatelessWidget {
  const _HeaderAction({required this.label, required this.onPressed, required this.filled});

  final String label;
  final VoidCallback onPressed;
  final bool filled;

  @override
  Widget build(BuildContext context) {
    return filled ? FilledButton(onPressed: onPressed, child: Text(label)) : OutlinedButton(onPressed: onPressed, child: Text(label));
  }
}

class _StatusDot extends StatelessWidget {
  const _StatusDot({required this.label, required this.tone});

  final String label;
  final String tone;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: BoxDecoration(
        color: _toneColor(tone).withValues(alpha: 0.10),
        border: Border.all(color: _toneColor(tone).withValues(alpha: 0.45)),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Container(width: 6, height: 6, decoration: BoxDecoration(color: _toneColor(tone), shape: BoxShape.circle)),
            const SizedBox(width: 5),
            ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 220),
              child: Text(label, maxLines: 1, overflow: TextOverflow.ellipsis, style: Theme.of(context).textTheme.labelSmall?.copyWith(color: const Color(0xFFE5EDF8))),
            ),
          ],
        ),
      ),
    );
  }
}

class _MutedText extends StatelessWidget {
  const _MutedText(this.label);
  final String label;
  @override
  Widget build(BuildContext context) => Text(label, style: Theme.of(context).textTheme.labelSmall?.copyWith(color: const Color(0xFF8FA1B8)));
}

class _InlineError extends StatelessWidget {
  const _InlineError({required this.message});

  final String message;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: double.infinity,
      margin: const EdgeInsets.fromLTRB(14, 0, 14, 10),
      padding: const EdgeInsets.all(10),
      decoration: BoxDecoration(
        color: const Color(0xFF23131A),
        border: Border.all(color: const Color(0xFF7C3340)),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Text(message, maxLines: 3, overflow: TextOverflow.ellipsis, style: Theme.of(context).textTheme.bodySmall?.copyWith(color: const Color(0xFFFFC9CF))),
    );
  }
}

class _InlineNotice extends StatelessWidget {
  const _InlineNotice({required this.message, required this.tone});

  final String message;
  final String tone;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: BoxDecoration(
        color: _toneColor(tone).withValues(alpha: 0.10),
        border: Border.all(color: _toneColor(tone).withValues(alpha: 0.40)),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Padding(
        padding: const EdgeInsets.all(10),
        child: Text(
          message,
          maxLines: 3,
          overflow: TextOverflow.ellipsis,
          style: Theme.of(context).textTheme.bodySmall?.copyWith(color: const Color(0xFFE5EDF8)),
        ),
      ),
    );
  }
}

String _friendlyError(String error) {
  if (error.contains('flutter_rust_bridge') || error.contains('RustLib') || error.contains('Bad state')) {
    return 'Agent Runtime bridge is not ready. Restart the app, then refresh discovery.';
  }
  if (error.toLowerCase().contains('http') || error.toLowerCase().contains('sync failed')) {
    return 'Runtime did not respond. Check the service, then refresh discovery.';
  }
  return error;
}

bool _isConnected(AgentRuntimeWorkbenchData data) {
  return data.connectionTone == 'success' || data.connectionState == 'streaming' || data.connectionState == 'connected';
}

String _loginStatusLabel(AgentRuntimeWorkbenchData data) {
  if (data.connectionState == 'connecting') {
    return 'Connecting';
  }
  if (data.errorMessage != null) {
    return 'Needs attention';
  }
  return 'Not connected';
}

String _discoveryStateText(AgentRuntimeDiscoveryInfo info) {
  if (info.connectable) {
    return 'Ready to connect';
  }
  switch (info.state) {
    case 'notLoaded':
      return 'Not checked yet';
    case 'noDiscoveryFile':
    case 'missingProfile':
      return 'No profile found';
    case 'malformedProfile':
      return 'Profile needs repair';
    case 'staleProfile':
    case 'staleDiscovery':
      return 'Profile is stale';
    case 'unhealthy':
    case 'remoteUnhealthy':
      return 'Health check failed';
    case 'unreachable':
    case 'remoteUnreachable':
      return 'Runtime unreachable';
    case 'runningHealthy':
    case 'remoteHealthy':
      return 'Ready to connect';
    default:
      return info.title;
  }
}

String _cleanSectionCopy(String value) {
  return value
      .replaceAll('Rust-owned ', '')
      .replaceAll('DB-backed immutable ', '')
      .replaceAll('execute_code/Starlark memories · ', '')
      .replaceAll('Grouped by operational state', 'Sessions needing attention')
      .replaceAll('inspector plus feedback', 'Review saved workflows')
      .replaceAll('Operations event stream, not a chat transcript', 'Recent activity for the selected session')
      .replaceAll('Hydrate the runtime projection to', 'Connect to')
      .replaceAll('Connect to hydrate', 'Connect to load')
      .replaceAll('No runtime action queue is loaded.', 'No items need attention.')
      .replaceAll('Controller detail', 'Runtime detail')
      .replaceAll('controller facts', 'runtime facts')
      .replaceAll('runtime facts', 'Runtime status')
      .trim();
}

bool _isUserFacingFact(AgentRuntimeFact fact) {
  final text = '${fact.label} ${fact.value}'.toLowerCase();
  return !text.contains('rinf') &&
      !text.contains('json packet') &&
      !text.contains('api') &&
      !text.contains('debug') &&
      !text.contains('postgres') &&
      !text.contains('source of truth') &&
      !text.contains('selected session') &&
      !text.contains('connection streaming');
}

String _displayCopy(String value) {
  final next = _cleanSectionCopy(value)
      .replaceAll('tool.call execute_code', 'Execute code')
      .replaceAll('tool.call', 'Tool work')
      .replaceAll('Starlark completed', 'Code run completed')
      .replaceAll('approval.requested', 'Approval requested')
      .replaceAll('cmd.rg.audit', 'Command review')
      .replaceAll('rg · audit', 'Search audit')
      .replaceAll('ownerApproval', 'Owner approval')
      .replaceAll('runtime-allow', 'Runtime allow')
      .replaceAll('Runtime allow · project workspace', 'Project workspace')
      .replaceAll('project:alpha', 'Project alpha')
      .replaceAll('add · Project alpha · Owner approval', 'Project approval needed')
      .replaceAll('project agent-runtime · ', '')
      .replaceAll('Recover generated API drift', 'Review interface changes')
      .replaceAll('Recover generated interface drift', 'Review interface changes')
      .replaceAll('Use bounded output artifacts', 'Use saved output excerpts')
      .replaceAll('generated packet ids', 'saved identifiers')
      .replaceAll('API', 'interface')
      .replaceAll('Dart', 'app')
      .replaceAll('app bindings', 'the app')
      .replaceAll('artifact handle', 'saved output')
      .replaceAll('commandRegistryRequest', 'Registry request')
      .replaceAll('commandRegistry', 'Registry')
      .replaceAll('canDecide=true · canResume=false', 'Decision needed')
      .replaceAll('canDecide=false · canResume=true', 'Ready to resume')
      .replaceAll('canDecide=false · canResume=false', 'Waiting')
      .replaceAll(RegExp(r'can[A-Za-z]+=(true|false)( · )?'), '')
      .replaceAll(RegExp(r'\bcmd\.'), '')
      .replaceAll(RegExp(r'\btool\.'), '')
      .replaceAll(RegExp(r'/Users/[^ ]+'), 'project workspace')
      .replaceAll('Runtime allow · project workspace', 'Project workspace')
      .replaceAll(RegExp(r'\s+·\s+$'), '')
      .trim();
  return next.isEmpty ? 'Waiting' : next;
}

class _Panel extends StatelessWidget {
  const _Panel({required this.title, required this.child, this.subtitle});

  final String title;
  final String? subtitle;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.all(12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(title, style: theme.textTheme.labelLarge?.copyWith(color: Colors.white, fontWeight: FontWeight.w700)),
          if (subtitle case final subtitle?)
            Padding(
              padding: const EdgeInsets.only(top: 3),
              child: Text(subtitle, style: theme.textTheme.bodySmall?.copyWith(color: const Color(0xFF8091A8))),
            ),
          const SizedBox(height: 12),
          Expanded(child: child),
        ],
      ),
    );
  }
}

class _SessionTile extends StatelessWidget {
  const _SessionTile(this.item);

  final AgentRuntimeSessionItem item;

  @override
  Widget build(BuildContext context) {
    return _DenseTile(
      title: item.title,
      subtitle: item.subtitle,
      trailing: item.status,
      eyebrow: item.groupLabel,
      tone: item.tone,
    );
  }
}

class _TimelineTile extends StatelessWidget {
  const _TimelineTile(this.item);

  final AgentRuntimeTimelineItem item;

  @override
  Widget build(BuildContext context) {
    return _DenseTile(
      title: item.title,
      subtitle: item.subtitle,
      trailing: item.status,
      eyebrow: item.status,
      tone: item.tone,
    );
  }
}

class _ActionTile extends StatelessWidget {
  const _ActionTile(
    this.item, {
    this.onApprovalApprove,
    this.onApprovalDeny,
    this.onApprovalResume,
    this.onCommandRegistryApprove,
    this.onCommandRegistryDeny,
    this.onCommandRegistryPreview,
    this.onCommandRegistryApply,
  });

  final AgentRuntimeActionItem item;
  final VoidCallback? onApprovalApprove;
  final VoidCallback? onApprovalDeny;
  final VoidCallback? onApprovalResume;
  final VoidCallback? onCommandRegistryApprove;
  final VoidCallback? onCommandRegistryDeny;
  final VoidCallback? onCommandRegistryPreview;
  final VoidCallback? onCommandRegistryApply;

  @override
  Widget build(BuildContext context) {
    final actions = <Widget>[];
    if (item.kind == 'approval') {
      actions.add(OutlinedButton(onPressed: onApprovalApprove, child: const Text('Approve')));
    } else if (item.kind == 'approvalDeny') {
      actions.add(OutlinedButton(onPressed: onApprovalDeny, child: const Text('Deny')));
    } else if (item.kind == 'approvalResume') {
      actions.add(OutlinedButton(onPressed: onApprovalResume, child: const Text('Resume')));
    } else if (item.kind == 'commandRegistryRequest') {
      actions.add(OutlinedButton(onPressed: onCommandRegistryApprove, child: const Text('Approve')));
    } else if (item.kind == 'commandRegistryDeny') {
      actions.add(OutlinedButton(onPressed: onCommandRegistryDeny, child: const Text('Deny')));
    } else if (item.kind == 'commandRegistryPreview') {
      actions.add(OutlinedButton(onPressed: onCommandRegistryPreview, child: const Text('Preview Decision')));
    } else if (item.kind == 'commandRegistryApply') {
      actions.add(OutlinedButton(onPressed: onCommandRegistryApply, child: const Text('Apply')));
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _DenseTile(
          title: item.title,
          subtitle: item.subtitle,
          trailing: item.stateText,
          eyebrow: item.kind,
          tone: item.tone,
        ),
        if (actions.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(top: 4, bottom: 8),
            child: Wrap(spacing: 6, runSpacing: 4, children: actions),
          ),
      ],
    );
  }
}

class _WorkflowMemoryPanel extends StatelessWidget {
  const _WorkflowMemoryPanel(
    this.workflowMemory, {
    this.onSelect,
    this.onAttempted,
    this.onHelpful,
    this.onNotHelpful,
  });

  final AgentRuntimeWorkflowMemoryData workflowMemory;
  final ValueChanged<AgentRuntimeWorkflowMemoryRow>? onSelect;
  final ValueChanged<AgentRuntimeWorkflowMemoryDetail>? onAttempted;
  final ValueChanged<AgentRuntimeWorkflowMemoryDetail>? onHelpful;
  final ValueChanged<AgentRuntimeWorkflowMemoryDetail>? onNotHelpful;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final detail = workflowMemory.selectedDetail;
    return Padding(
        padding: const EdgeInsets.symmetric(vertical: 4),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(workflowMemory.title, style: theme.textTheme.labelLarge?.copyWith(color: Colors.white, fontWeight: FontWeight.w700)),
            const SizedBox(height: 3),
            Text(_cleanSectionCopy(workflowMemory.subtitle), style: theme.textTheme.bodySmall?.copyWith(color: const Color(0xFF8FA1B8))),
            const SizedBox(height: 10),
            if (workflowMemory.rows.isEmpty)
              _EmptyState(title: workflowMemory.emptyTitle, body: _cleanSectionCopy(workflowMemory.emptyText))
            else ...[
              ...workflowMemory.rows.take(4).map(
                    (row) => Padding(
                      padding: const EdgeInsets.only(bottom: 6),
                      child: InkWell(
                        borderRadius: BorderRadius.circular(8),
                        onTap: onSelect == null ? null : () => onSelect!(row),
                        child: _DenseTile(
                          title: row.title,
                          subtitle: row.subtitle,
                          trailing: row.selected ? 'selected' : 'available',
                          eyebrow: row.scopeType,
                          tone: row.selected ? 'success' : row.tone,
                        ),
                      ),
                    ),
                  ),
              if (detail != null) ...[
                const SizedBox(height: 8),
                Wrap(
                  spacing: 6,
                  runSpacing: 6,
                  children: [
                    OutlinedButton(onPressed: detail.feedbackEnabled && onAttempted != null ? () => onAttempted!(detail) : null, child: const Text('Attempted')),
                    OutlinedButton(onPressed: detail.feedbackEnabled && onHelpful != null ? () => onHelpful!(detail) : null, child: const Text('Helpful')),
                    OutlinedButton(onPressed: detail.feedbackEnabled && onNotHelpful != null ? () => onNotHelpful!(detail) : null, child: const Text('Not helpful')),
                  ],
                ),
              ],
            ],
          ],
        ),
    );
  }
}

class _ReadOnlyCodeBlock extends StatefulWidget {
  const _ReadOnlyCodeBlock({required this.source});

  final String source;

  @override
  State<_ReadOnlyCodeBlock> createState() => _ReadOnlyCodeBlockState();
}

class _ReadOnlyCodeBlockState extends State<_ReadOnlyCodeBlock> {
  late final TextEditingController _controller;

  @override
  void initState() {
    super.initState();
    _controller = TextEditingController(text: widget.source);
  }

  @override
  void didUpdateWidget(covariant _ReadOnlyCodeBlock oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.source != widget.source) {
      _controller.text = widget.source;
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return ClipRRect(
      borderRadius: BorderRadius.circular(8),
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: const Color(0xFF07101A),
          border: Border.all(color: const Color(0xFF1F2B3A)),
        ),
        child: AgentRuntimeCodeEditor(controller: _controller, readOnly: true),
      ),
    );
  }
}

class _RoleAdminPanel extends StatefulWidget {
  const _RoleAdminPanel(
    this.roleAdmin, {
    this.onValidate,
    this.onCreate,
    this.onUpdate,
    this.onExport,
    this.onArchive,
    this.onUnarchive,
    this.onActivate,
  });

  final AgentRuntimeRoleAdminData roleAdmin;
  final ValueChanged<AgentRuntimeRoleEditorDraft>? onValidate;
  final ValueChanged<AgentRuntimeRoleEditorDraft>? onCreate;
  final ValueChanged<AgentRuntimeRoleEditorDraft>? onUpdate;
  final ValueChanged<String>? onExport;
  final ValueChanged<String>? onArchive;
  final ValueChanged<String>? onUnarchive;
  final AgentRuntimeRoleVersionAction? onActivate;

  @override
  State<_RoleAdminPanel> createState() => _RoleAdminPanelState();
}

class _RoleAdminPanelState extends State<_RoleAdminPanel> {
  late final TextEditingController _roleIdController;
  late final TextEditingController _versionController;
  late final TextEditingController _displayNameController;
  late final TextEditingController _modelController;
  late final TextEditingController _reasoningController;
  late final TextEditingController _capabilitiesController;
  late final TextEditingController _policyController;
  late final TextEditingController _routingModeController;
  late final TextEditingController _defaultRecipientController;
  late final TextEditingController _allowedRecipientsController;
  late final TextEditingController _routingReservedController;
  late final TextEditingController _lifecycleReservedController;
  late final TextEditingController _instructionController;
  bool _listed = true;
  bool _ownerVisible = true;
  bool _canSpawnAgents = false;
  bool _canArchiveAgents = false;
  bool _showEditor = false;
  String? _loadedDraftKey;

  @override
  void initState() {
    super.initState();
    _roleIdController = TextEditingController();
    _versionController = TextEditingController();
    _displayNameController = TextEditingController();
    _modelController = TextEditingController();
    _reasoningController = TextEditingController();
    _capabilitiesController = TextEditingController();
    _policyController = TextEditingController();
    _routingModeController = TextEditingController();
    _defaultRecipientController = TextEditingController();
    _allowedRecipientsController = TextEditingController();
    _routingReservedController = TextEditingController();
    _lifecycleReservedController = TextEditingController();
    _instructionController = TextEditingController();
    _loadDraft(widget.roleAdmin.editorDraft);
  }

  @override
  void didUpdateWidget(covariant _RoleAdminPanel oldWidget) {
    super.didUpdateWidget(oldWidget);
    _loadDraft(widget.roleAdmin.editorDraft);
  }

  @override
  void dispose() {
    _roleIdController.dispose();
    _versionController.dispose();
    _displayNameController.dispose();
    _modelController.dispose();
    _reasoningController.dispose();
    _capabilitiesController.dispose();
    _policyController.dispose();
    _routingModeController.dispose();
    _defaultRecipientController.dispose();
    _allowedRecipientsController.dispose();
    _routingReservedController.dispose();
    _lifecycleReservedController.dispose();
    _instructionController.dispose();
    super.dispose();
  }

  void _loadDraft(AgentRuntimeRoleEditorDraft? draft) {
    final key = draft == null
        ? '__empty__'
        : '${draft.roleId}|${draft.version}|${draft.instructionText.hashCode}|${draft.policy.length}|${draft.capabilities.length}';
    if (_loadedDraftKey == key) {
      return;
    }
    _loadedDraftKey = key;
    final next = draft ?? const AgentRuntimeRoleEditorDraft(
      roleId: 'new-runtime-role',
      version: '1.0.0',
      displayName: 'New Runtime Role',
      model: 'gpt-5.4-mini',
      reasoningEffort: 'medium',
      instructionText: 'Write role instructions here.',
      capabilities: ['tool.execute_code'],
      policy: [AgentRuntimeRolePolicyRow(action: 'tool.execute_code', decision: 'allow')],
      routingMode: 'direct',
      routingReservedActions: ['message.send'],
      defaultRecipient: 'owner',
      allowedRecipients: ['owner'],
      listed: true,
      ownerVisible: true,
      canSpawnAgents: false,
      canArchiveAgents: false,
      lifecycleReservedActions: ['agent.archive'],
    );
    _roleIdController.text = next.roleId;
    _versionController.text = next.version;
    _displayNameController.text = next.displayName;
    _modelController.text = next.model;
    _reasoningController.text = next.reasoningEffort;
    _instructionController.text = next.instructionText;
    _capabilitiesController.text = next.capabilities.join('\n');
    _policyController.text = next.policy.map((row) => '${row.action}=${row.decision}').join('\n');
    _routingModeController.text = next.routingMode;
    _defaultRecipientController.text = next.defaultRecipient ?? '';
    _allowedRecipientsController.text = next.allowedRecipients.join('\n');
    _routingReservedController.text = next.routingReservedActions.join('\n');
    _lifecycleReservedController.text = next.lifecycleReservedActions.join('\n');
    _listed = next.listed;
    _ownerVisible = next.ownerVisible;
    _canSpawnAgents = next.canSpawnAgents;
    _canArchiveAgents = next.canArchiveAgents;
  }

  AgentRuntimeRoleEditorDraft _editedDraft() {
    return AgentRuntimeRoleEditorDraft(
      roleId: _roleIdController.text.trim(),
      version: _versionController.text.trim(),
      displayName: _displayNameController.text.trim(),
      model: _modelController.text.trim(),
      reasoningEffort: _reasoningController.text.trim(),
      instructionText: _instructionController.text,
      capabilities: _lines(_capabilitiesController.text),
      policy: _policyRows(_policyController.text),
      routingMode: _routingModeController.text.trim().isEmpty ? 'direct' : _routingModeController.text.trim(),
      routingReservedActions: _lines(_routingReservedController.text),
      defaultRecipient: _defaultRecipientController.text.trim().isEmpty ? null : _defaultRecipientController.text.trim(),
      allowedRecipients: _lines(_allowedRecipientsController.text),
      listed: _listed,
      ownerVisible: _ownerVisible,
      canSpawnAgents: _canSpawnAgents,
      canArchiveAgents: _canArchiveAgents,
      lifecycleReservedActions: _lines(_lifecycleReservedController.text),
    );
  }

  List<String> _lines(String value) {
    return value
        .split(RegExp(r'[\n,]'))
        .map((item) => item.trim())
        .where((item) => item.isNotEmpty)
        .toList(growable: false);
  }

  List<AgentRuntimeRolePolicyRow> _policyRows(String value) {
    return value
        .split('\n')
        .map((line) => line.trim())
        .where((line) => line.isNotEmpty)
        .map((line) {
          final separator = line.contains('=') ? '=' : ':';
          final parts = line.split(separator);
          return AgentRuntimeRolePolicyRow(
            action: parts.first.trim(),
            decision: parts.length > 1 ? parts.sublist(1).join(separator).trim() : 'deny',
          );
        })
        .toList(growable: false);
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final roleAdmin = widget.roleAdmin;
    final detail = roleAdmin.selectedDetail;
    return Padding(
        padding: const EdgeInsets.symmetric(vertical: 4),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(roleAdmin.title, style: theme.textTheme.labelLarge?.copyWith(color: Colors.white, fontWeight: FontWeight.w700)),
            const SizedBox(height: 3),
            Text(_cleanSectionCopy(roleAdmin.subtitle), style: theme.textTheme.bodySmall?.copyWith(color: const Color(0xFF8FA1B8))),
            const SizedBox(height: 10),
            if (roleAdmin.rows.isEmpty)
              _EmptyState(title: roleAdmin.emptyTitle, body: _cleanSectionCopy(roleAdmin.emptyText))
            else ...[
              ...roleAdmin.rows.take(4).map((row) => Padding(
                    padding: const EdgeInsets.only(bottom: 6),
                    child: _DenseTile(
                      title: row.title,
                      subtitle: row.status == 'active' ? 'Current role version' : 'Available role version',
                      trailing: row.status,
                      eyebrow: 'role',
                      tone: row.tone,
                    ),
                  )),
              if (detail != null) ...[
                const SizedBox(height: 8),
                _FactRow(AgentRuntimeFact(label: 'Selected role', value: '${detail.displayName} · ${detail.version}')),
              ],
              if (roleAdmin.versionRows.isNotEmpty) ...[
                const SizedBox(height: 8),
                Text('Immutable versions', style: theme.textTheme.labelMedium?.copyWith(color: Colors.white70)),
                const SizedBox(height: 6),
                ...roleAdmin.versionRows.map(
                  (version) => Padding(
                    padding: const EdgeInsets.only(bottom: 6),
                    child: _RoleVersionTile(
                      row: version,
                      roleId: detail?.id ?? _roleIdController.text.trim(),
                      onActivate: widget.onActivate,
                    ),
                  ),
                ),
              ],
              const SizedBox(height: 8),
              OutlinedButton(
                onPressed: () => setState(() => _showEditor = !_showEditor),
                child: Text(_showEditor ? 'Hide editor' : 'Edit role'),
              ),
              if (_showEditor) ...[
                const SizedBox(height: 8),
                _RoleDraftEditor(
                  instructionController: _instructionController,
                  roleIdController: _roleIdController,
                  versionController: _versionController,
                  displayNameController: _displayNameController,
                  modelController: _modelController,
                  reasoningController: _reasoningController,
                  capabilitiesController: _capabilitiesController,
                  policyController: _policyController,
                  routingModeController: _routingModeController,
                  defaultRecipientController: _defaultRecipientController,
                  allowedRecipientsController: _allowedRecipientsController,
                  routingReservedController: _routingReservedController,
                  lifecycleReservedController: _lifecycleReservedController,
                  listed: _listed,
                  ownerVisible: _ownerVisible,
                  canSpawnAgents: _canSpawnAgents,
                  canArchiveAgents: _canArchiveAgents,
                  onListedChanged: (value) => setState(() => _listed = value),
                  onOwnerVisibleChanged: (value) => setState(() => _ownerVisible = value),
                  onCanSpawnAgentsChanged: (value) => setState(() => _canSpawnAgents = value),
                  onCanArchiveAgentsChanged: (value) => setState(() => _canArchiveAgents = value),
                ),
                const SizedBox(height: 8),
                Wrap(
                  spacing: 6,
                  runSpacing: 6,
                  children: [
                    OutlinedButton(onPressed: widget.onValidate == null ? null : () => widget.onValidate!(_editedDraft()), child: const Text('Validate')),
                    OutlinedButton(onPressed: widget.onCreate == null ? null : () => widget.onCreate!(_editedDraft()), child: const Text('Create')),
                    OutlinedButton(onPressed: widget.onUpdate == null ? null : () => widget.onUpdate!(_editedDraft()), child: const Text('Update')),
                    OutlinedButton(onPressed: widget.onExport == null ? null : () => widget.onExport!(_editedDraft().roleId), child: const Text('Export')),
                    OutlinedButton(onPressed: widget.onArchive == null ? null : () => widget.onArchive!(_editedDraft().roleId), child: const Text('Archive')),
                    OutlinedButton(onPressed: widget.onUnarchive == null ? null : () => widget.onUnarchive!(_editedDraft().roleId), child: const Text('Unarchive')),
                  ],
                ),
              ],
              if (roleAdmin.validationErrors.isNotEmpty) ...[
                const SizedBox(height: 8),
                ...roleAdmin.validationErrors.map(
                  (error) => Text(error, style: theme.textTheme.bodySmall?.copyWith(color: const Color(0xFFFFA8B4))),
                ),
              ],
            ],
          ],
        ),
    );
  }
}

class _RoleDraftEditor extends StatelessWidget {
  const _RoleDraftEditor({
    required this.instructionController,
    required this.roleIdController,
    required this.versionController,
    required this.displayNameController,
    required this.modelController,
    required this.reasoningController,
    required this.capabilitiesController,
    required this.policyController,
    required this.routingModeController,
    required this.defaultRecipientController,
    required this.allowedRecipientsController,
    required this.routingReservedController,
    required this.lifecycleReservedController,
    required this.listed,
    required this.ownerVisible,
    required this.canSpawnAgents,
    required this.canArchiveAgents,
    required this.onListedChanged,
    required this.onOwnerVisibleChanged,
    required this.onCanSpawnAgentsChanged,
    required this.onCanArchiveAgentsChanged,
  });

  final TextEditingController instructionController;
  final TextEditingController roleIdController;
  final TextEditingController versionController;
  final TextEditingController displayNameController;
  final TextEditingController modelController;
  final TextEditingController reasoningController;
  final TextEditingController capabilitiesController;
  final TextEditingController policyController;
  final TextEditingController routingModeController;
  final TextEditingController defaultRecipientController;
  final TextEditingController allowedRecipientsController;
  final TextEditingController routingReservedController;
  final TextEditingController lifecycleReservedController;
  final bool listed;
  final bool ownerVisible;
  final bool canSpawnAgents;
  final bool canArchiveAgents;
  final ValueChanged<bool> onListedChanged;
  final ValueChanged<bool> onOwnerVisibleChanged;
  final ValueChanged<bool> onCanSpawnAgentsChanged;
  final ValueChanged<bool> onCanArchiveAgentsChanged;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Expanded(child: _EditorTextField(label: 'Role key', controller: roleIdController)),
            const SizedBox(width: 6),
            Expanded(child: _EditorTextField(label: 'Version', controller: versionController)),
          ],
        ),
        const SizedBox(height: 6),
        _EditorTextField(label: 'Display name', controller: displayNameController),
        const SizedBox(height: 6),
        Row(
          children: [
            Expanded(child: _EditorTextField(label: 'Model', controller: modelController)),
            const SizedBox(width: 6),
            Expanded(child: _EditorTextField(label: 'Reasoning effort', controller: reasoningController)),
          ],
        ),
        const SizedBox(height: 6),
        _EditorTextField(label: 'Capabilities', controller: capabilitiesController, maxLines: 3),
        const SizedBox(height: 6),
        _EditorTextField(label: 'Permission rules', controller: policyController, maxLines: 4),
        const SizedBox(height: 6),
        Row(
          children: [
            Expanded(child: _EditorTextField(label: 'Routing mode', controller: routingModeController)),
            const SizedBox(width: 6),
            Expanded(child: _EditorTextField(label: 'Default recipient', controller: defaultRecipientController)),
          ],
        ),
        const SizedBox(height: 6),
        _EditorTextField(label: 'Allowed recipients', controller: allowedRecipientsController, maxLines: 2),
        const SizedBox(height: 6),
        _EditorTextField(label: 'Reserved routing', controller: routingReservedController, maxLines: 2),
        const SizedBox(height: 6),
        _EditorTextField(label: 'Reserved lifecycle', controller: lifecycleReservedController, maxLines: 2),
        const SizedBox(height: 6),
        Wrap(
          spacing: 12,
          runSpacing: 2,
          children: [
            _EditorSwitch(label: 'Listed', value: listed, onChanged: onListedChanged),
            _EditorSwitch(label: 'Owner visible', value: ownerVisible, onChanged: onOwnerVisibleChanged),
            _EditorSwitch(label: 'Can spawn agents', value: canSpawnAgents, onChanged: onCanSpawnAgentsChanged),
            _EditorSwitch(label: 'Can archive agents', value: canArchiveAgents, onChanged: onCanArchiveAgentsChanged),
          ],
        ),
        const SizedBox(height: 6),
        Text('Instructions', style: theme.textTheme.labelSmall?.copyWith(color: const Color(0xFF93A5BC))),
        const SizedBox(height: 4),
        SizedBox(
          height: 120,
          child: ClipRRect(
            borderRadius: BorderRadius.circular(8),
            child: DecoratedBox(
              decoration: const BoxDecoration(color: Color(0xFF07101A)),
              child: AgentRuntimeCodeEditor(
                controller: instructionController,
              ),
            ),
          ),
        ),
      ],
    );
  }
}

class _EditorTextField extends StatelessWidget {
  const _EditorTextField({required this.label, required this.controller, this.maxLines = 1});

  final String label;
  final TextEditingController controller;
  final int maxLines;

  @override
  Widget build(BuildContext context) {
    return TextField(
      controller: controller,
      maxLines: maxLines,
      minLines: 1,
      style: const TextStyle(color: Color(0xFFE5EDF8), fontSize: 12),
      decoration: InputDecoration(
        labelText: label,
        isDense: true,
      ),
    );
  }
}

class _EditorSwitch extends StatelessWidget {
  const _EditorSwitch({required this.label, required this.value, required this.onChanged});

  final String label;
  final bool value;
  final ValueChanged<bool> onChanged;

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Switch(value: value, onChanged: onChanged),
        Text(label, style: Theme.of(context).textTheme.bodySmall?.copyWith(color: const Color(0xFFB7C4D8))),
      ],
    );
  }
}

class _RoleVersionTile extends StatelessWidget {
  const _RoleVersionTile({
    required this.row,
    required this.roleId,
    this.onActivate,
  });

  final AgentRuntimeRoleVersionRow row;
  final String roleId;
  final AgentRuntimeRoleVersionAction? onActivate;

  bool get _isCurrent => row.status == 'current' || row.status == 'active';

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final createdAt = row.createdAt == null || row.createdAt!.isEmpty ? 'created time unavailable' : row.createdAt!;
    return Padding(
        padding: const EdgeInsets.fromLTRB(0, 8, 0, 8),
        child: Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    'version ${row.version}',
                    style: theme.textTheme.bodySmall?.copyWith(color: Colors.white, fontWeight: FontWeight.w700),
                  ),
                  const SizedBox(height: 2),
                  Text(
                    createdAt,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: theme.textTheme.bodySmall?.copyWith(color: const Color(0xFF8FA1B8)),
                  ),
                ],
              ),
            ),
            const SizedBox(width: 8),
            _Chip(label: row.status, tone: _isCurrent ? 'success' : 'info'),
            const SizedBox(width: 6),
            OutlinedButton(
              onPressed: _isCurrent || roleId.isEmpty || onActivate == null ? null : () => onActivate!(roleId, row.versionId),
              child: const Text('Activate'),
            ),
          ],
        ),
    );
  }
}

class _DenseTile extends StatelessWidget {
  const _DenseTile({
    required this.title,
    required this.subtitle,
    required this.trailing,
    required this.eyebrow,
    required this.tone,
  });

  final String title;
  final String subtitle;
  final String trailing;
  final String eyebrow;
  final String tone;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final displayTitle = _displayCopy(title);
    final displaySubtitle = _displayCopy(subtitle);
    final displayTrailing = _displayCopy(trailing);
    final displayEyebrow = _displayCopy(eyebrow);
    return Padding(
        padding: const EdgeInsets.fromLTRB(0, 9, 0, 9),
        child: Row(
          children: [
            Container(width: 3, height: 54, color: _toneColor(tone)),
            const SizedBox(width: 10),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(displayEyebrow.toUpperCase(), maxLines: 1, overflow: TextOverflow.ellipsis, style: theme.textTheme.labelSmall?.copyWith(color: _toneColor(tone), letterSpacing: 0.5)),
                  const SizedBox(height: 3),
                  Text(displayTitle, maxLines: 1, overflow: TextOverflow.ellipsis, style: theme.textTheme.bodyMedium?.copyWith(color: Colors.white)),
                  const SizedBox(height: 3),
                  Text(displaySubtitle, maxLines: 2, overflow: TextOverflow.ellipsis, style: theme.textTheme.bodySmall?.copyWith(color: const Color(0xFF97A6BA))),
                ],
              ),
            ),
            const SizedBox(width: 8),
            ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 110),
              child: Text(
                displayTrailing,
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                textAlign: TextAlign.right,
                style: theme.textTheme.labelSmall?.copyWith(color: _toneColor(tone), fontWeight: FontWeight.w700),
              ),
            ),
          ],
        ),
    );
  }
}

class _FactRow extends StatelessWidget {
  const _FactRow(this.fact);

  final AgentRuntimeFact fact;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: Row(
        children: [
          Expanded(child: Text(_displayCopy(fact.label), style: theme.textTheme.bodySmall?.copyWith(color: const Color(0xFF7D8DA3)))),
          Flexible(child: Text(_displayCopy(fact.value), textAlign: TextAlign.right, overflow: TextOverflow.ellipsis, style: theme.textTheme.bodySmall?.copyWith(color: Colors.white))),
        ],
      ),
    );
  }
}

class _Chip extends StatelessWidget {
  const _Chip({required this.label, this.tone = 'info'});

  final String label;
  final String tone;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
      decoration: BoxDecoration(
        color: _toneColor(tone).withValues(alpha: 0.14),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: _toneColor(tone).withValues(alpha: 0.55)),
      ),
      child: Text(label, style: Theme.of(context).textTheme.labelSmall?.copyWith(color: const Color(0xFFE5EDF8))),
    );
  }
}

class _EmptyState extends StatelessWidget {
  const _EmptyState({required this.title, required this.body});

  final String title;
  final String body;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(14),
        child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                title,
                textAlign: TextAlign.center,
                style: Theme.of(context).textTheme.labelLarge?.copyWith(color: Colors.white),
              ),
              const SizedBox(height: 6),
              Text(
                body,
                textAlign: TextAlign.center,
                style: Theme.of(context).textTheme.bodySmall?.copyWith(color: const Color(0xFF7E8DA2)),
              ),
            ],
          ),
      ),
    );
  }
}

Color _toneColor(String tone) {
  switch (tone) {
    case 'success':
      return const Color(0xFF4FD18B);
    case 'warning':
      return const Color(0xFFE6B450);
    case 'danger':
      return const Color(0xFFFF6B7A);
    case 'muted':
      return const Color(0xFF6E7F95);
    default:
      return const Color(0xFF74B5FF);
  }
}
