import 'dart:convert';

import 'package:http/http.dart' as http;
import 'package:web_socket_channel/web_socket_channel.dart';

import '../models/workbench_models.dart';
import '../models/workbench_view_data.dart';
import 'workbench_gateway.dart';

class BridgeWorkbenchGateway implements WorkbenchGateway {
  BridgeWorkbenchGateway({
    http.Client? client,
    Uri? baseUri,
  })  : _client = client ?? http.Client(),
        _baseUri = baseUri ?? Uri.parse('http://127.0.0.1:42080');

  final http.Client _client;
  final Uri _baseUri;
  WebSocketChannel? _channel;

  @override
  Future<WorkbenchViewData> loadInitialView() async {
    final snapshot = await _fetchSnapshot();
    final threadId = _preferredThreadId(snapshot);
    return _buildWorkbench(snapshot, selectedThreadId: threadId);
  }

  @override
  Future<WorkbenchViewData> selectThread(String threadId, WorkbenchViewData current) async {
    final snapshot = await _fetchSnapshot();
    return _buildWorkbench(snapshot, selectedThreadId: threadId);
  }

  @override
  Future<WorkbenchViewData> createProject({
    required String name,
    required String rootPath,
    required String defaultCwd,
  }) async {
    final response = await _client.post(
      _baseUri.resolve('/projects'),
      headers: {'Content-Type': 'application/json'},
      body: jsonEncode({
        'name': name,
        'rootPath': rootPath,
        'defaultCWD': defaultCwd,
      }),
    );
    if (response.statusCode != 200) {
      throw StateError('Create project failed with ${response.statusCode}');
    }
    final snapshot = await _fetchSnapshot();
    final selectedThreadId = _preferredThreadId(snapshot);
    return _buildWorkbench(snapshot, selectedThreadId: selectedThreadId);
  }

  @override
  Future<WorkbenchViewData> createThread({
    required String title,
    String role = 'worker',
  }) async {
    final selectedProjectId = _selectedProjectId(await _fetchSnapshot());
    final response = await _client.post(
      _baseUri.resolve('/threads'),
      headers: {'Content-Type': 'application/json'},
      body: jsonEncode({
        'projectId': selectedProjectId,
        'title': title,
        'role': role,
      }),
    );
    if (response.statusCode != 200) {
      throw StateError('Create thread failed with ${response.statusCode}');
    }
    final payload = jsonDecode(response.body) as Map<String, dynamic>;
    final threadId = payload['threadId'] as String?;
    final snapshot = await _fetchSnapshot();
    return _buildWorkbench(snapshot, selectedThreadId: threadId ?? _preferredThreadId(snapshot));
  }

  @override
  Future<void> sendMessage({
    required String threadId,
    required String text,
  }) async {
    final response = await _client.post(
      _baseUri.resolve('/threads/$threadId/messages'),
      headers: {'Content-Type': 'application/json'},
      body: jsonEncode({'text': text}),
    );
    if (response.statusCode != 200) {
      throw StateError('Send message failed with ${response.statusCode}');
    }
  }

  @override
  Future<void> decideApproval({
    required String senderThreadId,
    required String approvalId,
    required String decision,
    String? message,
  }) async {
    final response = await _client.post(
      _baseUri.resolve('/orchestrator/approval-decision'),
      headers: {'Content-Type': 'application/json'},
      body: jsonEncode({
        'senderThreadId': senderThreadId,
        'approvalId': approvalId,
        'decision': decision,
        if (message != null && message.trim().isNotEmpty) 'message': message.trim(),
      }),
    );
    if (response.statusCode != 200) {
      throw StateError('Approval decision failed with ${response.statusCode}');
    }
  }

  @override
  Future<WorkbenchViewData> selectProject(String? projectId) async {
    final response = await _client.post(
      _baseUri.resolve('/projects/select'),
      headers: {'Content-Type': 'application/json'},
      body: jsonEncode({'projectId': projectId}),
    );
    if (response.statusCode != 200) {
      throw StateError('Select project failed with ${response.statusCode}');
    }
    final snapshot = await _fetchSnapshot();
    final selectedThreadId = _preferredThreadId(snapshot);
    return _buildWorkbench(snapshot, selectedThreadId: selectedThreadId);
  }

  @override
  Future<WorkbenchViewData> deleteProject(String projectId) async {
    final response = await _client.delete(_baseUri.resolve('/projects/$projectId'));
    if (response.statusCode != 200) {
      throw StateError('Delete project failed with ${response.statusCode}');
    }
    final snapshot = await _fetchSnapshot();
    final selectedThreadId = _preferredThreadId(snapshot);
    return _buildWorkbench(snapshot, selectedThreadId: selectedThreadId);
  }

  @override
  Stream<WorkbenchViewData> watch({
    required WorkbenchViewData current,
    required String? selectedThreadId,
  }) async* {
    final wsUri = _baseUri.replace(
      scheme: _baseUri.scheme == 'https' ? 'wss' : 'ws',
      path: '/ws',
    );
    _channel?.sink.close();
    final channel = WebSocketChannel.connect(wsUri);
    _channel = channel;
    channel.sink.add(jsonEncode({'type': 'hello', 'payload': {}}));
    if (selectedThreadId != null && selectedThreadId.isNotEmpty) {
      channel.sink.add(
        jsonEncode({
          'type': 'command',
          'payload': {
            'id': 'thread-select-$selectedThreadId',
            'command': {
              'name': 'threadSelectionSet',
              'payload': {'threadId': selectedThreadId},
            },
          },
        }),
      );
    }

    var latest = current;
    var currentSelectedThreadId = selectedThreadId;
    await for (final raw in channel.stream) {
      if (raw is! String) {
        continue;
      }
      final decoded = jsonDecode(raw);
      if (decoded is! Map<String, dynamic>) {
        continue;
      }
      final type = decoded['type'];
      if (type != 'event') {
        continue;
      }
      final payload = decoded['payload'];
      if (payload is! Map<String, dynamic>) {
        continue;
      }
      final event = payload['event'];
      if (event is! Map<String, dynamic>) {
        continue;
      }
      final eventName = event['name'] as String?;
      final eventData = event['data'];
      switch (eventName) {
        case 'appStateSnapshot':
          if (eventData is Map<String, dynamic>) {
            latest = await _buildWorkbench(
              eventData,
              selectedThreadId: currentSelectedThreadId ?? _preferredThreadId(eventData),
              preservedMessages: latest.chatEntries,
            );
            yield latest;
          }
          break;
        case 'threadMessagesChanged':
          if (eventData is Map<String, dynamic>) {
            final threadId = eventData['threadID'] as String? ?? eventData['threadId'] as String?;
            if (threadId == null || threadId != currentSelectedThreadId) {
              continue;
            }
            currentSelectedThreadId = threadId;
            final messages = _chatEntriesFromThreadPayload(eventData);
            latest = latest.copyWith(
              chatEntries: messages,
              composerHint: latest.selection.threadName == 'No Thread Selected'
                  ? latest.composerHint
                  : 'Live bridge session attached to ${latest.selection.threadName}.',
            );
            yield latest;
          }
          break;
        default:
          break;
      }
    }
  }

  @override
  void dispose() {
    _channel?.sink.close();
    _channel = null;
    _client.close();
  }

  Future<Map<String, dynamic>> _fetchSnapshot() async {
    final response = await _client.get(_baseUri.resolve('/state/snapshot'));
    if (response.statusCode != 200) {
      throw StateError('Bridge snapshot failed with ${response.statusCode}');
    }
    return jsonDecode(response.body) as Map<String, dynamic>;
  }

  String? _preferredThreadId(Map<String, dynamic> snapshot) {
    final threads = _extractThreadRecords(snapshot);
    if (threads.isEmpty) {
      return null;
    }
    final runningIds = _runningThreadIds(snapshot);
    for (final thread in threads) {
      if (runningIds.contains(thread.id)) {
        return thread.id;
      }
    }
    return threads.first.id;
  }

  Future<WorkbenchViewData> _buildWorkbench(
    Map<String, dynamic> snapshot, {
    required String? selectedThreadId,
    List<ChatEntry>? preservedMessages,
  }) async {
    final qaHarnessSummary = await _fetchQaHarnessSummary();
    final connectionStatus = (snapshot['connectionStatus'] as String?) ?? 'unknown';
    final selectedProjectId = _selectedProjectId(snapshot);
    final runningIds = _runningThreadIds(snapshot);
    final projectRecords = _extractProjectRecords(snapshot);
    final threadRecords = _extractThreadRecords(snapshot);
    final projects = projectRecords
        .map((record) => ProjectItem(
              id: record.id,
              name: record.name,
              rootPath: record.rootPath,
              defaultCwd: record.defaultCwd,
              autoRouteReplies: false,
              routeApprovalRequests: true,
              preferredModelProvider: null,
              orchestratorDefaultModel: null,
              orchestratorDefaultReasoningEffort: null,
              workerDefaultModel: null,
              workerDefaultReasoningEffort: null,
              qaDefaultModel: null,
              qaDefaultReasoningEffort: null,
              orchestratorDeveloperInstructions: null,
              workerDeveloperInstructions: null,
              qaDeveloperInstructions: null,
              operatorDeveloperInstructions: null,
              hiddenDeveloperInstructions: null,
              isSelected: record.id == selectedProjectId,
            ))
        .toList();
    final threads = threadRecords
        .map((record) => ThreadItem(
              id: record.id,
              title: record.displayName,
              role: record.role,
              projectName: record.projectName,
              preview: record.preview,
              isRunning: runningIds.contains(record.id),
              unreadCount: 0,
            ))
        .toList();

    final selected = threadRecords.cast<_ThreadRecord?>().firstWhere(
          (record) => record?.id == selectedThreadId,
          orElse: () => threadRecords.isNotEmpty ? threadRecords.first : null,
        );

    final messages = selected == null
        ? <ChatEntry>[]
        : (preservedMessages != null &&
                selectedThreadId != null &&
                selected.id == selectedThreadId)
            ? preservedMessages
            : await _fetchThreadMessages(selected.id);
    final selection = WorkspaceSelection(
      projectId: selected?.projectId ?? selectedProjectId,
      projectRootPath: selected?.projectRoot,
      projectOrchestratorThreadId: null,
      projectOrchestratorName: null,
      threadId: selected?.id,
      threadRole: selected?.role,
      projectName: selected?.projectName ?? 'No Project',
      threadName: selected?.displayName ?? 'No Thread Selected',
      connectionLabel: 'Bridge ${connectionStatus[0].toUpperCase()}${connectionStatus.substring(1)}',
    );

    final workspaceFiles = selected == null
        ? const <WorkspaceFile>[]
        : [
            WorkspaceFile(
              path: selected.cwd,
              kind: 'cwd',
              status: 'active',
            ),
            WorkspaceFile(
              path: selected.projectRoot,
              kind: 'project',
              status: 'mounted',
            ),
          ];

    final inspectorFacts = selected == null
        ? const <InspectorFact>[]
        : [
            InspectorFact(label: 'Role', value: selected.role),
            InspectorFact(label: 'Model', value: selected.model ?? 'default'),
            InspectorFact(label: 'Sandbox', value: selected.sandboxMode ?? 'default'),
            InspectorFact(
              label: 'Network',
              value: selected.networkAccess == null
                  ? 'default'
                  : (selected.networkAccess! ? 'enabled' : 'disabled'),
            ),
            InspectorFact(label: 'Project', value: selected.projectName),
            if (selected.hookBranchName != null)
              InspectorFact(label: 'Branch', value: selected.hookBranchName!),
            if (selected.hookWorktreePath != null)
              InspectorFact(
                label: 'Worktree',
                value: selected.hookWorktreePath!,
              ),
            if (selected.hookBaseUrl != null)
              InspectorFact(label: 'Base URL', value: selected.hookBaseUrl!),
            if (selected.hookTelemetryStatus != null)
              InspectorFact(
                label: 'Hook',
                value: selected.hookTelemetryDetail == null
                    ? selected.hookTelemetryStatus!
                    : '${selected.hookTelemetryStatus!}: ${selected.hookTelemetryDetail!}',
              ),
          ];

    return WorkbenchViewData(
      projects: projects,
      selection: selection,
      threads: threads,
      availableModels: const [],
      threadGroups: const [],
      chatEntries: messages,
      contextWindowRemainingPercent: null,
      workspaceFiles: workspaceFiles,
      inspectorFacts: inspectorFacts,
      pendingApprovals: _extractPendingApprovals(snapshot),
      workerMetadata: null,
      statusHeadline:
          'Bridge ${connectionStatus[0].toUpperCase()}${connectionStatus.substring(1)} · ${_qaHarnessHeadline(qaHarnessSummary)}',
      statusDetail:
          '${threads.length} visible threads across ${_projectCount(snapshot)} projects. ${_qaHarnessDetail(qaHarnessSummary)}',
      composerHint: '',
    );
  }

  Future<Map<String, dynamic>?> _fetchQaHarnessSummary() async {
    final response = await _client.get(_baseUri.resolve('/services/qa-harness/summary'));
    if (response.statusCode != 200) {
      return null;
    }
    final decoded = jsonDecode(response.body);
    return decoded is Map<String, dynamic> ? decoded : null;
  }

  String _qaHarnessHeadline(Map<String, dynamic>? summary) {
    if (summary == null) {
      return 'QA Harness Unknown';
    }
    final reachable = summary['reachable'] == true;
    final configuredProjects = summary['configured_projects'] as int? ?? 0;
    final configuredDevices = summary['configured_devices'] as int? ?? 0;
    if (!reachable) {
      return 'QA Harness Offline';
    }
    if (configuredProjects == 0 && configuredDevices == 0) {
      return 'QA Harness Empty';
    }
    return 'QA Harness ${configuredProjects}P/${configuredDevices}D';
  }

  String _qaHarnessDetail(Map<String, dynamic>? summary) {
    if (summary == null) {
      return 'QA harness summary unavailable.';
    }
    final detail = summary['detail'] as String?;
    if (detail != null && detail.isNotEmpty) {
      return detail;
    }
    return 'QA harness summary unavailable.';
  }

  String? _selectedProjectId(Map<String, dynamic> snapshot) {
    final state = snapshot['state'];
    if (state is! Map<String, dynamic>) return null;
    return state['selectedProjectID'] as String? ?? state['selectedProjectId'] as String?;
  }

  int _projectCount(Map<String, dynamic> snapshot) {
    final state = snapshot['state'];
    if (state is! Map<String, dynamic>) return 0;
    final projects = state['projects'];
    if (projects is! Map<String, dynamic>) return 0;
    return projects.length;
  }

  Set<String> _runningThreadIds(Map<String, dynamic> snapshot) {
    final threadCache = snapshot['threadCache'];
    if (threadCache is! Map<String, dynamic>) return const <String>{};
    final running = threadCache['runningThreadIDs'] ?? threadCache['runningThreadIds'];
    if (running is! List) return const <String>{};
    return running.whereType<String>().toSet();
  }

  List<_ThreadRecord> _extractThreadRecords(Map<String, dynamic> snapshot) {
    final state = snapshot['state'];
    if (state is! Map<String, dynamic>) return const <_ThreadRecord>[];
    final projects = state['projects'];
    if (projects is! Map<String, dynamic>) return const <_ThreadRecord>[];

    final records = <_ThreadRecord>[];
    for (final entry in projects.entries) {
      final project = entry.value;
      if (project is! Map<String, dynamic>) {
        continue;
      }
      final projectName = (project['name'] as String?) ?? entry.key;
      final projectRoot = (project['projectRoot'] as String?) ?? '';
      final projectId = (project['id'] as String?) ?? entry.key;
      final agents = project['agents'];
      if (agents is! Map<String, dynamic>) {
        continue;
      }
      for (final agentEntry in agents.entries) {
        final agent = agentEntry.value;
        if (agent is! Map<String, dynamic>) {
          continue;
        }
        final archived = agent['archived'] == true;
        final role = (agent['role'] as String?) ?? 'worker';
        if (archived || role == 'hidden') {
          continue;
        }
        final hookLifecycle = agent['robdexHookLifecycle'];
        final hookLifecycleMap = hookLifecycle is Map<String, dynamic>
            ? hookLifecycle
            : const <String, dynamic>{};
        final hookTelemetry = agent['robdexHookTelemetry'];
        final hookTelemetryMap = hookTelemetry is Map<String, dynamic>
            ? hookTelemetry
            : const <String, dynamic>{};
        final hookArtifacts = hookLifecycleMap['artifacts'];
        final hookArtifactMap = hookArtifacts is Map<String, dynamic>
            ? hookArtifacts
            : const <String, dynamic>{};
        records.add(
          _ThreadRecord(
            id: agentEntry.key,
            projectId: projectId,
            projectName: projectName,
            projectRoot: projectRoot,
            displayName: (agent['displayName'] as String?) ?? agentEntry.key,
            role: role,
            cwd: (agent['cwd'] as String?) ?? projectRoot,
            sandboxMode: agent['sandboxMode'] as String?,
            networkAccess: agent['networkAccess'] as bool?,
            model: agent['model'] as String?,
            hookBranchName: (hookLifecycleMap['branchName'] as String?) ??
                (hookArtifactMap['branchName'] as String?),
            hookWorktreePath:
                (hookLifecycleMap['worktreePath'] as String?) ??
                    (hookArtifactMap['worktreePath'] as String?),
            hookBaseUrl: (hookLifecycleMap['baseUrl'] as String?) ??
                (hookArtifactMap['baseUrl'] as String?),
            hookTelemetryStatus: hookTelemetryMap['status'] as String?,
            hookTelemetryDetail: hookTelemetryMap['detail'] as String?,
            preview: '${role.toUpperCase()} · ${(agent['cwd'] as String?) ?? projectRoot}',
          ),
        );
      }
    }

    records.sort((left, right) {
      final projectCompare = left.projectName.compareTo(right.projectName);
      if (projectCompare != 0) {
        return projectCompare;
      }
      return left.displayName.compareTo(right.displayName);
    });
    return records;
  }

  List<_ProjectRecord> _extractProjectRecords(Map<String, dynamic> snapshot) {
    final state = snapshot['state'];
    if (state is! Map<String, dynamic>) return const <_ProjectRecord>[];
    final projects = state['projects'];
    if (projects is! Map<String, dynamic>) return const <_ProjectRecord>[];

    final records = <_ProjectRecord>[];
    for (final entry in projects.entries) {
      final project = entry.value;
      if (project is! Map<String, dynamic>) {
        continue;
      }
      final archived = project['archived'] == true;
      if (archived) {
        continue;
      }
      records.add(
        _ProjectRecord(
          id: (project['id'] as String?) ?? entry.key,
          name: (project['name'] as String?) ?? entry.key,
          rootPath: (project['projectRoot'] as String?) ?? '',
          defaultCwd: (project['cwd'] as String?) ??
              (project['projectRoot'] as String?) ??
              '',
        ),
      );
    }
    records.sort((left, right) => left.name.compareTo(right.name));
    return records;
  }

  List<PendingApprovalItem> _extractPendingApprovals(Map<String, dynamic> snapshot) {
    final approvals = snapshot['pendingApprovals'];
    if (approvals is! List) {
      return const <PendingApprovalItem>[];
    }
    return approvals.whereType<Map<String, dynamic>>().map((approval) {
      final fileChanges = approval['fileChanges'];
      final filePaths = fileChanges is List
          ? fileChanges
              .whereType<Map<String, dynamic>>()
              .map((change) => change['path'])
              .whereType<String>()
              .toSet()
              .toList()
          : const <String>[];
      return PendingApprovalItem(
        id: (approval['id'] as String?) ?? '',
        threadId: (approval['threadID'] as String?) ?? (approval['threadId'] as String?) ?? '',
        kind: _approvalKindLabel(approval['kind']),
        title: (approval['title'] as String?) ?? 'Approval Request',
        detail: approval['detail'] as String? ?? approval['approvalReason'] as String?,
        command: approval['command'] as String?,
        commandCwd: approval['commandCWD'] as String? ?? approval['commandCwd'] as String?,
        filePaths: filePaths,
      );
    }).where((item) => item.id.isNotEmpty).toList();
  }

  String _approvalKindLabel(Object? kind) {
    if (kind is Map<String, dynamic> && kind.isNotEmpty) {
      return kind.keys.first;
    }
    return 'approval';
  }

  Future<List<ChatEntry>> _fetchThreadMessages(String threadId) async {
    final response = await _client.get(
      _baseUri.resolve('/threads/messages').replace(
        queryParameters: {'thread_id': threadId},
      ),
    );
    if (response.statusCode != 200) {
      return [
        ChatEntry(
          id: 'messages-unavailable',
          author: 'Bridge',
          displayLabel: 'Bridge',
          timestampLabel: 'now',
          body: 'Thread history unavailable (${response.statusCode}).',
          isTool: true,
        ),
      ];
    }

    final payload = jsonDecode(response.body) as Map<String, dynamic>;
    final messages = payload['messages'];
    if (messages is! List) {
      return const <ChatEntry>[];
    }

    return _chatEntriesFromThreadPayload(payload);
  }

  List<ChatEntry> _chatEntriesFromThreadPayload(Map<String, dynamic> payload) {
    final messages = payload['messages'];
    if (messages is! List) {
      return const <ChatEntry>[];
    }
    return messages
        .whereType<Map<String, dynamic>>()
        .map(_chatEntryFromMessage)
        .toList();
  }

  ChatEntry _chatEntryFromMessage(Map<String, dynamic> message) {
    final role = message['role'] as String?;
    final toolMetadata = message['toolMetadata'] as Map<String, dynamic>?;
    final kind = toolMetadata?['kind'] as String?;
    final status = toolMetadata?['status'] as String?;
    final body = (message['text'] as String?) ?? '';
    final subtitle = message['subtitle'] as String?;
    final author = _authorForRole(role);
    final planItems = _parsePlanItems(message, kind, body);

    return ChatEntry(
      id: (message['id'] as String?) ?? 'message',
      author: author,
      displayLabel: _displayLabelForMessage(
        author,
        kind,
        subtitle,
        body,
        planItems: planItems,
      ),
      timestampLabel: _formatTimestamp(message['createdAt']),
      body: body,
      subtitle: subtitle,
      kind: kind,
      status: status,
      processId: toolMetadata?['processId'] as String?,
      command: toolMetadata?['command'] as String?,
      output: toolMetadata?['output'] as String?,
      planItems: planItems,
      isTool: role == 'tool',
      isStreaming: _isStreaming(message),
    );
  }

  List<PlanChecklistItem> _parsePlanItems(
    Map<String, dynamic> message,
    String? kind,
    String body,
  ) {
    final toolMetadata = message['toolMetadata'] as Map<String, dynamic>?;
    final items = toolMetadata?['items'];
    if (kind == 'todoList' || kind == 'todo_list') {
      final parsed = _parseStructuredPlanItems(items);
      if (parsed.isNotEmpty) {
        return parsed;
      }
    }
    return _parsePlaintextPlanItems(body);
  }

  List<PlanChecklistItem> _parseStructuredPlanItems(Object? items) {
    if (items is! List) {
      return const <PlanChecklistItem>[];
    }
    return items
        .whereType<Map<String, dynamic>>()
        .map(
          (item) => PlanChecklistItem(
            text: (item['text'] as String?) ?? '',
            completed: item['completed'] as bool? ?? false,
            status: item['status'] as String?,
          ),
        )
        .where((item) => item.text.trim().isNotEmpty)
        .toList(growable: false);
  }

  List<PlanChecklistItem> _parsePlaintextPlanItems(String body) {
    final lines = body.replaceAll('\r\n', '\n').split('\n');
    final items = <PlanChecklistItem>[];
    final pattern = RegExp(r'^\[(pending|in_progress|completed)\]\s+(.*)$');
    for (final rawLine in lines) {
      final line = rawLine.trim();
      final match = pattern.firstMatch(line);
      if (match == null) {
        continue;
      }
      final status = match.group(1);
      final text = match.group(2)?.trim() ?? '';
      if (text.isEmpty) {
        continue;
      }
      items.add(
        PlanChecklistItem(
          text: text,
          completed: status == 'completed',
          status: status,
        ),
      );
    }
    return items;
  }

  String _displayLabelForMessage(
    String author,
    String? kind,
    String? subtitle,
    String body,
    {List<PlanChecklistItem> planItems = const <PlanChecklistItem>[]}
  ) {
    if (planItems.isNotEmpty ||
        kind == 'todoList' ||
        kind == 'todo_list' ||
        subtitle?.trim().toLowerCase() == 'turn plan') {
      return 'Plan Update';
    }
    if (kind == null || kind.isEmpty) {
      return author;
    }
    if (kind == 'todoList' || kind == 'todo_list') {
      return 'Plan Update';
    }
    switch (kind) {
      case 'commandExecution':
        return 'Command';
      case 'mcpToolCall':
        return subtitle?.trim().isNotEmpty == true ? subtitle!.trim() : 'MCP Tool';
      case 'fileChange':
        if (body.trim().toLowerCase() == 'turn diff updated' ||
            (subtitle?.toLowerCase().contains('git diff') ?? false)) {
          return 'Diff';
        }
        return 'File Change';
      default:
        return kind;
    }
  }

  String _authorForRole(String? role) {
    switch (role) {
      case 'assistant':
        return 'Assistant';
      case 'user':
        return 'User';
      case 'tool':
        return 'Tool';
      default:
        return role ?? 'Unknown';
    }
  }

  String _formatTimestamp(Object? value) {
    final seconds = switch (value) {
      int intValue => intValue,
      double doubleValue => doubleValue.floor(),
      String textValue => int.tryParse(textValue) ?? 0,
      _ => 0,
    };
    if (seconds <= 0) {
      return 'now';
    }
    final dateTime = DateTime.fromMillisecondsSinceEpoch(seconds * 1000);
    final hh = dateTime.hour.toString().padLeft(2, '0');
    final mm = dateTime.minute.toString().padLeft(2, '0');
    return '$hh:$mm';
  }

  bool _isStreaming(Map<String, dynamic> message) {
    final toolMetadata = message['toolMetadata'];
    if (toolMetadata is Map<String, dynamic>) {
      final kind = toolMetadata['kind'] as String?;
      final subtitle = (message['subtitle'] as String?) ?? '';
      final body = (message['text'] as String?) ?? '';
      final isTurnDiff = kind == 'fileChange' &&
          (body.toLowerCase() == 'turn diff updated' ||
              subtitle.toLowerCase().contains('git diff'));
      if (isTurnDiff) {
        return false;
      }
      return toolMetadata['status'] == 'inProgress' ||
          toolMetadata['status'] == 'in_progress';
    }
    return false;
  }
}

class _ThreadRecord {
  const _ThreadRecord({
    required this.id,
    required this.projectId,
    required this.projectName,
    required this.projectRoot,
    required this.displayName,
    required this.role,
    required this.cwd,
    required this.sandboxMode,
    required this.networkAccess,
    required this.model,
    required this.hookBranchName,
    required this.hookWorktreePath,
    required this.hookBaseUrl,
    required this.hookTelemetryStatus,
    required this.hookTelemetryDetail,
    required this.preview,
  });

  final String id;
  final String projectId;
  final String projectName;
  final String projectRoot;
  final String displayName;
  final String role;
  final String cwd;
  final String? sandboxMode;
  final bool? networkAccess;
  final String? model;
  final String? hookBranchName;
  final String? hookWorktreePath;
  final String? hookBaseUrl;
  final String? hookTelemetryStatus;
  final String? hookTelemetryDetail;
  final String preview;
}

class _ProjectRecord {
  const _ProjectRecord({
    required this.id,
    required this.name,
    required this.rootPath,
    required this.defaultCwd,
  });

  final String id;
  final String name;
  final String rootPath;
  final String defaultCwd;
}
