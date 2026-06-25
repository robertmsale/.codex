// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


abstract class AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperation();

  void serialize(BinarySerializer serializer);

  static AgentRuntimeGuiOperation deserialize(BinaryDeserializer deserializer) {
    int index = deserializer.deserializeVariantIndex();
    switch (index) {
      case 0: return AgentRuntimeGuiOperationSelectSession.load(deserializer);
      case 1: return AgentRuntimeGuiOperationSelectWorkflowMemory.load(deserializer);
      case 2: return AgentRuntimeGuiOperationCreateSession.load(deserializer);
      case 3: return AgentRuntimeGuiOperationListProjects.load(deserializer);
      case 4: return AgentRuntimeGuiOperationCreateProject.load(deserializer);
      case 5: return AgentRuntimeGuiOperationUpdateProject.load(deserializer);
      case 6: return AgentRuntimeGuiOperationArchiveProject.load(deserializer);
      case 7: return AgentRuntimeGuiOperationUnarchiveProject.load(deserializer);
      case 8: return AgentRuntimeGuiOperationUpdateRuntimeSettings.load(deserializer);
      case 9: return AgentRuntimeGuiOperationUpdateSessionSettings.load(deserializer);
      case 10: return AgentRuntimeGuiOperationSendMessage.load(deserializer);
      case 11: return AgentRuntimeGuiOperationTerminateProcess.load(deserializer);
      case 12: return AgentRuntimeGuiOperationInputProcess.load(deserializer);
      case 13: return AgentRuntimeGuiOperationFlushProcess.load(deserializer);
      case 14: return AgentRuntimeGuiOperationCompactSession.load(deserializer);
      case 15: return AgentRuntimeGuiOperationGrantGodMode.load(deserializer);
      case 16: return AgentRuntimeGuiOperationRevokeGodMode.load(deserializer);
      case 17: return AgentRuntimeGuiOperationCloseSession.load(deserializer);
      case 18: return AgentRuntimeGuiOperationArchiveSession.load(deserializer);
      case 19: return AgentRuntimeGuiOperationForkSession.load(deserializer);
      case 20: return AgentRuntimeGuiOperationDecideApproval.load(deserializer);
      case 21: return AgentRuntimeGuiOperationResumeApproval.load(deserializer);
      case 22: return AgentRuntimeGuiOperationListCommandRegistry.load(deserializer);
      case 23: return AgentRuntimeGuiOperationShowCommand.load(deserializer);
      case 24: return AgentRuntimeGuiOperationListCommandRegistryRequests.load(deserializer);
      case 25: return AgentRuntimeGuiOperationShowCommandRegistryRequest.load(deserializer);
      case 26: return AgentRuntimeGuiOperationPreviewCommandRegistryRequest.load(deserializer);
      case 27: return AgentRuntimeGuiOperationDecideCommandRegistryRequest.load(deserializer);
      case 28: return AgentRuntimeGuiOperationApplyCommandRegistryRequest.load(deserializer);
      case 29: return AgentRuntimeGuiOperationWorkflowMemoryFeedback.load(deserializer);
      case 30: return AgentRuntimeGuiOperationRoleEditorOptions.load(deserializer);
      case 31: return AgentRuntimeGuiOperationValidateRoleDraft.load(deserializer);
      case 32: return AgentRuntimeGuiOperationCreateRoleFromDraft.load(deserializer);
      case 33: return AgentRuntimeGuiOperationUpdateRoleFromDraft.load(deserializer);
      case 34: return AgentRuntimeGuiOperationShowRoleDetail.load(deserializer);
      case 35: return AgentRuntimeGuiOperationListRoleVersions.load(deserializer);
      case 36: return AgentRuntimeGuiOperationShowRoleVersion.load(deserializer);
      case 37: return AgentRuntimeGuiOperationExportRole.load(deserializer);
      case 38: return AgentRuntimeGuiOperationActivateRoleVersion.load(deserializer);
      case 39: return AgentRuntimeGuiOperationArchiveRole.load(deserializer);
      case 40: return AgentRuntimeGuiOperationUnarchiveRole.load(deserializer);
      case 41: return AgentRuntimeGuiOperationSetRequirements.load(deserializer);
      case 42: return AgentRuntimeGuiOperationClearRequirements.load(deserializer);
      case 43: return AgentRuntimeGuiOperationShowRequirementsStatus.load(deserializer);
      case 44: return AgentRuntimeGuiOperationListRequirementsPackets.load(deserializer);
      case 45: return AgentRuntimeGuiOperationLoadFullSizeImage.load(deserializer);
      default: throw Exception('Unknown variant index for AgentRuntimeGuiOperation: ' + index.toString());
    }
  }

  Uint8List bincodeSerialize() {
      final serializer = BincodeSerializer();
      serialize(serializer);
      return serializer.bytes;
  }

  static AgentRuntimeGuiOperation bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeGuiOperation.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }
}


@immutable
class AgentRuntimeGuiOperationSelectSession extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationSelectSession({
    required this.sessionId,
  }) : super();

  static AgentRuntimeGuiOperationSelectSession load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationSelectSession(
      sessionId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String sessionId;

  AgentRuntimeGuiOperationSelectSession copyWith({
    String? sessionId,
  }) {
    return AgentRuntimeGuiOperationSelectSession(
      sessionId: sessionId ?? this.sessionId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(0);
    serializer.serializeString(sessionId);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationSelectSession
      && sessionId == other.sessionId;
  }

  @override
  int get hashCode => sessionId.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'sessionId: $sessionId'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationSelectSession';
  }
}

@immutable
class AgentRuntimeGuiOperationSelectWorkflowMemory extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationSelectWorkflowMemory({
    required this.memoryId,
  }) : super();

  static AgentRuntimeGuiOperationSelectWorkflowMemory load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationSelectWorkflowMemory(
      memoryId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String memoryId;

  AgentRuntimeGuiOperationSelectWorkflowMemory copyWith({
    String? memoryId,
  }) {
    return AgentRuntimeGuiOperationSelectWorkflowMemory(
      memoryId: memoryId ?? this.memoryId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(1);
    serializer.serializeString(memoryId);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationSelectWorkflowMemory
      && memoryId == other.memoryId;
  }

  @override
  int get hashCode => memoryId.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'memoryId: $memoryId'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationSelectWorkflowMemory';
  }
}

@immutable
class AgentRuntimeGuiOperationCreateSession extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationCreateSession({
    required this.role,
    required this.project,
    required this.model,
    required this.workdir,
    required this.worktreeRoot,
    required this.title,
    required this.name,
  }) : super();

  static AgentRuntimeGuiOperationCreateSession load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationCreateSession(
      role: deserializer.deserializeString(),
      project: deserializer.deserializeString(),
      model: deserializer.deserializeString(),
      workdir: deserializer.deserializeString(),
      worktreeRoot: deserializer.deserializeString(),
      title: deserializer.deserializeString(),
      name: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String role;
  final String project;
  final String model;
  final String workdir;
  final String worktreeRoot;
  final String title;
  final String name;

  AgentRuntimeGuiOperationCreateSession copyWith({
    String? role,
    String? project,
    String? model,
    String? workdir,
    String? worktreeRoot,
    String? title,
    String? name,
  }) {
    return AgentRuntimeGuiOperationCreateSession(
      role: role ?? this.role,
      project: project ?? this.project,
      model: model ?? this.model,
      workdir: workdir ?? this.workdir,
      worktreeRoot: worktreeRoot ?? this.worktreeRoot,
      title: title ?? this.title,
      name: name ?? this.name,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(2);
    serializer.serializeString(role);
    serializer.serializeString(project);
    serializer.serializeString(model);
    serializer.serializeString(workdir);
    serializer.serializeString(worktreeRoot);
    serializer.serializeString(title);
    serializer.serializeString(name);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationCreateSession
      && role == other.role
      && project == other.project
      && model == other.model
      && workdir == other.workdir
      && worktreeRoot == other.worktreeRoot
      && title == other.title
      && name == other.name;
  }

  @override
  int get hashCode => Object.hash(
        role,
        project,
        model,
        workdir,
        worktreeRoot,
        title,
        name,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'role: $role, '
        'project: $project, '
        'model: $model, '
        'workdir: $workdir, '
        'worktreeRoot: $worktreeRoot, '
        'title: $title, '
        'name: $name'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationCreateSession';
  }
}

@immutable
class AgentRuntimeGuiOperationListProjects extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationListProjects(
  ) : super();

  static AgentRuntimeGuiOperationListProjects load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationListProjects(
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(3);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationListProjects;
  }

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationListProjects';
  }
}

@immutable
class AgentRuntimeGuiOperationCreateProject extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationCreateProject({
    required this.projectKey,
    required this.displayName,
    required this.defaultWorkdir,
    required this.defaultWorktreeRoot,
    required this.defaultRoleId,
    required this.defaultModel,
  }) : super();

  static AgentRuntimeGuiOperationCreateProject load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationCreateProject(
      projectKey: deserializer.deserializeString(),
      displayName: deserializer.deserializeString(),
      defaultWorkdir: deserializer.deserializeString(),
      defaultWorktreeRoot: deserializer.deserializeString(),
      defaultRoleId: deserializer.deserializeString(),
      defaultModel: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String projectKey;
  final String displayName;
  final String defaultWorkdir;
  final String defaultWorktreeRoot;
  final String defaultRoleId;
  final String defaultModel;

  AgentRuntimeGuiOperationCreateProject copyWith({
    String? projectKey,
    String? displayName,
    String? defaultWorkdir,
    String? defaultWorktreeRoot,
    String? defaultRoleId,
    String? defaultModel,
  }) {
    return AgentRuntimeGuiOperationCreateProject(
      projectKey: projectKey ?? this.projectKey,
      displayName: displayName ?? this.displayName,
      defaultWorkdir: defaultWorkdir ?? this.defaultWorkdir,
      defaultWorktreeRoot: defaultWorktreeRoot ?? this.defaultWorktreeRoot,
      defaultRoleId: defaultRoleId ?? this.defaultRoleId,
      defaultModel: defaultModel ?? this.defaultModel,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(4);
    serializer.serializeString(projectKey);
    serializer.serializeString(displayName);
    serializer.serializeString(defaultWorkdir);
    serializer.serializeString(defaultWorktreeRoot);
    serializer.serializeString(defaultRoleId);
    serializer.serializeString(defaultModel);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationCreateProject
      && projectKey == other.projectKey
      && displayName == other.displayName
      && defaultWorkdir == other.defaultWorkdir
      && defaultWorktreeRoot == other.defaultWorktreeRoot
      && defaultRoleId == other.defaultRoleId
      && defaultModel == other.defaultModel;
  }

  @override
  int get hashCode => Object.hash(
        projectKey,
        displayName,
        defaultWorkdir,
        defaultWorktreeRoot,
        defaultRoleId,
        defaultModel,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'projectKey: $projectKey, '
        'displayName: $displayName, '
        'defaultWorkdir: $defaultWorkdir, '
        'defaultWorktreeRoot: $defaultWorktreeRoot, '
        'defaultRoleId: $defaultRoleId, '
        'defaultModel: $defaultModel'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationCreateProject';
  }
}

@immutable
class AgentRuntimeGuiOperationUpdateProject extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationUpdateProject({
    required this.projectKey,
    required this.displayName,
    required this.defaultWorkdir,
    required this.defaultWorktreeRoot,
    required this.defaultRoleId,
    required this.defaultModel,
  }) : super();

  static AgentRuntimeGuiOperationUpdateProject load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationUpdateProject(
      projectKey: deserializer.deserializeString(),
      displayName: deserializer.deserializeString(),
      defaultWorkdir: deserializer.deserializeString(),
      defaultWorktreeRoot: deserializer.deserializeString(),
      defaultRoleId: deserializer.deserializeString(),
      defaultModel: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String projectKey;
  final String displayName;
  final String defaultWorkdir;
  final String defaultWorktreeRoot;
  final String defaultRoleId;
  final String defaultModel;

  AgentRuntimeGuiOperationUpdateProject copyWith({
    String? projectKey,
    String? displayName,
    String? defaultWorkdir,
    String? defaultWorktreeRoot,
    String? defaultRoleId,
    String? defaultModel,
  }) {
    return AgentRuntimeGuiOperationUpdateProject(
      projectKey: projectKey ?? this.projectKey,
      displayName: displayName ?? this.displayName,
      defaultWorkdir: defaultWorkdir ?? this.defaultWorkdir,
      defaultWorktreeRoot: defaultWorktreeRoot ?? this.defaultWorktreeRoot,
      defaultRoleId: defaultRoleId ?? this.defaultRoleId,
      defaultModel: defaultModel ?? this.defaultModel,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(5);
    serializer.serializeString(projectKey);
    serializer.serializeString(displayName);
    serializer.serializeString(defaultWorkdir);
    serializer.serializeString(defaultWorktreeRoot);
    serializer.serializeString(defaultRoleId);
    serializer.serializeString(defaultModel);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationUpdateProject
      && projectKey == other.projectKey
      && displayName == other.displayName
      && defaultWorkdir == other.defaultWorkdir
      && defaultWorktreeRoot == other.defaultWorktreeRoot
      && defaultRoleId == other.defaultRoleId
      && defaultModel == other.defaultModel;
  }

  @override
  int get hashCode => Object.hash(
        projectKey,
        displayName,
        defaultWorkdir,
        defaultWorktreeRoot,
        defaultRoleId,
        defaultModel,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'projectKey: $projectKey, '
        'displayName: $displayName, '
        'defaultWorkdir: $defaultWorkdir, '
        'defaultWorktreeRoot: $defaultWorktreeRoot, '
        'defaultRoleId: $defaultRoleId, '
        'defaultModel: $defaultModel'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationUpdateProject';
  }
}

@immutable
class AgentRuntimeGuiOperationArchiveProject extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationArchiveProject({
    required this.projectKey,
  }) : super();

  static AgentRuntimeGuiOperationArchiveProject load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationArchiveProject(
      projectKey: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String projectKey;

  AgentRuntimeGuiOperationArchiveProject copyWith({
    String? projectKey,
  }) {
    return AgentRuntimeGuiOperationArchiveProject(
      projectKey: projectKey ?? this.projectKey,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(6);
    serializer.serializeString(projectKey);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationArchiveProject
      && projectKey == other.projectKey;
  }

  @override
  int get hashCode => projectKey.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'projectKey: $projectKey'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationArchiveProject';
  }
}

@immutable
class AgentRuntimeGuiOperationUnarchiveProject extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationUnarchiveProject({
    required this.projectKey,
  }) : super();

  static AgentRuntimeGuiOperationUnarchiveProject load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationUnarchiveProject(
      projectKey: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String projectKey;

  AgentRuntimeGuiOperationUnarchiveProject copyWith({
    String? projectKey,
  }) {
    return AgentRuntimeGuiOperationUnarchiveProject(
      projectKey: projectKey ?? this.projectKey,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(7);
    serializer.serializeString(projectKey);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationUnarchiveProject
      && projectKey == other.projectKey;
  }

  @override
  int get hashCode => projectKey.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'projectKey: $projectKey'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationUnarchiveProject';
  }
}

@immutable
class AgentRuntimeGuiOperationUpdateRuntimeSettings extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationUpdateRuntimeSettings({
    required this.baseUrl,
    required this.selectedProjectId,
  }) : super();

  static AgentRuntimeGuiOperationUpdateRuntimeSettings load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationUpdateRuntimeSettings(
      baseUrl: deserializer.deserializeString(),
      selectedProjectId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String baseUrl;
  final String selectedProjectId;

  AgentRuntimeGuiOperationUpdateRuntimeSettings copyWith({
    String? baseUrl,
    String? selectedProjectId,
  }) {
    return AgentRuntimeGuiOperationUpdateRuntimeSettings(
      baseUrl: baseUrl ?? this.baseUrl,
      selectedProjectId: selectedProjectId ?? this.selectedProjectId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(8);
    serializer.serializeString(baseUrl);
    serializer.serializeString(selectedProjectId);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationUpdateRuntimeSettings
      && baseUrl == other.baseUrl
      && selectedProjectId == other.selectedProjectId;
  }

  @override
  int get hashCode => Object.hash(
        baseUrl,
        selectedProjectId,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'baseUrl: $baseUrl, '
        'selectedProjectId: $selectedProjectId'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationUpdateRuntimeSettings';
  }
}

@immutable
class AgentRuntimeGuiOperationUpdateSessionSettings extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationUpdateSessionSettings({
    required this.sessionId,
    required this.project,
    required this.role,
    required this.model,
    required this.workdir,
    required this.worktreeRoot,
    required this.title,
    required this.name,
    required this.tracked,
  }) : super();

  static AgentRuntimeGuiOperationUpdateSessionSettings load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationUpdateSessionSettings(
      sessionId: deserializer.deserializeString(),
      project: deserializer.deserializeString(),
      role: deserializer.deserializeString(),
      model: deserializer.deserializeString(),
      workdir: deserializer.deserializeString(),
      worktreeRoot: deserializer.deserializeString(),
      title: deserializer.deserializeString(),
      name: deserializer.deserializeString(),
      tracked: deserializer.deserializeBool(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String sessionId;
  final String project;
  final String role;
  final String model;
  final String workdir;
  final String worktreeRoot;
  final String title;
  final String name;
  final bool tracked;

  AgentRuntimeGuiOperationUpdateSessionSettings copyWith({
    String? sessionId,
    String? project,
    String? role,
    String? model,
    String? workdir,
    String? worktreeRoot,
    String? title,
    String? name,
    bool? tracked,
  }) {
    return AgentRuntimeGuiOperationUpdateSessionSettings(
      sessionId: sessionId ?? this.sessionId,
      project: project ?? this.project,
      role: role ?? this.role,
      model: model ?? this.model,
      workdir: workdir ?? this.workdir,
      worktreeRoot: worktreeRoot ?? this.worktreeRoot,
      title: title ?? this.title,
      name: name ?? this.name,
      tracked: tracked ?? this.tracked,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(9);
    serializer.serializeString(sessionId);
    serializer.serializeString(project);
    serializer.serializeString(role);
    serializer.serializeString(model);
    serializer.serializeString(workdir);
    serializer.serializeString(worktreeRoot);
    serializer.serializeString(title);
    serializer.serializeString(name);
    serializer.serializeBool(tracked);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationUpdateSessionSettings
      && sessionId == other.sessionId
      && project == other.project
      && role == other.role
      && model == other.model
      && workdir == other.workdir
      && worktreeRoot == other.worktreeRoot
      && title == other.title
      && name == other.name
      && tracked == other.tracked;
  }

  @override
  int get hashCode => Object.hash(
        sessionId,
        project,
        role,
        model,
        workdir,
        worktreeRoot,
        title,
        name,
        tracked,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'sessionId: $sessionId, '
        'project: $project, '
        'role: $role, '
        'model: $model, '
        'workdir: $workdir, '
        'worktreeRoot: $worktreeRoot, '
        'title: $title, '
        'name: $name, '
        'tracked: $tracked'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationUpdateSessionSettings';
  }
}

@immutable
class AgentRuntimeGuiOperationSendMessage extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationSendMessage({
    required this.sessionId,
    required this.message,
  }) : super();

  static AgentRuntimeGuiOperationSendMessage load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationSendMessage(
      sessionId: deserializer.deserializeString(),
      message: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String sessionId;
  final String message;

  AgentRuntimeGuiOperationSendMessage copyWith({
    String? sessionId,
    String? message,
  }) {
    return AgentRuntimeGuiOperationSendMessage(
      sessionId: sessionId ?? this.sessionId,
      message: message ?? this.message,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(10);
    serializer.serializeString(sessionId);
    serializer.serializeString(message);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationSendMessage
      && sessionId == other.sessionId
      && message == other.message;
  }

  @override
  int get hashCode => Object.hash(
        sessionId,
        message,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'sessionId: $sessionId, '
        'message: $message'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationSendMessage';
  }
}

@immutable
class AgentRuntimeGuiOperationTerminateProcess extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationTerminateProcess({
    required this.sessionId,
    required this.handle,
  }) : super();

  static AgentRuntimeGuiOperationTerminateProcess load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationTerminateProcess(
      sessionId: deserializer.deserializeString(),
      handle: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String sessionId;
  final String handle;

  AgentRuntimeGuiOperationTerminateProcess copyWith({
    String? sessionId,
    String? handle,
  }) {
    return AgentRuntimeGuiOperationTerminateProcess(
      sessionId: sessionId ?? this.sessionId,
      handle: handle ?? this.handle,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(11);
    serializer.serializeString(sessionId);
    serializer.serializeString(handle);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationTerminateProcess
      && sessionId == other.sessionId
      && handle == other.handle;
  }

  @override
  int get hashCode => Object.hash(
        sessionId,
        handle,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'sessionId: $sessionId, '
        'handle: $handle'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationTerminateProcess';
  }
}

@immutable
class AgentRuntimeGuiOperationInputProcess extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationInputProcess({
    required this.sessionId,
    required this.handle,
    required this.text,
  }) : super();

  static AgentRuntimeGuiOperationInputProcess load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationInputProcess(
      sessionId: deserializer.deserializeString(),
      handle: deserializer.deserializeString(),
      text: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String sessionId;
  final String handle;
  final String text;

  AgentRuntimeGuiOperationInputProcess copyWith({
    String? sessionId,
    String? handle,
    String? text,
  }) {
    return AgentRuntimeGuiOperationInputProcess(
      sessionId: sessionId ?? this.sessionId,
      handle: handle ?? this.handle,
      text: text ?? this.text,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(12);
    serializer.serializeString(sessionId);
    serializer.serializeString(handle);
    serializer.serializeString(text);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationInputProcess
      && sessionId == other.sessionId
      && handle == other.handle
      && text == other.text;
  }

  @override
  int get hashCode => Object.hash(
        sessionId,
        handle,
        text,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'sessionId: $sessionId, '
        'handle: $handle, '
        'text: $text'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationInputProcess';
  }
}

@immutable
class AgentRuntimeGuiOperationFlushProcess extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationFlushProcess({
    required this.sessionId,
    required this.handle,
  }) : super();

  static AgentRuntimeGuiOperationFlushProcess load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationFlushProcess(
      sessionId: deserializer.deserializeString(),
      handle: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String sessionId;
  final String handle;

  AgentRuntimeGuiOperationFlushProcess copyWith({
    String? sessionId,
    String? handle,
  }) {
    return AgentRuntimeGuiOperationFlushProcess(
      sessionId: sessionId ?? this.sessionId,
      handle: handle ?? this.handle,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(13);
    serializer.serializeString(sessionId);
    serializer.serializeString(handle);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationFlushProcess
      && sessionId == other.sessionId
      && handle == other.handle;
  }

  @override
  int get hashCode => Object.hash(
        sessionId,
        handle,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'sessionId: $sessionId, '
        'handle: $handle'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationFlushProcess';
  }
}

@immutable
class AgentRuntimeGuiOperationCompactSession extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationCompactSession({
    required this.sessionId,
    required this.throughTurn,
  }) : super();

  static AgentRuntimeGuiOperationCompactSession load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationCompactSession(
      sessionId: deserializer.deserializeString(),
      throughTurn: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String sessionId;
  final String throughTurn;

  AgentRuntimeGuiOperationCompactSession copyWith({
    String? sessionId,
    String? throughTurn,
  }) {
    return AgentRuntimeGuiOperationCompactSession(
      sessionId: sessionId ?? this.sessionId,
      throughTurn: throughTurn ?? this.throughTurn,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(14);
    serializer.serializeString(sessionId);
    serializer.serializeString(throughTurn);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationCompactSession
      && sessionId == other.sessionId
      && throughTurn == other.throughTurn;
  }

  @override
  int get hashCode => Object.hash(
        sessionId,
        throughTurn,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'sessionId: $sessionId, '
        'throughTurn: $throughTurn'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationCompactSession';
  }
}

@immutable
class AgentRuntimeGuiOperationGrantGodMode extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationGrantGodMode({
    required this.sessionId,
    required this.reason,
  }) : super();

  static AgentRuntimeGuiOperationGrantGodMode load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationGrantGodMode(
      sessionId: deserializer.deserializeString(),
      reason: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String sessionId;
  final String reason;

  AgentRuntimeGuiOperationGrantGodMode copyWith({
    String? sessionId,
    String? reason,
  }) {
    return AgentRuntimeGuiOperationGrantGodMode(
      sessionId: sessionId ?? this.sessionId,
      reason: reason ?? this.reason,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(15);
    serializer.serializeString(sessionId);
    serializer.serializeString(reason);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationGrantGodMode
      && sessionId == other.sessionId
      && reason == other.reason;
  }

  @override
  int get hashCode => Object.hash(
        sessionId,
        reason,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'sessionId: $sessionId, '
        'reason: $reason'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationGrantGodMode';
  }
}

@immutable
class AgentRuntimeGuiOperationRevokeGodMode extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationRevokeGodMode({
    required this.sessionId,
    required this.reason,
  }) : super();

  static AgentRuntimeGuiOperationRevokeGodMode load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationRevokeGodMode(
      sessionId: deserializer.deserializeString(),
      reason: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String sessionId;
  final String reason;

  AgentRuntimeGuiOperationRevokeGodMode copyWith({
    String? sessionId,
    String? reason,
  }) {
    return AgentRuntimeGuiOperationRevokeGodMode(
      sessionId: sessionId ?? this.sessionId,
      reason: reason ?? this.reason,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(16);
    serializer.serializeString(sessionId);
    serializer.serializeString(reason);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationRevokeGodMode
      && sessionId == other.sessionId
      && reason == other.reason;
  }

  @override
  int get hashCode => Object.hash(
        sessionId,
        reason,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'sessionId: $sessionId, '
        'reason: $reason'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationRevokeGodMode';
  }
}

@immutable
class AgentRuntimeGuiOperationCloseSession extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationCloseSession({
    required this.sessionId,
    required this.reason,
  }) : super();

  static AgentRuntimeGuiOperationCloseSession load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationCloseSession(
      sessionId: deserializer.deserializeString(),
      reason: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String sessionId;
  final String reason;

  AgentRuntimeGuiOperationCloseSession copyWith({
    String? sessionId,
    String? reason,
  }) {
    return AgentRuntimeGuiOperationCloseSession(
      sessionId: sessionId ?? this.sessionId,
      reason: reason ?? this.reason,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(17);
    serializer.serializeString(sessionId);
    serializer.serializeString(reason);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationCloseSession
      && sessionId == other.sessionId
      && reason == other.reason;
  }

  @override
  int get hashCode => Object.hash(
        sessionId,
        reason,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'sessionId: $sessionId, '
        'reason: $reason'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationCloseSession';
  }
}

@immutable
class AgentRuntimeGuiOperationArchiveSession extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationArchiveSession({
    required this.sessionId,
  }) : super();

  static AgentRuntimeGuiOperationArchiveSession load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationArchiveSession(
      sessionId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String sessionId;

  AgentRuntimeGuiOperationArchiveSession copyWith({
    String? sessionId,
  }) {
    return AgentRuntimeGuiOperationArchiveSession(
      sessionId: sessionId ?? this.sessionId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(18);
    serializer.serializeString(sessionId);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationArchiveSession
      && sessionId == other.sessionId;
  }

  @override
  int get hashCode => sessionId.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'sessionId: $sessionId'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationArchiveSession';
  }
}

@immutable
class AgentRuntimeGuiOperationForkSession extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationForkSession({
    required this.sessionId,
    required this.atTurn,
  }) : super();

  static AgentRuntimeGuiOperationForkSession load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationForkSession(
      sessionId: deserializer.deserializeString(),
      atTurn: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String sessionId;
  final String atTurn;

  AgentRuntimeGuiOperationForkSession copyWith({
    String? sessionId,
    String? atTurn,
  }) {
    return AgentRuntimeGuiOperationForkSession(
      sessionId: sessionId ?? this.sessionId,
      atTurn: atTurn ?? this.atTurn,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(19);
    serializer.serializeString(sessionId);
    serializer.serializeString(atTurn);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationForkSession
      && sessionId == other.sessionId
      && atTurn == other.atTurn;
  }

  @override
  int get hashCode => Object.hash(
        sessionId,
        atTurn,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'sessionId: $sessionId, '
        'atTurn: $atTurn'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationForkSession';
  }
}

@immutable
class AgentRuntimeGuiOperationDecideApproval extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationDecideApproval({
    required this.approvalId,
    required this.decision,
    required this.reason,
  }) : super();

  static AgentRuntimeGuiOperationDecideApproval load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationDecideApproval(
      approvalId: deserializer.deserializeString(),
      decision: deserializer.deserializeString(),
      reason: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String approvalId;
  final String decision;
  final String reason;

  AgentRuntimeGuiOperationDecideApproval copyWith({
    String? approvalId,
    String? decision,
    String? reason,
  }) {
    return AgentRuntimeGuiOperationDecideApproval(
      approvalId: approvalId ?? this.approvalId,
      decision: decision ?? this.decision,
      reason: reason ?? this.reason,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(20);
    serializer.serializeString(approvalId);
    serializer.serializeString(decision);
    serializer.serializeString(reason);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationDecideApproval
      && approvalId == other.approvalId
      && decision == other.decision
      && reason == other.reason;
  }

  @override
  int get hashCode => Object.hash(
        approvalId,
        decision,
        reason,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'approvalId: $approvalId, '
        'decision: $decision, '
        'reason: $reason'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationDecideApproval';
  }
}

@immutable
class AgentRuntimeGuiOperationResumeApproval extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationResumeApproval({
    required this.approvalId,
  }) : super();

  static AgentRuntimeGuiOperationResumeApproval load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationResumeApproval(
      approvalId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String approvalId;

  AgentRuntimeGuiOperationResumeApproval copyWith({
    String? approvalId,
  }) {
    return AgentRuntimeGuiOperationResumeApproval(
      approvalId: approvalId ?? this.approvalId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(21);
    serializer.serializeString(approvalId);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationResumeApproval
      && approvalId == other.approvalId;
  }

  @override
  int get hashCode => approvalId.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'approvalId: $approvalId'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationResumeApproval';
  }
}

@immutable
class AgentRuntimeGuiOperationListCommandRegistry extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationListCommandRegistry({
    required this.sessionId,
    required this.projectKey,
  }) : super();

  static AgentRuntimeGuiOperationListCommandRegistry load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationListCommandRegistry(
      sessionId: deserializer.deserializeString(),
      projectKey: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String sessionId;
  final String projectKey;

  AgentRuntimeGuiOperationListCommandRegistry copyWith({
    String? sessionId,
    String? projectKey,
  }) {
    return AgentRuntimeGuiOperationListCommandRegistry(
      sessionId: sessionId ?? this.sessionId,
      projectKey: projectKey ?? this.projectKey,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(22);
    serializer.serializeString(sessionId);
    serializer.serializeString(projectKey);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationListCommandRegistry
      && sessionId == other.sessionId
      && projectKey == other.projectKey;
  }

  @override
  int get hashCode => Object.hash(
        sessionId,
        projectKey,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'sessionId: $sessionId, '
        'projectKey: $projectKey'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationListCommandRegistry';
  }
}

@immutable
class AgentRuntimeGuiOperationShowCommand extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationShowCommand({
    required this.actionId,
    required this.sessionId,
    required this.projectKey,
  }) : super();

  static AgentRuntimeGuiOperationShowCommand load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationShowCommand(
      actionId: deserializer.deserializeString(),
      sessionId: deserializer.deserializeString(),
      projectKey: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String actionId;
  final String sessionId;
  final String projectKey;

  AgentRuntimeGuiOperationShowCommand copyWith({
    String? actionId,
    String? sessionId,
    String? projectKey,
  }) {
    return AgentRuntimeGuiOperationShowCommand(
      actionId: actionId ?? this.actionId,
      sessionId: sessionId ?? this.sessionId,
      projectKey: projectKey ?? this.projectKey,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(23);
    serializer.serializeString(actionId);
    serializer.serializeString(sessionId);
    serializer.serializeString(projectKey);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationShowCommand
      && actionId == other.actionId
      && sessionId == other.sessionId
      && projectKey == other.projectKey;
  }

  @override
  int get hashCode => Object.hash(
        actionId,
        sessionId,
        projectKey,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'actionId: $actionId, '
        'sessionId: $sessionId, '
        'projectKey: $projectKey'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationShowCommand';
  }
}

@immutable
class AgentRuntimeGuiOperationListCommandRegistryRequests extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationListCommandRegistryRequests(
  ) : super();

  static AgentRuntimeGuiOperationListCommandRegistryRequests load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationListCommandRegistryRequests(
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(24);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationListCommandRegistryRequests;
  }

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationListCommandRegistryRequests';
  }
}

@immutable
class AgentRuntimeGuiOperationShowCommandRegistryRequest extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationShowCommandRegistryRequest({
    required this.requestId,
  }) : super();

  static AgentRuntimeGuiOperationShowCommandRegistryRequest load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationShowCommandRegistryRequest(
      requestId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String requestId;

  AgentRuntimeGuiOperationShowCommandRegistryRequest copyWith({
    String? requestId,
  }) {
    return AgentRuntimeGuiOperationShowCommandRegistryRequest(
      requestId: requestId ?? this.requestId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(25);
    serializer.serializeString(requestId);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationShowCommandRegistryRequest
      && requestId == other.requestId;
  }

  @override
  int get hashCode => requestId.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'requestId: $requestId'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationShowCommandRegistryRequest';
  }
}

@immutable
class AgentRuntimeGuiOperationPreviewCommandRegistryRequest extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationPreviewCommandRegistryRequest({
    required this.requestId,
    required this.decision,
  }) : super();

  static AgentRuntimeGuiOperationPreviewCommandRegistryRequest load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationPreviewCommandRegistryRequest(
      requestId: deserializer.deserializeString(),
      decision: AgentRuntimeCommandRegistryDecisionInput.deserialize(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String requestId;
  final AgentRuntimeCommandRegistryDecisionInput decision;

  AgentRuntimeGuiOperationPreviewCommandRegistryRequest copyWith({
    String? requestId,
    AgentRuntimeCommandRegistryDecisionInput? decision,
  }) {
    return AgentRuntimeGuiOperationPreviewCommandRegistryRequest(
      requestId: requestId ?? this.requestId,
      decision: decision ?? this.decision,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(26);
    serializer.serializeString(requestId);
    decision.serialize(serializer);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationPreviewCommandRegistryRequest
      && requestId == other.requestId
      && decision == other.decision;
  }

  @override
  int get hashCode => Object.hash(
        requestId,
        decision,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'requestId: $requestId, '
        'decision: $decision'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationPreviewCommandRegistryRequest';
  }
}

@immutable
class AgentRuntimeGuiOperationDecideCommandRegistryRequest extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationDecideCommandRegistryRequest({
    required this.requestId,
    required this.decision,
  }) : super();

  static AgentRuntimeGuiOperationDecideCommandRegistryRequest load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationDecideCommandRegistryRequest(
      requestId: deserializer.deserializeString(),
      decision: AgentRuntimeCommandRegistryDecisionInput.deserialize(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String requestId;
  final AgentRuntimeCommandRegistryDecisionInput decision;

  AgentRuntimeGuiOperationDecideCommandRegistryRequest copyWith({
    String? requestId,
    AgentRuntimeCommandRegistryDecisionInput? decision,
  }) {
    return AgentRuntimeGuiOperationDecideCommandRegistryRequest(
      requestId: requestId ?? this.requestId,
      decision: decision ?? this.decision,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(27);
    serializer.serializeString(requestId);
    decision.serialize(serializer);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationDecideCommandRegistryRequest
      && requestId == other.requestId
      && decision == other.decision;
  }

  @override
  int get hashCode => Object.hash(
        requestId,
        decision,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'requestId: $requestId, '
        'decision: $decision'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationDecideCommandRegistryRequest';
  }
}

@immutable
class AgentRuntimeGuiOperationApplyCommandRegistryRequest extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationApplyCommandRegistryRequest({
    required this.requestId,
    required this.sessionId,
  }) : super();

  static AgentRuntimeGuiOperationApplyCommandRegistryRequest load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationApplyCommandRegistryRequest(
      requestId: deserializer.deserializeString(),
      sessionId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String requestId;
  final String sessionId;

  AgentRuntimeGuiOperationApplyCommandRegistryRequest copyWith({
    String? requestId,
    String? sessionId,
  }) {
    return AgentRuntimeGuiOperationApplyCommandRegistryRequest(
      requestId: requestId ?? this.requestId,
      sessionId: sessionId ?? this.sessionId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(28);
    serializer.serializeString(requestId);
    serializer.serializeString(sessionId);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationApplyCommandRegistryRequest
      && requestId == other.requestId
      && sessionId == other.sessionId;
  }

  @override
  int get hashCode => Object.hash(
        requestId,
        sessionId,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'requestId: $requestId, '
        'sessionId: $sessionId'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationApplyCommandRegistryRequest';
  }
}

@immutable
class AgentRuntimeGuiOperationWorkflowMemoryFeedback extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationWorkflowMemoryFeedback({
    required this.memoryId,
    required this.sessionId,
    required this.feedback,
    required this.payload,
  }) : super();

  static AgentRuntimeGuiOperationWorkflowMemoryFeedback load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationWorkflowMemoryFeedback(
      memoryId: deserializer.deserializeString(),
      sessionId: deserializer.deserializeString(),
      feedback: deserializer.deserializeString(),
      payload: AgentRuntimeWorkflowMemoryFeedbackPayload.deserialize(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String memoryId;
  final String sessionId;
  final String feedback;
  final AgentRuntimeWorkflowMemoryFeedbackPayload payload;

  AgentRuntimeGuiOperationWorkflowMemoryFeedback copyWith({
    String? memoryId,
    String? sessionId,
    String? feedback,
    AgentRuntimeWorkflowMemoryFeedbackPayload? payload,
  }) {
    return AgentRuntimeGuiOperationWorkflowMemoryFeedback(
      memoryId: memoryId ?? this.memoryId,
      sessionId: sessionId ?? this.sessionId,
      feedback: feedback ?? this.feedback,
      payload: payload ?? this.payload,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(29);
    serializer.serializeString(memoryId);
    serializer.serializeString(sessionId);
    serializer.serializeString(feedback);
    payload.serialize(serializer);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationWorkflowMemoryFeedback
      && memoryId == other.memoryId
      && sessionId == other.sessionId
      && feedback == other.feedback
      && payload == other.payload;
  }

  @override
  int get hashCode => Object.hash(
        memoryId,
        sessionId,
        feedback,
        payload,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'memoryId: $memoryId, '
        'sessionId: $sessionId, '
        'feedback: $feedback, '
        'payload: $payload'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationWorkflowMemoryFeedback';
  }
}

@immutable
class AgentRuntimeGuiOperationRoleEditorOptions extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationRoleEditorOptions(
  ) : super();

  static AgentRuntimeGuiOperationRoleEditorOptions load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationRoleEditorOptions(
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(30);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationRoleEditorOptions;
  }

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationRoleEditorOptions';
  }
}

@immutable
class AgentRuntimeGuiOperationValidateRoleDraft extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationValidateRoleDraft({
    required this.draft,
  }) : super();

  static AgentRuntimeGuiOperationValidateRoleDraft load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationValidateRoleDraft(
      draft: AgentRuntimeRoleEditorDraft.deserialize(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final AgentRuntimeRoleEditorDraft draft;

  AgentRuntimeGuiOperationValidateRoleDraft copyWith({
    AgentRuntimeRoleEditorDraft? draft,
  }) {
    return AgentRuntimeGuiOperationValidateRoleDraft(
      draft: draft ?? this.draft,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(31);
    draft.serialize(serializer);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationValidateRoleDraft
      && draft == other.draft;
  }

  @override
  int get hashCode => draft.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'draft: $draft'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationValidateRoleDraft';
  }
}

@immutable
class AgentRuntimeGuiOperationCreateRoleFromDraft extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationCreateRoleFromDraft({
    required this.draft,
  }) : super();

  static AgentRuntimeGuiOperationCreateRoleFromDraft load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationCreateRoleFromDraft(
      draft: AgentRuntimeRoleEditorDraft.deserialize(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final AgentRuntimeRoleEditorDraft draft;

  AgentRuntimeGuiOperationCreateRoleFromDraft copyWith({
    AgentRuntimeRoleEditorDraft? draft,
  }) {
    return AgentRuntimeGuiOperationCreateRoleFromDraft(
      draft: draft ?? this.draft,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(32);
    draft.serialize(serializer);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationCreateRoleFromDraft
      && draft == other.draft;
  }

  @override
  int get hashCode => draft.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'draft: $draft'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationCreateRoleFromDraft';
  }
}

@immutable
class AgentRuntimeGuiOperationUpdateRoleFromDraft extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationUpdateRoleFromDraft({
    required this.roleId,
    required this.draft,
  }) : super();

  static AgentRuntimeGuiOperationUpdateRoleFromDraft load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationUpdateRoleFromDraft(
      roleId: deserializer.deserializeString(),
      draft: AgentRuntimeRoleEditorDraft.deserialize(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String roleId;
  final AgentRuntimeRoleEditorDraft draft;

  AgentRuntimeGuiOperationUpdateRoleFromDraft copyWith({
    String? roleId,
    AgentRuntimeRoleEditorDraft? draft,
  }) {
    return AgentRuntimeGuiOperationUpdateRoleFromDraft(
      roleId: roleId ?? this.roleId,
      draft: draft ?? this.draft,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(33);
    serializer.serializeString(roleId);
    draft.serialize(serializer);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationUpdateRoleFromDraft
      && roleId == other.roleId
      && draft == other.draft;
  }

  @override
  int get hashCode => Object.hash(
        roleId,
        draft,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'roleId: $roleId, '
        'draft: $draft'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationUpdateRoleFromDraft';
  }
}

@immutable
class AgentRuntimeGuiOperationShowRoleDetail extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationShowRoleDetail({
    required this.roleId,
  }) : super();

  static AgentRuntimeGuiOperationShowRoleDetail load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationShowRoleDetail(
      roleId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String roleId;

  AgentRuntimeGuiOperationShowRoleDetail copyWith({
    String? roleId,
  }) {
    return AgentRuntimeGuiOperationShowRoleDetail(
      roleId: roleId ?? this.roleId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(34);
    serializer.serializeString(roleId);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationShowRoleDetail
      && roleId == other.roleId;
  }

  @override
  int get hashCode => roleId.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'roleId: $roleId'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationShowRoleDetail';
  }
}

@immutable
class AgentRuntimeGuiOperationListRoleVersions extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationListRoleVersions({
    required this.roleId,
  }) : super();

  static AgentRuntimeGuiOperationListRoleVersions load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationListRoleVersions(
      roleId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String roleId;

  AgentRuntimeGuiOperationListRoleVersions copyWith({
    String? roleId,
  }) {
    return AgentRuntimeGuiOperationListRoleVersions(
      roleId: roleId ?? this.roleId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(35);
    serializer.serializeString(roleId);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationListRoleVersions
      && roleId == other.roleId;
  }

  @override
  int get hashCode => roleId.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'roleId: $roleId'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationListRoleVersions';
  }
}

@immutable
class AgentRuntimeGuiOperationShowRoleVersion extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationShowRoleVersion({
    required this.versionId,
  }) : super();

  static AgentRuntimeGuiOperationShowRoleVersion load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationShowRoleVersion(
      versionId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String versionId;

  AgentRuntimeGuiOperationShowRoleVersion copyWith({
    String? versionId,
  }) {
    return AgentRuntimeGuiOperationShowRoleVersion(
      versionId: versionId ?? this.versionId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(36);
    serializer.serializeString(versionId);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationShowRoleVersion
      && versionId == other.versionId;
  }

  @override
  int get hashCode => versionId.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'versionId: $versionId'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationShowRoleVersion';
  }
}

@immutable
class AgentRuntimeGuiOperationExportRole extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationExportRole({
    required this.roleId,
  }) : super();

  static AgentRuntimeGuiOperationExportRole load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationExportRole(
      roleId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String roleId;

  AgentRuntimeGuiOperationExportRole copyWith({
    String? roleId,
  }) {
    return AgentRuntimeGuiOperationExportRole(
      roleId: roleId ?? this.roleId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(37);
    serializer.serializeString(roleId);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationExportRole
      && roleId == other.roleId;
  }

  @override
  int get hashCode => roleId.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'roleId: $roleId'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationExportRole';
  }
}

@immutable
class AgentRuntimeGuiOperationActivateRoleVersion extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationActivateRoleVersion({
    required this.roleId,
    required this.versionId,
  }) : super();

  static AgentRuntimeGuiOperationActivateRoleVersion load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationActivateRoleVersion(
      roleId: deserializer.deserializeString(),
      versionId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String roleId;
  final String versionId;

  AgentRuntimeGuiOperationActivateRoleVersion copyWith({
    String? roleId,
    String? versionId,
  }) {
    return AgentRuntimeGuiOperationActivateRoleVersion(
      roleId: roleId ?? this.roleId,
      versionId: versionId ?? this.versionId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(38);
    serializer.serializeString(roleId);
    serializer.serializeString(versionId);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationActivateRoleVersion
      && roleId == other.roleId
      && versionId == other.versionId;
  }

  @override
  int get hashCode => Object.hash(
        roleId,
        versionId,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'roleId: $roleId, '
        'versionId: $versionId'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationActivateRoleVersion';
  }
}

@immutable
class AgentRuntimeGuiOperationArchiveRole extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationArchiveRole({
    required this.roleId,
  }) : super();

  static AgentRuntimeGuiOperationArchiveRole load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationArchiveRole(
      roleId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String roleId;

  AgentRuntimeGuiOperationArchiveRole copyWith({
    String? roleId,
  }) {
    return AgentRuntimeGuiOperationArchiveRole(
      roleId: roleId ?? this.roleId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(39);
    serializer.serializeString(roleId);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationArchiveRole
      && roleId == other.roleId;
  }

  @override
  int get hashCode => roleId.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'roleId: $roleId'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationArchiveRole';
  }
}

@immutable
class AgentRuntimeGuiOperationUnarchiveRole extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationUnarchiveRole({
    required this.roleId,
  }) : super();

  static AgentRuntimeGuiOperationUnarchiveRole load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationUnarchiveRole(
      roleId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String roleId;

  AgentRuntimeGuiOperationUnarchiveRole copyWith({
    String? roleId,
  }) {
    return AgentRuntimeGuiOperationUnarchiveRole(
      roleId: roleId ?? this.roleId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(40);
    serializer.serializeString(roleId);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationUnarchiveRole
      && roleId == other.roleId;
  }

  @override
  int get hashCode => roleId.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'roleId: $roleId'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationUnarchiveRole';
  }
}

@immutable
class AgentRuntimeGuiOperationSetRequirements extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationSetRequirements({
    required this.sessionId,
    required this.title,
    required this.requirements,
  }) : super();

  static AgentRuntimeGuiOperationSetRequirements load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationSetRequirements(
      sessionId: deserializer.deserializeString(),
      title: deserializer.deserializeString(),
      requirements: TraitHelpers.deserializeVectorAgentRuntimeRequirementInput(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String sessionId;
  final String title;
  final List<AgentRuntimeRequirementInput> requirements;

  AgentRuntimeGuiOperationSetRequirements copyWith({
    String? sessionId,
    String? title,
    List<AgentRuntimeRequirementInput>? requirements,
  }) {
    return AgentRuntimeGuiOperationSetRequirements(
      sessionId: sessionId ?? this.sessionId,
      title: title ?? this.title,
      requirements: requirements ?? this.requirements,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(41);
    serializer.serializeString(sessionId);
    serializer.serializeString(title);
    TraitHelpers.serializeVectorAgentRuntimeRequirementInput(requirements, serializer);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationSetRequirements
      && sessionId == other.sessionId
      && title == other.title
      && listEquals(requirements, other.requirements);
  }

  @override
  int get hashCode => Object.hash(
        sessionId,
        title,
        requirements,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'sessionId: $sessionId, '
        'title: $title, '
        'requirements: $requirements'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationSetRequirements';
  }
}

@immutable
class AgentRuntimeGuiOperationClearRequirements extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationClearRequirements({
    required this.sessionId,
  }) : super();

  static AgentRuntimeGuiOperationClearRequirements load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationClearRequirements(
      sessionId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String sessionId;

  AgentRuntimeGuiOperationClearRequirements copyWith({
    String? sessionId,
  }) {
    return AgentRuntimeGuiOperationClearRequirements(
      sessionId: sessionId ?? this.sessionId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(42);
    serializer.serializeString(sessionId);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationClearRequirements
      && sessionId == other.sessionId;
  }

  @override
  int get hashCode => sessionId.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'sessionId: $sessionId'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationClearRequirements';
  }
}

@immutable
class AgentRuntimeGuiOperationShowRequirementsStatus extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationShowRequirementsStatus({
    required this.sessionId,
  }) : super();

  static AgentRuntimeGuiOperationShowRequirementsStatus load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationShowRequirementsStatus(
      sessionId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String sessionId;

  AgentRuntimeGuiOperationShowRequirementsStatus copyWith({
    String? sessionId,
  }) {
    return AgentRuntimeGuiOperationShowRequirementsStatus(
      sessionId: sessionId ?? this.sessionId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(43);
    serializer.serializeString(sessionId);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationShowRequirementsStatus
      && sessionId == other.sessionId;
  }

  @override
  int get hashCode => sessionId.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'sessionId: $sessionId'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationShowRequirementsStatus';
  }
}

@immutable
class AgentRuntimeGuiOperationListRequirementsPackets extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationListRequirementsPackets({
    required this.sessionId,
  }) : super();

  static AgentRuntimeGuiOperationListRequirementsPackets load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationListRequirementsPackets(
      sessionId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String sessionId;

  AgentRuntimeGuiOperationListRequirementsPackets copyWith({
    String? sessionId,
  }) {
    return AgentRuntimeGuiOperationListRequirementsPackets(
      sessionId: sessionId ?? this.sessionId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(44);
    serializer.serializeString(sessionId);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationListRequirementsPackets
      && sessionId == other.sessionId;
  }

  @override
  int get hashCode => sessionId.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'sessionId: $sessionId'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationListRequirementsPackets';
  }
}


@immutable
class AgentRuntimeGuiOperationLoadFullSizeImage extends AgentRuntimeGuiOperation {
  const AgentRuntimeGuiOperationLoadFullSizeImage({
    required this.sessionId,
    required this.imageArtifactId,
  }) : super();

  static AgentRuntimeGuiOperationLoadFullSizeImage load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGuiOperationLoadFullSizeImage(
      sessionId: deserializer.deserializeString(),
      imageArtifactId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String sessionId;
  final String imageArtifactId;

  AgentRuntimeGuiOperationLoadFullSizeImage copyWith({
    String? sessionId,
    String? imageArtifactId,
  }) {
    return AgentRuntimeGuiOperationLoadFullSizeImage(
      sessionId: sessionId ?? this.sessionId,
      imageArtifactId: imageArtifactId ?? this.imageArtifactId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(45);
    serializer.serializeString(sessionId);
    serializer.serializeString(imageArtifactId);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeGuiOperationLoadFullSizeImage
      && sessionId == other.sessionId
      && imageArtifactId == other.imageArtifactId;
  }

  @override
  int get hashCode => Object.hash(sessionId, imageArtifactId);

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'sessionId: $sessionId, '
        'imageArtifactId: $imageArtifactId'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGuiOperationLoadFullSizeImage';
  }
}
