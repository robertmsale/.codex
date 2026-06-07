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
    required this.defaultModelId,
    required this.defaultReasoningEffort,
    required this.defaultSandboxMode,
    required this.defaultApprovalPolicy,
    required this.defaultNetworkAccessMode,
    required this.roleRuntimeDefaultsJson,
    required this.orchestratorModelId,
    required this.orchestratorReasoningEffort,
    required this.workerModelId,
    required this.workerReasoningEffort,
    required this.qaModelId,
    required this.qaReasoningEffort,
    required this.designerModelId,
    required this.designerReasoningEffort,
    required this.plannerModelId,
    required this.plannerReasoningEffort,
    required this.requirementsReviewerModelId,
    required this.requirementsReviewerReasoningEffort,
    required this.orchestratorDeveloperInstructions,
    required this.workerDeveloperInstructions,
    required this.qaDeveloperInstructions,
    required this.designerDeveloperInstructions,
    required this.operatorDeveloperInstructions,
    required this.hiddenDeveloperInstructions,
    required this.permanentRequirementComposables,
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
      defaultModelId: deserializer.deserializeString(),
      defaultReasoningEffort: deserializer.deserializeString(),
      defaultSandboxMode: deserializer.deserializeString(),
      defaultApprovalPolicy: deserializer.deserializeString(),
      defaultNetworkAccessMode: deserializer.deserializeString(),
      roleRuntimeDefaultsJson: deserializer.deserializeString(),
      orchestratorModelId: deserializer.deserializeString(),
      orchestratorReasoningEffort: deserializer.deserializeString(),
      workerModelId: deserializer.deserializeString(),
      workerReasoningEffort: deserializer.deserializeString(),
      qaModelId: deserializer.deserializeString(),
      qaReasoningEffort: deserializer.deserializeString(),
      designerModelId: deserializer.deserializeString(),
      designerReasoningEffort: deserializer.deserializeString(),
      plannerModelId: deserializer.deserializeString(),
      plannerReasoningEffort: deserializer.deserializeString(),
      requirementsReviewerModelId: deserializer.deserializeString(),
      requirementsReviewerReasoningEffort: deserializer.deserializeString(),
      orchestratorDeveloperInstructions: deserializer.deserializeString(),
      workerDeveloperInstructions: deserializer.deserializeString(),
      qaDeveloperInstructions: deserializer.deserializeString(),
      designerDeveloperInstructions: deserializer.deserializeString(),
      operatorDeveloperInstructions: deserializer.deserializeString(),
      hiddenDeveloperInstructions: deserializer.deserializeString(),
      permanentRequirementComposables: TraitHelpers.deserializeVectorStr(deserializer),
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
  final String defaultModelId;
  final String defaultReasoningEffort;
  final String defaultSandboxMode;
  final String defaultApprovalPolicy;
  final String defaultNetworkAccessMode;
  final String roleRuntimeDefaultsJson;
  final String orchestratorModelId;
  final String orchestratorReasoningEffort;
  final String workerModelId;
  final String workerReasoningEffort;
  final String qaModelId;
  final String qaReasoningEffort;
  final String designerModelId;
  final String designerReasoningEffort;
  final String plannerModelId;
  final String plannerReasoningEffort;
  final String requirementsReviewerModelId;
  final String requirementsReviewerReasoningEffort;
  final String orchestratorDeveloperInstructions;
  final String workerDeveloperInstructions;
  final String qaDeveloperInstructions;
  final String designerDeveloperInstructions;
  final String operatorDeveloperInstructions;
  final String hiddenDeveloperInstructions;
  final List<String> permanentRequirementComposables;

  UpdateProjectSignal copyWith({
    String? projectId,
    String? name,
    String? defaultCwd,
    bool? autoRouteReplies,
    bool? routeApprovalRequests,
    String? preferredModelProvider,
    String? defaultModelId,
    String? defaultReasoningEffort,
    String? defaultSandboxMode,
    String? defaultApprovalPolicy,
    String? defaultNetworkAccessMode,
    String? roleRuntimeDefaultsJson,
    String? orchestratorModelId,
    String? orchestratorReasoningEffort,
    String? workerModelId,
    String? workerReasoningEffort,
    String? qaModelId,
    String? qaReasoningEffort,
    String? designerModelId,
    String? designerReasoningEffort,
    String? plannerModelId,
    String? plannerReasoningEffort,
    String? requirementsReviewerModelId,
    String? requirementsReviewerReasoningEffort,
    String? orchestratorDeveloperInstructions,
    String? workerDeveloperInstructions,
    String? qaDeveloperInstructions,
    String? designerDeveloperInstructions,
    String? operatorDeveloperInstructions,
    String? hiddenDeveloperInstructions,
    List<String>? permanentRequirementComposables,
  }) {
    return UpdateProjectSignal(
      projectId: projectId ?? this.projectId,
      name: name ?? this.name,
      defaultCwd: defaultCwd ?? this.defaultCwd,
      autoRouteReplies: autoRouteReplies ?? this.autoRouteReplies,
      routeApprovalRequests: routeApprovalRequests ?? this.routeApprovalRequests,
      preferredModelProvider: preferredModelProvider ?? this.preferredModelProvider,
      defaultModelId: defaultModelId ?? this.defaultModelId,
      defaultReasoningEffort: defaultReasoningEffort ?? this.defaultReasoningEffort,
      defaultSandboxMode: defaultSandboxMode ?? this.defaultSandboxMode,
      defaultApprovalPolicy: defaultApprovalPolicy ?? this.defaultApprovalPolicy,
      defaultNetworkAccessMode: defaultNetworkAccessMode ?? this.defaultNetworkAccessMode,
      roleRuntimeDefaultsJson: roleRuntimeDefaultsJson ?? this.roleRuntimeDefaultsJson,
      orchestratorModelId: orchestratorModelId ?? this.orchestratorModelId,
      orchestratorReasoningEffort: orchestratorReasoningEffort ?? this.orchestratorReasoningEffort,
      workerModelId: workerModelId ?? this.workerModelId,
      workerReasoningEffort: workerReasoningEffort ?? this.workerReasoningEffort,
      qaModelId: qaModelId ?? this.qaModelId,
      qaReasoningEffort: qaReasoningEffort ?? this.qaReasoningEffort,
      designerModelId: designerModelId ?? this.designerModelId,
      designerReasoningEffort: designerReasoningEffort ?? this.designerReasoningEffort,
      plannerModelId: plannerModelId ?? this.plannerModelId,
      plannerReasoningEffort: plannerReasoningEffort ?? this.plannerReasoningEffort,
      requirementsReviewerModelId: requirementsReviewerModelId ?? this.requirementsReviewerModelId,
      requirementsReviewerReasoningEffort: requirementsReviewerReasoningEffort ?? this.requirementsReviewerReasoningEffort,
      orchestratorDeveloperInstructions: orchestratorDeveloperInstructions ?? this.orchestratorDeveloperInstructions,
      workerDeveloperInstructions: workerDeveloperInstructions ?? this.workerDeveloperInstructions,
      qaDeveloperInstructions: qaDeveloperInstructions ?? this.qaDeveloperInstructions,
      designerDeveloperInstructions: designerDeveloperInstructions ?? this.designerDeveloperInstructions,
      operatorDeveloperInstructions: operatorDeveloperInstructions ?? this.operatorDeveloperInstructions,
      hiddenDeveloperInstructions: hiddenDeveloperInstructions ?? this.hiddenDeveloperInstructions,
      permanentRequirementComposables: permanentRequirementComposables ?? this.permanentRequirementComposables,
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
    serializer.serializeString(defaultModelId);
    serializer.serializeString(defaultReasoningEffort);
    serializer.serializeString(defaultSandboxMode);
    serializer.serializeString(defaultApprovalPolicy);
    serializer.serializeString(defaultNetworkAccessMode);
    serializer.serializeString(roleRuntimeDefaultsJson);
    serializer.serializeString(orchestratorModelId);
    serializer.serializeString(orchestratorReasoningEffort);
    serializer.serializeString(workerModelId);
    serializer.serializeString(workerReasoningEffort);
    serializer.serializeString(qaModelId);
    serializer.serializeString(qaReasoningEffort);
    serializer.serializeString(designerModelId);
    serializer.serializeString(designerReasoningEffort);
    serializer.serializeString(plannerModelId);
    serializer.serializeString(plannerReasoningEffort);
    serializer.serializeString(requirementsReviewerModelId);
    serializer.serializeString(requirementsReviewerReasoningEffort);
    serializer.serializeString(orchestratorDeveloperInstructions);
    serializer.serializeString(workerDeveloperInstructions);
    serializer.serializeString(qaDeveloperInstructions);
    serializer.serializeString(designerDeveloperInstructions);
    serializer.serializeString(operatorDeveloperInstructions);
    serializer.serializeString(hiddenDeveloperInstructions);
    TraitHelpers.serializeVectorStr(permanentRequirementComposables, serializer);
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
      && defaultModelId == other.defaultModelId
      && defaultReasoningEffort == other.defaultReasoningEffort
      && defaultSandboxMode == other.defaultSandboxMode
      && defaultApprovalPolicy == other.defaultApprovalPolicy
      && defaultNetworkAccessMode == other.defaultNetworkAccessMode
      && roleRuntimeDefaultsJson == other.roleRuntimeDefaultsJson
      && orchestratorModelId == other.orchestratorModelId
      && orchestratorReasoningEffort == other.orchestratorReasoningEffort
      && workerModelId == other.workerModelId
      && workerReasoningEffort == other.workerReasoningEffort
      && qaModelId == other.qaModelId
      && qaReasoningEffort == other.qaReasoningEffort
      && designerModelId == other.designerModelId
      && designerReasoningEffort == other.designerReasoningEffort
      && plannerModelId == other.plannerModelId
      && plannerReasoningEffort == other.plannerReasoningEffort
      && requirementsReviewerModelId == other.requirementsReviewerModelId
      && requirementsReviewerReasoningEffort == other.requirementsReviewerReasoningEffort
      && orchestratorDeveloperInstructions == other.orchestratorDeveloperInstructions
      && workerDeveloperInstructions == other.workerDeveloperInstructions
      && qaDeveloperInstructions == other.qaDeveloperInstructions
      && designerDeveloperInstructions == other.designerDeveloperInstructions
      && operatorDeveloperInstructions == other.operatorDeveloperInstructions
      && hiddenDeveloperInstructions == other.hiddenDeveloperInstructions
      && listEquals(permanentRequirementComposables, other.permanentRequirementComposables);
  }

  @override
  int get hashCode => Object.hashAll([
        projectId,
        name,
        defaultCwd,
        autoRouteReplies,
        routeApprovalRequests,
        preferredModelProvider,
        defaultModelId,
        defaultReasoningEffort,
        defaultSandboxMode,
        defaultApprovalPolicy,
        defaultNetworkAccessMode,
        roleRuntimeDefaultsJson,
        orchestratorModelId,
        orchestratorReasoningEffort,
        workerModelId,
        workerReasoningEffort,
        qaModelId,
        qaReasoningEffort,
        designerModelId,
        designerReasoningEffort,
        plannerModelId,
        plannerReasoningEffort,
        requirementsReviewerModelId,
        requirementsReviewerReasoningEffort,
        orchestratorDeveloperInstructions,
        workerDeveloperInstructions,
        qaDeveloperInstructions,
        designerDeveloperInstructions,
        operatorDeveloperInstructions,
        hiddenDeveloperInstructions,
        permanentRequirementComposables,
      ]);

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
        'defaultModelId: $defaultModelId, '
        'defaultReasoningEffort: $defaultReasoningEffort, '
        'defaultSandboxMode: $defaultSandboxMode, '
        'defaultApprovalPolicy: $defaultApprovalPolicy, '
        'defaultNetworkAccessMode: $defaultNetworkAccessMode, '
        'roleRuntimeDefaultsJson: $roleRuntimeDefaultsJson, '
        'orchestratorModelId: $orchestratorModelId, '
        'orchestratorReasoningEffort: $orchestratorReasoningEffort, '
        'workerModelId: $workerModelId, '
        'workerReasoningEffort: $workerReasoningEffort, '
        'qaModelId: $qaModelId, '
        'qaReasoningEffort: $qaReasoningEffort, '
        'designerModelId: $designerModelId, '
        'designerReasoningEffort: $designerReasoningEffort, '
        'plannerModelId: $plannerModelId, '
        'plannerReasoningEffort: $plannerReasoningEffort, '
        'requirementsReviewerModelId: $requirementsReviewerModelId, '
        'requirementsReviewerReasoningEffort: $requirementsReviewerReasoningEffort, '
        'orchestratorDeveloperInstructions: $orchestratorDeveloperInstructions, '
        'workerDeveloperInstructions: $workerDeveloperInstructions, '
        'qaDeveloperInstructions: $qaDeveloperInstructions, '
        'designerDeveloperInstructions: $designerDeveloperInstructions, '
        'operatorDeveloperInstructions: $operatorDeveloperInstructions, '
        'hiddenDeveloperInstructions: $hiddenDeveloperInstructions, '
        'permanentRequirementComposables: $permanentRequirementComposables'
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
