// ignore_for_file: type=lint, type=warning
part of 'signals.dart';
class TraitHelpers {
  static void serializeVectorAgentRuntimeActionRow(List<AgentRuntimeActionRow> value, BinarySerializer serializer) {
    serializer.serializeLength(value.length);
    for (final item in value) {
        item.serialize(serializer);
    }
  }

  static List<AgentRuntimeActionRow> deserializeVectorAgentRuntimeActionRow(BinaryDeserializer deserializer) {
    final length = deserializer.deserializeLength();
    return List.generate(length, (_) => AgentRuntimeActionRow.deserialize(deserializer));
  }

  static void serializeVectorAgentRuntimeBadge(List<AgentRuntimeBadge> value, BinarySerializer serializer) {
    serializer.serializeLength(value.length);
    for (final item in value) {
        item.serialize(serializer);
    }
  }

  static List<AgentRuntimeBadge> deserializeVectorAgentRuntimeBadge(BinaryDeserializer deserializer) {
    final length = deserializer.deserializeLength();
    return List.generate(length, (_) => AgentRuntimeBadge.deserialize(deserializer));
  }

  static void serializeVectorAgentRuntimeChatEntry(List<AgentRuntimeChatEntry> value, BinarySerializer serializer) {
    serializer.serializeLength(value.length);
    for (final item in value) {
        item.serialize(serializer);
    }
  }

  static List<AgentRuntimeChatEntry> deserializeVectorAgentRuntimeChatEntry(BinaryDeserializer deserializer) {
    final length = deserializer.deserializeLength();
    return List.generate(length, (_) => AgentRuntimeChatEntry.deserialize(deserializer));
  }

  static void serializeVectorAgentRuntimeFact(List<AgentRuntimeFact> value, BinarySerializer serializer) {
    serializer.serializeLength(value.length);
    for (final item in value) {
        item.serialize(serializer);
    }
  }

  static List<AgentRuntimeFact> deserializeVectorAgentRuntimeFact(BinaryDeserializer deserializer) {
    final length = deserializer.deserializeLength();
    return List.generate(length, (_) => AgentRuntimeFact.deserialize(deserializer));
  }

  static void serializeVectorAgentRuntimeModelOption(List<AgentRuntimeModelOption> value, BinarySerializer serializer) {
    serializer.serializeLength(value.length);
    for (final item in value) {
        item.serialize(serializer);
    }
  }

  static List<AgentRuntimeModelOption> deserializeVectorAgentRuntimeModelOption(BinaryDeserializer deserializer) {
    final length = deserializer.deserializeLength();
    return List.generate(length, (_) => AgentRuntimeModelOption.deserialize(deserializer));
  }

  static void serializeVectorAgentRuntimeOperationSurface(List<AgentRuntimeOperationSurface> value, BinarySerializer serializer) {
    serializer.serializeLength(value.length);
    for (final item in value) {
        item.serialize(serializer);
    }
  }

  static List<AgentRuntimeOperationSurface> deserializeVectorAgentRuntimeOperationSurface(BinaryDeserializer deserializer) {
    final length = deserializer.deserializeLength();
    return List.generate(length, (_) => AgentRuntimeOperationSurface.deserialize(deserializer));
  }

  static void serializeVectorAgentRuntimeRolePolicyEntry(List<AgentRuntimeRolePolicyEntry> value, BinarySerializer serializer) {
    serializer.serializeLength(value.length);
    for (final item in value) {
        item.serialize(serializer);
    }
  }

  static List<AgentRuntimeRolePolicyEntry> deserializeVectorAgentRuntimeRolePolicyEntry(BinaryDeserializer deserializer) {
    final length = deserializer.deserializeLength();
    return List.generate(length, (_) => AgentRuntimeRolePolicyEntry.deserialize(deserializer));
  }

  static void serializeVectorAgentRuntimeRolePolicyRow(List<AgentRuntimeRolePolicyRow> value, BinarySerializer serializer) {
    serializer.serializeLength(value.length);
    for (final item in value) {
        item.serialize(serializer);
    }
  }

  static List<AgentRuntimeRolePolicyRow> deserializeVectorAgentRuntimeRolePolicyRow(BinaryDeserializer deserializer) {
    final length = deserializer.deserializeLength();
    return List.generate(length, (_) => AgentRuntimeRolePolicyRow.deserialize(deserializer));
  }

  static void serializeVectorAgentRuntimeRoleRow(List<AgentRuntimeRoleRow> value, BinarySerializer serializer) {
    serializer.serializeLength(value.length);
    for (final item in value) {
        item.serialize(serializer);
    }
  }

  static List<AgentRuntimeRoleRow> deserializeVectorAgentRuntimeRoleRow(BinaryDeserializer deserializer) {
    final length = deserializer.deserializeLength();
    return List.generate(length, (_) => AgentRuntimeRoleRow.deserialize(deserializer));
  }

  static void serializeVectorAgentRuntimeRoleVersionRow(List<AgentRuntimeRoleVersionRow> value, BinarySerializer serializer) {
    serializer.serializeLength(value.length);
    for (final item in value) {
        item.serialize(serializer);
    }
  }

  static List<AgentRuntimeRoleVersionRow> deserializeVectorAgentRuntimeRoleVersionRow(BinaryDeserializer deserializer) {
    final length = deserializer.deserializeLength();
    return List.generate(length, (_) => AgentRuntimeRoleVersionRow.deserialize(deserializer));
  }

  static void serializeVectorAgentRuntimeSessionRow(List<AgentRuntimeSessionRow> value, BinarySerializer serializer) {
    serializer.serializeLength(value.length);
    for (final item in value) {
        item.serialize(serializer);
    }
  }

  static List<AgentRuntimeSessionRow> deserializeVectorAgentRuntimeSessionRow(BinaryDeserializer deserializer) {
    final length = deserializer.deserializeLength();
    return List.generate(length, (_) => AgentRuntimeSessionRow.deserialize(deserializer));
  }

  static void serializeVectorAgentRuntimeShellProjectRow(List<AgentRuntimeShellProjectRow> value, BinarySerializer serializer) {
    serializer.serializeLength(value.length);
    for (final item in value) {
        item.serialize(serializer);
    }
  }

  static List<AgentRuntimeShellProjectRow> deserializeVectorAgentRuntimeShellProjectRow(BinaryDeserializer deserializer) {
    final length = deserializer.deserializeLength();
    return List.generate(length, (_) => AgentRuntimeShellProjectRow.deserialize(deserializer));
  }

  static void serializeVectorAgentRuntimeShellRolePresentation(List<AgentRuntimeShellRolePresentation> value, BinarySerializer serializer) {
    serializer.serializeLength(value.length);
    for (final item in value) {
        item.serialize(serializer);
    }
  }

  static List<AgentRuntimeShellRolePresentation> deserializeVectorAgentRuntimeShellRolePresentation(BinaryDeserializer deserializer) {
    final length = deserializer.deserializeLength();
    return List.generate(length, (_) => AgentRuntimeShellRolePresentation.deserialize(deserializer));
  }

  static void serializeVectorAgentRuntimeTimelineRow(List<AgentRuntimeTimelineRow> value, BinarySerializer serializer) {
    serializer.serializeLength(value.length);
    for (final item in value) {
        item.serialize(serializer);
    }
  }

  static List<AgentRuntimeTimelineRow> deserializeVectorAgentRuntimeTimelineRow(BinaryDeserializer deserializer) {
    final length = deserializer.deserializeLength();
    return List.generate(length, (_) => AgentRuntimeTimelineRow.deserialize(deserializer));
  }

  static void serializeVectorAgentRuntimeWorkflowMemoryEvent(List<AgentRuntimeWorkflowMemoryEvent> value, BinarySerializer serializer) {
    serializer.serializeLength(value.length);
    for (final item in value) {
        item.serialize(serializer);
    }
  }

  static List<AgentRuntimeWorkflowMemoryEvent> deserializeVectorAgentRuntimeWorkflowMemoryEvent(BinaryDeserializer deserializer) {
    final length = deserializer.deserializeLength();
    return List.generate(length, (_) => AgentRuntimeWorkflowMemoryEvent.deserialize(deserializer));
  }

  static void serializeVectorAgentRuntimeWorkflowMemoryRow(List<AgentRuntimeWorkflowMemoryRow> value, BinarySerializer serializer) {
    serializer.serializeLength(value.length);
    for (final item in value) {
        item.serialize(serializer);
    }
  }

  static List<AgentRuntimeWorkflowMemoryRow> deserializeVectorAgentRuntimeWorkflowMemoryRow(BinaryDeserializer deserializer) {
    final length = deserializer.deserializeLength();
    return List.generate(length, (_) => AgentRuntimeWorkflowMemoryRow.deserialize(deserializer));
  }

  static void serializeVectorStr(List<String> value, BinarySerializer serializer) {
    serializer.serializeLength(value.length);
    for (final item in value) {
        serializer.serializeString(item);
    }
  }

  static List<String> deserializeVectorStr(BinaryDeserializer deserializer) {
    final length = deserializer.deserializeLength();
    return List.generate(length, (_) => deserializer.deserializeString());
  }

  static void serializeVectorU8(List<int> value, BinarySerializer serializer) {
    serializer.serializeLength(value.length);
    for (final item in value) {
        serializer.serializeUint8(item);
    }
  }

  static List<int> deserializeVectorU8(BinaryDeserializer deserializer) {
    final length = deserializer.deserializeLength();
    return List.generate(length, (_) => deserializer.deserializeUint8());
  }

}

