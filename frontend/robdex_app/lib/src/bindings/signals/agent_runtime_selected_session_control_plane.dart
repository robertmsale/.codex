// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeSelectedSessionControlPlane {
  const AgentRuntimeSelectedSessionControlPlane({
    required this.sessionId,
    required this.title,
    required this.name,
    required this.status,
    required this.roleId,
    required this.projectKey,
    required this.activeModel,
    required this.workdir,
    required this.worktreeRoot,
    required this.tracked,
    required this.modelOptions,
    required this.godMode,
    required this.managedProcesses,
    required this.approvals,
    required this.commandRequests,
    required this.requirementsReview,
    required this.runningServers,
    required this.imageArtifacts,
    required this.quickActions,
  });

  static AgentRuntimeSelectedSessionControlPlane deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeSelectedSessionControlPlane(
      sessionId: deserializer.deserializeString(),
      title: deserializer.deserializeString(),
      name: deserializer.deserializeString(),
      status: deserializer.deserializeString(),
      roleId: deserializer.deserializeString(),
      projectKey: deserializer.deserializeString(),
      activeModel: deserializer.deserializeString(),
      workdir: deserializer.deserializeString(),
      worktreeRoot: deserializer.deserializeString(),
      tracked: deserializer.deserializeBool(),
      modelOptions: TraitHelpers.deserializeVectorAgentRuntimeModelOption(deserializer),
      godMode: AgentRuntimeGodModeState.deserialize(deserializer),
      managedProcesses: TraitHelpers.deserializeVectorAgentRuntimeManagedProcessRow(deserializer),
      approvals: TraitHelpers.deserializeVectorAgentRuntimeApprovalCard(deserializer),
      commandRequests: TraitHelpers.deserializeVectorAgentRuntimeCommandRequestCard(deserializer),
      requirementsReview: AgentRuntimeRequirementsReviewPanel.deserialize(deserializer),
      runningServers: TraitHelpers.deserializeVectorAgentRuntimeFact(deserializer),
      imageArtifacts: TraitHelpers.deserializeVectorAgentRuntimeFact(deserializer),
      quickActions: TraitHelpers.deserializeVectorAgentRuntimeActionAvailability(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeSelectedSessionControlPlane bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeSelectedSessionControlPlane.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String sessionId;
  final String title;
  final String name;
  final String status;
  final String roleId;
  final String projectKey;
  final String activeModel;
  final String workdir;
  final String worktreeRoot;
  final bool tracked;
  final List<AgentRuntimeModelOption> modelOptions;
  final AgentRuntimeGodModeState godMode;
  final List<AgentRuntimeManagedProcessRow> managedProcesses;
  final List<AgentRuntimeApprovalCard> approvals;
  final List<AgentRuntimeCommandRequestCard> commandRequests;
  final AgentRuntimeRequirementsReviewPanel requirementsReview;
  final List<AgentRuntimeFact> runningServers;
  final List<AgentRuntimeFact> imageArtifacts;
  final List<AgentRuntimeActionAvailability> quickActions;

  AgentRuntimeSelectedSessionControlPlane copyWith({
    String? sessionId,
    String? title,
    String? name,
    String? status,
    String? roleId,
    String? projectKey,
    String? activeModel,
    String? workdir,
    String? worktreeRoot,
    bool? tracked,
    List<AgentRuntimeModelOption>? modelOptions,
    AgentRuntimeGodModeState? godMode,
    List<AgentRuntimeManagedProcessRow>? managedProcesses,
    List<AgentRuntimeApprovalCard>? approvals,
    List<AgentRuntimeCommandRequestCard>? commandRequests,
    AgentRuntimeRequirementsReviewPanel? requirementsReview,
    List<AgentRuntimeFact>? runningServers,
    List<AgentRuntimeFact>? imageArtifacts,
    List<AgentRuntimeActionAvailability>? quickActions,
  }) {
    return AgentRuntimeSelectedSessionControlPlane(
      sessionId: sessionId ?? this.sessionId,
      title: title ?? this.title,
      name: name ?? this.name,
      status: status ?? this.status,
      roleId: roleId ?? this.roleId,
      projectKey: projectKey ?? this.projectKey,
      activeModel: activeModel ?? this.activeModel,
      workdir: workdir ?? this.workdir,
      worktreeRoot: worktreeRoot ?? this.worktreeRoot,
      tracked: tracked ?? this.tracked,
      modelOptions: modelOptions ?? this.modelOptions,
      godMode: godMode ?? this.godMode,
      managedProcesses: managedProcesses ?? this.managedProcesses,
      approvals: approvals ?? this.approvals,
      commandRequests: commandRequests ?? this.commandRequests,
      requirementsReview: requirementsReview ?? this.requirementsReview,
      runningServers: runningServers ?? this.runningServers,
      imageArtifacts: imageArtifacts ?? this.imageArtifacts,
      quickActions: quickActions ?? this.quickActions,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(sessionId);
    serializer.serializeString(title);
    serializer.serializeString(name);
    serializer.serializeString(status);
    serializer.serializeString(roleId);
    serializer.serializeString(projectKey);
    serializer.serializeString(activeModel);
    serializer.serializeString(workdir);
    serializer.serializeString(worktreeRoot);
    serializer.serializeBool(tracked);
    TraitHelpers.serializeVectorAgentRuntimeModelOption(modelOptions, serializer);
    godMode.serialize(serializer);
    TraitHelpers.serializeVectorAgentRuntimeManagedProcessRow(managedProcesses, serializer);
    TraitHelpers.serializeVectorAgentRuntimeApprovalCard(approvals, serializer);
    TraitHelpers.serializeVectorAgentRuntimeCommandRequestCard(commandRequests, serializer);
    requirementsReview.serialize(serializer);
    TraitHelpers.serializeVectorAgentRuntimeFact(runningServers, serializer);
    TraitHelpers.serializeVectorAgentRuntimeFact(imageArtifacts, serializer);
    TraitHelpers.serializeVectorAgentRuntimeActionAvailability(quickActions, serializer);
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

    return other is AgentRuntimeSelectedSessionControlPlane
      && sessionId == other.sessionId
      && title == other.title
      && name == other.name
      && status == other.status
      && roleId == other.roleId
      && projectKey == other.projectKey
      && activeModel == other.activeModel
      && workdir == other.workdir
      && worktreeRoot == other.worktreeRoot
      && tracked == other.tracked
      && listEquals(modelOptions, other.modelOptions)
      && godMode == other.godMode
      && listEquals(managedProcesses, other.managedProcesses)
      && listEquals(approvals, other.approvals)
      && listEquals(commandRequests, other.commandRequests)
      && requirementsReview == other.requirementsReview
      && listEquals(runningServers, other.runningServers)
      && listEquals(imageArtifacts, other.imageArtifacts)
      && listEquals(quickActions, other.quickActions);
  }

  @override
  int get hashCode => Object.hash(
        sessionId,
        title,
        name,
        status,
        roleId,
        projectKey,
        activeModel,
        workdir,
        worktreeRoot,
        tracked,
        modelOptions,
        godMode,
        managedProcesses,
        approvals,
        commandRequests,
        requirementsReview,
        runningServers,
        imageArtifacts,
        quickActions,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'sessionId: $sessionId, '
        'title: $title, '
        'name: $name, '
        'status: $status, '
        'roleId: $roleId, '
        'projectKey: $projectKey, '
        'activeModel: $activeModel, '
        'workdir: $workdir, '
        'worktreeRoot: $worktreeRoot, '
        'tracked: $tracked, '
        'modelOptions: $modelOptions, '
        'godMode: $godMode, '
        'managedProcesses: $managedProcesses, '
        'approvals: $approvals, '
        'commandRequests: $commandRequests, '
        'requirementsReview: $requirementsReview, '
        'runningServers: $runningServers, '
        'imageArtifacts: $imageArtifacts, '
        'quickActions: $quickActions'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeSelectedSessionControlPlane';
  }
}
