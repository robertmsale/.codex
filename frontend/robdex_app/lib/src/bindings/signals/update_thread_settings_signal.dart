// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class UpdateThreadSettingsSignal {
  const UpdateThreadSettingsSignal({
    required this.role,
    required this.approvalPolicy,
    required this.sandboxMode,
    required this.networkAccessMode,
    required this.modelId,
    required this.reasoningEffort,
    required this.serviceTier,
  });

  static UpdateThreadSettingsSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = UpdateThreadSettingsSignal(
      role: deserializer.deserializeString(),
      approvalPolicy: deserializer.deserializeString(),
      sandboxMode: deserializer.deserializeString(),
      networkAccessMode: deserializer.deserializeString(),
      modelId: deserializer.deserializeString(),
      reasoningEffort: deserializer.deserializeString(),
      serviceTier: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static UpdateThreadSettingsSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = UpdateThreadSettingsSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String role;
  final String approvalPolicy;
  final String sandboxMode;
  final String networkAccessMode;
  final String modelId;
  final String reasoningEffort;
  final String serviceTier;

  UpdateThreadSettingsSignal copyWith({
    String? role,
    String? approvalPolicy,
    String? sandboxMode,
    String? networkAccessMode,
    String? modelId,
    String? reasoningEffort,
    String? serviceTier,
  }) {
    return UpdateThreadSettingsSignal(
      role: role ?? this.role,
      approvalPolicy: approvalPolicy ?? this.approvalPolicy,
      sandboxMode: sandboxMode ?? this.sandboxMode,
      networkAccessMode: networkAccessMode ?? this.networkAccessMode,
      modelId: modelId ?? this.modelId,
      reasoningEffort: reasoningEffort ?? this.reasoningEffort,
      serviceTier: serviceTier ?? this.serviceTier,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(role);
    serializer.serializeString(approvalPolicy);
    serializer.serializeString(sandboxMode);
    serializer.serializeString(networkAccessMode);
    serializer.serializeString(modelId);
    serializer.serializeString(reasoningEffort);
    serializer.serializeString(serviceTier);
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

    return other is UpdateThreadSettingsSignal
      && role == other.role
      && approvalPolicy == other.approvalPolicy
      && sandboxMode == other.sandboxMode
      && networkAccessMode == other.networkAccessMode
      && modelId == other.modelId
      && reasoningEffort == other.reasoningEffort
      && serviceTier == other.serviceTier;
  }

  @override
  int get hashCode => Object.hash(
        role,
        approvalPolicy,
        sandboxMode,
        networkAccessMode,
        modelId,
        reasoningEffort,
        serviceTier,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'role: $role, '
        'approvalPolicy: $approvalPolicy, '
        'sandboxMode: $sandboxMode, '
        'networkAccessMode: $networkAccessMode, '
        'modelId: $modelId, '
        'reasoningEffort: $reasoningEffort, '
        'serviceTier: $serviceTier'
        ')';
      return true;
    }());

    return fullString ?? 'UpdateThreadSettingsSignal';
  }
}

extension UpdateThreadSettingsSignalDartSignalExt on UpdateThreadSettingsSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_update_thread_settings_signal',
      messageBytes,
      binary,
    );
  }
}
