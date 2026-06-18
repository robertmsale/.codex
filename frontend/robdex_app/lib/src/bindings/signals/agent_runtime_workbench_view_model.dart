// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeWorkbenchViewModel {
  const AgentRuntimeWorkbenchViewModel({
    required this.discovery,
    required this.remoteDiscovery,
    required this.importedRemoteDiscovery,
    required this.connectionState,
    required this.connectionTone,
    required this.baseUrl,
    required this.statusLabel,
    required this.watermarkLabel,
    required this.statusBadges,
    required this.selectedSessionLabel,
    required this.sessionsTitle,
    required this.sessionsSubtitle,
    required this.timelineTitle,
    required this.timelineSubtitle,
    required this.actionsTitle,
    required this.actionsSubtitle,
    required this.detailTitle,
    required this.detailSubtitle,
    required this.sessionsEmptyTitle,
    required this.sessionsEmptyText,
    required this.timelineEmptyTitle,
    required this.timelineEmptyText,
    required this.actionsEmptyTitle,
    required this.actionsEmptyText,
    required this.sessions,
    required this.timeline,
    required this.actions,
    required this.roleAdmin,
    required this.workflowMemory,
    required this.controllerFacts,
    required this.outputLog,
    required this.pendingRequestCount,
    required this.errorMessage,
    required this.hasErrorMessage,
    required this.shell,
  });

  static AgentRuntimeWorkbenchViewModel deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeWorkbenchViewModel(
      discovery: AgentRuntimeDiscoveryView.deserialize(deserializer),
      remoteDiscovery: AgentRuntimeDiscoveryView.deserialize(deserializer),
      importedRemoteDiscovery: AgentRuntimeDiscoveryView.deserialize(deserializer),
      connectionState: deserializer.deserializeString(),
      connectionTone: deserializer.deserializeString(),
      baseUrl: deserializer.deserializeString(),
      statusLabel: deserializer.deserializeString(),
      watermarkLabel: deserializer.deserializeString(),
      statusBadges: TraitHelpers.deserializeVectorAgentRuntimeBadge(deserializer),
      selectedSessionLabel: deserializer.deserializeString(),
      sessionsTitle: deserializer.deserializeString(),
      sessionsSubtitle: deserializer.deserializeString(),
      timelineTitle: deserializer.deserializeString(),
      timelineSubtitle: deserializer.deserializeString(),
      actionsTitle: deserializer.deserializeString(),
      actionsSubtitle: deserializer.deserializeString(),
      detailTitle: deserializer.deserializeString(),
      detailSubtitle: deserializer.deserializeString(),
      sessionsEmptyTitle: deserializer.deserializeString(),
      sessionsEmptyText: deserializer.deserializeString(),
      timelineEmptyTitle: deserializer.deserializeString(),
      timelineEmptyText: deserializer.deserializeString(),
      actionsEmptyTitle: deserializer.deserializeString(),
      actionsEmptyText: deserializer.deserializeString(),
      sessions: TraitHelpers.deserializeVectorAgentRuntimeSessionRow(deserializer),
      timeline: TraitHelpers.deserializeVectorAgentRuntimeTimelineRow(deserializer),
      actions: TraitHelpers.deserializeVectorAgentRuntimeActionRow(deserializer),
      roleAdmin: AgentRuntimeRoleAdminView.deserialize(deserializer),
      workflowMemory: AgentRuntimeWorkflowMemoryView.deserialize(deserializer),
      controllerFacts: TraitHelpers.deserializeVectorAgentRuntimeFact(deserializer),
      outputLog: TraitHelpers.deserializeVectorStr(deserializer),
      pendingRequestCount: deserializer.deserializeInt64(),
      errorMessage: deserializer.deserializeString(),
      hasErrorMessage: deserializer.deserializeBool(),
      shell: AgentRuntimeConversationShellViewModel.deserialize(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeWorkbenchViewModel bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeWorkbenchViewModel.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final AgentRuntimeDiscoveryView discovery;
  final AgentRuntimeDiscoveryView remoteDiscovery;
  final AgentRuntimeDiscoveryView importedRemoteDiscovery;
  final String connectionState;
  final String connectionTone;
  final String baseUrl;
  final String statusLabel;
  final String watermarkLabel;
  final List<AgentRuntimeBadge> statusBadges;
  final String selectedSessionLabel;
  final String sessionsTitle;
  final String sessionsSubtitle;
  final String timelineTitle;
  final String timelineSubtitle;
  final String actionsTitle;
  final String actionsSubtitle;
  final String detailTitle;
  final String detailSubtitle;
  final String sessionsEmptyTitle;
  final String sessionsEmptyText;
  final String timelineEmptyTitle;
  final String timelineEmptyText;
  final String actionsEmptyTitle;
  final String actionsEmptyText;
  final List<AgentRuntimeSessionRow> sessions;
  final List<AgentRuntimeTimelineRow> timeline;
  final List<AgentRuntimeActionRow> actions;
  final AgentRuntimeRoleAdminView roleAdmin;
  final AgentRuntimeWorkflowMemoryView workflowMemory;
  final List<AgentRuntimeFact> controllerFacts;
  final List<String> outputLog;
  final int pendingRequestCount;
  final String errorMessage;
  final bool hasErrorMessage;
  final AgentRuntimeConversationShellViewModel shell;

  AgentRuntimeWorkbenchViewModel copyWith({
    AgentRuntimeDiscoveryView? discovery,
    AgentRuntimeDiscoveryView? remoteDiscovery,
    AgentRuntimeDiscoveryView? importedRemoteDiscovery,
    String? connectionState,
    String? connectionTone,
    String? baseUrl,
    String? statusLabel,
    String? watermarkLabel,
    List<AgentRuntimeBadge>? statusBadges,
    String? selectedSessionLabel,
    String? sessionsTitle,
    String? sessionsSubtitle,
    String? timelineTitle,
    String? timelineSubtitle,
    String? actionsTitle,
    String? actionsSubtitle,
    String? detailTitle,
    String? detailSubtitle,
    String? sessionsEmptyTitle,
    String? sessionsEmptyText,
    String? timelineEmptyTitle,
    String? timelineEmptyText,
    String? actionsEmptyTitle,
    String? actionsEmptyText,
    List<AgentRuntimeSessionRow>? sessions,
    List<AgentRuntimeTimelineRow>? timeline,
    List<AgentRuntimeActionRow>? actions,
    AgentRuntimeRoleAdminView? roleAdmin,
    AgentRuntimeWorkflowMemoryView? workflowMemory,
    List<AgentRuntimeFact>? controllerFacts,
    List<String>? outputLog,
    int? pendingRequestCount,
    String? errorMessage,
    bool? hasErrorMessage,
    AgentRuntimeConversationShellViewModel? shell,
  }) {
    return AgentRuntimeWorkbenchViewModel(
      discovery: discovery ?? this.discovery,
      remoteDiscovery: remoteDiscovery ?? this.remoteDiscovery,
      importedRemoteDiscovery: importedRemoteDiscovery ?? this.importedRemoteDiscovery,
      connectionState: connectionState ?? this.connectionState,
      connectionTone: connectionTone ?? this.connectionTone,
      baseUrl: baseUrl ?? this.baseUrl,
      statusLabel: statusLabel ?? this.statusLabel,
      watermarkLabel: watermarkLabel ?? this.watermarkLabel,
      statusBadges: statusBadges ?? this.statusBadges,
      selectedSessionLabel: selectedSessionLabel ?? this.selectedSessionLabel,
      sessionsTitle: sessionsTitle ?? this.sessionsTitle,
      sessionsSubtitle: sessionsSubtitle ?? this.sessionsSubtitle,
      timelineTitle: timelineTitle ?? this.timelineTitle,
      timelineSubtitle: timelineSubtitle ?? this.timelineSubtitle,
      actionsTitle: actionsTitle ?? this.actionsTitle,
      actionsSubtitle: actionsSubtitle ?? this.actionsSubtitle,
      detailTitle: detailTitle ?? this.detailTitle,
      detailSubtitle: detailSubtitle ?? this.detailSubtitle,
      sessionsEmptyTitle: sessionsEmptyTitle ?? this.sessionsEmptyTitle,
      sessionsEmptyText: sessionsEmptyText ?? this.sessionsEmptyText,
      timelineEmptyTitle: timelineEmptyTitle ?? this.timelineEmptyTitle,
      timelineEmptyText: timelineEmptyText ?? this.timelineEmptyText,
      actionsEmptyTitle: actionsEmptyTitle ?? this.actionsEmptyTitle,
      actionsEmptyText: actionsEmptyText ?? this.actionsEmptyText,
      sessions: sessions ?? this.sessions,
      timeline: timeline ?? this.timeline,
      actions: actions ?? this.actions,
      roleAdmin: roleAdmin ?? this.roleAdmin,
      workflowMemory: workflowMemory ?? this.workflowMemory,
      controllerFacts: controllerFacts ?? this.controllerFacts,
      outputLog: outputLog ?? this.outputLog,
      pendingRequestCount: pendingRequestCount ?? this.pendingRequestCount,
      errorMessage: errorMessage ?? this.errorMessage,
      hasErrorMessage: hasErrorMessage ?? this.hasErrorMessage,
      shell: shell ?? this.shell,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    discovery.serialize(serializer);
    remoteDiscovery.serialize(serializer);
    importedRemoteDiscovery.serialize(serializer);
    serializer.serializeString(connectionState);
    serializer.serializeString(connectionTone);
    serializer.serializeString(baseUrl);
    serializer.serializeString(statusLabel);
    serializer.serializeString(watermarkLabel);
    TraitHelpers.serializeVectorAgentRuntimeBadge(statusBadges, serializer);
    serializer.serializeString(selectedSessionLabel);
    serializer.serializeString(sessionsTitle);
    serializer.serializeString(sessionsSubtitle);
    serializer.serializeString(timelineTitle);
    serializer.serializeString(timelineSubtitle);
    serializer.serializeString(actionsTitle);
    serializer.serializeString(actionsSubtitle);
    serializer.serializeString(detailTitle);
    serializer.serializeString(detailSubtitle);
    serializer.serializeString(sessionsEmptyTitle);
    serializer.serializeString(sessionsEmptyText);
    serializer.serializeString(timelineEmptyTitle);
    serializer.serializeString(timelineEmptyText);
    serializer.serializeString(actionsEmptyTitle);
    serializer.serializeString(actionsEmptyText);
    TraitHelpers.serializeVectorAgentRuntimeSessionRow(sessions, serializer);
    TraitHelpers.serializeVectorAgentRuntimeTimelineRow(timeline, serializer);
    TraitHelpers.serializeVectorAgentRuntimeActionRow(actions, serializer);
    roleAdmin.serialize(serializer);
    workflowMemory.serialize(serializer);
    TraitHelpers.serializeVectorAgentRuntimeFact(controllerFacts, serializer);
    TraitHelpers.serializeVectorStr(outputLog, serializer);
    serializer.serializeInt64(pendingRequestCount);
    serializer.serializeString(errorMessage);
    serializer.serializeBool(hasErrorMessage);
    shell.serialize(serializer);
    serializer.decreaseContainerDepth();
  }

  Uint8List bincodeSerialize() {
      final serializer = BincodeSerializer();
      serialize(serializer);
      return serializer.bytes;
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeWorkbenchViewModel
      && discovery == other.discovery
      && remoteDiscovery == other.remoteDiscovery
      && importedRemoteDiscovery == other.importedRemoteDiscovery
      && connectionState == other.connectionState
      && connectionTone == other.connectionTone
      && baseUrl == other.baseUrl
      && statusLabel == other.statusLabel
      && watermarkLabel == other.watermarkLabel
      && listEquals(statusBadges, other.statusBadges)
      && selectedSessionLabel == other.selectedSessionLabel
      && sessionsTitle == other.sessionsTitle
      && sessionsSubtitle == other.sessionsSubtitle
      && timelineTitle == other.timelineTitle
      && timelineSubtitle == other.timelineSubtitle
      && actionsTitle == other.actionsTitle
      && actionsSubtitle == other.actionsSubtitle
      && detailTitle == other.detailTitle
      && detailSubtitle == other.detailSubtitle
      && sessionsEmptyTitle == other.sessionsEmptyTitle
      && sessionsEmptyText == other.sessionsEmptyText
      && timelineEmptyTitle == other.timelineEmptyTitle
      && timelineEmptyText == other.timelineEmptyText
      && actionsEmptyTitle == other.actionsEmptyTitle
      && actionsEmptyText == other.actionsEmptyText
      && listEquals(sessions, other.sessions)
      && listEquals(timeline, other.timeline)
      && listEquals(actions, other.actions)
      && roleAdmin == other.roleAdmin
      && workflowMemory == other.workflowMemory
      && listEquals(controllerFacts, other.controllerFacts)
      && listEquals(outputLog, other.outputLog)
      && pendingRequestCount == other.pendingRequestCount
      && errorMessage == other.errorMessage
      && hasErrorMessage == other.hasErrorMessage
      && shell == other.shell;
  }

  @override
  int get hashCode => Object.hashAll([
        discovery,
        remoteDiscovery,
        importedRemoteDiscovery,
        connectionState,
        connectionTone,
        baseUrl,
        statusLabel,
        watermarkLabel,
        statusBadges,
        selectedSessionLabel,
        sessionsTitle,
        sessionsSubtitle,
        timelineTitle,
        timelineSubtitle,
        actionsTitle,
        actionsSubtitle,
        detailTitle,
        detailSubtitle,
        sessionsEmptyTitle,
        sessionsEmptyText,
        timelineEmptyTitle,
        timelineEmptyText,
        actionsEmptyTitle,
        actionsEmptyText,
        sessions,
        timeline,
        actions,
        roleAdmin,
        workflowMemory,
        controllerFacts,
        outputLog,
        pendingRequestCount,
        errorMessage,
        hasErrorMessage,
        shell,
      ]);

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'discovery: $discovery, '
        'remoteDiscovery: $remoteDiscovery, '
        'importedRemoteDiscovery: $importedRemoteDiscovery, '
        'connectionState: $connectionState, '
        'connectionTone: $connectionTone, '
        'baseUrl: $baseUrl, '
        'statusLabel: $statusLabel, '
        'watermarkLabel: $watermarkLabel, '
        'statusBadges: $statusBadges, '
        'selectedSessionLabel: $selectedSessionLabel, '
        'sessionsTitle: $sessionsTitle, '
        'sessionsSubtitle: $sessionsSubtitle, '
        'timelineTitle: $timelineTitle, '
        'timelineSubtitle: $timelineSubtitle, '
        'actionsTitle: $actionsTitle, '
        'actionsSubtitle: $actionsSubtitle, '
        'detailTitle: $detailTitle, '
        'detailSubtitle: $detailSubtitle, '
        'sessionsEmptyTitle: $sessionsEmptyTitle, '
        'sessionsEmptyText: $sessionsEmptyText, '
        'timelineEmptyTitle: $timelineEmptyTitle, '
        'timelineEmptyText: $timelineEmptyText, '
        'actionsEmptyTitle: $actionsEmptyTitle, '
        'actionsEmptyText: $actionsEmptyText, '
        'sessions: $sessions, '
        'timeline: $timeline, '
        'actions: $actions, '
        'roleAdmin: $roleAdmin, '
        'workflowMemory: $workflowMemory, '
        'controllerFacts: $controllerFacts, '
        'outputLog: $outputLog, '
        'pendingRequestCount: $pendingRequestCount, '
        'errorMessage: $errorMessage, '
        'hasErrorMessage: $hasErrorMessage, '
        'shell: $shell'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeWorkbenchViewModel';
  }
}
