class AgentRuntimeControlTowerData {
  const AgentRuntimeControlTowerData({
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
    required this.controllerFacts,
    required this.outputLog,
    required this.pendingRequestCount,
    this.errorMessage,
  });

  final String connectionState;
  final String connectionTone;
  final String baseUrl;
  final String statusLabel;
  final String watermarkLabel;
  final List<AgentRuntimeStatusBadge> statusBadges;
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
  final List<AgentRuntimeSessionItem> sessions;
  final List<AgentRuntimeTimelineItem> timeline;
  final List<AgentRuntimeActionItem> actions;
  final List<AgentRuntimeFact> controllerFacts;
  final List<String> outputLog;
  final int pendingRequestCount;
  final String? errorMessage;

  factory AgentRuntimeControlTowerData.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeControlTowerData(
      connectionState: '${json['connectionState'] ?? 'disconnected'}',
      connectionTone: '${json['connectionTone'] ?? 'muted'}',
      baseUrl: '${json['baseUrl'] ?? ''}',
      statusLabel: '${json['statusLabel'] ?? 'No projection packet'}',
      watermarkLabel: '${json['watermarkLabel'] ?? '—'}',
      statusBadges: _objects(json['statusBadges']).map(AgentRuntimeStatusBadge.fromJson).toList(growable: false),
      selectedSessionLabel: '${json['selectedSessionLabel'] ?? 'none selected'}',
      sessionsTitle: '${json['sessionsTitle'] ?? 'Sessions'}',
      sessionsSubtitle: '${json['sessionsSubtitle'] ?? ''}',
      timelineTitle: '${json['timelineTitle'] ?? 'Selected session stream'}',
      timelineSubtitle: '${json['timelineSubtitle'] ?? ''}',
      actionsTitle: '${json['actionsTitle'] ?? 'Action queue'}',
      actionsSubtitle: '${json['actionsSubtitle'] ?? ''}',
      detailTitle: '${json['detailTitle'] ?? 'Controller detail'}',
      detailSubtitle: '${json['detailSubtitle'] ?? ''}',
      sessionsEmptyTitle: '${json['sessionsEmptyTitle'] ?? 'No sessions'}',
      sessionsEmptyText: '${json['sessionsEmptyText'] ?? 'No sessions are visible.'}',
      timelineEmptyTitle: '${json['timelineEmptyTitle'] ?? 'No timeline'}',
      timelineEmptyText: '${json['timelineEmptyText'] ?? 'Select a session to inspect its timeline.'}',
      actionsEmptyTitle: '${json['actionsEmptyTitle'] ?? 'No action required'}',
      actionsEmptyText: '${json['actionsEmptyText'] ?? 'No action items need attention.'}',
      sessions: _objects(json['sessions']).map(AgentRuntimeSessionItem.fromJson).toList(growable: false),
      timeline: _objects(json['timeline']).map(AgentRuntimeTimelineItem.fromJson).toList(growable: false),
      actions: _objects(json['actions']).map(AgentRuntimeActionItem.fromJson).toList(growable: false),
      controllerFacts: _objects(json['controllerFacts']).map(AgentRuntimeFact.fromJson).toList(growable: false),
      outputLog: (json['outputLog'] as List<dynamic>? ?? const []).map((value) => '$value').toList(growable: false),
      pendingRequestCount: (json['pendingRequestCount'] as num?)?.toInt() ?? 0,
      errorMessage: json['errorMessage'] as String?,
    );
  }

  AgentRuntimeControlTowerData copyWith({
    String? connectionState,
    String? connectionTone,
    String? baseUrl,
    String? statusLabel,
    String? watermarkLabel,
    List<AgentRuntimeStatusBadge>? statusBadges,
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
    List<AgentRuntimeSessionItem>? sessions,
    List<AgentRuntimeTimelineItem>? timeline,
    List<AgentRuntimeActionItem>? actions,
    List<AgentRuntimeFact>? controllerFacts,
    List<String>? outputLog,
    int? pendingRequestCount,
    String? errorMessage,
  }) {
    return AgentRuntimeControlTowerData(
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
      controllerFacts: controllerFacts ?? this.controllerFacts,
      outputLog: outputLog ?? this.outputLog,
      pendingRequestCount: pendingRequestCount ?? this.pendingRequestCount,
      errorMessage: errorMessage ?? this.errorMessage,
    );
  }
}

class AgentRuntimeStatusBadge {
  const AgentRuntimeStatusBadge({
    required this.label,
    required this.value,
    required this.tone,
  });

  final String label;
  final String value;
  final String tone;

  factory AgentRuntimeStatusBadge.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeStatusBadge(
      label: '${json['label'] ?? ''}',
      value: '${json['value'] ?? ''}',
      tone: '${json['tone'] ?? 'info'}',
    );
  }
}

class AgentRuntimeSessionItem {
  const AgentRuntimeSessionItem({
    required this.id,
    required this.title,
    required this.status,
    required this.subtitle,
    required this.groupLabel,
    required this.tone,
  });

  final String id;
  final String title;
  final String status;
  final String subtitle;
  final String groupLabel;
  final String tone;

  factory AgentRuntimeSessionItem.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeSessionItem(
      id: '${json['id'] ?? 'session'}',
      title: '${json['title'] ?? json['id'] ?? 'Session'}',
      status: '${json['status'] ?? 'unknown'}',
      subtitle: '${json['subtitle'] ?? ''}',
      groupLabel: '${json['groupLabel'] ?? 'Sessions'}',
      tone: '${json['tone'] ?? 'info'}',
    );
  }
}

class AgentRuntimeTimelineItem {
  const AgentRuntimeTimelineItem({
    required this.id,
    required this.title,
    required this.subtitle,
    required this.status,
    required this.tone,
  });

  final String id;
  final String title;
  final String subtitle;
  final String status;
  final String tone;

  factory AgentRuntimeTimelineItem.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeTimelineItem(
      id: '${json['id'] ?? 'event'}',
      title: '${json['title'] ?? 'event'}',
      subtitle: '${json['subtitle'] ?? ''}',
      status: '${json['status'] ?? ''}',
      tone: '${json['tone'] ?? 'info'}',
    );
  }
}

class AgentRuntimeActionItem {
  const AgentRuntimeActionItem({
    required this.id,
    required this.title,
    required this.subtitle,
    required this.kind,
    required this.stateText,
    required this.tone,
  });

  final String id;
  final String title;
  final String subtitle;
  final String kind;
  final String stateText;
  final String tone;

  factory AgentRuntimeActionItem.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeActionItem(
      id: '${json['id'] ?? 'action'}',
      title: '${json['title'] ?? 'Action'}',
      subtitle: '${json['subtitle'] ?? ''}',
      kind: '${json['kind'] ?? 'action'}',
      stateText: '${json['stateText'] ?? json['kind'] ?? 'Action'}',
      tone: '${json['tone'] ?? 'info'}',
    );
  }
}

class AgentRuntimeFact {
  const AgentRuntimeFact({
    required this.label,
    required this.value,
  });

  final String label;
  final String value;

  factory AgentRuntimeFact.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeFact(
      label: '${json['label'] ?? ''}',
      value: '${json['value'] ?? ''}',
    );
  }
}

Iterable<Map<String, dynamic>> _objects(Object? value) {
  return (value as List<dynamic>? ?? const [])
      .whereType<Map>()
      .map((item) => Map<String, dynamic>.from(item));
}
