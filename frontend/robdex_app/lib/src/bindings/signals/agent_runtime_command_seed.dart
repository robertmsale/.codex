// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeCommandSeed {
  const AgentRuntimeCommandSeed({
    required this.actionId,
    required this.binaryName,
    required this.candidatePaths,
    required this.starlarkObject,
    required this.starlarkMethod,
    required this.argvPrefix,
    required this.defaultCwd,
    required this.cwdPolicy,
    required this.envPolicy,
    required this.syncAllowed,
    required this.asyncAllowed,
    required this.maxRuntimeMs,
    required this.hasMaxRuntimeMs,
    required this.endOfTurnBehavior,
    required this.endOfSessionBehavior,
    required this.stdinPolicy,
    required this.minAwaitMs,
    required this.maxAwaitMs,
    required this.outputBufferBytes,
    required this.terminateGraceMs,
    required this.outputLimitBytes,
    required this.mutationClass,
    required this.modelDescription,
    required this.allowCwdArg,
    required this.allowArgsArg,
    required this.forbiddenArgs,
    required this.executionPolicy,
  });

  static AgentRuntimeCommandSeed deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeCommandSeed(
      actionId: deserializer.deserializeString(),
      binaryName: deserializer.deserializeString(),
      candidatePaths: TraitHelpers.deserializeVectorStr(deserializer),
      starlarkObject: deserializer.deserializeString(),
      starlarkMethod: deserializer.deserializeString(),
      argvPrefix: TraitHelpers.deserializeVectorStr(deserializer),
      defaultCwd: deserializer.deserializeString(),
      cwdPolicy: deserializer.deserializeString(),
      envPolicy: deserializer.deserializeString(),
      syncAllowed: deserializer.deserializeBool(),
      asyncAllowed: deserializer.deserializeBool(),
      maxRuntimeMs: deserializer.deserializeInt64(),
      hasMaxRuntimeMs: deserializer.deserializeBool(),
      endOfTurnBehavior: deserializer.deserializeString(),
      endOfSessionBehavior: deserializer.deserializeString(),
      stdinPolicy: deserializer.deserializeString(),
      minAwaitMs: deserializer.deserializeInt64(),
      maxAwaitMs: deserializer.deserializeInt64(),
      outputBufferBytes: deserializer.deserializeInt64(),
      terminateGraceMs: deserializer.deserializeInt64(),
      outputLimitBytes: deserializer.deserializeInt64(),
      mutationClass: deserializer.deserializeString(),
      modelDescription: deserializer.deserializeString(),
      allowCwdArg: deserializer.deserializeBool(),
      allowArgsArg: deserializer.deserializeBool(),
      forbiddenArgs: TraitHelpers.deserializeVectorStr(deserializer),
      executionPolicy: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeCommandSeed bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeCommandSeed.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String actionId;
  final String binaryName;
  final List<String> candidatePaths;
  final String starlarkObject;
  final String starlarkMethod;
  final List<String> argvPrefix;
  final String defaultCwd;
  final String cwdPolicy;
  final String envPolicy;
  final bool syncAllowed;
  final bool asyncAllowed;
  final int maxRuntimeMs;
  final bool hasMaxRuntimeMs;
  final String endOfTurnBehavior;
  final String endOfSessionBehavior;
  final String stdinPolicy;
  final int minAwaitMs;
  final int maxAwaitMs;
  final int outputBufferBytes;
  final int terminateGraceMs;
  final int outputLimitBytes;
  final String mutationClass;
  final String modelDescription;
  final bool allowCwdArg;
  final bool allowArgsArg;
  final List<String> forbiddenArgs;
  final String executionPolicy;

  AgentRuntimeCommandSeed copyWith({
    String? actionId,
    String? binaryName,
    List<String>? candidatePaths,
    String? starlarkObject,
    String? starlarkMethod,
    List<String>? argvPrefix,
    String? defaultCwd,
    String? cwdPolicy,
    String? envPolicy,
    bool? syncAllowed,
    bool? asyncAllowed,
    int? maxRuntimeMs,
    bool? hasMaxRuntimeMs,
    String? endOfTurnBehavior,
    String? endOfSessionBehavior,
    String? stdinPolicy,
    int? minAwaitMs,
    int? maxAwaitMs,
    int? outputBufferBytes,
    int? terminateGraceMs,
    int? outputLimitBytes,
    String? mutationClass,
    String? modelDescription,
    bool? allowCwdArg,
    bool? allowArgsArg,
    List<String>? forbiddenArgs,
    String? executionPolicy,
  }) {
    return AgentRuntimeCommandSeed(
      actionId: actionId ?? this.actionId,
      binaryName: binaryName ?? this.binaryName,
      candidatePaths: candidatePaths ?? this.candidatePaths,
      starlarkObject: starlarkObject ?? this.starlarkObject,
      starlarkMethod: starlarkMethod ?? this.starlarkMethod,
      argvPrefix: argvPrefix ?? this.argvPrefix,
      defaultCwd: defaultCwd ?? this.defaultCwd,
      cwdPolicy: cwdPolicy ?? this.cwdPolicy,
      envPolicy: envPolicy ?? this.envPolicy,
      syncAllowed: syncAllowed ?? this.syncAllowed,
      asyncAllowed: asyncAllowed ?? this.asyncAllowed,
      maxRuntimeMs: maxRuntimeMs ?? this.maxRuntimeMs,
      hasMaxRuntimeMs: hasMaxRuntimeMs ?? this.hasMaxRuntimeMs,
      endOfTurnBehavior: endOfTurnBehavior ?? this.endOfTurnBehavior,
      endOfSessionBehavior: endOfSessionBehavior ?? this.endOfSessionBehavior,
      stdinPolicy: stdinPolicy ?? this.stdinPolicy,
      minAwaitMs: minAwaitMs ?? this.minAwaitMs,
      maxAwaitMs: maxAwaitMs ?? this.maxAwaitMs,
      outputBufferBytes: outputBufferBytes ?? this.outputBufferBytes,
      terminateGraceMs: terminateGraceMs ?? this.terminateGraceMs,
      outputLimitBytes: outputLimitBytes ?? this.outputLimitBytes,
      mutationClass: mutationClass ?? this.mutationClass,
      modelDescription: modelDescription ?? this.modelDescription,
      allowCwdArg: allowCwdArg ?? this.allowCwdArg,
      allowArgsArg: allowArgsArg ?? this.allowArgsArg,
      forbiddenArgs: forbiddenArgs ?? this.forbiddenArgs,
      executionPolicy: executionPolicy ?? this.executionPolicy,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(actionId);
    serializer.serializeString(binaryName);
    TraitHelpers.serializeVectorStr(candidatePaths, serializer);
    serializer.serializeString(starlarkObject);
    serializer.serializeString(starlarkMethod);
    TraitHelpers.serializeVectorStr(argvPrefix, serializer);
    serializer.serializeString(defaultCwd);
    serializer.serializeString(cwdPolicy);
    serializer.serializeString(envPolicy);
    serializer.serializeBool(syncAllowed);
    serializer.serializeBool(asyncAllowed);
    serializer.serializeInt64(maxRuntimeMs);
    serializer.serializeBool(hasMaxRuntimeMs);
    serializer.serializeString(endOfTurnBehavior);
    serializer.serializeString(endOfSessionBehavior);
    serializer.serializeString(stdinPolicy);
    serializer.serializeInt64(minAwaitMs);
    serializer.serializeInt64(maxAwaitMs);
    serializer.serializeInt64(outputBufferBytes);
    serializer.serializeInt64(terminateGraceMs);
    serializer.serializeInt64(outputLimitBytes);
    serializer.serializeString(mutationClass);
    serializer.serializeString(modelDescription);
    serializer.serializeBool(allowCwdArg);
    serializer.serializeBool(allowArgsArg);
    TraitHelpers.serializeVectorStr(forbiddenArgs, serializer);
    serializer.serializeString(executionPolicy);
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

    return other is AgentRuntimeCommandSeed
      && actionId == other.actionId
      && binaryName == other.binaryName
      && listEquals(candidatePaths, other.candidatePaths)
      && starlarkObject == other.starlarkObject
      && starlarkMethod == other.starlarkMethod
      && listEquals(argvPrefix, other.argvPrefix)
      && defaultCwd == other.defaultCwd
      && cwdPolicy == other.cwdPolicy
      && envPolicy == other.envPolicy
      && syncAllowed == other.syncAllowed
      && asyncAllowed == other.asyncAllowed
      && maxRuntimeMs == other.maxRuntimeMs
      && hasMaxRuntimeMs == other.hasMaxRuntimeMs
      && endOfTurnBehavior == other.endOfTurnBehavior
      && endOfSessionBehavior == other.endOfSessionBehavior
      && stdinPolicy == other.stdinPolicy
      && minAwaitMs == other.minAwaitMs
      && maxAwaitMs == other.maxAwaitMs
      && outputBufferBytes == other.outputBufferBytes
      && terminateGraceMs == other.terminateGraceMs
      && outputLimitBytes == other.outputLimitBytes
      && mutationClass == other.mutationClass
      && modelDescription == other.modelDescription
      && allowCwdArg == other.allowCwdArg
      && allowArgsArg == other.allowArgsArg
      && listEquals(forbiddenArgs, other.forbiddenArgs)
      && executionPolicy == other.executionPolicy;
  }

  @override
  int get hashCode => Object.hashAll([
        actionId,
        binaryName,
        candidatePaths,
        starlarkObject,
        starlarkMethod,
        argvPrefix,
        defaultCwd,
        cwdPolicy,
        envPolicy,
        syncAllowed,
        asyncAllowed,
        maxRuntimeMs,
        hasMaxRuntimeMs,
        endOfTurnBehavior,
        endOfSessionBehavior,
        stdinPolicy,
        minAwaitMs,
        maxAwaitMs,
        outputBufferBytes,
        terminateGraceMs,
        outputLimitBytes,
        mutationClass,
        modelDescription,
        allowCwdArg,
        allowArgsArg,
        forbiddenArgs,
        executionPolicy,
      ]);

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'actionId: $actionId, '
        'binaryName: $binaryName, '
        'candidatePaths: $candidatePaths, '
        'starlarkObject: $starlarkObject, '
        'starlarkMethod: $starlarkMethod, '
        'argvPrefix: $argvPrefix, '
        'defaultCwd: $defaultCwd, '
        'cwdPolicy: $cwdPolicy, '
        'envPolicy: $envPolicy, '
        'syncAllowed: $syncAllowed, '
        'asyncAllowed: $asyncAllowed, '
        'maxRuntimeMs: $maxRuntimeMs, '
        'hasMaxRuntimeMs: $hasMaxRuntimeMs, '
        'endOfTurnBehavior: $endOfTurnBehavior, '
        'endOfSessionBehavior: $endOfSessionBehavior, '
        'stdinPolicy: $stdinPolicy, '
        'minAwaitMs: $minAwaitMs, '
        'maxAwaitMs: $maxAwaitMs, '
        'outputBufferBytes: $outputBufferBytes, '
        'terminateGraceMs: $terminateGraceMs, '
        'outputLimitBytes: $outputLimitBytes, '
        'mutationClass: $mutationClass, '
        'modelDescription: $modelDescription, '
        'allowCwdArg: $allowCwdArg, '
        'allowArgsArg: $allowArgsArg, '
        'forbiddenArgs: $forbiddenArgs, '
        'executionPolicy: $executionPolicy'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeCommandSeed';
  }
}
