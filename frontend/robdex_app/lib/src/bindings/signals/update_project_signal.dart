// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class UpdateProjectSignal {
  const UpdateProjectSignal({
    required this.projectId,
    required this.name,
    required this.defaultCwd,
    required this.autoRouteReplies,
    required this.routeApprovalRequests,
    required this.preferredModelProvider,
    required this.orchestratorModelId,
    required this.orchestratorReasoningEffort,
    required this.workerModelId,
    required this.workerReasoningEffort,
    required this.qaModelId,
    required this.qaReasoningEffort,
  });

  static UpdateProjectSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = UpdateProjectSignal(
      projectId: deserializer.deserializeString(),
      name: deserializer.deserializeString(),
      defaultCwd: deserializer.deserializeString(),
      autoRouteReplies: deserializer.deserializeBool(),
      routeApprovalRequests: deserializer.deserializeBool(),
      preferredModelProvider: deserializer.deserializeString(),
      orchestratorModelId: deserializer.deserializeString(),
      orchestratorReasoningEffort: deserializer.deserializeString(),
      workerModelId: deserializer.deserializeString(),
      workerReasoningEffort: deserializer.deserializeString(),
      qaModelId: deserializer.deserializeString(),
      qaReasoningEffort: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static UpdateProjectSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = UpdateProjectSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String projectId;
  final String name;
  final String defaultCwd;
  final bool autoRouteReplies;
  final bool routeApprovalRequests;
  final String preferredModelProvider;
  final String orchestratorModelId;
  final String orchestratorReasoningEffort;
  final String workerModelId;
  final String workerReasoningEffort;
  final String qaModelId;
  final String qaReasoningEffort;

  UpdateProjectSignal copyWith({
    String? projectId,
    String? name,
    String? defaultCwd,
    bool? autoRouteReplies,
    bool? routeApprovalRequests,
    String? preferredModelProvider,
    String? orchestratorModelId,
    String? orchestratorReasoningEffort,
    String? workerModelId,
    String? workerReasoningEffort,
    String? qaModelId,
    String? qaReasoningEffort,
  }) {
    return UpdateProjectSignal(
      projectId: projectId ?? this.projectId,
      name: name ?? this.name,
      defaultCwd: defaultCwd ?? this.defaultCwd,
      autoRouteReplies: autoRouteReplies ?? this.autoRouteReplies,
      routeApprovalRequests: routeApprovalRequests ?? this.routeApprovalRequests,
      preferredModelProvider: preferredModelProvider ?? this.preferredModelProvider,
      orchestratorModelId: orchestratorModelId ?? this.orchestratorModelId,
      orchestratorReasoningEffort: orchestratorReasoningEffort ?? this.orchestratorReasoningEffort,
      workerModelId: workerModelId ?? this.workerModelId,
      workerReasoningEffort: workerReasoningEffort ?? this.workerReasoningEffort,
      qaModelId: qaModelId ?? this.qaModelId,
      qaReasoningEffort: qaReasoningEffort ?? this.qaReasoningEffort,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(projectId);
    serializer.serializeString(name);
    serializer.serializeString(defaultCwd);
    serializer.serializeBool(autoRouteReplies);
    serializer.serializeBool(routeApprovalRequests);
    serializer.serializeString(preferredModelProvider);
    serializer.serializeString(orchestratorModelId);
    serializer.serializeString(orchestratorReasoningEffort);
    serializer.serializeString(workerModelId);
    serializer.serializeString(workerReasoningEffort);
    serializer.serializeString(qaModelId);
    serializer.serializeString(qaReasoningEffort);
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

    return other is UpdateProjectSignal
      && projectId == other.projectId
      && name == other.name
      && defaultCwd == other.defaultCwd
      && autoRouteReplies == other.autoRouteReplies
      && routeApprovalRequests == other.routeApprovalRequests
      && preferredModelProvider == other.preferredModelProvider
      && orchestratorModelId == other.orchestratorModelId
      && orchestratorReasoningEffort == other.orchestratorReasoningEffort
      && workerModelId == other.workerModelId
      && workerReasoningEffort == other.workerReasoningEffort
      && qaModelId == other.qaModelId
      && qaReasoningEffort == other.qaReasoningEffort;
  }

  @override
  int get hashCode => Object.hash(
        projectId,
        name,
        defaultCwd,
        autoRouteReplies,
        routeApprovalRequests,
        preferredModelProvider,
        orchestratorModelId,
        orchestratorReasoningEffort,
        workerModelId,
        workerReasoningEffort,
        qaModelId,
        qaReasoningEffort,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'projectId: $projectId, '
        'name: $name, '
        'defaultCwd: $defaultCwd, '
        'autoRouteReplies: $autoRouteReplies, '
        'routeApprovalRequests: $routeApprovalRequests, '
        'preferredModelProvider: $preferredModelProvider, '
        'orchestratorModelId: $orchestratorModelId, '
        'orchestratorReasoningEffort: $orchestratorReasoningEffort, '
        'workerModelId: $workerModelId, '
        'workerReasoningEffort: $workerReasoningEffort, '
        'qaModelId: $qaModelId, '
        'qaReasoningEffort: $qaReasoningEffort'
        ')';
      return true;
    }());

    return fullString ?? 'UpdateProjectSignal';
  }
}

extension UpdateProjectSignalDartSignalExt on UpdateProjectSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_update_project_signal',
      messageBytes,
      binary,
    );
  }
}
