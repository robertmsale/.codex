class AgentRuntimeControlTowerData {
  const AgentRuntimeControlTowerData({
    required this.connectionState,
    required this.baseUrl,
    required this.statusLabel,
    required this.watermarkLabel,
    required this.sessions,
    required this.timeline,
    required this.actions,
    required this.controllerFacts,
    required this.outputLog,
    required this.pendingRequestCount,
    this.errorMessage,
  });

  final String connectionState;
  final String baseUrl;
  final String statusLabel;
  final String watermarkLabel;
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
      baseUrl: '${json['baseUrl'] ?? ''}',
      statusLabel: '${json['statusLabel'] ?? 'No projection packet'}',
      watermarkLabel: '${json['watermarkLabel'] ?? '—'}',
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
    String? baseUrl,
    String? statusLabel,
    String? watermarkLabel,
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
      baseUrl: baseUrl ?? this.baseUrl,
      statusLabel: statusLabel ?? this.statusLabel,
      watermarkLabel: watermarkLabel ?? this.watermarkLabel,
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

class AgentRuntimeSessionItem {
  const AgentRuntimeSessionItem({
    required this.id,
    required this.title,
    required this.status,
    required this.subtitle,
  });

  final String id;
  final String title;
  final String status;
  final String subtitle;

  factory AgentRuntimeSessionItem.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeSessionItem(
      id: '${json['id'] ?? 'session'}',
      title: '${json['title'] ?? json['id'] ?? 'Session'}',
      status: '${json['status'] ?? 'unknown'}',
      subtitle: '${json['subtitle'] ?? ''}',
    );
  }
}

class AgentRuntimeTimelineItem {
  const AgentRuntimeTimelineItem({
    required this.id,
    required this.title,
    required this.subtitle,
    required this.status,
  });

  final String id;
  final String title;
  final String subtitle;
  final String status;

  factory AgentRuntimeTimelineItem.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeTimelineItem(
      id: '${json['id'] ?? 'event'}',
      title: '${json['title'] ?? 'event'}',
      subtitle: '${json['subtitle'] ?? ''}',
      status: '${json['status'] ?? ''}',
    );
  }
}

class AgentRuntimeActionItem {
  const AgentRuntimeActionItem({
    required this.id,
    required this.title,
    required this.subtitle,
    required this.kind,
  });

  final String id;
  final String title;
  final String subtitle;
  final String kind;

  factory AgentRuntimeActionItem.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeActionItem(
      id: '${json['id'] ?? 'action'}',
      title: '${json['title'] ?? 'Action'}',
      subtitle: '${json['subtitle'] ?? ''}',
      kind: '${json['kind'] ?? 'action'}',
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
