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
}

class AgentRuntimeFact {
  const AgentRuntimeFact({
    required this.label,
    required this.value,
  });

  final String label;
  final String value;
}
