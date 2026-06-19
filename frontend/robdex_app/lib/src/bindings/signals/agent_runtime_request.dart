// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


abstract class AgentRuntimeRequest {
  const AgentRuntimeRequest();

  void serialize(BinarySerializer serializer);

  static AgentRuntimeRequest deserialize(BinaryDeserializer deserializer) {
    int index = deserializer.deserializeVariantIndex();
    switch (index) {
      case 0: return AgentRuntimeRequestRefreshDiscovery.load(deserializer);
      case 1: return AgentRuntimeRequestRefreshIcloudRemoteDiscovery.load(deserializer);
      case 2: return AgentRuntimeRequestImportRemoteProfileDocument.load(deserializer);
      case 3: return AgentRuntimeRequestRefreshImportedRemoteProfile.load(deserializer);
      case 4: return AgentRuntimeRequestConnectDiscoveredRuntime.load(deserializer);
      case 5: return AgentRuntimeRequestConnectIcloudRemoteRuntime.load(deserializer);
      case 6: return AgentRuntimeRequestConnectImportedRemoteRuntime.load(deserializer);
      case 7: return AgentRuntimeRequestConnect.load(deserializer);
      case 8: return AgentRuntimeRequestSelectProject.load(deserializer);
      case 9: return AgentRuntimeRequestHydrate.load(deserializer);
      case 10: return AgentRuntimeRequestRehydrate.load(deserializer);
      case 11: return AgentRuntimeRequestDisconnect.load(deserializer);
      case 12: return AgentRuntimeRequestDispatchOperation.load(deserializer);
      default: throw Exception('Unknown variant index for AgentRuntimeRequest: ' + index.toString());
    }
  }

  Uint8List bincodeSerialize() {
      final serializer = BincodeSerializer();
      serialize(serializer);
      return serializer.bytes;
  }

  static AgentRuntimeRequest bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeRequest.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }
}


@immutable
class AgentRuntimeRequestRefreshDiscovery extends AgentRuntimeRequest {
  const AgentRuntimeRequestRefreshDiscovery({
    required this.discoveryPath,
  }) : super();

  static AgentRuntimeRequestRefreshDiscovery load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRequestRefreshDiscovery(
      discoveryPath: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String discoveryPath;

  AgentRuntimeRequestRefreshDiscovery copyWith({
    String? discoveryPath,
  }) {
    return AgentRuntimeRequestRefreshDiscovery(
      discoveryPath: discoveryPath ?? this.discoveryPath,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(0);
    serializer.serializeString(discoveryPath);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeRequestRefreshDiscovery
      && discoveryPath == other.discoveryPath;
  }

  @override
  int get hashCode => discoveryPath.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'discoveryPath: $discoveryPath'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRequestRefreshDiscovery';
  }
}

@immutable
class AgentRuntimeRequestRefreshIcloudRemoteDiscovery extends AgentRuntimeRequest {
  const AgentRuntimeRequestRefreshIcloudRemoteDiscovery({
    required this.profilePath,
  }) : super();

  static AgentRuntimeRequestRefreshIcloudRemoteDiscovery load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRequestRefreshIcloudRemoteDiscovery(
      profilePath: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String profilePath;

  AgentRuntimeRequestRefreshIcloudRemoteDiscovery copyWith({
    String? profilePath,
  }) {
    return AgentRuntimeRequestRefreshIcloudRemoteDiscovery(
      profilePath: profilePath ?? this.profilePath,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(1);
    serializer.serializeString(profilePath);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeRequestRefreshIcloudRemoteDiscovery
      && profilePath == other.profilePath;
  }

  @override
  int get hashCode => profilePath.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'profilePath: $profilePath'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRequestRefreshIcloudRemoteDiscovery';
  }
}

@immutable
class AgentRuntimeRequestImportRemoteProfileDocument extends AgentRuntimeRequest {
  const AgentRuntimeRequestImportRemoteProfileDocument({
    required this.profilePath,
  }) : super();

  static AgentRuntimeRequestImportRemoteProfileDocument load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRequestImportRemoteProfileDocument(
      profilePath: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String profilePath;

  AgentRuntimeRequestImportRemoteProfileDocument copyWith({
    String? profilePath,
  }) {
    return AgentRuntimeRequestImportRemoteProfileDocument(
      profilePath: profilePath ?? this.profilePath,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(2);
    serializer.serializeString(profilePath);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeRequestImportRemoteProfileDocument
      && profilePath == other.profilePath;
  }

  @override
  int get hashCode => profilePath.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'profilePath: $profilePath'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRequestImportRemoteProfileDocument';
  }
}

@immutable
class AgentRuntimeRequestRefreshImportedRemoteProfile extends AgentRuntimeRequest {
  const AgentRuntimeRequestRefreshImportedRemoteProfile(
  ) : super();

  static AgentRuntimeRequestRefreshImportedRemoteProfile load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRequestRefreshImportedRemoteProfile(
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

    return other is AgentRuntimeRequestRefreshImportedRemoteProfile;
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

    return fullString ?? 'AgentRuntimeRequestRefreshImportedRemoteProfile';
  }
}

@immutable
class AgentRuntimeRequestConnectDiscoveredRuntime extends AgentRuntimeRequest {
  const AgentRuntimeRequestConnectDiscoveredRuntime({
    required this.discoveryPath,
    required this.selectedSessionId,
  }) : super();

  static AgentRuntimeRequestConnectDiscoveredRuntime load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRequestConnectDiscoveredRuntime(
      discoveryPath: deserializer.deserializeString(),
      selectedSessionId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String discoveryPath;
  final String selectedSessionId;

  AgentRuntimeRequestConnectDiscoveredRuntime copyWith({
    String? discoveryPath,
    String? selectedSessionId,
  }) {
    return AgentRuntimeRequestConnectDiscoveredRuntime(
      discoveryPath: discoveryPath ?? this.discoveryPath,
      selectedSessionId: selectedSessionId ?? this.selectedSessionId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(4);
    serializer.serializeString(discoveryPath);
    serializer.serializeString(selectedSessionId);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeRequestConnectDiscoveredRuntime
      && discoveryPath == other.discoveryPath
      && selectedSessionId == other.selectedSessionId;
  }

  @override
  int get hashCode => Object.hash(
        discoveryPath,
        selectedSessionId,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'discoveryPath: $discoveryPath, '
        'selectedSessionId: $selectedSessionId'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRequestConnectDiscoveredRuntime';
  }
}

@immutable
class AgentRuntimeRequestConnectIcloudRemoteRuntime extends AgentRuntimeRequest {
  const AgentRuntimeRequestConnectIcloudRemoteRuntime({
    required this.profilePath,
    required this.selectedSessionId,
  }) : super();

  static AgentRuntimeRequestConnectIcloudRemoteRuntime load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRequestConnectIcloudRemoteRuntime(
      profilePath: deserializer.deserializeString(),
      selectedSessionId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String profilePath;
  final String selectedSessionId;

  AgentRuntimeRequestConnectIcloudRemoteRuntime copyWith({
    String? profilePath,
    String? selectedSessionId,
  }) {
    return AgentRuntimeRequestConnectIcloudRemoteRuntime(
      profilePath: profilePath ?? this.profilePath,
      selectedSessionId: selectedSessionId ?? this.selectedSessionId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(5);
    serializer.serializeString(profilePath);
    serializer.serializeString(selectedSessionId);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeRequestConnectIcloudRemoteRuntime
      && profilePath == other.profilePath
      && selectedSessionId == other.selectedSessionId;
  }

  @override
  int get hashCode => Object.hash(
        profilePath,
        selectedSessionId,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'profilePath: $profilePath, '
        'selectedSessionId: $selectedSessionId'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRequestConnectIcloudRemoteRuntime';
  }
}

@immutable
class AgentRuntimeRequestConnectImportedRemoteRuntime extends AgentRuntimeRequest {
  const AgentRuntimeRequestConnectImportedRemoteRuntime({
    required this.selectedSessionId,
  }) : super();

  static AgentRuntimeRequestConnectImportedRemoteRuntime load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRequestConnectImportedRemoteRuntime(
      selectedSessionId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String selectedSessionId;

  AgentRuntimeRequestConnectImportedRemoteRuntime copyWith({
    String? selectedSessionId,
  }) {
    return AgentRuntimeRequestConnectImportedRemoteRuntime(
      selectedSessionId: selectedSessionId ?? this.selectedSessionId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(6);
    serializer.serializeString(selectedSessionId);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeRequestConnectImportedRemoteRuntime
      && selectedSessionId == other.selectedSessionId;
  }

  @override
  int get hashCode => selectedSessionId.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'selectedSessionId: $selectedSessionId'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRequestConnectImportedRemoteRuntime';
  }
}

@immutable
class AgentRuntimeRequestConnect extends AgentRuntimeRequest {
  const AgentRuntimeRequestConnect({
    required this.baseUrl,
    required this.selectedSessionId,
  }) : super();

  static AgentRuntimeRequestConnect load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRequestConnect(
      baseUrl: deserializer.deserializeString(),
      selectedSessionId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String baseUrl;
  final String selectedSessionId;

  AgentRuntimeRequestConnect copyWith({
    String? baseUrl,
    String? selectedSessionId,
  }) {
    return AgentRuntimeRequestConnect(
      baseUrl: baseUrl ?? this.baseUrl,
      selectedSessionId: selectedSessionId ?? this.selectedSessionId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(7);
    serializer.serializeString(baseUrl);
    serializer.serializeString(selectedSessionId);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeRequestConnect
      && baseUrl == other.baseUrl
      && selectedSessionId == other.selectedSessionId;
  }

  @override
  int get hashCode => Object.hash(
        baseUrl,
        selectedSessionId,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'baseUrl: $baseUrl, '
        'selectedSessionId: $selectedSessionId'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRequestConnect';
  }
}

@immutable
class AgentRuntimeRequestSelectProject extends AgentRuntimeRequest {
  const AgentRuntimeRequestSelectProject({
    required this.projectId,
  }) : super();

  static AgentRuntimeRequestSelectProject load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRequestSelectProject(
      projectId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String projectId;

  AgentRuntimeRequestSelectProject copyWith({
    String? projectId,
  }) {
    return AgentRuntimeRequestSelectProject(
      projectId: projectId ?? this.projectId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(8);
    serializer.serializeString(projectId);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeRequestSelectProject
      && projectId == other.projectId;
  }

  @override
  int get hashCode => projectId.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'projectId: $projectId'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRequestSelectProject';
  }
}

@immutable
class AgentRuntimeRequestHydrate extends AgentRuntimeRequest {
  const AgentRuntimeRequestHydrate({
    required this.selectedSessionId,
  }) : super();

  static AgentRuntimeRequestHydrate load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRequestHydrate(
      selectedSessionId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String selectedSessionId;

  AgentRuntimeRequestHydrate copyWith({
    String? selectedSessionId,
  }) {
    return AgentRuntimeRequestHydrate(
      selectedSessionId: selectedSessionId ?? this.selectedSessionId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(9);
    serializer.serializeString(selectedSessionId);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeRequestHydrate
      && selectedSessionId == other.selectedSessionId;
  }

  @override
  int get hashCode => selectedSessionId.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'selectedSessionId: $selectedSessionId'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRequestHydrate';
  }
}

@immutable
class AgentRuntimeRequestRehydrate extends AgentRuntimeRequest {
  const AgentRuntimeRequestRehydrate({
    required this.selectedSessionId,
  }) : super();

  static AgentRuntimeRequestRehydrate load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRequestRehydrate(
      selectedSessionId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String selectedSessionId;

  AgentRuntimeRequestRehydrate copyWith({
    String? selectedSessionId,
  }) {
    return AgentRuntimeRequestRehydrate(
      selectedSessionId: selectedSessionId ?? this.selectedSessionId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(10);
    serializer.serializeString(selectedSessionId);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeRequestRehydrate
      && selectedSessionId == other.selectedSessionId;
  }

  @override
  int get hashCode => selectedSessionId.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'selectedSessionId: $selectedSessionId'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRequestRehydrate';
  }
}

@immutable
class AgentRuntimeRequestDisconnect extends AgentRuntimeRequest {
  const AgentRuntimeRequestDisconnect(
  ) : super();

  static AgentRuntimeRequestDisconnect load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRequestDisconnect(
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(11);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeRequestDisconnect;
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

    return fullString ?? 'AgentRuntimeRequestDisconnect';
  }
}

@immutable
class AgentRuntimeRequestDispatchOperation extends AgentRuntimeRequest {
  const AgentRuntimeRequestDispatchOperation({
    required this.operation,
  }) : super();

  static AgentRuntimeRequestDispatchOperation load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRequestDispatchOperation(
      operation: AgentRuntimeGuiOperation.deserialize(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final AgentRuntimeGuiOperation operation;

  AgentRuntimeRequestDispatchOperation copyWith({
    AgentRuntimeGuiOperation? operation,
  }) {
    return AgentRuntimeRequestDispatchOperation(
      operation: operation ?? this.operation,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(12);
    operation.serialize(serializer);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeRequestDispatchOperation
      && operation == other.operation;
  }

  @override
  int get hashCode => operation.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'operation: $operation'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRequestDispatchOperation';
  }
}
