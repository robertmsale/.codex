// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeControllerState {
  const AgentRuntimeControllerState({
    required this.connectionState,
    required this.selectedSessionId,
    required this.hasSelectedSessionId,
    required this.baseUrl,
    required this.lastError,
    required this.hasLastError,
  });

  static AgentRuntimeControllerState deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeControllerState(
      connectionState: deserializer.deserializeString(),
      selectedSessionId: deserializer.deserializeString(),
      hasSelectedSessionId: deserializer.deserializeBool(),
      baseUrl: deserializer.deserializeString(),
      lastError: deserializer.deserializeString(),
      hasLastError: deserializer.deserializeBool(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeControllerState bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeControllerState.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String connectionState;
  final String selectedSessionId;
  final bool hasSelectedSessionId;
  final String baseUrl;
  final String lastError;
  final bool hasLastError;

  AgentRuntimeControllerState copyWith({
    String? connectionState,
    String? selectedSessionId,
    bool? hasSelectedSessionId,
    String? baseUrl,
    String? lastError,
    bool? hasLastError,
  }) {
    return AgentRuntimeControllerState(
      connectionState: connectionState ?? this.connectionState,
      selectedSessionId: selectedSessionId ?? this.selectedSessionId,
      hasSelectedSessionId: hasSelectedSessionId ?? this.hasSelectedSessionId,
      baseUrl: baseUrl ?? this.baseUrl,
      lastError: lastError ?? this.lastError,
      hasLastError: hasLastError ?? this.hasLastError,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(connectionState);
    serializer.serializeString(selectedSessionId);
    serializer.serializeBool(hasSelectedSessionId);
    serializer.serializeString(baseUrl);
    serializer.serializeString(lastError);
    serializer.serializeBool(hasLastError);
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

    return other is AgentRuntimeControllerState
      && connectionState == other.connectionState
      && selectedSessionId == other.selectedSessionId
      && hasSelectedSessionId == other.hasSelectedSessionId
      && baseUrl == other.baseUrl
      && lastError == other.lastError
      && hasLastError == other.hasLastError;
  }

  @override
  int get hashCode => Object.hash(
        connectionState,
        selectedSessionId,
        hasSelectedSessionId,
        baseUrl,
        lastError,
        hasLastError,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'connectionState: $connectionState, '
        'selectedSessionId: $selectedSessionId, '
        'hasSelectedSessionId: $hasSelectedSessionId, '
        'baseUrl: $baseUrl, '
        'lastError: $lastError, '
        'hasLastError: $hasLastError'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeControllerState';
  }
}
