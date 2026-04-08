// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class CreateThreadSignal {
  const CreateThreadSignal({
    required this.projectId,
    required this.title,
    required this.initialPrompt,
    required this.role,
    required this.approvalPolicy,
    required this.sandboxMode,
    required this.networkAccessMode,
    required this.modelId,
    required this.reasoningEffort,
  });

  static CreateThreadSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = CreateThreadSignal(
      projectId: deserializer.deserializeString(),
      title: deserializer.deserializeString(),
      initialPrompt: deserializer.deserializeString(),
      role: deserializer.deserializeString(),
      approvalPolicy: deserializer.deserializeString(),
      sandboxMode: deserializer.deserializeString(),
      networkAccessMode: deserializer.deserializeString(),
      modelId: deserializer.deserializeString(),
      reasoningEffort: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static CreateThreadSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = CreateThreadSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String projectId;
  final String title;
  final String initialPrompt;
  final String role;
  final String approvalPolicy;
  final String sandboxMode;
  final String networkAccessMode;
  final String modelId;
  final String reasoningEffort;

  CreateThreadSignal copyWith({
    String? projectId,
    String? title,
    String? initialPrompt,
    String? role,
    String? approvalPolicy,
    String? sandboxMode,
    String? networkAccessMode,
    String? modelId,
    String? reasoningEffort,
  }) {
    return CreateThreadSignal(
      projectId: projectId ?? this.projectId,
      title: title ?? this.title,
      initialPrompt: initialPrompt ?? this.initialPrompt,
      role: role ?? this.role,
      approvalPolicy: approvalPolicy ?? this.approvalPolicy,
      sandboxMode: sandboxMode ?? this.sandboxMode,
      networkAccessMode: networkAccessMode ?? this.networkAccessMode,
      modelId: modelId ?? this.modelId,
      reasoningEffort: reasoningEffort ?? this.reasoningEffort,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(projectId);
    serializer.serializeString(title);
    serializer.serializeString(initialPrompt);
    serializer.serializeString(role);
    serializer.serializeString(approvalPolicy);
    serializer.serializeString(sandboxMode);
    serializer.serializeString(networkAccessMode);
    serializer.serializeString(modelId);
    serializer.serializeString(reasoningEffort);
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

    return other is CreateThreadSignal
      && projectId == other.projectId
      && title == other.title
      && initialPrompt == other.initialPrompt
      && role == other.role
      && approvalPolicy == other.approvalPolicy
      && sandboxMode == other.sandboxMode
      && networkAccessMode == other.networkAccessMode
      && modelId == other.modelId
      && reasoningEffort == other.reasoningEffort;
  }

  @override
  int get hashCode => Object.hash(
        projectId,
        title,
        initialPrompt,
        role,
        approvalPolicy,
        sandboxMode,
        networkAccessMode,
        modelId,
        reasoningEffort,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'projectId: $projectId, '
        'title: $title, '
        'initialPrompt: $initialPrompt, '
        'role: $role, '
        'approvalPolicy: $approvalPolicy, '
        'sandboxMode: $sandboxMode, '
        'networkAccessMode: $networkAccessMode, '
        'modelId: $modelId, '
        'reasoningEffort: $reasoningEffort'
        ')';
      return true;
    }());

    return fullString ?? 'CreateThreadSignal';
  }
}

extension CreateThreadSignalDartSignalExt on CreateThreadSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_create_thread_signal',
      messageBytes,
      binary,
    );
  }
}
