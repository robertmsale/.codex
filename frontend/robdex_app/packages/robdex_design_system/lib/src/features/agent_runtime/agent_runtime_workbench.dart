import 'package:flutter/material.dart';

import '../../core/models/agent_runtime_workbench_models.dart';
import 'agent_runtime_code_editor.dart';

typedef AgentRuntimeRoleVersionAction = void Function(String roleId, String versionId);
typedef AgentRuntimeActionReason = void Function(AgentRuntimeActionItem action, String reason);
typedef AgentRuntimeProcessInputAction = void Function(String handle, String text);
typedef AgentRuntimeCommandRegistryDecisionAction = void Function(
  AgentRuntimeActionItem action,
  AgentRuntimeCommandRegistryDecisionDraft decision,
);
typedef AgentRuntimeCommandRegistryListAction = void Function(String sessionId, String projectKey);
typedef AgentRuntimeCommandRegistryShowCommandAction = void Function(AgentRuntimeActionItem action, String sessionId, String projectKey);

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
    this.onRoleShowDetail,
    this.onRoleShowVersions,
    this.onRoleShowVersionData,
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
    this.onCompactSession,
    this.onGodModeGrant,
    this.onGodModeRevoke,
    this.onApprovalApprove,
    this.onApprovalDeny,
    this.onApprovalResume,
    this.onCommandRegistryApprove,
    this.onCommandRegistryDeny,
    this.onCommandRegistryPreview,
    this.onCommandRegistryApply,
    this.onCommandRegistryReview,
    this.onCommandRegistryShowCommand,
    this.onCommandRegistryListInstalled,
    this.onCommandRegistryListRequests,
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
  final ValueChanged<String>? onRoleShowDetail;
  final ValueChanged<String>? onRoleShowVersions;
  final ValueChanged<String>? onRoleShowVersionData;
  final ValueChanged<AgentRuntimeWorkflowMemoryRow>? onWorkflowMemorySelect;
  final ValueChanged<AgentRuntimeWorkflowMemoryDetail>? onWorkflowMemoryAttempted;
  final ValueChanged<AgentRuntimeWorkflowMemoryDetail>? onWorkflowMemoryHelpful;
  final ValueChanged<AgentRuntimeWorkflowMemoryDetail>? onWorkflowMemoryNotHelpful;
  final ValueChanged<String>? onSessionClose;
  final ValueChanged<String>? onSessionArchive;
  final ValueChanged<String>? onSessionFork;
  final ValueChanged<String>? onProcessTerminate;
  final AgentRuntimeProcessInputAction? onProcessInput;
  final ValueChanged<String>? onProcessFlush;
  final ValueChanged<AgentRuntimeActionItem>? onCompactSession;
  final ValueChanged<AgentRuntimeActionItem>? onGodModeGrant;
  final ValueChanged<AgentRuntimeActionItem>? onGodModeRevoke;
  final AgentRuntimeActionReason? onApprovalApprove;
  final AgentRuntimeActionReason? onApprovalDeny;
  final ValueChanged<AgentRuntimeActionItem>? onApprovalResume;
  final AgentRuntimeCommandRegistryDecisionAction? onCommandRegistryApprove;
  final AgentRuntimeCommandRegistryDecisionAction? onCommandRegistryDeny;
  final AgentRuntimeCommandRegistryDecisionAction? onCommandRegistryPreview;
  final ValueChanged<AgentRuntimeActionItem>? onCommandRegistryApply;
  final ValueChanged<AgentRuntimeActionItem>? onCommandRegistryReview;
  final AgentRuntimeCommandRegistryShowCommandAction? onCommandRegistryShowCommand;
  final AgentRuntimeCommandRegistryListAction? onCommandRegistryListInstalled;
  final VoidCallback? onCommandRegistryListRequests;

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
                                  onRoleShowDetail: onRoleShowDetail,
                                  onRoleShowVersions: onRoleShowVersions,
                                  onRoleShowVersionData: onRoleShowVersionData,
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
                                  onCompactSession: onCompactSession,
                                  onGodModeGrant: onGodModeGrant,
                                  onGodModeRevoke: onGodModeRevoke,
                                  onApprovalApprove: onApprovalApprove,
                                  onApprovalDeny: onApprovalDeny,
                                  onApprovalResume: onApprovalResume,
                                  onCommandRegistryApprove: onCommandRegistryApprove,
                                  onCommandRegistryDeny: onCommandRegistryDeny,
                                  onCommandRegistryPreview: onCommandRegistryPreview,
                                  onCommandRegistryApply: onCommandRegistryApply,
                                  onCommandRegistryReview: onCommandRegistryReview,
                                  onCommandRegistryShowCommand: onCommandRegistryShowCommand,
                                  onCommandRegistryListInstalled: onCommandRegistryListInstalled,
                                  onCommandRegistryListRequests: onCommandRegistryListRequests)),
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
                                          onRoleShowDetail: onRoleShowDetail,
                                          onRoleShowVersions: onRoleShowVersions,
                                          onRoleShowVersionData: onRoleShowVersionData,
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
                                          onCompactSession: onCompactSession,
                                          onGodModeGrant: onGodModeGrant,
                                          onGodModeRevoke: onGodModeRevoke,
                                          onApprovalApprove: onApprovalApprove,
                                          onApprovalDeny: onApprovalDeny,
                                          onApprovalResume: onApprovalResume,
                                          onCommandRegistryApprove: onCommandRegistryApprove,
                                          onCommandRegistryDeny: onCommandRegistryDeny,
                                          onCommandRegistryPreview: onCommandRegistryPreview,
                                          onCommandRegistryApply: onCommandRegistryApply,
                                          onCommandRegistryReview: onCommandRegistryReview,
                                          onCommandRegistryShowCommand: onCommandRegistryShowCommand,
                                          onCommandRegistryListInstalled: onCommandRegistryListInstalled,
                                          onCommandRegistryListRequests: onCommandRegistryListRequests),
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
      decoration: const InputDecoration(labelText: 'Runtime URL', helperText: 'Manual connection when discovery is unavailable'),
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
    this.onRoleShowDetail,
    this.onRoleShowVersions,
    this.onRoleShowVersionData,
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
    this.onCompactSession,
    this.onGodModeGrant,
    this.onGodModeRevoke,
    this.onApprovalApprove,
    this.onApprovalDeny,
    this.onApprovalResume,
    this.onCommandRegistryApprove,
    this.onCommandRegistryDeny,
    this.onCommandRegistryPreview,
    this.onCommandRegistryApply,
    this.onCommandRegistryReview,
    this.onCommandRegistryShowCommand,
    this.onCommandRegistryListInstalled,
    this.onCommandRegistryListRequests,
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
  final ValueChanged<String>? onRoleShowDetail;
  final ValueChanged<String>? onRoleShowVersions;
  final ValueChanged<String>? onRoleShowVersionData;
  final ValueChanged<AgentRuntimeWorkflowMemoryRow>? onWorkflowMemorySelect;
  final ValueChanged<AgentRuntimeWorkflowMemoryDetail>? onWorkflowMemoryAttempted;
  final ValueChanged<AgentRuntimeWorkflowMemoryDetail>? onWorkflowMemoryHelpful;
  final ValueChanged<AgentRuntimeWorkflowMemoryDetail>? onWorkflowMemoryNotHelpful;
  final ValueChanged<String>? onSessionClose;
  final ValueChanged<String>? onSessionArchive;
  final ValueChanged<String>? onSessionFork;
  final ValueChanged<String>? onProcessTerminate;
  final AgentRuntimeProcessInputAction? onProcessInput;
  final ValueChanged<String>? onProcessFlush;
  final ValueChanged<AgentRuntimeActionItem>? onCompactSession;
  final ValueChanged<AgentRuntimeActionItem>? onGodModeGrant;
  final ValueChanged<AgentRuntimeActionItem>? onGodModeRevoke;
  final AgentRuntimeActionReason? onApprovalApprove;
  final AgentRuntimeActionReason? onApprovalDeny;
  final ValueChanged<AgentRuntimeActionItem>? onApprovalResume;
  final AgentRuntimeCommandRegistryDecisionAction? onCommandRegistryApprove;
  final AgentRuntimeCommandRegistryDecisionAction? onCommandRegistryDeny;
  final AgentRuntimeCommandRegistryDecisionAction? onCommandRegistryPreview;
  final ValueChanged<AgentRuntimeActionItem>? onCommandRegistryApply;
  final ValueChanged<AgentRuntimeActionItem>? onCommandRegistryReview;
  final AgentRuntimeCommandRegistryShowCommandAction? onCommandRegistryShowCommand;
  final AgentRuntimeCommandRegistryListAction? onCommandRegistryListInstalled;
  final VoidCallback? onCommandRegistryListRequests;

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
                  onProcessInput: onProcessInput,
                  onApprovalApprove: onApprovalApprove,
                  onApprovalDeny: onApprovalDeny,
                  onCommandRegistryApprove: onCommandRegistryApprove,
                  onCommandRegistryDeny: onCommandRegistryDeny,
                  onCommandRegistryPreview: onCommandRegistryPreview,
                ))
            .toList(growable: false);
    return _Panel(
      title: _cleanSectionCopy(data.detailTitle),
      subtitle: _cleanSectionCopy(data.detailSubtitle),
      child: ListView(
        children: [
          if (focusSurfaceId == 'commandRegistry') ...[
            _CommandRegistrySurfaceControls(
              selectedSessionId: _selectedSessionId(data),
              projectKey: '',
              onListInstalled: onCommandRegistryListInstalled,
              onListRequests: onCommandRegistryListRequests,
            ),
            const SizedBox(height: 10),
          ],
          if (focusSurfaceId == 'roleAdmin') ...[
            const _InlineNotice(
              message: 'Role Manager opens as a full-screen page from the Runtime Operations menu.',
              tone: 'info',
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
                onApprovalApprove: onApprovalApprove == null ? null : (reason) => onApprovalApprove!(action, reason),
                onApprovalDeny: onApprovalDeny == null ? null : (reason) => onApprovalDeny!(action, reason),
                onApprovalResume: onApprovalResume == null ? null : () => onApprovalResume!(action),
                onCommandRegistryApprove: onCommandRegistryApprove,
                onCommandRegistryDeny: onCommandRegistryDeny,
                onCommandRegistryPreview: onCommandRegistryPreview,
                onCommandRegistryApply: onCommandRegistryApply == null ? null : () => onCommandRegistryApply!(action),
                onCommandRegistryReview: onCommandRegistryReview == null ? null : () => onCommandRegistryReview!(action),
              ),
            ),
            const SizedBox(height: 10),
          ],
          if (focusSurfaceId == null) ...[
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
          return null;
        case 'processFlush':
          return onProcessFlush == null ? null : () => onProcessFlush!(action.id);
        case 'compactionManual':
          return onCompactSession == null ? null : () => onCompactSession!(action);
        case 'compactionUnavailable':
          return null;
        case 'godModeGrant':
          return onGodModeGrant == null ? null : () => onGodModeGrant!(action);
        case 'godModeRevoke':
          return onGodModeRevoke == null ? null : () => onGodModeRevoke!(action);
        case 'approval':
          return null;
        case 'approvalDeny':
          return null;
        case 'approvalResume':
          return onApprovalResume == null ? null : () => onApprovalResume!(action);
        case 'commandRegistryPreview':
          return null;
        case 'commandRegistryRequest':
          return null;
        case 'commandRegistryDeny':
          return null;
        case 'commandRegistryShow':
          return onCommandRegistryShowCommand == null ? null : () => onCommandRegistryShowCommand!(action, _selectedSessionId(data), '');
        case 'commandRegistryReview':
          return onCommandRegistryReview == null ? null : () => onCommandRegistryReview!(action);
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
    this.onRoleShowDetail,
    this.onRoleShowVersions,
    this.onRoleShowVersionData,
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
    this.onCompactSession,
    this.onGodModeGrant,
    this.onGodModeRevoke,
    this.onApprovalApprove,
    this.onApprovalDeny,
    this.onApprovalResume,
    this.onCommandRegistryApprove,
    this.onCommandRegistryDeny,
    this.onCommandRegistryPreview,
    this.onCommandRegistryApply,
    this.onCommandRegistryReview,
    this.onCommandRegistryShowCommand,
    this.onCommandRegistryListInstalled,
    this.onCommandRegistryListRequests,
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
  final ValueChanged<String>? onRoleShowDetail;
  final ValueChanged<String>? onRoleShowVersions;
  final ValueChanged<String>? onRoleShowVersionData;
  final ValueChanged<AgentRuntimeWorkflowMemoryRow>? onWorkflowMemorySelect;
  final ValueChanged<AgentRuntimeWorkflowMemoryDetail>? onWorkflowMemoryAttempted;
  final ValueChanged<AgentRuntimeWorkflowMemoryDetail>? onWorkflowMemoryHelpful;
  final ValueChanged<AgentRuntimeWorkflowMemoryDetail>? onWorkflowMemoryNotHelpful;
  final ValueChanged<String>? onSessionClose;
  final ValueChanged<String>? onSessionArchive;
  final ValueChanged<String>? onSessionFork;
  final ValueChanged<String>? onProcessTerminate;
  final AgentRuntimeProcessInputAction? onProcessInput;
  final ValueChanged<String>? onProcessFlush;
  final ValueChanged<AgentRuntimeActionItem>? onCompactSession;
  final ValueChanged<AgentRuntimeActionItem>? onGodModeGrant;
  final ValueChanged<AgentRuntimeActionItem>? onGodModeRevoke;
  final AgentRuntimeActionReason? onApprovalApprove;
  final AgentRuntimeActionReason? onApprovalDeny;
  final ValueChanged<AgentRuntimeActionItem>? onApprovalResume;
  final AgentRuntimeCommandRegistryDecisionAction? onCommandRegistryApprove;
  final AgentRuntimeCommandRegistryDecisionAction? onCommandRegistryDeny;
  final AgentRuntimeCommandRegistryDecisionAction? onCommandRegistryPreview;
  final ValueChanged<AgentRuntimeActionItem>? onCommandRegistryApply;
  final ValueChanged<AgentRuntimeActionItem>? onCommandRegistryReview;
  final AgentRuntimeCommandRegistryShowCommandAction? onCommandRegistryShowCommand;
  final AgentRuntimeCommandRegistryListAction? onCommandRegistryListInstalled;
  final VoidCallback? onCommandRegistryListRequests;

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
      onRoleShowDetail: onRoleShowDetail,
      onRoleShowVersions: onRoleShowVersions,
      onRoleShowVersionData: onRoleShowVersionData,
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
      onCommandRegistryReview: onCommandRegistryReview,
      onCommandRegistryShowCommand: onCommandRegistryShowCommand,
      onCommandRegistryListInstalled: onCommandRegistryListInstalled,
      onCommandRegistryListRequests: onCommandRegistryListRequests,
      onCompactSession: onCompactSession,
      onGodModeGrant: onGodModeGrant,
      onGodModeRevoke: onGodModeRevoke,
    );
  }
}

class AgentRuntimeRoleManagerPage extends StatefulWidget {
  const AgentRuntimeRoleManagerPage({
    super.key,
    required this.data,
    this.onClose,
    this.onValidate,
    this.onCreate,
    this.onUpdate,
    this.onExport,
    this.onArchive,
    this.onUnarchive,
    this.onActivate,
    this.onShowDetail,
    this.onShowVersions,
    this.onShowVersionData,
  });

  final AgentRuntimeRoleAdminData data;
  final VoidCallback? onClose;
  final ValueChanged<AgentRuntimeRoleEditorDraft>? onValidate;
  final ValueChanged<AgentRuntimeRoleEditorDraft>? onCreate;
  final ValueChanged<AgentRuntimeRoleEditorDraft>? onUpdate;
  final ValueChanged<String>? onExport;
  final ValueChanged<String>? onArchive;
  final ValueChanged<String>? onUnarchive;
  final AgentRuntimeRoleVersionAction? onActivate;
  final ValueChanged<String>? onShowDetail;
  final ValueChanged<String>? onShowVersions;
  final ValueChanged<String>? onShowVersionData;

  @override
  State<AgentRuntimeRoleManagerPage> createState() => _AgentRuntimeRoleManagerPageState();
}

class _AgentRuntimeRoleManagerPageState extends State<AgentRuntimeRoleManagerPage> {
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
  String? _loadedDraftKey;
  bool _localNewDraft = false;

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
    _loadDraft(widget.data.editorDraft);
    for (final controller in [
      _roleIdController,
      _versionController,
      _displayNameController,
      _modelController,
      _reasoningController,
      _capabilitiesController,
      _policyController,
      _routingModeController,
      _defaultRecipientController,
      _allowedRecipientsController,
      _routingReservedController,
      _lifecycleReservedController,
      _instructionController,
    ]) {
      controller.addListener(_refreshDraftPreview);
    }
  }

  @override
  void didUpdateWidget(covariant AgentRuntimeRoleManagerPage oldWidget) {
    super.didUpdateWidget(oldWidget);
    _loadDraft(widget.data.editorDraft);
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

  void _refreshDraftPreview() {
    if (mounted) {
      setState(() {});
    }
  }

  void _loadDraft(AgentRuntimeRoleEditorDraft? draft) {
    if (_localNewDraft && draft != null && draft.roleId != _roleIdController.text.trim()) {
      return;
    }
    final key = draft == null
        ? '__empty__'
        : '${draft.roleId}|${draft.version}|${draft.instructionText.hashCode}|${draft.policy.length}|${draft.capabilities.length}';
    if (_loadedDraftKey == key) {
      return;
    }
    _loadedDraftKey = key;
    final options = widget.data.editorOptions;
    final defaultCapability = options.capabilities.isEmpty ? '' : options.capabilities.first;
    final defaultPolicyDecision = options.policyDecisions.isEmpty ? '' : options.policyDecisions.first;
    final defaultRecipient = options.recipients.isEmpty ? '' : options.recipients.first;
    final defaultReservedAction = options.reservedActions.isEmpty ? '' : options.reservedActions.first;
    final next = draft ?? AgentRuntimeRoleEditorDraft(
      roleId: 'new-runtime-role',
      version: '1.0.0',
      displayName: 'New Runtime Role',
      model: options.models.isEmpty ? '' : options.models.first,
      reasoningEffort: options.reasoningEfforts.contains('medium') ? 'medium' : options.reasoningEfforts.isEmpty ? '' : options.reasoningEfforts.first,
      instructionText: 'Write role instructions here.',
      capabilities: defaultCapability.isEmpty ? const [] : [defaultCapability],
      policy: defaultCapability.isEmpty || defaultPolicyDecision.isEmpty ? const [] : [AgentRuntimeRolePolicyRow(action: defaultCapability, decision: defaultPolicyDecision)],
      routingMode: options.routingModes.isEmpty ? '' : options.routingModes.first,
      routingReservedActions: defaultReservedAction.isEmpty ? const [] : [defaultReservedAction],
      defaultRecipient: defaultRecipient.isEmpty ? null : defaultRecipient,
      allowedRecipients: defaultRecipient.isEmpty ? const [] : [defaultRecipient],
      listed: true,
      ownerVisible: true,
      canSpawnAgents: false,
      canArchiveAgents: false,
      lifecycleReservedActions: defaultReservedAction.isEmpty ? const [] : [defaultReservedAction],
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

  void _startNewDraft() {
    _localNewDraft = true;
    _loadedDraftKey = null;
    _loadDraft(null);
  }

  AgentRuntimeRoleEditorDraft _editedDraft() {
    return AgentRuntimeRoleEditorDraft(
      roleId: _roleIdController.text.trim(),
      version: _versionController.text.trim(),
      displayName: _displayNameController.text.trim(),
      model: _modelController.text.trim(),
      reasoningEffort: _reasoningController.text.trim(),
      instructionText: _instructionController.text,
      capabilities: _lineList(_capabilitiesController.text),
      policy: _rolePolicyRows(_policyController.text),
      routingMode: _routingModeController.text.trim().isEmpty ? 'direct' : _routingModeController.text.trim(),
      routingReservedActions: _lineList(_routingReservedController.text),
      defaultRecipient: _defaultRecipientController.text.trim().isEmpty ? null : _defaultRecipientController.text.trim(),
      allowedRecipients: _lineList(_allowedRecipientsController.text),
      listed: _listed,
      ownerVisible: _ownerVisible,
      canSpawnAgents: _canSpawnAgents,
      canArchiveAgents: _canArchiveAgents,
      lifecycleReservedActions: _lineList(_lifecycleReservedController.text),
    );
  }

  @override
  Widget build(BuildContext context) {
    final roleAdmin = widget.data;
    final detail = roleAdmin.selectedDetail;
    final draft = _editedDraft();
    final knownRoleIds = {for (final row in roleAdmin.rows) row.id};
    final saveCreatesRole = _localNewDraft || !knownRoleIds.contains(draft.roleId);
    final narrow = MediaQuery.sizeOf(context).width < 720;
    return Material(
      color: const Color(0xFF05090F),
      child: SafeArea(
        child: Column(
          children: [
            _RoleManagerTopBar(
              title: detail?.displayName ?? draft.displayName,
              version: draft.version,
              status: detail?.status ?? 'draft',
              onClose: widget.onClose,
              onValidate: widget.onValidate == null ? null : () => widget.onValidate!(_editedDraft()),
              onSave: saveCreatesRole
                  ? widget.onCreate == null
                      ? null
                      : () {
                          _localNewDraft = false;
                          widget.onCreate!(_editedDraft());
                        }
                  : widget.onUpdate == null
                      ? null
                      : () => widget.onUpdate!(_editedDraft()),
              onActivate: null,
              onExport: widget.onExport == null ? null : () => widget.onExport!(_editedDraft().roleId),
            ),
            Expanded(
              child: narrow
                  ? ListView(
                      padding: const EdgeInsets.all(12),
                      children: [
                        SizedBox(height: 220, child: _RoleManagerRolesColumn(data: roleAdmin, onNewRole: widget.onCreate == null ? null : _startNewDraft, onSelectRole: widget.onShowDetail)),
                        const SizedBox(height: 12),
                        SizedBox(
                          height: 1180,
                          child: _RoleManagerMainColumn(
                            draft: draft,
                            detail: detail,
                            data: roleAdmin,
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
                        ),
                        const SizedBox(height: 12),
                        SizedBox(
                          height: 820,
                          child: _RoleManagerRightColumn(
                            data: roleAdmin,
                            detail: detail,
                            draft: draft,
                            onArchive: widget.onArchive,
                            onUnarchive: widget.onUnarchive,
                            onShowDetail: widget.onShowDetail,
                            onShowVersions: widget.onShowVersions,
                            onShowVersionData: widget.onShowVersionData,
                            onActivate: widget.onActivate,
                          ),
                        ),
                      ],
                    )
                  : Row(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        SizedBox(width: 276, child: _RoleManagerRolesColumn(data: roleAdmin, onNewRole: widget.onCreate == null ? null : _startNewDraft, onSelectRole: widget.onShowDetail)),
                        const VerticalDivider(width: 1, color: Color(0xFF182231)),
                        Expanded(
                          child: _RoleManagerMainColumn(
                            draft: draft,
                            detail: detail,
                            data: roleAdmin,
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
                        ),
                        const VerticalDivider(width: 1, color: Color(0xFF182231)),
                        SizedBox(
                          width: 322,
                          child: _RoleManagerRightColumn(
                            data: roleAdmin,
                            detail: detail,
                            draft: draft,
                            onArchive: widget.onArchive,
                            onUnarchive: widget.onUnarchive,
                            onShowDetail: widget.onShowDetail,
                            onShowVersions: widget.onShowVersions,
                            onShowVersionData: widget.onShowVersionData,
                            onActivate: widget.onActivate,
                          ),
                        ),
                      ],
                    ),
            ),
          ],
        ),
      ),
    );
  }
}

class _RoleManagerTopBar extends StatelessWidget {
  const _RoleManagerTopBar({
    required this.title,
    required this.version,
    required this.status,
    this.onClose,
    this.onValidate,
    this.onSave,
    this.onActivate,
    this.onExport,
  });

  final String title;
  final String version;
  final String status;
  final VoidCallback? onClose;
  final VoidCallback? onValidate;
  final VoidCallback? onSave;
  final VoidCallback? onActivate;
  final VoidCallback? onExport;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return LayoutBuilder(
      builder: (context, constraints) {
        final narrow = constraints.maxWidth < 1420;
        final actions = [
          OutlinedButton(onPressed: onValidate, child: const Text('Validate Draft')),
          OutlinedButton(onPressed: onSave, child: const Text('Save Version')),
          FilledButton(onPressed: onActivate, child: const Text('Activate Version')),
          OutlinedButton.icon(onPressed: onExport, icon: const Icon(Icons.file_upload_outlined, size: 16), label: const Text('Export')),
        ];
        return Container(
          height: narrow ? 88 : 46,
          padding: const EdgeInsets.symmetric(horizontal: 10),
          decoration: const BoxDecoration(
            color: Color(0xFF080D14),
            border: Border(bottom: BorderSide(color: Color(0xFF1A2433))),
          ),
          child: narrow
              ? Column(
                  children: [
                    SizedBox(
                      height: 44,
                      child: Row(
                        children: [
                          IconButton(onPressed: onClose, tooltip: 'Back', icon: const Icon(Icons.arrow_back_rounded)),
                          Expanded(child: Text('Role Manager', overflow: TextOverflow.ellipsis, style: theme.textTheme.titleSmall?.copyWith(color: Colors.white, fontWeight: FontWeight.w800))),
                          _Chip(label: status.isEmpty ? 'Draft' : _roleManagerHumanLabel(status), tone: status == 'active' ? 'success' : 'warning'),
                          const SizedBox(width: 6),
                          _Chip(label: 'v$version', tone: 'info'),
                        ],
                      ),
                    ),
                    SizedBox(
                      height: 40,
                      child: ListView.separated(
                        scrollDirection: Axis.horizontal,
                        itemBuilder: (context, index) => actions[index],
                        separatorBuilder: (_, _) => const SizedBox(width: 8),
                        itemCount: actions.length,
                      ),
                    ),
                  ],
                )
              : Row(
                  children: [
                    IconButton(onPressed: onClose, tooltip: 'Back', icon: const Icon(Icons.arrow_back_rounded)),
                    const SizedBox(width: 4),
                    Text('Role Manager', style: theme.textTheme.titleSmall?.copyWith(color: Colors.white, fontWeight: FontWeight.w800)),
                    const SizedBox(width: 14),
                    Flexible(child: Text('Agent Runtime / Roles / $title', overflow: TextOverflow.ellipsis, style: theme.textTheme.bodySmall?.copyWith(color: const Color(0xFF93A5BC)))),
                    const SizedBox(width: 12),
                    _Chip(label: status.isEmpty ? 'Draft' : _roleManagerHumanLabel(status), tone: status == 'active' ? 'success' : 'warning'),
                    const SizedBox(width: 8),
                    _Chip(label: 'v$version', tone: 'info'),
                    const Spacer(),
                    for (final action in actions) ...[
                      action,
                      const SizedBox(width: 8),
                    ],
                  ],
                ),
        );
      },
    );
  }
}

class _RoleManagerRolesColumn extends StatelessWidget {
  const _RoleManagerRolesColumn({required this.data, this.onNewRole, this.onSelectRole});

  final AgentRuntimeRoleAdminData data;
  final VoidCallback? onNewRole;
  final ValueChanged<String>? onSelectRole;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.all(10),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Expanded(child: Text('Roles', style: theme.textTheme.labelLarge?.copyWith(color: Colors.white, fontWeight: FontWeight.w800))),
              OutlinedButton.icon(
                onPressed: onNewRole,
                icon: const Icon(Icons.add, size: 14),
                label: const Text('New Role'),
              ),
            ],
          ),
          const SizedBox(height: 8),
          if (data.rows.isEmpty)
            _EmptyState(title: data.emptyTitle, body: _cleanSectionCopy(data.emptyText))
          else
            Expanded(
              child: ListView.separated(
                itemCount: data.rows.length,
                separatorBuilder: (_, _) => const Divider(height: 1, color: Color(0xFF182231)),
                itemBuilder: (context, index) {
                  final row = data.rows[index];
                  final selected = data.selectedDetail?.id == row.id || (index == 0 && data.selectedDetail == null);
                  return _RoleManagerRoleTile(row: row, selected: selected, onTap: onSelectRole == null ? null : () => onSelectRole!(row.id));
                },
              ),
            ),
        ],
      ),
    );
  }
}

class _RoleManagerRoleTile extends StatelessWidget {
  const _RoleManagerRoleTile({required this.row, required this.selected, this.onTap});

  final AgentRuntimeRoleRow row;
  final bool selected;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final subtitle = row.subtitle.split('·').map((part) => part.trim()).where((part) => part.isNotEmpty).toList(growable: false);
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(4),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
        decoration: BoxDecoration(
          color: Colors.transparent,
          borderRadius: BorderRadius.circular(4),
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            AnimatedContainer(
              duration: const Duration(milliseconds: 120),
              width: 3,
              height: 48,
              margin: const EdgeInsets.only(right: 9, top: 1),
              decoration: BoxDecoration(
                color: selected ? const Color(0xFF2E69FF) : Colors.transparent,
                borderRadius: BorderRadius.circular(999),
              ),
            ),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      Expanded(child: Text(row.id, maxLines: 1, overflow: TextOverflow.ellipsis, style: theme.textTheme.bodySmall?.copyWith(color: Colors.white, fontWeight: FontWeight.w700))),
                      Icon(Icons.circle, size: 7, color: _toneColor(row.tone)),
                      const SizedBox(width: 6),
                      Text(_roleManagerHumanLabel(row.status), style: theme.textTheme.labelSmall?.copyWith(color: _toneColor(row.tone), fontWeight: FontWeight.w700)),
                    ],
                  ),
                  const SizedBox(height: 5),
                  Text(row.title, maxLines: 1, overflow: TextOverflow.ellipsis, style: theme.textTheme.bodySmall?.copyWith(color: const Color(0xFFC6D3E4))),
                  if (subtitle.isNotEmpty) ...[
                    const SizedBox(height: 6),
                    Wrap(
                      spacing: 6,
                      runSpacing: 4,
                      children: [
                        for (final item in subtitle) _MiniPill(_roleManagerHumanLabel(item)),
                        if (selected && row.currentVersionId != null) const _MiniPill('Current'),
                      ],
                    ),
                  ],
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _RoleManagerMainColumn extends StatelessWidget {
  const _RoleManagerMainColumn({
    required this.draft,
    required this.detail,
    required this.data,
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

  final AgentRuntimeRoleEditorDraft draft;
  final AgentRuntimeRoleDetail? detail;
  final AgentRuntimeRoleAdminData data;
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
    final policy = _rolePolicyRows(policyController.text);
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        Wrap(
          spacing: 12,
          runSpacing: 6,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 460),
              child: Text(draft.displayName, overflow: TextOverflow.ellipsis, style: theme.textTheme.headlineSmall?.copyWith(color: Colors.white, fontWeight: FontWeight.w800)),
            ),
            _InlineStatus(
              label: data.validationErrors.isEmpty ? 'No validation errors' : '${data.validationErrors.length} validation issue${data.validationErrors.length == 1 ? '' : 's'}',
              tone: data.validationErrors.isEmpty ? 'success' : 'danger',
            ),
          ],
        ),
        const SizedBox(height: 14),
        LayoutBuilder(
          builder: (context, constraints) {
            final narrowFields = constraints.maxWidth < 920;
            final identity = Wrap(
              spacing: 12,
              runSpacing: 10,
              children: [
                SizedBox(width: 260, child: _RoleManagerLabeledField(label: 'Role ID', child: _EditorTextField(label: '', controller: roleIdController))),
                SizedBox(width: 120, child: _RoleManagerLabeledField(label: 'Version', child: _EditorTextField(label: '', controller: versionController))),
                SizedBox(width: 300, child: _RoleManagerLabeledField(label: 'Display Name', child: _EditorTextField(label: '', controller: displayNameController))),
              ],
            );
            final modelDefaults = Wrap(
              spacing: 12,
              runSpacing: 10,
              children: [
                _RoleManagerLabeledField(
                  label: 'Model',
                  child: _EnumSelect(key: const ValueKey('roleEditor.model'), label: '', value: modelController.text, values: _withCurrentOptions(data.editorOptions.models, modelController.text), onChanged: (value) => modelController.text = value),
                ),
                _RoleManagerLabeledField(
                  label: 'Reasoning Effort',
                  child: _EnumSelect(key: const ValueKey('roleEditor.reasoning'), label: '', value: reasoningController.text, values: _withCurrentOptions(data.editorOptions.reasoningEfforts, reasoningController.text), onChanged: (value) => reasoningController.text = value),
                ),
              ],
            );
            final saveNote = Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                const Icon(Icons.lock_outline_rounded, size: 14, color: Color(0xFF93A5BC)),
                const SizedBox(width: 5),
                Flexible(child: Text('Saves a new role version', overflow: TextOverflow.ellipsis, style: theme.textTheme.labelSmall?.copyWith(color: const Color(0xFFB7C4D8)))),
              ],
            );
            if (narrowFields) {
              return Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    identity,
                    const SizedBox(height: 6),
                    Text('Model Defaults', style: theme.textTheme.labelSmall?.copyWith(color: Colors.white, fontWeight: FontWeight.w800)),
                    const SizedBox(height: 4),
                    Wrap(spacing: 12, runSpacing: 10, crossAxisAlignment: WrapCrossAlignment.center, children: [modelDefaults, saveNote]),
                  ],
              );
            }
            return Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(children: [Expanded(child: identity)]),
                const SizedBox(height: 6),
                Row(
                  children: [
                    SizedBox(width: 220, child: Text('Model Defaults', style: theme.textTheme.labelSmall?.copyWith(color: Colors.white, fontWeight: FontWeight.w800))),
                    Expanded(child: modelDefaults),
                    saveNote,
                  ],
                ),
              ],
            );
          },
        ),
        const SizedBox(height: 10),
        SizedBox(
          height: 238,
          child: _RoleInstructionsEditorChrome(
            controller: instructionController,
            byteCount: instructionController.text.length,
          ),
        ),
        const SizedBox(height: 14),
        Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            if (!data.editorOptions.isCompleteForPrimaryAuthoring)
              const _InlineNotice(
                message: 'Role editor options are unavailable. Refresh before editing policies, routing, capabilities, or lifecycle authority.',
                tone: 'warning',
              )
            else
              _RoleManagerStructuredControls(
                capabilitiesController: capabilitiesController,
                policyController: policyController,
                routingModeController: routingModeController,
                defaultRecipientController: defaultRecipientController,
                allowedRecipientsController: allowedRecipientsController,
                routingReservedController: routingReservedController,
                lifecycleReservedController: lifecycleReservedController,
                options: data.editorOptions,
                listed: listed,
                ownerVisible: ownerVisible,
                canSpawnAgents: canSpawnAgents,
                canArchiveAgents: canArchiveAgents,
                onListedChanged: onListedChanged,
                onOwnerVisibleChanged: onOwnerVisibleChanged,
                onCanSpawnAgentsChanged: onCanSpawnAgentsChanged,
                onCanArchiveAgentsChanged: onCanArchiveAgentsChanged,
              ),
            const SizedBox(height: 12),
            _RoleEditorSection(
              title: 'Policy preview',
              child: _PolicyTable(policy: policy.isEmpty ? draft.policy : policy),
            ),
          ],
        ),
      ],
    );
  }
}

class _RoleManagerStructuredControls extends StatelessWidget {
  const _RoleManagerStructuredControls({
    required this.capabilitiesController,
    required this.policyController,
    required this.routingModeController,
    required this.defaultRecipientController,
    required this.allowedRecipientsController,
    required this.routingReservedController,
    required this.lifecycleReservedController,
    required this.options,
    required this.listed,
    required this.ownerVisible,
    required this.canSpawnAgents,
    required this.canArchiveAgents,
    required this.onListedChanged,
    required this.onOwnerVisibleChanged,
    required this.onCanSpawnAgentsChanged,
    required this.onCanArchiveAgentsChanged,
  });

  final TextEditingController capabilitiesController;
  final TextEditingController policyController;
  final TextEditingController routingModeController;
  final TextEditingController defaultRecipientController;
  final TextEditingController allowedRecipientsController;
  final TextEditingController routingReservedController;
  final TextEditingController lifecycleReservedController;
  final AgentRuntimeRoleEditorOptions options;
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
    final allowedRecipients = _lineList(allowedRecipientsController.text);
    final routingReserved = _lineList(routingReservedController.text);
    final lifecycleReserved = _lineList(lifecycleReservedController.text);
    final capabilities = _lineList(capabilitiesController.text);
    final policyRows = _rolePolicyRows(policyController.text);
    return LayoutBuilder(
      builder: (context, constraints) {
        final narrow = constraints.maxWidth < 560;
        final capabilitiesEditor = _RoleEditorSection(
          title: 'Capabilities',
          child: _OptionChecklist(
            keyPrefix: 'capability',
            values: options.capabilities,
            selected: capabilities,
            onChanged: (next) {
              _setLineList(capabilitiesController, next);
              final currentPolicy = {for (final row in _rolePolicyRows(policyController.text)) row.action: row.decision};
              final decision = options.policyDecisions.contains('allow') ? 'allow' : options.policyDecisions.first;
              final synced = [
                for (final action in next)
                  AgentRuntimeRolePolicyRow(action: action, decision: currentPolicy[action] ?? decision),
              ];
              _setPolicyRows(policyController, synced);
            },
          ),
        );
        final policyEditor = _RoleEditorSection(
          title: 'Policy decisions',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              for (final row in policyRows)
                Padding(
                  padding: const EdgeInsets.only(bottom: 8),
                  child: Wrap(
                    spacing: 10,
                    runSpacing: 8,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: [
                      SizedBox(width: 240, child: Text(_roleManagerCompactPolicyAction(row.action), overflow: TextOverflow.ellipsis, style: Theme.of(context).textTheme.bodySmall?.copyWith(color: const Color(0xFFD6E2F2)))),
                      _EnumSelect(
                        key: ValueKey('roleEditor.policy.${row.action}'),
                        label: 'Decision',
                        value: row.decision,
                        values: _withCurrentOptions(options.policyDecisions, row.decision),
                        onChanged: (value) {
                          _setPolicyRows(policyController, [
                            for (final current in _rolePolicyRows(policyController.text))
                              AgentRuntimeRolePolicyRow(action: current.action, decision: current.action == row.action ? value : current.decision),
                          ]);
                        },
                        displayLabel: _roleManagerHumanLabel,
                      ),
                    ],
                  ),
                ),
              if (policyRows.isEmpty)
                Text('Select capabilities to add policy rows.', style: Theme.of(context).textTheme.bodySmall?.copyWith(color: const Color(0xFF93A5BC))),
            ],
          ),
        );
        final left = _RoleEditorSection(
          title: 'Routing',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Wrap(
                spacing: 10,
                runSpacing: 10,
                children: [
                  _EnumSelect(key: const ValueKey('roleEditor.routing.mode'), label: 'Routing mode', value: routingModeController.text, values: _withCurrentOptions(options.routingModes, routingModeController.text), onChanged: (value) => routingModeController.text = value, displayLabel: _roleManagerHumanLabel),
                  _EnumSelect(key: const ValueKey('roleEditor.routing.defaultRecipient'), label: 'Default recipient', value: defaultRecipientController.text, values: _withCurrentOptions(options.recipients, defaultRecipientController.text), onChanged: (value) => defaultRecipientController.text = value, displayLabel: _roleManagerHumanLabel),
                ],
              ),
              const SizedBox(height: 8),
              Text('Allowed recipients', style: Theme.of(context).textTheme.labelSmall?.copyWith(color: const Color(0xFF93A5BC))),
              const SizedBox(height: 6),
              _OptionChecklist(
                keyPrefix: 'recipient',
                values: options.recipients,
                selected: allowedRecipients,
                onChanged: (next) {
                  final defaultRecipient = defaultRecipientController.text.trim();
                  if (defaultRecipient.isNotEmpty && !next.contains(defaultRecipient)) {
                    defaultRecipientController.text = next.isEmpty ? '' : next.first;
                  }
                  _setLineList(allowedRecipientsController, next);
                },
                labelBuilder: _roleManagerHumanLabel,
              ),
              const SizedBox(height: 10),
              _OptionChips(
                title: 'Reserved routing actions',
                values: options.reservedActions,
                selected: routingReserved,
                keyPrefix: 'routingReserved',
                onChanged: (next) => _setLineList(routingReservedController, next),
              ),
            ],
          ),
        );
        final middle = _RoleEditorSection(
          title: 'Lifecycle Authority',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              _EditorSwitch(key: const ValueKey('roleEditor.canSpawnAgents'), label: 'Can spawn agents', value: canSpawnAgents, onChanged: onCanSpawnAgentsChanged),
              _EditorSwitch(key: const ValueKey('roleEditor.canArchiveAgents'), label: 'Can archive agents', value: canArchiveAgents, onChanged: onCanArchiveAgentsChanged),
              const SizedBox(height: 8),
              _OptionChips(
                title: 'Reserved lifecycle actions',
                values: options.reservedActions,
                selected: lifecycleReserved,
                keyPrefix: 'lifecycleReserved',
                onChanged: (next) => _setLineList(lifecycleReservedController, next),
              ),
            ],
          ),
        );
        final right = _RoleEditorSection(
          title: 'Visibility',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              _EditorSwitch(key: const ValueKey('roleEditor.listed'), label: 'Listed', value: listed, onChanged: onListedChanged),
              _EditorSwitch(key: const ValueKey('roleEditor.ownerVisible'), label: 'Owner visible', value: ownerVisible, onChanged: onOwnerVisibleChanged),
            ],
          ),
        );
        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            if (narrow)
              Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [capabilitiesEditor, const _RoleEditorDivider(), policyEditor])
            else
              Row(crossAxisAlignment: CrossAxisAlignment.start, children: [Expanded(child: capabilitiesEditor), const SizedBox(width: 18), Expanded(child: policyEditor)]),
            const _RoleEditorDivider(),
            if (narrow)
              Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [left, const _RoleEditorDivider(), middle, const _RoleEditorDivider(), right])
            else
              Row(crossAxisAlignment: CrossAxisAlignment.start, children: [Expanded(child: left), const SizedBox(width: 18), Expanded(child: middle), const SizedBox(width: 18), Expanded(child: right)]),
          ],
        );
      },
    );
  }
}

class _RoleEditorDivider extends StatelessWidget {
  const _RoleEditorDivider();

  @override
  Widget build(BuildContext context) {
    return const Padding(
      padding: EdgeInsets.symmetric(vertical: 14),
      child: Divider(height: 1, color: Color(0xFF1B2A3A)),
    );
  }
}

class _RoleEditorSection extends StatelessWidget {
  const _RoleEditorSection({required this.title, required this.child});

  final String title;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(title, style: theme.textTheme.labelMedium?.copyWith(color: Colors.white, fontWeight: FontWeight.w800)),
        const SizedBox(height: 8),
        child,
      ],
    );
  }
}

void _setLineList(TextEditingController controller, List<String> values) {
  controller.text = _dedupeRoleManagerOptions(values).join('\n');
}

void _setPolicyRows(TextEditingController controller, List<AgentRuntimeRolePolicyRow> rows) {
  controller.text = rows.map((row) => '${row.action}=${row.decision}').join('\n');
}

class _OptionChecklist extends StatelessWidget {
  const _OptionChecklist({
    required this.values,
    required this.selected,
    required this.onChanged,
    this.labelBuilder,
    this.keyPrefix = 'option',
  });

  final List<String> values;
  final List<String> selected;
  final ValueChanged<List<String>> onChanged;
  final String Function(String value)? labelBuilder;
  final String keyPrefix;

  @override
  Widget build(BuildContext context) {
    final options = _dedupeRoleManagerOptions(values);
    if (options.isEmpty) {
      return Text('No options available.', style: Theme.of(context).textTheme.bodySmall?.copyWith(color: const Color(0xFF93A5BC)));
    }
    return Wrap(
      spacing: 8,
      runSpacing: 6,
      children: [
        for (final option in options)
          FilterChip(
            key: ValueKey('roleEditor.$keyPrefix.$option'),
            label: Text((labelBuilder ?? _roleManagerHumanLabel)(option)),
            labelStyle: TextStyle(
              color: selected.contains(option) ? Colors.white : const Color(0xFFD6E2F2),
              fontWeight: selected.contains(option) ? FontWeight.w700 : FontWeight.w500,
            ),
            selectedColor: const Color(0xFF245B82),
            checkmarkColor: Colors.white,
            selected: selected.contains(option),
            onSelected: (checked) {
              final next = [...selected];
              if (checked) {
                if (!next.contains(option)) next.add(option);
              } else {
                next.remove(option);
              }
              onChanged(next);
            },
          ),
      ],
    );
  }
}

class _OptionChips extends StatelessWidget {
  const _OptionChips({
    required this.title,
    required this.values,
    required this.selected,
    required this.onChanged,
    this.keyPrefix = 'option',
  });

  final String title;
  final List<String> values;
  final List<String> selected;
  final ValueChanged<List<String>> onChanged;
  final String keyPrefix;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(title, style: Theme.of(context).textTheme.labelSmall?.copyWith(color: const Color(0xFF93A5BC))),
        const SizedBox(height: 6),
        _OptionChecklist(keyPrefix: keyPrefix, values: values, selected: selected, onChanged: onChanged),
      ],
    );
  }
}

class _RoleManagerRightColumn extends StatelessWidget {
  const _RoleManagerRightColumn({
    required this.data,
    required this.detail,
    required this.draft,
    this.onArchive,
    this.onUnarchive,
    this.onShowDetail,
    this.onShowVersions,
    this.onShowVersionData,
    this.onActivate,
  });

  final AgentRuntimeRoleAdminData data;
  final AgentRuntimeRoleDetail? detail;
  final AgentRuntimeRoleEditorDraft draft;
  final ValueChanged<String>? onArchive;
  final ValueChanged<String>? onUnarchive;
  final ValueChanged<String>? onShowDetail;
  final ValueChanged<String>? onShowVersions;
  final ValueChanged<String>? onShowVersionData;
  final AgentRuntimeRoleVersionAction? onActivate;

  @override
  Widget build(BuildContext context) {
    return ListView(
      padding: const EdgeInsets.all(12),
      children: [
        _RoleEditorSection(
          title: 'Validation',
          child: data.validationErrors.isEmpty
              ? Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                  const _InlineStatus(label: 'No validation errors', tone: 'success'),
                  if (data.actionStates.isEmpty)
                    const _FactRow(AgentRuntimeFact(label: 'Actions', value: 'No pending role actions'))
                  else
                    for (final action in data.actionStates.take(4))
                      _FactRow(AgentRuntimeFact(label: action.title, value: action.stateText)),
                ])
              : Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    for (final error in data.validationErrors) _InlineStatus(label: error, tone: 'danger'),
                  ],
                ),
        ),
        const _RoleEditorDivider(),
        _RoleEditorSection(
          title: 'Version History',
          child: Column(
            children: [
              for (final version in data.versionRows)
                _RoleVersionTile(
                  row: version,
                  roleId: detail?.id ?? draft.roleId,
                  onActivate: onActivate,
                  onShowVersionData: onShowVersionData,
                ),
            ],
          ),
        ),
        const _RoleEditorDivider(),
        _RoleEditorSection(
          title: 'Role Actions',
          child: Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              OutlinedButton(onPressed: detail == null || onShowDetail == null ? null : () => onShowDetail!(detail!.id), child: const Text('Show detail')),
              OutlinedButton(onPressed: detail == null || onShowVersions == null ? null : () => onShowVersions!(detail!.id), child: const Text('Show versions')),
              OutlinedButton(onPressed: onArchive == null ? null : () => onArchive!(draft.roleId), child: const Text('Archive')),
              OutlinedButton(onPressed: onUnarchive == null ? null : () => onUnarchive!(draft.roleId), child: const Text('Unarchive')),
            ],
          ),
        ),
      ],
    );
  }
}

class _RoleInstructionsEditorChrome extends StatelessWidget {
  const _RoleInstructionsEditorChrome({required this.controller, required this.byteCount});

  final TextEditingController controller;
  final int byteCount;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: BoxDecoration(
        color: const Color(0xFF07101A),
        border: Border.all(color: const Color(0xFF1B2A3A)),
        borderRadius: BorderRadius.circular(4),
      ),
      child: Column(
        children: [
          Container(
            height: 34,
            decoration: const BoxDecoration(
              color: Color(0xFF0A1119),
              border: Border(bottom: BorderSide(color: Color(0xFF1B2A3A))),
            ),
            child: Row(
              children: [
                Container(
                  height: 34,
                  padding: const EdgeInsets.symmetric(horizontal: 10),
                  decoration: const BoxDecoration(
                    border: Border(right: BorderSide(color: Color(0xFF1B2A3A))),
                  ),
                  child: Row(
                    children: [
                      const Icon(Icons.description_outlined, size: 15, color: Color(0xFFB7C4D8)),
                      const SizedBox(width: 7),
                      Text('instructions.md', style: Theme.of(context).textTheme.labelMedium?.copyWith(color: Colors.white, fontWeight: FontWeight.w800)),
                      const SizedBox(width: 8),
                      const Icon(Icons.close_rounded, size: 14, color: Color(0xFF8FA1B8)),
                    ],
                  ),
                ),
                IconButton(onPressed: null, icon: const Icon(Icons.add, size: 16), visualDensity: VisualDensity.compact),
                const Spacer(),
                const Icon(Icons.open_in_full_rounded, size: 14, color: Color(0xFF8FA1B8)),
                const SizedBox(width: 12),
                const Icon(Icons.splitscreen_rounded, size: 14, color: Color(0xFF8FA1B8)),
                const SizedBox(width: 12),
                const Icon(Icons.more_vert_rounded, size: 16, color: Color(0xFF8FA1B8)),
                const SizedBox(width: 8),
              ],
            ),
          ),
          Expanded(
            child: Row(
              children: [
                Expanded(
                  child: Semantics(
                    label: 'Role instructions code editor',
                    textField: true,
                    child: AgentRuntimeCodeEditor(key: const ValueKey('roleEditor.instructions'), controller: controller),
                  ),
                ),
                Container(
                  width: 104,
                  margin: const EdgeInsets.fromLTRB(8, 10, 8, 10),
                  decoration: BoxDecoration(
                    color: const Color(0xFF0B1628),
                    border: Border.all(color: const Color(0xFF1E3148)),
                    borderRadius: BorderRadius.circular(3),
                  ),
                  child: CustomPaint(painter: _RoleEditorMinimapPainter()),
                ),
              ],
            ),
          ),
          Container(
            height: 24,
            padding: const EdgeInsets.symmetric(horizontal: 10),
            decoration: const BoxDecoration(
              color: Color(0xFF0A1119),
              border: Border(top: BorderSide(color: Color(0xFF1B2A3A))),
            ),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                Text('Markdown', style: Theme.of(context).textTheme.labelSmall?.copyWith(color: const Color(0xFFB7C4D8))),
                const SizedBox(width: 6),
                const Icon(Icons.keyboard_arrow_down_rounded, size: 14, color: Color(0xFF8FA1B8)),
                const SizedBox(width: 12),
                Text('UTF-8', style: Theme.of(context).textTheme.labelSmall?.copyWith(color: const Color(0xFFB7C4D8))),
                const SizedBox(width: 12),
                Text('$byteCount bytes', style: Theme.of(context).textTheme.labelSmall?.copyWith(color: const Color(0xFFB7C4D8))),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _RoleEditorMinimapPainter extends CustomPainter {
  @override
  void paint(Canvas canvas, Size size) {
    final dim = Paint()..color = const Color(0xFF203147);
    final bright = Paint()..color = const Color(0xFF42C8FF);
    final accent = Paint()..color = const Color(0xFFE6B450);
    for (var i = 0; i < 18; i++) {
      final y = 8.0 + i * 5.0;
      final width = (i % 4 == 0 ? size.width * 0.72 : i % 3 == 0 ? size.width * 0.48 : size.width * 0.62);
      canvas.drawRRect(RRect.fromRectAndRadius(Rect.fromLTWH(8, y, width - 14, 1.6), const Radius.circular(1)), dim);
    }
    canvas.drawRRect(RRect.fromRectAndRadius(Rect.fromLTWH(8, 58, size.width * 0.68, 1.8), const Radius.circular(1)), bright);
    canvas.drawRRect(RRect.fromRectAndRadius(Rect.fromLTWH(8, 78, size.width * 0.48, 1.8), const Radius.circular(1)), accent);
    final viewport = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1
      ..color = const Color(0xFF2E69FF);
    canvas.drawRRect(RRect.fromRectAndRadius(Rect.fromLTWH(4, 10, size.width - 8, 70), const Radius.circular(2)), viewport);
  }

  @override
  bool shouldRepaint(covariant CustomPainter oldDelegate) => false;
}

class _RoleManagerLabeledField extends StatelessWidget {
  const _RoleManagerLabeledField({required this.label, required this.child});

  final String label;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 220,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(label, style: Theme.of(context).textTheme.labelSmall?.copyWith(color: const Color(0xFF93A5BC))),
          const SizedBox(height: 5),
          child,
        ],
      ),
    );
  }
}

class _PolicyTable extends StatelessWidget {
  const _PolicyTable({required this.policy});

  final List<AgentRuntimeRolePolicyRow> policy;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 214,
      child: Table(
        columnWidths: const {0: FlexColumnWidth(1.9), 1: FlexColumnWidth(0.9), 2: FlexColumnWidth(0.7), 3: FlexColumnWidth(0.9), 4: FlexColumnWidth(1.15)},
        border: TableBorder.all(color: const Color(0xFF203046)),
        children: [
          TableRow(children: [
            _TableCell('Capability', header: true),
            _TableCell('Decision', header: true),
            _TableCell('Enforcement', header: true),
            _TableCell('Approval target', header: true),
            _TableCell('Notes', header: true),
          ]),
          for (final row in policy.take(11))
            TableRow(children: [
              _TableCell(_roleManagerCompactPolicyAction(row.action)),
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 3),
                child: Align(alignment: Alignment.centerLeft, child: _MiniPill(row.decision == 'ownerApproval' ? 'Owner' : _roleManagerHumanLabel(row.decision))),
              ),
              _TableCell('hard'),
              _TableCell(row.decision == 'ownerApproval' ? 'owner' : '—'),
              _TableCell(_roleManagerPolicyNote(row.action)),
            ]),
          if (policy.isEmpty)
            TableRow(children: [
              _TableCell('No policy rows configured'),
              _TableCell('—'),
              _TableCell('—'),
              _TableCell('—'),
              _TableCell('—'),
            ]),
        ],
      ),
    );
  }
}

String _roleManagerCompactPolicyAction(String value) {
  switch (value) {
    case 'tool.execute_code':
      return 'Execute code';
    case 'tool.request_command_registry_change':
      return 'Registry change';
    case 'workflow_memory.remember_project':
      return 'Remember project';
    case 'workflow_memory.remember_global':
      return 'Remember global';
    case 'workflow_memory.feedback':
      return 'Memory feedback';
    case 'command_registry.request':
      return 'Registry info';
    case 'command_registry.decide':
      return 'Decide command';
    case 'command_registry.apply':
      return 'Apply registry';
    default:
      return _roleManagerHumanLabel(value);
  }
}

class _TableCell extends StatelessWidget {
  const _TableCell(this.text, {this.header = false});
  final String text;
  final bool header;
  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
      child: Text(text, maxLines: 1, overflow: TextOverflow.ellipsis, style: Theme.of(context).textTheme.labelSmall?.copyWith(color: header ? const Color(0xFFB7C4D8) : Colors.white, fontWeight: header ? FontWeight.w700 : FontWeight.w400)),
    );
  }
}

class _InlineStatus extends StatelessWidget {
  const _InlineStatus({required this.label, required this.tone});

  final String label;
  final String tone;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 5),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.check_circle, size: 13, color: _toneColor(tone)),
          const SizedBox(width: 6),
          Flexible(child: Text(label, overflow: TextOverflow.ellipsis, style: Theme.of(context).textTheme.bodySmall?.copyWith(color: tone == 'warning' ? const Color(0xFFE6B450) : const Color(0xFFC6D3E4), fontWeight: tone == 'success' ? FontWeight.w700 : FontWeight.w400))),
        ],
      ),
    );
  }
}

class _MiniPill extends StatelessWidget {
  const _MiniPill(this.label);
  final String label;
  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      decoration: BoxDecoration(color: const Color(0xFF131C28), borderRadius: BorderRadius.circular(4), border: Border.all(color: const Color(0xFF25354A))),
      child: Text(label, maxLines: 1, overflow: TextOverflow.ellipsis, style: Theme.of(context).textTheme.labelSmall?.copyWith(color: const Color(0xFFE5EDF8), fontSize: 10)),
    );
  }
}

List<String> _withCurrentOptions(List<String> options, String current) {
  final trimmed = current.trim();
  final next = _dedupeRoleManagerOptions(options);
  return trimmed.isEmpty || next.contains(trimmed) ? next : [...next, trimmed];
}

List<String> _dedupeRoleManagerOptions(List<String> options) {
  final next = <String>[];
  for (final option in options) {
    final trimmed = option.trim();
    if (trimmed.isNotEmpty && !next.contains(trimmed)) {
      next.add(trimmed);
    }
  }
  return next;
}

// ignore: unused_element
List<String> _withCurrentListOptions(List<String> options, List<String> current) {
  final next = _dedupeRoleManagerOptions(options);
  for (final item in current) {
    final trimmed = item.trim();
    if (trimmed.isNotEmpty && !next.contains(trimmed)) {
      next.add(trimmed);
    }
  }
  return next;
}

String _identityLabel(String value) => value;

String _roleManagerHumanLabel(String value) {
  final trimmed = value.trim();
  if (trimmed.isEmpty) {
    return 'None';
  }
  if (RegExp(r'^\d+(?:\.\d+)+(?:-[A-Za-z0-9.-]+)?$').hasMatch(trimmed) || trimmed.startsWith('gpt-')) {
    return trimmed;
  }
  const named = {
    'allow': 'Allow',
    'deny': 'Deny',
    'approved': 'Approved',
    'denied': 'Denied',
    'active': 'Active',
    'archived': 'Archived',
    'draft': 'Draft',
    'current': 'Current',
    'historical': 'Historical',
    'available': 'Available',
    'ownerApproval': 'Owner approval',
    'direct': 'Direct',
    'project': 'Project',
    'global': 'Global',
    'session': 'Session',
    'fixed': 'Fixed',
    'inherit': 'Inherit',
    'clean': 'Clean',
    'restricted': 'Restricted',
    'wait': 'Wait',
    'continue': 'Continue',
    'terminate': 'Terminate',
    'block': 'Block',
    'readOnly': 'Read only',
    'workspace': 'Workspace',
    'destructive': 'Destructive',
    'owner': 'Owner',
    'hidden': 'Hidden',
    'owner-only': 'Owner only',
    'owner-visible': 'Owner visible',
    'command.registry': 'Command registry',
    'workflow.memory': 'Workflow memory',
    'message.send': 'Send message',
    'message.route': 'Route message',
    'agent.spawn': 'Spawn agent',
    'agent.archive': 'Archive agent',
    'tool.execute_code': 'Execute code',
    'tool.request_command_registry_change': 'Request command registry change',
    'command_registry.apply': 'Apply command registry request',
    'command_registry.decide': 'Decide command registry request',
    'command_registry.request': 'Request command registry change',
    'workflow_memory.feedback': 'Workflow memory feedback',
    'workflow_memory.remember.global': 'Remember globally',
    'workflow_memory.remember.project': 'Remember for project',
    'workflow_memory.search': 'Search workflow memory',
    'fs.read': 'Read file',
    'fs.write': 'Write file',
    'patch.apply': 'Apply patch',
  };
  final direct = named[trimmed];
  if (direct != null) {
    return direct;
  }
  final spaced = trimmed.replaceAll('_', ' ').replaceAll('.', ' ');
  return spaced
      .split(RegExp(r'\s+'))
      .where((part) => part.isNotEmpty)
      .map((part) => part.length == 1 ? part.toUpperCase() : '${part[0].toUpperCase()}${part.substring(1)}')
      .join(' ');
}

String _roleManagerPolicyNote(String action) {
  const notes = {
    'tool.execute_code': 'Native code execution',
    'tool.request_command_registry_change': 'Registry change request',
    'fs.read': 'Read workspace files',
    'fs.write': 'Write to workspace',
    'patch.apply': 'Apply patches',
    'workflow_memory.search': 'Search memory',
    'workflow_memory.feedback': 'Submit feedback',
    'command_registry.apply': 'Apply registry change',
    'command_registry.decide': 'Decide on command',
    'command_registry.request': 'Request registry info',
  };
  return notes[action] ?? _roleManagerHumanLabel(action);
}

List<AgentRuntimeRolePolicyRow> _rolePolicyRows(String value) {
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

String _selectedSessionId(AgentRuntimeWorkbenchData data) {
  return data.sessions.isEmpty ? '' : data.sessions.first.id;
}

class _CommandRegistrySurfaceControls extends StatelessWidget {
  const _CommandRegistrySurfaceControls({
    required this.selectedSessionId,
    required this.projectKey,
    this.onListInstalled,
    this.onListRequests,
  });

  final String selectedSessionId;
  final String projectKey;
  final AgentRuntimeCommandRegistryListAction? onListInstalled;
  final VoidCallback? onListRequests;

  @override
  Widget build(BuildContext context) {
    return Wrap(
      spacing: 8,
      runSpacing: 8,
      children: [
        OutlinedButton(
          onPressed: onListInstalled == null || selectedSessionId.isEmpty ? null : () => onListInstalled!(selectedSessionId, projectKey),
          child: const Text('Refresh installed commands'),
        ),
        OutlinedButton(
          onPressed: onListRequests,
          child: const Text('Refresh pending requests'),
        ),
      ],
    );
  }
}

class _OperationsRow {
  const _OperationsRow(this.label, this.value);
  final String label;
  final String value;
}

class _OperationsSection extends StatelessWidget {
  const _OperationsSection({
    required this.title,
    required this.rows,
    this.actions = const [],
    this.onActionPressed,
    this.onProcessInput,
    this.onApprovalApprove,
    this.onApprovalDeny,
    this.onCommandRegistryApprove,
    this.onCommandRegistryDeny,
    this.onCommandRegistryPreview,
  });

  final String title;
  final List<_OperationsRow> rows;
  final List<AgentRuntimeActionItem> actions;
  final VoidCallback? Function(AgentRuntimeActionItem action)? onActionPressed;
  final AgentRuntimeProcessInputAction? onProcessInput;
  final AgentRuntimeActionReason? onApprovalApprove;
  final AgentRuntimeActionReason? onApprovalDeny;
  final AgentRuntimeCommandRegistryDecisionAction? onCommandRegistryApprove;
  final AgentRuntimeCommandRegistryDecisionAction? onCommandRegistryDeny;
  final AgentRuntimeCommandRegistryDecisionAction? onCommandRegistryPreview;

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
                  if (action.kind == 'processInput')
                    _ProcessInputControl(action: action, onSubmit: onProcessInput)
                  else if (action.kind == 'approval')
                    _OperationApprovalDecisionControl(action: action, label: 'Approve', onSubmit: onApprovalApprove)
                  else if (action.kind == 'approvalDeny')
                    _OperationApprovalDecisionControl(action: action, label: 'Deny', onSubmit: onApprovalDeny)
                  else if (_isCommandRegistryDecisionAction(action))
                    _CommandRegistryDecisionControl(
                      action: action,
                      onApprove: onCommandRegistryApprove,
                      onDeny: onCommandRegistryDeny,
                      onPreview: onCommandRegistryPreview,
                    )
                  else
                    OutlinedButton(
                      onPressed: onActionPressed?.call(action),
                      child: Text(action.kind == 'processTerminate' ? 'Terminate process' : _cleanSectionCopy(action.title)),
                    ),
              ],
            ),
          ],
        ],
      ),
    );
  }
}

class _ProcessInputControl extends StatefulWidget {
  const _ProcessInputControl({required this.action, required this.onSubmit});

  final AgentRuntimeActionItem action;
  final AgentRuntimeProcessInputAction? onSubmit;

  @override
  State<_ProcessInputControl> createState() => _ProcessInputControlState();
}

class _ProcessInputControlState extends State<_ProcessInputControl> {
  late final TextEditingController _controller;
  String? _error;

  @override
  void initState() {
    super.initState();
    _controller = TextEditingController();
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final policyRejected = widget.action.stateText.toLowerCase().contains('disabled') || widget.action.stateText.toLowerCase().contains('rejected');
    final disabled = widget.onSubmit == null || policyRejected;
    return SizedBox(
      width: 260,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          TextField(
            key: ValueKey('agentRuntime.process.input.${widget.action.id}'),
            enabled: !disabled,
            controller: _controller,
            decoration: InputDecoration(
              labelText: 'Process input',
              hintText: policyRejected ? _cleanSectionCopy(widget.action.stateText) : disabled ? 'Input unavailable for this process policy' : 'Text to send to stdin',
              errorText: _error,
              isDense: true,
              border: const OutlineInputBorder(),
            ),
          ),
          const SizedBox(height: 6),
          OutlinedButton(
            onPressed: disabled ? null : _submit,
            child: const Text('Send input'),
          ),
          if (policyRejected)
            Padding(
              padding: const EdgeInsets.only(top: 4),
              child: Text('Stdin is disabled by process policy.', style: Theme.of(context).textTheme.bodySmall?.copyWith(color: const Color(0xFF8FA1B8))),
            ),
        ],
      ),
    );
  }

  void _submit() {
    final text = _controller.text;
    if (text.isEmpty) {
      setState(() => _error = 'Input text is required.');
      return;
    }
    setState(() => _error = null);
    widget.onSubmit?.call(widget.action.id, text);
  }
}

class _OperationApprovalDecisionControl extends StatefulWidget {
  const _OperationApprovalDecisionControl({required this.action, required this.label, required this.onSubmit});

  final AgentRuntimeActionItem action;
  final String label;
  final AgentRuntimeActionReason? onSubmit;

  @override
  State<_OperationApprovalDecisionControl> createState() => _OperationApprovalDecisionControlState();
}

class _OperationApprovalDecisionControlState extends State<_OperationApprovalDecisionControl> {
  late final TextEditingController _controller;
  String? _error;

  @override
  void initState() {
    super.initState();
    _controller = TextEditingController();
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 260,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          TextField(
            key: ValueKey('agentRuntime.approval.reason.${widget.action.id}'),
            controller: _controller,
            enabled: widget.onSubmit != null,
            decoration: InputDecoration(
              labelText: 'Decision reason',
              hintText: 'Reason required',
              errorText: _error,
              isDense: true,
              border: const OutlineInputBorder(),
            ),
          ),
          const SizedBox(height: 6),
          OutlinedButton(onPressed: widget.onSubmit == null ? null : _submit, child: Text(widget.label)),
        ],
      ),
    );
  }

  void _submit() {
    final reason = _controller.text.trim();
    if (reason.isEmpty) {
      setState(() => _error = 'Reason is required.');
      return;
    }
    setState(() => _error = null);
    widget.onSubmit?.call(widget.action, reason);
  }
}

bool _isCommandRegistryDecisionAction(AgentRuntimeActionItem action) {
  return action.kind == 'commandRegistryRequest' || action.kind == 'commandRegistryDeny' || action.kind == 'commandRegistryPreview';
}

class _CommandRegistryDecisionControl extends StatefulWidget {
  const _CommandRegistryDecisionControl({
    required this.action,
    required this.onApprove,
    required this.onDeny,
    required this.onPreview,
  });

  final AgentRuntimeActionItem action;
  final AgentRuntimeCommandRegistryDecisionAction? onApprove;
  final AgentRuntimeCommandRegistryDecisionAction? onDeny;
  final AgentRuntimeCommandRegistryDecisionAction? onPreview;

  @override
  State<_CommandRegistryDecisionControl> createState() => _CommandRegistryDecisionControlState();
}

class _CommandRegistryDecisionControlState extends State<_CommandRegistryDecisionControl> {
  late String _status;
  late String _scopeType;
  late String _policyDecision;
  late String _cwdPolicy;
  late String _envPolicy;
  late String _stdinPolicy;
  late String _endOfTurnBehavior;
  late String _endOfSessionBehavior;
  late String _mutationClass;
  late String _executionPolicy;
  late final TextEditingController _projectKeyController;
  late final TextEditingController _reasonController;
  late final TextEditingController _actionIdController;
  late final TextEditingController _displayNameController;
  late final TextEditingController _binaryController;
  late final TextEditingController _cwdController;
  late final TextEditingController _maxRuntimeController;
  late final TextEditingController _descriptionController;
  late List<String> _argvTemplate;
  late List<String> _forbiddenArgs;
  bool _syncAllowed = true;
  bool _asyncAllowed = false;
  bool _allowCwdArg = false;
  bool _allowArgsArg = false;

  @override
  void initState() {
    super.initState();
    final draft = _initialCommandRegistryDecisionDraft(widget.action, widget.action.kind == 'commandRegistryDeny' ? 'denied' : 'approved');
    _status = draft.status;
    _scopeType = draft.scopeType;
    _policyDecision = draft.policyDecision;
    _cwdPolicy = draft.cwdPolicy;
    _envPolicy = draft.envPolicy;
    _stdinPolicy = draft.stdinPolicy;
    _endOfTurnBehavior = draft.endOfTurnBehavior;
    _endOfSessionBehavior = draft.endOfSessionBehavior;
    _mutationClass = draft.mutationClass;
    _executionPolicy = draft.executionPolicy;
    _projectKeyController = TextEditingController(text: draft.projectKey);
    _reasonController = TextEditingController(text: draft.policyReason);
    _actionIdController = TextEditingController(text: draft.actionId);
    _displayNameController = TextEditingController(text: draft.displayName);
    _binaryController = TextEditingController(text: draft.binaryName);
    _argvTemplate = List<String>.from(draft.argvTemplate);
    _cwdController = TextEditingController(text: draft.defaultCwd);
    _maxRuntimeController = TextEditingController(text: draft.maxRuntimeMs?.toString() ?? '');
    _descriptionController = TextEditingController(text: draft.modelDescription);
    _forbiddenArgs = List<String>.from(draft.forbiddenArgs);
    _syncAllowed = draft.syncAllowed;
    _asyncAllowed = draft.asyncAllowed;
    _allowCwdArg = draft.allowCwdArg;
    _allowArgsArg = draft.allowArgsArg;
  }

  @override
  void dispose() {
    _projectKeyController.dispose();
    _reasonController.dispose();
    _actionIdController.dispose();
    _displayNameController.dispose();
    _binaryController.dispose();
    _cwdController.dispose();
    _maxRuntimeController.dispose();
    _descriptionController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 560,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text('Final template editor', style: Theme.of(context).textTheme.labelMedium?.copyWith(color: Colors.white70, fontWeight: FontWeight.w800)),
          const SizedBox(height: 8),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              _EnumSelect(label: 'Decision', value: _status, values: const ['approved', 'denied'], onChanged: (value) => setState(() => _status = value), displayLabel: _roleManagerHumanLabel),
              _EnumSelect(label: 'Scope', value: _scopeType, values: const ['project', 'global', 'session'], onChanged: (value) => setState(() => _scopeType = value), displayLabel: _roleManagerHumanLabel),
              _EnumSelect(label: 'Execution policy', value: _policyDecision, values: const ['allow', 'deny'], onChanged: (value) => setState(() => _policyDecision = value), displayLabel: _roleManagerHumanLabel),
              _SizedTextField(label: 'Project key', controller: _projectKeyController),
              _SizedTextField(label: 'Reason', controller: _reasonController),
              _SizedTextField(label: 'Action id', controller: _actionIdController),
              _SizedTextField(label: 'Display name', controller: _displayNameController),
              _SizedTextField(label: 'Binary', controller: _binaryController),
              _TokenListEditor(label: 'Argv template', values: _argvTemplate, onChanged: (next) => setState(() => _argvTemplate = next)),
              _SizedTextField(label: 'Default cwd', controller: _cwdController),
              _EnumSelect(label: 'Cwd policy', value: _cwdPolicy, values: const ['project', 'fixed', 'session'], onChanged: (value) => setState(() => _cwdPolicy = value), displayLabel: _roleManagerHumanLabel),
              _EnumSelect(label: 'Env policy', value: _envPolicy, values: const ['inherit', 'clean', 'restricted'], onChanged: (value) => setState(() => _envPolicy = value), displayLabel: _roleManagerHumanLabel),
              _EnumSelect(label: 'Stdin policy', value: _stdinPolicy, values: const ['deny', 'allow'], onChanged: (value) => setState(() => _stdinPolicy = value), displayLabel: _roleManagerHumanLabel),
              _EnumSelect(label: 'End of turn', value: _endOfTurnBehavior, values: const ['wait', 'continue', 'terminate'], onChanged: (value) => setState(() => _endOfTurnBehavior = value), displayLabel: _roleManagerHumanLabel),
              _EnumSelect(label: 'End of session', value: _endOfSessionBehavior, values: const ['terminate', 'block', 'continue'], onChanged: (value) => setState(() => _endOfSessionBehavior = value), displayLabel: _roleManagerHumanLabel),
              _EnumSelect(label: 'Mutation class', value: _mutationClass, values: const ['readOnly', 'workspace', 'destructive'], onChanged: (value) => setState(() => _mutationClass = value), displayLabel: _roleManagerHumanLabel),
              _EnumSelect(label: 'Execute', value: _executionPolicy, values: const ['allow', 'deny'], onChanged: (value) => setState(() => _executionPolicy = value), displayLabel: _roleManagerHumanLabel),
              _SizedTextField(label: 'Max runtime ms', controller: _maxRuntimeController),
              _SizedTextField(label: 'Model description', controller: _descriptionController, maxLines: 2),
              _TokenListEditor(label: 'Forbidden args', values: _forbiddenArgs, onChanged: (next) => setState(() => _forbiddenArgs = next)),
            ],
          ),
          Wrap(
            spacing: 12,
            runSpacing: 2,
            children: [
              _EditorSwitch(label: 'Sync allowed', value: _syncAllowed, onChanged: (value) => setState(() => _syncAllowed = value)),
              _EditorSwitch(label: 'Async allowed', value: _asyncAllowed, onChanged: (value) => setState(() => _asyncAllowed = value)),
              _EditorSwitch(label: 'Allow cwd arg', value: _allowCwdArg, onChanged: (value) => setState(() => _allowCwdArg = value)),
              _EditorSwitch(label: 'Allow args arg', value: _allowArgsArg, onChanged: (value) => setState(() => _allowArgsArg = value)),
            ],
          ),
          Wrap(
            spacing: 8,
            runSpacing: 6,
            children: [
              OutlinedButton(onPressed: widget.onPreview == null ? null : () => widget.onPreview!(widget.action, _draft()), child: const Text('Preview Decision')),
              OutlinedButton(onPressed: widget.onApprove == null ? null : () => widget.onApprove!(widget.action, _draft().copyWith(status: 'approved')), child: const Text('Approve')),
              OutlinedButton(onPressed: widget.onDeny == null ? null : () => widget.onDeny!(widget.action, _draft().copyWith(status: 'denied')), child: const Text('Deny')),
            ],
          ),
        ],
      ),
    );
  }

  AgentRuntimeCommandRegistryDecisionDraft _draft() {
    return AgentRuntimeCommandRegistryDecisionDraft(
      status: _status,
      scopeType: _scopeType,
      projectKey: _projectKeyController.text.trim(),
      policyDecision: _policyDecision,
      policyReason: _reasonController.text.trim(),
      actionId: _actionIdController.text.trim(),
      displayName: _displayNameController.text.trim(),
      binaryName: _binaryController.text.trim(),
      argvTemplate: _argvTemplate,
      defaultCwd: _cwdController.text.trim(),
      cwdPolicy: _cwdPolicy,
      envPolicy: _envPolicy,
      stdinPolicy: _stdinPolicy,
      syncAllowed: _syncAllowed,
      asyncAllowed: _asyncAllowed,
      maxRuntimeMs: int.tryParse(_maxRuntimeController.text.trim()),
      endOfTurnBehavior: _endOfTurnBehavior,
      endOfSessionBehavior: _endOfSessionBehavior,
      mutationClass: _mutationClass,
      modelDescription: _descriptionController.text.trim(),
      allowCwdArg: _allowCwdArg,
      allowArgsArg: _allowArgsArg,
      forbiddenArgs: _forbiddenArgs,
      executionPolicy: _executionPolicy,
    );
  }
}

List<String> _lineList(String value) {
  return value
      .split(RegExp(r'[\n,]'))
      .map((part) => part.trim())
      .where((part) => part.isNotEmpty)
      .toList(growable: false);
}

class _TokenListEditor extends StatefulWidget {
  const _TokenListEditor({required this.label, required this.values, required this.onChanged});

  final String label;
  final List<String> values;
  final ValueChanged<List<String>> onChanged;

  @override
  State<_TokenListEditor> createState() => _TokenListEditorState();
}

class _TokenListEditorState extends State<_TokenListEditor> {
  late final TextEditingController _controller;

  @override
  void initState() {
    super.initState();
    _controller = TextEditingController();
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 360,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(widget.label, style: Theme.of(context).textTheme.labelSmall?.copyWith(color: const Color(0xFF93A5BC))),
          const SizedBox(height: 4),
          Wrap(
            spacing: 6,
            runSpacing: 6,
            children: [
              for (final value in widget.values)
                InputChip(
                  label: Text(value),
                  onDeleted: () => widget.onChanged(widget.values.where((item) => item != value).toList(growable: false)),
                ),
            ],
          ),
          const SizedBox(height: 4),
          Row(
            children: [
              Expanded(
                child: TextField(
                  controller: _controller,
                  decoration: const InputDecoration(
                    labelText: 'Add item',
                    isDense: true,
                    border: OutlineInputBorder(),
                  ),
                  onSubmitted: (_) => _add(),
                ),
              ),
              const SizedBox(width: 6),
              OutlinedButton(onPressed: _add, child: const Text('Add')),
            ],
          ),
        ],
      ),
    );
  }

  void _add() {
    final value = _controller.text.trim();
    if (value.isEmpty || widget.values.contains(value)) {
      return;
    }
    _controller.clear();
    widget.onChanged([...widget.values, value]);
  }
}

class _SizedTextField extends StatelessWidget {
  const _SizedTextField({required this.label, required this.controller, this.maxLines = 1});

  final String label;
  final TextEditingController controller;
  final int maxLines;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 172,
      child: TextField(
        controller: controller,
        maxLines: maxLines,
        minLines: 1,
        decoration: InputDecoration(labelText: label, isDense: true, border: const OutlineInputBorder()),
      ),
    );
  }
}

class _EnumSelect extends StatelessWidget {
  const _EnumSelect({
    super.key,
    required this.label,
    required this.value,
    required this.values,
    required this.onChanged,
    this.displayLabel,
  });

  final String label;
  final String value;
  final List<String> values;
  final ValueChanged<String> onChanged;
  final String Function(String value)? displayLabel;

  @override
  Widget build(BuildContext context) {
    final effectiveValues = _dedupeRoleManagerOptions(values);
    if (effectiveValues.isEmpty) {
      return SizedBox(
        width: 220,
        child: TextField(
          enabled: false,
          decoration: InputDecoration(labelText: label.isEmpty ? null : label, hintText: 'Unavailable', isDense: true, border: const OutlineInputBorder()),
        ),
      );
    }
    final trimmedValue = value.trim();
    final effective = effectiveValues.contains(trimmedValue) ? trimmedValue : effectiveValues.first;
    return SizedBox(
      width: 220,
      child: DropdownButtonFormField<String>(
        initialValue: effective,
        decoration: InputDecoration(labelText: label, isDense: true, border: const OutlineInputBorder()),
        isExpanded: true,
        items: effectiveValues.map((item) => DropdownMenuItem(value: item, child: Text((displayLabel ?? _identityLabel)(item)))).toList(growable: false),
        onChanged: (next) {
          if (next != null) {
            onChanged(next);
          }
        },
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

AgentRuntimeCommandRegistryDecisionDraft _initialCommandRegistryDecisionDraft(
  AgentRuntimeActionItem action,
  String status,
) {
  final normalizedTitle = action.title
      .toLowerCase()
      .replaceAll(RegExp(r'[^a-z0-9]+'), '.')
      .replaceAll(RegExp(r'^\\.+|\\.+$'), '');
  final actionId = normalizedTitle.isEmpty ? 'command.request' : normalizedTitle;
  return AgentRuntimeCommandRegistryDecisionDraft(
    status: status,
    scopeType: 'project',
    projectKey: '',
    policyDecision: status == 'denied' ? 'deny' : 'allow',
    policyReason: action.subtitle.isEmpty ? 'Reviewed in Agent Runtime Command Registry.' : action.subtitle,
    actionId: actionId,
    displayName: action.title.isEmpty ? 'Command request' : action.title,
    binaryName: actionId.split('.').first,
    argvTemplate: const <String>[],
    defaultCwd: '.',
    cwdPolicy: 'project',
    envPolicy: 'inherit',
    stdinPolicy: 'deny',
    syncAllowed: true,
    asyncAllowed: false,
    maxRuntimeMs: null,
    endOfTurnBehavior: 'terminate',
    endOfSessionBehavior: 'terminate',
    mutationClass: 'readOnly',
    modelDescription: action.subtitle,
    allowCwdArg: false,
    allowArgsArg: false,
    forbiddenArgs: const <String>[],
    executionPolicy: status == 'denied' ? 'deny' : 'allow',
  );
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
      .replaceAll('project agent-runtime', 'Project workspace')
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

class _ActionTile extends StatefulWidget {
  const _ActionTile(
    this.item, {
    this.onApprovalApprove,
    this.onApprovalDeny,
    this.onApprovalResume,
    this.onCommandRegistryApprove,
    this.onCommandRegistryDeny,
    this.onCommandRegistryPreview,
    this.onCommandRegistryApply,
    this.onCommandRegistryReview,
  });

  final AgentRuntimeActionItem item;
  final ValueChanged<String>? onApprovalApprove;
  final ValueChanged<String>? onApprovalDeny;
  final VoidCallback? onApprovalResume;
  final AgentRuntimeCommandRegistryDecisionAction? onCommandRegistryApprove;
  final AgentRuntimeCommandRegistryDecisionAction? onCommandRegistryDeny;
  final AgentRuntimeCommandRegistryDecisionAction? onCommandRegistryPreview;
  final VoidCallback? onCommandRegistryApply;
  final VoidCallback? onCommandRegistryReview;

  @override
  State<_ActionTile> createState() => _ActionTileState();
}

class _ActionTileState extends State<_ActionTile> {
  late final TextEditingController _reasonController;
  String? _reasonError;

  @override
  void initState() {
    super.initState();
    _reasonController = TextEditingController();
  }

  @override
  void dispose() {
    _reasonController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final actions = <Widget>[];
    if (widget.item.kind == 'approval') {
      actions.add(OutlinedButton(onPressed: widget.onApprovalApprove == null ? null : () => _submitReason(widget.onApprovalApprove!), child: const Text('Approve')));
    } else if (widget.item.kind == 'approvalDeny') {
      actions.add(OutlinedButton(onPressed: widget.onApprovalDeny == null ? null : () => _submitReason(widget.onApprovalDeny!), child: const Text('Deny')));
    } else if (widget.item.kind == 'approvalResume') {
      actions.add(OutlinedButton(onPressed: widget.onApprovalResume, child: const Text('Resume')));
    } else if (_isCommandRegistryDecisionAction(widget.item)) {
      actions.add(_CommandRegistryDecisionControl(
        action: widget.item,
        onApprove: widget.onCommandRegistryApprove,
        onDeny: widget.onCommandRegistryDeny,
        onPreview: widget.onCommandRegistryPreview,
      ));
    } else if (widget.item.kind == 'commandRegistryReview') {
      actions.add(OutlinedButton(onPressed: widget.onCommandRegistryReview, child: const Text('Review')));
    } else if (widget.item.kind == 'commandRegistryApply') {
      actions.add(OutlinedButton(onPressed: widget.onCommandRegistryApply, child: const Text('Apply')));
    }
    final needsReason = widget.item.kind == 'approval' || widget.item.kind == 'approvalDeny';
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _DenseTile(
          title: widget.item.title,
          subtitle: widget.item.subtitle,
          trailing: widget.item.stateText,
          eyebrow: widget.item.kind,
          tone: widget.item.tone,
        ),
        if (needsReason) ...[
          const SizedBox(height: 6),
          TextField(
            key: ValueKey('agentRuntime.approval.reason.${widget.item.id}'),
            controller: _reasonController,
            minLines: 1,
            maxLines: 3,
            decoration: InputDecoration(
              labelText: 'Decision reason',
              hintText: 'Explain the approval decision',
              errorText: _reasonError,
              isDense: true,
              border: const OutlineInputBorder(),
            ),
          ),
        ],
        if (actions.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(top: 4, bottom: 8),
            child: Wrap(spacing: 6, runSpacing: 4, children: actions),
          ),
      ],
    );
  }

  void _submitReason(ValueChanged<String> action) {
    final reason = _reasonController.text.trim();
    if (reason.isEmpty) {
      setState(() => _reasonError = 'Reason is required.');
      return;
    }
    setState(() => _reasonError = null);
    action(reason);
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
                _FactRow(AgentRuntimeFact(label: 'Selected memory', value: detail.title)),
                _FactRow(AgentRuntimeFact(label: 'Scope', value: _displayCopy(detail.scopeLabel))),
                _FactRow(AgentRuntimeFact(label: 'Origin', value: detail.sourceSessionId.isEmpty ? 'Saved workflow' : 'Saved from a session')),
                _FactRow(AgentRuntimeFact(label: 'Summary', value: detail.summary)),
                _FactRow(const AgentRuntimeFact(label: 'Source details', value: 'Available in Diagnostics')),
                if (workflowMemory.recentEvents.isNotEmpty) ...[
                  const SizedBox(height: 8),
                  Text('Recent events', style: theme.textTheme.labelSmall?.copyWith(color: const Color(0xFF93A5BC))),
                  const SizedBox(height: 4),
                  ...workflowMemory.recentEvents.take(3).map(
                        (event) => _DenseTile(
                          title: event.title,
                          subtitle: event.subtitle,
                          trailing: event.createdAt ?? '',
                          eyebrow: 'memory event',
                          tone: event.tone,
                        ),
                      ),
                ],
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

// ignore: unused_element
class _StructuredListEditor extends StatelessWidget {
  const _StructuredListEditor({required this.label, required this.values, required this.selected, required this.onChanged});

  final String label;
  final List<String> values;
  final List<String> selected;
  final ValueChanged<List<String>> onChanged;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(label, style: Theme.of(context).textTheme.labelSmall?.copyWith(color: const Color(0xFF93A5BC))),
        const SizedBox(height: 4),
        Wrap(
          spacing: 6,
          runSpacing: 6,
          children: [
            for (final value in values)
              FilterChip(
                label: Text(_roleManagerHumanLabel(value)),
                labelStyle: TextStyle(color: selected.contains(value) ? const Color(0xFF04111C) : const Color(0xFFD6E2F2), fontWeight: selected.contains(value) ? FontWeight.w800 : FontWeight.w500),
                selectedColor: const Color(0xFF43C8FF),
                backgroundColor: const Color(0xFF132236),
                checkmarkColor: const Color(0xFF04111C),
                side: BorderSide(color: selected.contains(value) ? const Color(0xFF43C8FF) : const Color(0xFF263A52)),
                selected: selected.contains(value),
                onSelected: (enabled) {
                  final next = [...selected];
                  if (enabled && !next.contains(value)) {
                    next.add(value);
                  } else if (!enabled) {
                    next.remove(value);
                  }
                  onChanged(next);
                },
              ),
          ],
        ),
      ],
    );
  }
}

// ignore: unused_element
class _PolicyMatrixEditor extends StatelessWidget {
  const _PolicyMatrixEditor({required this.label, required this.actions, required this.decisions, required this.rows, required this.onChanged});

  final String label;
  final List<String> actions;
  final List<String> decisions;
  final List<AgentRuntimeRolePolicyRow> rows;
  final ValueChanged<List<AgentRuntimeRolePolicyRow>> onChanged;

  @override
  Widget build(BuildContext context) {
    final byAction = {for (final row in rows) row.action: row.decision};
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(label, style: Theme.of(context).textTheme.labelSmall?.copyWith(color: const Color(0xFF93A5BC))),
        const SizedBox(height: 4),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            for (final action in actions)
              SizedBox(
                width: 260,
                child: Row(
                  children: [
                    Expanded(child: Text(_roleManagerHumanLabel(action), overflow: TextOverflow.ellipsis, style: Theme.of(context).textTheme.bodySmall?.copyWith(color: const Color(0xFFE5EDF8)))),
                    const SizedBox(width: 6),
                    SizedBox(
                      width: 116,
                      child: DropdownButtonFormField<String>(
                        initialValue: decisions.contains(byAction[action]) ? byAction[action] : decisions.first,
                        isExpanded: true,
                        decoration: const InputDecoration(isDense: true, border: OutlineInputBorder()),
                        items: decisions.map((value) => DropdownMenuItem(value: value, child: Text(_roleManagerHumanLabel(value)))).toList(growable: false),
                        onChanged: (decision) {
                          if (decision == null) {
                            return;
                          }
                          final next = {...byAction, action: decision};
                          onChanged(next.entries.map((entry) => AgentRuntimeRolePolicyRow(action: entry.key, decision: entry.value)).toList(growable: false));
                        },
                      ),
                    ),
                  ],
                ),
              ),
          ],
        ),
      ],
    );
  }
}

class _EditorTextField extends StatelessWidget {
  const _EditorTextField({required this.label, required this.controller});

  final String label;
  final TextEditingController controller;

  @override
  Widget build(BuildContext context) {
    return TextField(
      controller: controller,
      maxLines: 1,
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
  const _EditorSwitch({super.key, required this.label, required this.value, required this.onChanged});

  final String label;
  final bool value;
  final ValueChanged<bool> onChanged;

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Switch(value: value, onChanged: onChanged),
        Flexible(child: Text(label, overflow: TextOverflow.ellipsis, style: Theme.of(context).textTheme.bodySmall?.copyWith(color: const Color(0xFFB7C4D8)))),
      ],
    );
  }
}

class _RoleVersionTile extends StatelessWidget {
  const _RoleVersionTile({
    required this.row,
    required this.roleId,
    this.onActivate,
    this.onShowVersionData,
  });

  final AgentRuntimeRoleVersionRow row;
  final String roleId;
  final AgentRuntimeRoleVersionAction? onActivate;
  final ValueChanged<String>? onShowVersionData;

  bool get _isCurrent => row.status == 'current' || row.status == 'active';

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final createdAt = row.createdAt == null || row.createdAt!.isEmpty ? 'created time unavailable' : row.createdAt!;
    final activateLabel = _isCurrent ? 'Current role version ${row.version}' : 'Activate role version ${row.version}';
    return Padding(
      padding: const EdgeInsets.fromLTRB(0, 8, 0, 8),
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
                    'v${row.version}',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: theme.textTheme.bodySmall?.copyWith(color: Colors.white, fontWeight: FontWeight.w800),
                  ),
                  const SizedBox(height: 2),
                  Text(
                    createdAt,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: theme.textTheme.bodySmall?.copyWith(color: const Color(0xFF8FA1B8)),
                  ),
                ],
              ),
            ),
            const SizedBox(width: 8),
            _Chip(label: row.status, tone: _isCurrent ? 'success' : 'info'),
          ],
        ),
          const SizedBox(height: 8),
          Wrap(
            spacing: 6,
            runSpacing: 6,
            children: [
              Semantics(
                label: 'Show data for role version ${row.version}',
                button: true,
                child: OutlinedButton(
                  onPressed: onShowVersionData == null ? null : () => onShowVersionData!(row.versionId),
                  child: const Text('Show data'),
                ),
              ),
              Semantics(
                label: activateLabel,
                button: true,
                child: OutlinedButton(
                  onPressed: _isCurrent || roleId.isEmpty || onActivate == null ? null : () => onActivate!(roleId, row.versionId),
                  child: Text(_isCurrent ? 'Current' : 'Activate'),
                ),
              ),
            ],
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
