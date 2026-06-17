// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeDiscoveryView {
  const AgentRuntimeDiscoveryView({
    required this.sourceType,
    required this.sourcePath,
    required this.state,
    required this.tone,
    required this.title,
    required this.message,
    required this.baseUrl,
    required this.hasBaseUrl,
    required this.healthUrl,
    required this.hasHealthUrl,
    required this.webSocketUrl,
    required this.hasWebSocketUrl,
    required this.runtimeIdentity,
    required this.hasRuntimeIdentity,
    required this.discoveryPath,
    required this.lastImportedAt,
    required this.hasLastImportedAt,
    required this.serviceState,
    required this.hasServiceState,
    required this.connectable,
    required this.diagnostics,
  });

  static AgentRuntimeDiscoveryView deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeDiscoveryView(
      sourceType: deserializer.deserializeString(),
      sourcePath: deserializer.deserializeString(),
      state: deserializer.deserializeString(),
      tone: deserializer.deserializeString(),
      title: deserializer.deserializeString(),
      message: deserializer.deserializeString(),
      baseUrl: deserializer.deserializeString(),
      hasBaseUrl: deserializer.deserializeBool(),
      healthUrl: deserializer.deserializeString(),
      hasHealthUrl: deserializer.deserializeBool(),
      webSocketUrl: deserializer.deserializeString(),
      hasWebSocketUrl: deserializer.deserializeBool(),
      runtimeIdentity: deserializer.deserializeString(),
      hasRuntimeIdentity: deserializer.deserializeBool(),
      discoveryPath: deserializer.deserializeString(),
      lastImportedAt: deserializer.deserializeString(),
      hasLastImportedAt: deserializer.deserializeBool(),
      serviceState: deserializer.deserializeString(),
      hasServiceState: deserializer.deserializeBool(),
      connectable: deserializer.deserializeBool(),
      diagnostics: TraitHelpers.deserializeVectorStr(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeDiscoveryView bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeDiscoveryView.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String sourceType;
  final String sourcePath;
  final String state;
  final String tone;
  final String title;
  final String message;
  final String baseUrl;
  final bool hasBaseUrl;
  final String healthUrl;
  final bool hasHealthUrl;
  final String webSocketUrl;
  final bool hasWebSocketUrl;
  final String runtimeIdentity;
  final bool hasRuntimeIdentity;
  final String discoveryPath;
  final String lastImportedAt;
  final bool hasLastImportedAt;
  final String serviceState;
  final bool hasServiceState;
  final bool connectable;
  final List<String> diagnostics;

  AgentRuntimeDiscoveryView copyWith({
    String? sourceType,
    String? sourcePath,
    String? state,
    String? tone,
    String? title,
    String? message,
    String? baseUrl,
    bool? hasBaseUrl,
    String? healthUrl,
    bool? hasHealthUrl,
    String? webSocketUrl,
    bool? hasWebSocketUrl,
    String? runtimeIdentity,
    bool? hasRuntimeIdentity,
    String? discoveryPath,
    String? lastImportedAt,
    bool? hasLastImportedAt,
    String? serviceState,
    bool? hasServiceState,
    bool? connectable,
    List<String>? diagnostics,
  }) {
    return AgentRuntimeDiscoveryView(
      sourceType: sourceType ?? this.sourceType,
      sourcePath: sourcePath ?? this.sourcePath,
      state: state ?? this.state,
      tone: tone ?? this.tone,
      title: title ?? this.title,
      message: message ?? this.message,
      baseUrl: baseUrl ?? this.baseUrl,
      hasBaseUrl: hasBaseUrl ?? this.hasBaseUrl,
      healthUrl: healthUrl ?? this.healthUrl,
      hasHealthUrl: hasHealthUrl ?? this.hasHealthUrl,
      webSocketUrl: webSocketUrl ?? this.webSocketUrl,
      hasWebSocketUrl: hasWebSocketUrl ?? this.hasWebSocketUrl,
      runtimeIdentity: runtimeIdentity ?? this.runtimeIdentity,
      hasRuntimeIdentity: hasRuntimeIdentity ?? this.hasRuntimeIdentity,
      discoveryPath: discoveryPath ?? this.discoveryPath,
      lastImportedAt: lastImportedAt ?? this.lastImportedAt,
      hasLastImportedAt: hasLastImportedAt ?? this.hasLastImportedAt,
      serviceState: serviceState ?? this.serviceState,
      hasServiceState: hasServiceState ?? this.hasServiceState,
      connectable: connectable ?? this.connectable,
      diagnostics: diagnostics ?? this.diagnostics,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(sourceType);
    serializer.serializeString(sourcePath);
    serializer.serializeString(state);
    serializer.serializeString(tone);
    serializer.serializeString(title);
    serializer.serializeString(message);
    serializer.serializeString(baseUrl);
    serializer.serializeBool(hasBaseUrl);
    serializer.serializeString(healthUrl);
    serializer.serializeBool(hasHealthUrl);
    serializer.serializeString(webSocketUrl);
    serializer.serializeBool(hasWebSocketUrl);
    serializer.serializeString(runtimeIdentity);
    serializer.serializeBool(hasRuntimeIdentity);
    serializer.serializeString(discoveryPath);
    serializer.serializeString(lastImportedAt);
    serializer.serializeBool(hasLastImportedAt);
    serializer.serializeString(serviceState);
    serializer.serializeBool(hasServiceState);
    serializer.serializeBool(connectable);
    TraitHelpers.serializeVectorStr(diagnostics, serializer);
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

    return other is AgentRuntimeDiscoveryView
      && sourceType == other.sourceType
      && sourcePath == other.sourcePath
      && state == other.state
      && tone == other.tone
      && title == other.title
      && message == other.message
      && baseUrl == other.baseUrl
      && hasBaseUrl == other.hasBaseUrl
      && healthUrl == other.healthUrl
      && hasHealthUrl == other.hasHealthUrl
      && webSocketUrl == other.webSocketUrl
      && hasWebSocketUrl == other.hasWebSocketUrl
      && runtimeIdentity == other.runtimeIdentity
      && hasRuntimeIdentity == other.hasRuntimeIdentity
      && discoveryPath == other.discoveryPath
      && lastImportedAt == other.lastImportedAt
      && hasLastImportedAt == other.hasLastImportedAt
      && serviceState == other.serviceState
      && hasServiceState == other.hasServiceState
      && connectable == other.connectable
      && listEquals(diagnostics, other.diagnostics);
  }

  @override
  int get hashCode => Object.hashAll([
        sourceType,
        sourcePath,
        state,
        tone,
        title,
        message,
        baseUrl,
        hasBaseUrl,
        healthUrl,
        hasHealthUrl,
        webSocketUrl,
        hasWebSocketUrl,
        runtimeIdentity,
        hasRuntimeIdentity,
        discoveryPath,
        lastImportedAt,
        hasLastImportedAt,
        serviceState,
        hasServiceState,
        connectable,
        diagnostics,
      ]);

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'sourceType: $sourceType, '
        'sourcePath: $sourcePath, '
        'state: $state, '
        'tone: $tone, '
        'title: $title, '
        'message: $message, '
        'baseUrl: $baseUrl, '
        'hasBaseUrl: $hasBaseUrl, '
        'healthUrl: $healthUrl, '
        'hasHealthUrl: $hasHealthUrl, '
        'webSocketUrl: $webSocketUrl, '
        'hasWebSocketUrl: $hasWebSocketUrl, '
        'runtimeIdentity: $runtimeIdentity, '
        'hasRuntimeIdentity: $hasRuntimeIdentity, '
        'discoveryPath: $discoveryPath, '
        'lastImportedAt: $lastImportedAt, '
        'hasLastImportedAt: $hasLastImportedAt, '
        'serviceState: $serviceState, '
        'hasServiceState: $hasServiceState, '
        'connectable: $connectable, '
        'diagnostics: $diagnostics'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeDiscoveryView';
  }
}
