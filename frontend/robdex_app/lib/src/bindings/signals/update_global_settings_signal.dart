// ignore_for_file: type=lint, type=warning
part of 'signals.dart';

@immutable
class UpdateGlobalSettingsSignal {
  const UpdateGlobalSettingsSignal({
    required this.approvalPolicy,
    required this.sandboxMode,
    required this.networkAccessMode,
  });

  final String approvalPolicy;
  final String sandboxMode;
  final String networkAccessMode;

  static UpdateGlobalSettingsSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = UpdateGlobalSettingsSignal(
      approvalPolicy: deserializer.deserializeString(),
      sandboxMode: deserializer.deserializeString(),
      networkAccessMode: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static UpdateGlobalSettingsSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = UpdateGlobalSettingsSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(approvalPolicy);
    serializer.serializeString(sandboxMode);
    serializer.serializeString(networkAccessMode);
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
    return other is UpdateGlobalSettingsSignal &&
        approvalPolicy == other.approvalPolicy &&
        sandboxMode == other.sandboxMode &&
        networkAccessMode == other.networkAccessMode;
  }

  @override
  int get hashCode => Object.hash(approvalPolicy, sandboxMode, networkAccessMode);
}

extension UpdateGlobalSettingsSignalDartSignalExt on UpdateGlobalSettingsSignal {
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_update_global_settings_signal',
      messageBytes,
      binary,
    );
  }
}
