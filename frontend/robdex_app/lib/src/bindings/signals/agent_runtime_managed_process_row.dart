// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeManagedProcessRow {
  const AgentRuntimeManagedProcessRow({
    required this.id,
    required this.handle,
    required this.command,
    required this.status,
    required this.startedAt,
    required this.endedAt,
    required this.cwd,
    required this.pid,
    required this.stdinPolicy,
    required this.endOfTurnBehavior,
    required this.endOfSessionBehavior,
    required this.latestOutputSummary,
    required this.canTerminate,
    required this.canFlush,
    required this.canInput,
  });

  static AgentRuntimeManagedProcessRow deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeManagedProcessRow(
      id: deserializer.deserializeString(),
      handle: deserializer.deserializeString(),
      command: deserializer.deserializeString(),
      status: deserializer.deserializeString(),
      startedAt: deserializer.deserializeString(),
      endedAt: deserializer.deserializeString(),
      cwd: deserializer.deserializeString(),
      pid: deserializer.deserializeString(),
      stdinPolicy: deserializer.deserializeString(),
      endOfTurnBehavior: deserializer.deserializeString(),
      endOfSessionBehavior: deserializer.deserializeString(),
      latestOutputSummary: deserializer.deserializeString(),
      canTerminate: deserializer.deserializeBool(),
      canFlush: deserializer.deserializeBool(),
      canInput: deserializer.deserializeBool(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeManagedProcessRow bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeManagedProcessRow.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String id;
  final String handle;
  final String command;
  final String status;
  final String startedAt;
  final String endedAt;
  final String cwd;
  final String pid;
  final String stdinPolicy;
  final String endOfTurnBehavior;
  final String endOfSessionBehavior;
  final String latestOutputSummary;
  final bool canTerminate;
  final bool canFlush;
  final bool canInput;

  AgentRuntimeManagedProcessRow copyWith({
    String? id,
    String? handle,
    String? command,
    String? status,
    String? startedAt,
    String? endedAt,
    String? cwd,
    String? pid,
    String? stdinPolicy,
    String? endOfTurnBehavior,
    String? endOfSessionBehavior,
    String? latestOutputSummary,
    bool? canTerminate,
    bool? canFlush,
    bool? canInput,
  }) {
    return AgentRuntimeManagedProcessRow(
      id: id ?? this.id,
      handle: handle ?? this.handle,
      command: command ?? this.command,
      status: status ?? this.status,
      startedAt: startedAt ?? this.startedAt,
      endedAt: endedAt ?? this.endedAt,
      cwd: cwd ?? this.cwd,
      pid: pid ?? this.pid,
      stdinPolicy: stdinPolicy ?? this.stdinPolicy,
      endOfTurnBehavior: endOfTurnBehavior ?? this.endOfTurnBehavior,
      endOfSessionBehavior: endOfSessionBehavior ?? this.endOfSessionBehavior,
      latestOutputSummary: latestOutputSummary ?? this.latestOutputSummary,
      canTerminate: canTerminate ?? this.canTerminate,
      canFlush: canFlush ?? this.canFlush,
      canInput: canInput ?? this.canInput,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(id);
    serializer.serializeString(handle);
    serializer.serializeString(command);
    serializer.serializeString(status);
    serializer.serializeString(startedAt);
    serializer.serializeString(endedAt);
    serializer.serializeString(cwd);
    serializer.serializeString(pid);
    serializer.serializeString(stdinPolicy);
    serializer.serializeString(endOfTurnBehavior);
    serializer.serializeString(endOfSessionBehavior);
    serializer.serializeString(latestOutputSummary);
    serializer.serializeBool(canTerminate);
    serializer.serializeBool(canFlush);
    serializer.serializeBool(canInput);
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

    return other is AgentRuntimeManagedProcessRow
      && id == other.id
      && handle == other.handle
      && command == other.command
      && status == other.status
      && startedAt == other.startedAt
      && endedAt == other.endedAt
      && cwd == other.cwd
      && pid == other.pid
      && stdinPolicy == other.stdinPolicy
      && endOfTurnBehavior == other.endOfTurnBehavior
      && endOfSessionBehavior == other.endOfSessionBehavior
      && latestOutputSummary == other.latestOutputSummary
      && canTerminate == other.canTerminate
      && canFlush == other.canFlush
      && canInput == other.canInput;
  }

  @override
  int get hashCode => Object.hash(
        id,
        handle,
        command,
        status,
        startedAt,
        endedAt,
        cwd,
        pid,
        stdinPolicy,
        endOfTurnBehavior,
        endOfSessionBehavior,
        latestOutputSummary,
        canTerminate,
        canFlush,
        canInput,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'id: $id, '
        'handle: $handle, '
        'command: $command, '
        'status: $status, '
        'startedAt: $startedAt, '
        'endedAt: $endedAt, '
        'cwd: $cwd, '
        'pid: $pid, '
        'stdinPolicy: $stdinPolicy, '
        'endOfTurnBehavior: $endOfTurnBehavior, '
        'endOfSessionBehavior: $endOfSessionBehavior, '
        'latestOutputSummary: $latestOutputSummary, '
        'canTerminate: $canTerminate, '
        'canFlush: $canFlush, '
        'canInput: $canInput'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeManagedProcessRow';
  }
}
