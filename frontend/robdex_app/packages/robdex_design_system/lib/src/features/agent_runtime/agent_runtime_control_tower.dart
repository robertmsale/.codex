import 'package:code_forge/code_forge.dart';
import 'package:flutter/material.dart';

import '../../core/models/agent_runtime_control_tower_models.dart';

typedef AgentRuntimeRoleVersionAction = void Function(String roleId, String versionId);

class AgentRuntimeControlTower extends StatelessWidget {
  const AgentRuntimeControlTower({
    super.key,
    required this.data,
    required this.baseUrlController,
    required this.onConnect,
    required this.onRefreshDiscovery,
    required this.onConnectDiscovered,
    required this.onPollStream,
    required this.onDisconnect,
    this.onRoleValidate,
    this.onRoleCreate,
    this.onRoleUpdate,
    this.onRoleExport,
    this.onRoleArchive,
    this.onRoleUnarchive,
    this.onRoleActivate,
  });

  final AgentRuntimeControlTowerData data;
  final TextEditingController baseUrlController;
  final VoidCallback onConnect;
  final VoidCallback onRefreshDiscovery;
  final VoidCallback onConnectDiscovered;
  final VoidCallback onPollStream;
  final VoidCallback onDisconnect;
  final ValueChanged<AgentRuntimeRoleEditorDraft>? onRoleValidate;
  final ValueChanged<AgentRuntimeRoleEditorDraft>? onRoleCreate;
  final ValueChanged<AgentRuntimeRoleEditorDraft>? onRoleUpdate;
  final ValueChanged<String>? onRoleExport;
  final ValueChanged<String>? onRoleArchive;
  final ValueChanged<String>? onRoleUnarchive;
  final AgentRuntimeRoleVersionAction? onRoleActivate;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return DecoratedBox(
      decoration: const BoxDecoration(color: Color(0xFF05090F)),
      child: SafeArea(
        child: Column(
          children: [
            _StatusStrip(
              data: data,
              baseUrlController: baseUrlController,
              onConnect: onConnect,
              onRefreshDiscovery: onRefreshDiscovery,
              onConnectDiscovered: onConnectDiscovered,
              onPollStream: onPollStream,
              onDisconnect: onDisconnect,
            ),
            _DiscoveryStrip(
              discovery: data.discovery,
              onRefreshDiscovery: onRefreshDiscovery,
              onConnectDiscovered: onConnectDiscovered,
            ),
            if (data.errorMessage case final error?)
              Container(
                width: double.infinity,
                margin: const EdgeInsets.fromLTRB(16, 0, 16, 10),
                padding: const EdgeInsets.all(10),
                decoration: BoxDecoration(
                  color: const Color(0xFF3A1217),
                  border: Border.all(color: const Color(0xFF92323D)),
                  borderRadius: BorderRadius.circular(10),
                ),
                child: Text(
                  error,
                  style: theme.textTheme.bodySmall?.copyWith(color: const Color(0xFFFFC9CF)),
                ),
              ),
            Expanded(
              child: Padding(
                padding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    SizedBox(
                      width: 260,
                      child: _Panel(
                        title: data.sessionsTitle,
                        subtitle: data.sessionsSubtitle,
                        child: data.sessions.isEmpty
                            ? _EmptyState(
                                title: data.sessionsEmptyTitle,
                                body: data.sessionsEmptyText,
                              )
                            : ListView.separated(
                                itemCount: data.sessions.length,
                                separatorBuilder: (_, _) => const SizedBox(height: 8),
                                itemBuilder: (context, index) => _SessionTile(data.sessions[index]),
                              ),
                      ),
                    ),
                    const SizedBox(width: 12),
                    Expanded(
                      child: _Panel(
                        title: data.timelineTitle,
                        subtitle: data.timelineSubtitle,
                        child: data.timeline.isEmpty
                            ? _EmptyState(
                                title: data.timelineEmptyTitle,
                                body: data.timelineEmptyText,
                              )
                            : ListView.separated(
                                itemCount: data.timeline.length,
                                separatorBuilder: (_, _) => const Divider(height: 16),
                                itemBuilder: (context, index) => _TimelineTile(data.timeline[index]),
                              ),
                      ),
                    ),
                    const SizedBox(width: 12),
                    SizedBox(
                      width: 300,
                      child: Column(
                        children: [
                          Expanded(
                            child: _Panel(
                              title: data.actionsTitle,
                              subtitle: data.actionsSubtitle,
                              child: data.actions.isEmpty
                                  ? _EmptyState(
                                      title: data.actionsEmptyTitle,
                                      body: data.actionsEmptyText,
                                    )
                                  : ListView.separated(
                                      itemCount: data.actions.length,
                                      separatorBuilder: (_, _) => const SizedBox(height: 8),
                                      itemBuilder: (context, index) => _ActionTile(data.actions[index]),
                                    ),
                            ),
                          ),
                          const SizedBox(height: 12),
                          Expanded(
                            child: _Panel(
                              title: data.detailTitle,
                              subtitle: data.detailSubtitle,
                              child: ListView(
                                children: [
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
                                  const SizedBox(height: 12),
                                  ...data.controllerFacts.map(_FactRow.new),
                                  const SizedBox(height: 10),
                                  Text(
                                    'Recent outputs',
                                    style: theme.textTheme.labelMedium?.copyWith(color: Colors.white70),
                                  ),
                                  const SizedBox(height: 6),
                                  ...data.outputLog.map(
                                    (line) => Text(
                                      line,
                                      style: theme.textTheme.bodySmall?.copyWith(color: const Color(0xFF98A6B8)),
                                    ),
                                  ),
                                ],
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _StatusStrip extends StatelessWidget {
  const _StatusStrip({
    required this.data,
    required this.baseUrlController,
    required this.onConnect,
    required this.onRefreshDiscovery,
    required this.onConnectDiscovered,
    required this.onPollStream,
    required this.onDisconnect,
  });

  final AgentRuntimeControlTowerData data;
  final TextEditingController baseUrlController;
  final VoidCallback onConnect;
  final VoidCallback onRefreshDiscovery;
  final VoidCallback onConnectDiscovered;
  final VoidCallback onPollStream;
  final VoidCallback onDisconnect;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      padding: const EdgeInsets.all(16),
      child: Row(
        children: [
          Text(
            'Agent Runtime Control Tower',
            style: theme.textTheme.titleMedium?.copyWith(color: Colors.white, fontWeight: FontWeight.w700),
          ),
          const SizedBox(width: 16),
          _Chip(label: data.connectionState, tone: data.connectionTone),
          const SizedBox(width: 8),
          _Chip(label: 'watermark ${data.watermarkLabel}', tone: 'info'),
          const SizedBox(width: 8),
          ...data.statusBadges.take(4).map(
                (badge) => Padding(
                  padding: const EdgeInsets.only(right: 8),
                  child: _MetricBadge(badge),
                ),
              ),
          const SizedBox(width: 16),
          Expanded(
            child: TextField(
              controller: baseUrlController,
              style: const TextStyle(color: Colors.white),
              decoration: const InputDecoration(
                labelText: 'Runtime base URL',
                isDense: true,
              ),
            ),
          ),
          const SizedBox(width: 10),
          FilledButton(onPressed: onConnect, child: const Text('Connect')),
          const SizedBox(width: 8),
          OutlinedButton(onPressed: onPollStream, child: Text('Poll${data.pendingRequestCount > 0 ? ' (${data.pendingRequestCount})' : ''}')),
          const SizedBox(width: 8),
          TextButton(onPressed: onDisconnect, child: const Text('Disconnect')),
        ],
      ),
    );
  }
}

class _DiscoveryStrip extends StatelessWidget {
  const _DiscoveryStrip({
    required this.discovery,
    required this.onRefreshDiscovery,
    required this.onConnectDiscovered,
  });

  final AgentRuntimeDiscoveryInfo discovery;
  final VoidCallback onRefreshDiscovery;
  final VoidCallback onConnectDiscovered;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      width: double.infinity,
      margin: const EdgeInsets.fromLTRB(16, 0, 16, 10),
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: const Color(0xFF0B111A),
        border: Border.all(color: _toneColor(discovery.tone).withValues(alpha: 0.55)),
        borderRadius: BorderRadius.circular(14),
      ),
      child: Row(
        children: [
          _Chip(label: discovery.state, tone: discovery.tone),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(discovery.title, style: theme.textTheme.labelLarge?.copyWith(color: Colors.white, fontWeight: FontWeight.w700)),
                const SizedBox(height: 3),
                Text(
                  '${discovery.message}${discovery.baseUrl == null ? '' : ' · ${discovery.baseUrl}'}',
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  style: theme.textTheme.bodySmall?.copyWith(color: const Color(0xFF94A5BC)),
                ),
              ],
            ),
          ),
          const SizedBox(width: 12),
          OutlinedButton(onPressed: onRefreshDiscovery, child: const Text('Refresh discovery')),
          const SizedBox(width: 8),
          FilledButton(
            onPressed: discovery.connectable ? onConnectDiscovered : null,
            child: const Text('Connect discovered'),
          ),
        ],
      ),
    );
  }
}

class _Panel extends StatelessWidget {
  const _Panel({required this.title, required this.child, this.subtitle});

  final String title;
  final String? subtitle;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: const Color(0xCC0B111A),
        border: Border.all(color: const Color(0xFF202B3A)),
        borderRadius: BorderRadius.circular(14),
      ),
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
  const _ActionTile(this.item);

  final AgentRuntimeActionItem item;

  @override
  Widget build(BuildContext context) {
    return _DenseTile(
      title: item.title,
      subtitle: item.subtitle,
      trailing: item.stateText,
      eyebrow: item.kind,
      tone: item.tone,
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
  late final CodeForgeController _instructionController;
  bool _listed = true;
  bool _ownerVisible = true;
  bool _canSpawnAgents = false;
  bool _canArchiveAgents = false;
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
    _instructionController = CodeForgeController();
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
    return DecoratedBox(
      decoration: BoxDecoration(
        color: const Color(0xFF0F1722),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: const Color(0xFF26364A)),
      ),
      child: Padding(
        padding: const EdgeInsets.all(10),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(roleAdmin.title, style: theme.textTheme.labelLarge?.copyWith(color: Colors.white, fontWeight: FontWeight.w700)),
            const SizedBox(height: 3),
            Text(roleAdmin.subtitle, style: theme.textTheme.bodySmall?.copyWith(color: const Color(0xFF8FA1B8))),
            const SizedBox(height: 10),
            if (roleAdmin.rows.isEmpty)
              _EmptyState(title: roleAdmin.emptyTitle, body: roleAdmin.emptyText)
            else ...[
              ...roleAdmin.rows.take(4).map((row) => Padding(
                    padding: const EdgeInsets.only(bottom: 6),
                    child: _DenseTile(
                      title: row.title,
                      subtitle: row.subtitle,
                      trailing: row.status,
                      eyebrow: row.id,
                      tone: row.tone,
                    ),
                  )),
              if (detail != null) ...[
                const SizedBox(height: 8),
                _FactRow(AgentRuntimeFact(label: 'Selected role', value: '${detail.displayName} · ${detail.version}')),
                _FactRow(AgentRuntimeFact(label: 'Model default', value: detail.model)),
                _FactRow(AgentRuntimeFact(label: 'Capabilities', value: detail.capabilities.length.toString())),
                _FactRow(AgentRuntimeFact(label: 'Policy entries', value: detail.policy.length.toString())),
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
              if (roleAdmin.validationErrors.isNotEmpty) ...[
                const SizedBox(height: 8),
                ...roleAdmin.validationErrors.map(
                  (error) => Text(error, style: theme.textTheme.bodySmall?.copyWith(color: const Color(0xFFFFA8B4))),
                ),
              ],
              if (roleAdmin.actionStates.isNotEmpty) ...[
                const SizedBox(height: 8),
                ...roleAdmin.actionStates.take(3).map((action) => Padding(
                      padding: const EdgeInsets.only(bottom: 6),
                      child: _ActionTile(action),
                    )),
              ],
            ],
          ],
        ),
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

  final CodeForgeController instructionController;
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
            Expanded(child: _EditorTextField(label: 'Role id', controller: roleIdController)),
            const SizedBox(width: 6),
            Expanded(child: _EditorTextField(label: 'Version', controller: versionController)),
          ],
        ),
        const SizedBox(height: 6),
        _EditorTextField(label: 'Display name', controller: displayNameController),
        const SizedBox(height: 6),
        Row(
          children: [
            Expanded(child: _EditorTextField(label: 'Model default', controller: modelController)),
            const SizedBox(width: 6),
            Expanded(child: _EditorTextField(label: 'Reasoning effort', controller: reasoningController)),
          ],
        ),
        const SizedBox(height: 6),
        _EditorTextField(label: 'Capabilities (one per line)', controller: capabilitiesController, maxLines: 3),
        const SizedBox(height: 6),
        _EditorTextField(label: 'Policy decisions (action=decision)', controller: policyController, maxLines: 4),
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
        _EditorTextField(label: 'Routing reserved actions', controller: routingReservedController, maxLines: 2),
        const SizedBox(height: 6),
        _EditorTextField(label: 'Lifecycle reserved actions', controller: lifecycleReservedController, maxLines: 2),
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
        Text('Instruction editor', style: theme.textTheme.labelSmall?.copyWith(color: const Color(0xFF93A5BC))),
        const SizedBox(height: 4),
        SizedBox(
          height: 120,
          child: ClipRRect(
            borderRadius: BorderRadius.circular(8),
            child: DecoratedBox(
              decoration: const BoxDecoration(color: Color(0xFF07101A)),
              child: CodeForge(
                controller: instructionController,
                readOnly: false,
                lineWrap: true,
                enableGutter: false,
                enableFolding: false,
                textStyle: const TextStyle(fontSize: 12, color: Color(0xFFE5EDF8)),
                innerPadding: const EdgeInsets.all(8),
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
    return DecoratedBox(
      decoration: BoxDecoration(
        color: const Color(0xFF0B131E),
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: _toneColor(_isCurrent ? 'success' : 'info').withValues(alpha: 0.45)),
      ),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(10, 8, 8, 8),
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
                    '${row.versionId} · $createdAt',
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
    return DecoratedBox(
      decoration: BoxDecoration(
        color: const Color(0xFF0F1722),
        borderRadius: BorderRadius.circular(10),
        border: Border(left: BorderSide(color: _toneColor(tone), width: 3)),
      ),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(11, 10, 10, 10),
        child: Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(eyebrow.toUpperCase(), maxLines: 1, overflow: TextOverflow.ellipsis, style: theme.textTheme.labelSmall?.copyWith(color: _toneColor(tone), letterSpacing: 0.5)),
                  const SizedBox(height: 3),
                  Text(title, maxLines: 1, overflow: TextOverflow.ellipsis, style: theme.textTheme.bodyMedium?.copyWith(color: Colors.white)),
                  const SizedBox(height: 3),
                  Text(subtitle, maxLines: 2, overflow: TextOverflow.ellipsis, style: theme.textTheme.bodySmall?.copyWith(color: const Color(0xFF97A6BA))),
                ],
              ),
            ),
            const SizedBox(width: 8),
            _Chip(label: trailing, tone: tone),
          ],
        ),
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
          Expanded(child: Text(fact.label, style: theme.textTheme.bodySmall?.copyWith(color: const Color(0xFF7D8DA3)))),
          Text(fact.value, style: theme.textTheme.bodySmall?.copyWith(color: Colors.white)),
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
        borderRadius: BorderRadius.circular(999),
        border: Border.all(color: _toneColor(tone).withValues(alpha: 0.55)),
      ),
      child: Text(label, style: Theme.of(context).textTheme.labelSmall?.copyWith(color: const Color(0xFFE5EDF8))),
    );
  }
}

class _MetricBadge extends StatelessWidget {
  const _MetricBadge(this.badge);

  final AgentRuntimeStatusBadge badge;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 6),
      decoration: BoxDecoration(
        color: const Color(0xFF0E1622),
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: _toneColor(badge.tone).withValues(alpha: 0.45)),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(badge.label, style: theme.textTheme.labelSmall?.copyWith(color: const Color(0xFF93A5BC))),
          const SizedBox(width: 6),
          Text(badge.value, style: theme.textTheme.labelMedium?.copyWith(color: Colors.white, fontWeight: FontWeight.w700)),
        ],
      ),
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
      child: Container(
        padding: const EdgeInsets.all(16),
        decoration: BoxDecoration(
          color: const Color(0xFF0F1722),
          borderRadius: BorderRadius.circular(12),
          border: Border.all(color: const Color(0xFF223047)),
        ),
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
