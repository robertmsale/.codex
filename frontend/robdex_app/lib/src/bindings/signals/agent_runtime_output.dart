// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


abstract class AgentRuntimeOutput {
  const AgentRuntimeOutput();

  void serialize(BinarySerializer serializer);

  static AgentRuntimeOutput deserialize(BinaryDeserializer deserializer) {
    int index = deserializer.deserializeVariantIndex();
    switch (index) {
      case 0: return AgentRuntimeOutputProjectionSnapshot.load(deserializer);
      case 1: return AgentRuntimeOutputControllerState.load(deserializer);
      case 2: return AgentRuntimeOutputOperationResult.load(deserializer);
      case 3: return AgentRuntimeOutputStreamOutcome.load(deserializer);
      case 4: return AgentRuntimeOutputError.load(deserializer);
      case 5: return AgentRuntimeOutputWorkbenchView.load(deserializer);
      default: throw Exception('Unknown variant index for AgentRuntimeOutput: ' + index.toString());
    }
  }

  Uint8List bincodeSerialize() {
      final serializer = BincodeSerializer();
      serialize(serializer);
      return serializer.bytes;
  }

  static AgentRuntimeOutput bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeOutput.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }
}


@immutable
class AgentRuntimeOutputProjectionSnapshot extends AgentRuntimeOutput {
  const AgentRuntimeOutputProjectionSnapshot({
    required this.projection,
  }) : super();

  static AgentRuntimeOutputProjectionSnapshot load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeOutputProjectionSnapshot(
      projection: AgentRuntimeProjectionSnapshot.deserialize(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final AgentRuntimeProjectionSnapshot projection;

  AgentRuntimeOutputProjectionSnapshot copyWith({
    AgentRuntimeProjectionSnapshot? projection,
  }) {
    return AgentRuntimeOutputProjectionSnapshot(
      projection: projection ?? this.projection,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(0);
    projection.serialize(serializer);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeOutputProjectionSnapshot
      && projection == other.projection;
  }

  @override
  int get hashCode => projection.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'projection: $projection'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeOutputProjectionSnapshot';
  }
}

@immutable
class AgentRuntimeOutputControllerState extends AgentRuntimeOutput {
  const AgentRuntimeOutputControllerState({
    required this.controllerState,
  }) : super();

  static AgentRuntimeOutputControllerState load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeOutputControllerState(
      controllerState: AgentRuntimeControllerState.deserialize(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final AgentRuntimeControllerState controllerState;

  AgentRuntimeOutputControllerState copyWith({
    AgentRuntimeControllerState? controllerState,
  }) {
    return AgentRuntimeOutputControllerState(
      controllerState: controllerState ?? this.controllerState,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(1);
    controllerState.serialize(serializer);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeOutputControllerState
      && controllerState == other.controllerState;
  }

  @override
  int get hashCode => controllerState.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'controllerState: $controllerState'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeOutputControllerState';
  }
}

@immutable
class AgentRuntimeOutputOperationResult extends AgentRuntimeOutput {
  const AgentRuntimeOutputOperationResult({
    required this.result,
  }) : super();

  static AgentRuntimeOutputOperationResult load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeOutputOperationResult(
      result: AgentRuntimeOperationResult.deserialize(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final AgentRuntimeOperationResult result;

  AgentRuntimeOutputOperationResult copyWith({
    AgentRuntimeOperationResult? result,
  }) {
    return AgentRuntimeOutputOperationResult(
      result: result ?? this.result,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(2);
    result.serialize(serializer);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeOutputOperationResult
      && result == other.result;
  }

  @override
  int get hashCode => result.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'result: $result'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeOutputOperationResult';
  }
}

@immutable
class AgentRuntimeOutputStreamOutcome extends AgentRuntimeOutput {
  const AgentRuntimeOutputStreamOutcome({
    required this.outcome,
    required this.projection,
    required this.hasProjection,
    required this.controllerState,
  }) : super();

  static AgentRuntimeOutputStreamOutcome load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeOutputStreamOutcome(
      outcome: AgentRuntimeStreamOutcome.deserialize(deserializer),
      projection: AgentRuntimeProjectionSnapshot.deserialize(deserializer),
      hasProjection: deserializer.deserializeBool(),
      controllerState: AgentRuntimeControllerState.deserialize(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final AgentRuntimeStreamOutcome outcome;
  final AgentRuntimeProjectionSnapshot projection;
  final bool hasProjection;
  final AgentRuntimeControllerState controllerState;

  AgentRuntimeOutputStreamOutcome copyWith({
    AgentRuntimeStreamOutcome? outcome,
    AgentRuntimeProjectionSnapshot? projection,
    bool? hasProjection,
    AgentRuntimeControllerState? controllerState,
  }) {
    return AgentRuntimeOutputStreamOutcome(
      outcome: outcome ?? this.outcome,
      projection: projection ?? this.projection,
      hasProjection: hasProjection ?? this.hasProjection,
      controllerState: controllerState ?? this.controllerState,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(3);
    outcome.serialize(serializer);
    projection.serialize(serializer);
    serializer.serializeBool(hasProjection);
    controllerState.serialize(serializer);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeOutputStreamOutcome
      && outcome == other.outcome
      && projection == other.projection
      && hasProjection == other.hasProjection
      && controllerState == other.controllerState;
  }

  @override
  int get hashCode => Object.hash(
        outcome,
        projection,
        hasProjection,
        controllerState,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'outcome: $outcome, '
        'projection: $projection, '
        'hasProjection: $hasProjection, '
        'controllerState: $controllerState'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeOutputStreamOutcome';
  }
}

@immutable
class AgentRuntimeOutputError extends AgentRuntimeOutput {
  const AgentRuntimeOutputError({
    required this.error,
  }) : super();

  static AgentRuntimeOutputError load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeOutputError(
      error: AgentRuntimeApiError.deserialize(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final AgentRuntimeApiError error;

  AgentRuntimeOutputError copyWith({
    AgentRuntimeApiError? error,
  }) {
    return AgentRuntimeOutputError(
      error: error ?? this.error,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(4);
    error.serialize(serializer);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeOutputError
      && error == other.error;
  }

  @override
  int get hashCode => error.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'error: $error'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeOutputError';
  }
}

@immutable
class AgentRuntimeOutputWorkbenchView extends AgentRuntimeOutput {
  const AgentRuntimeOutputWorkbenchView({
    required this.viewModel,
  }) : super();

  static AgentRuntimeOutputWorkbenchView load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeOutputWorkbenchView(
      viewModel: AgentRuntimeWorkbenchViewModel.deserialize(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final AgentRuntimeWorkbenchViewModel viewModel;

  AgentRuntimeOutputWorkbenchView copyWith({
    AgentRuntimeWorkbenchViewModel? viewModel,
  }) {
    return AgentRuntimeOutputWorkbenchView(
      viewModel: viewModel ?? this.viewModel,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(5);
    viewModel.serialize(serializer);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeOutputWorkbenchView
      && viewModel == other.viewModel;
  }

  @override
  int get hashCode => viewModel.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'viewModel: $viewModel'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeOutputWorkbenchView';
  }
}
