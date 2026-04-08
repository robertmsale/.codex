// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class SpawnAgentSignal {
  const SpawnAgentSignal({
    required this.name,
    required this.role,
    required this.prompt,
  });

  static SpawnAgentSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = SpawnAgentSignal(
      name: deserializer.deserializeString(),
      role: deserializer.deserializeString(),
      prompt: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static SpawnAgentSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = SpawnAgentSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String name;
  final String role;
  final String prompt;

  SpawnAgentSignal copyWith({
    String? name,
    String? role,
    String? prompt,
  }) {
    return SpawnAgentSignal(
      name: name ?? this.name,
      role: role ?? this.role,
      prompt: prompt ?? this.prompt,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(name);
    serializer.serializeString(role);
    serializer.serializeString(prompt);
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

    return other is SpawnAgentSignal
      && name == other.name
      && role == other.role
      && prompt == other.prompt;
  }

  @override
  int get hashCode => Object.hash(
        name,
        role,
        prompt,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'name: $name, '
        'role: $role, '
        'prompt: $prompt'
        ')';
      return true;
    }());

    return fullString ?? 'SpawnAgentSignal';
  }
}

extension SpawnAgentSignalDartSignalExt on SpawnAgentSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_spawn_agent_signal',
      messageBytes,
      binary,
    );
  }
}
