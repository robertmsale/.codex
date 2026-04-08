// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class CreateProjectSignal {
  const CreateProjectSignal({
    required this.name,
    required this.rootPath,
    required this.defaultCwd,
  });

  static CreateProjectSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = CreateProjectSignal(
      name: deserializer.deserializeString(),
      rootPath: deserializer.deserializeString(),
      defaultCwd: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static CreateProjectSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = CreateProjectSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String name;
  final String rootPath;
  final String defaultCwd;

  CreateProjectSignal copyWith({
    String? name,
    String? rootPath,
    String? defaultCwd,
  }) {
    return CreateProjectSignal(
      name: name ?? this.name,
      rootPath: rootPath ?? this.rootPath,
      defaultCwd: defaultCwd ?? this.defaultCwd,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(name);
    serializer.serializeString(rootPath);
    serializer.serializeString(defaultCwd);
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

    return other is CreateProjectSignal
      && name == other.name
      && rootPath == other.rootPath
      && defaultCwd == other.defaultCwd;
  }

  @override
  int get hashCode => Object.hash(
        name,
        rootPath,
        defaultCwd,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'name: $name, '
        'rootPath: $rootPath, '
        'defaultCwd: $defaultCwd'
        ')';
      return true;
    }());

    return fullString ?? 'CreateProjectSignal';
  }
}

extension CreateProjectSignalDartSignalExt on CreateProjectSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_create_project_signal',
      messageBytes,
      binary,
    );
  }
}
