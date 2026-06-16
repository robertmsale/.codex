import 'agent_runtime_control_tower_models.dart';

const mockAgentRuntimeDisconnected = AgentRuntimeControlTowerData(
  connectionState: 'disconnected',
  baseUrl: 'http://127.0.0.1:8765',
  statusLabel: 'No runtime connected',
  watermarkLabel: '—',
  sessions: [],
  timeline: [],
  actions: [],
  controllerFacts: [
    AgentRuntimeFact(label: 'Transport', value: 'Rinf JSON packets'),
    AgentRuntimeFact(label: 'Source of truth', value: 'Rust/Postgres'),
  ],
  outputLog: ['No discovery packet loaded'],
  pendingRequestCount: 0,
);

const mockAgentRuntimeConnecting = AgentRuntimeControlTowerData(
  connectionState: 'connecting',
  baseUrl: 'http://127.0.0.1:8765',
  statusLabel: 'Connecting through Rust transport',
  watermarkLabel: 'pending',
  sessions: [],
  timeline: [],
  actions: [],
  controllerFacts: [
    AgentRuntimeFact(label: 'Pending', value: 'connect-1'),
  ],
  outputLog: ['Sent GuiTransportRequestPacket.connect'],
  pendingRequestCount: 1,
);

const mockAgentRuntimeConnected = AgentRuntimeControlTowerData(
  connectionState: 'streaming',
  baseUrl: 'http://127.0.0.1:8765',
  statusLabel: 'Runtime healthy',
  watermarkLabel: '42',
  sessions: [
    AgentRuntimeSessionItem(
      id: 'session-a',
      title: 'Runtime validation',
      status: 'open',
      subtitle: 'runtime-allow · /Users/robertsale/.codex',
    ),
    AgentRuntimeSessionItem(
      id: 'session-b',
      title: 'Blocked command review',
      status: 'open',
      subtitle: '1 approval pending',
    ),
  ],
  timeline: [
    AgentRuntimeTimelineItem(
      id: 'event-41',
      title: 'tool.call execute_code',
      subtitle: 'Starlark completed',
      status: 'completed',
    ),
    AgentRuntimeTimelineItem(
      id: 'event-42',
      title: 'approval.requested',
      subtitle: 'cmd.rg.audit requires owner approval',
      status: 'pending',
    ),
  ],
  actions: [
    AgentRuntimeActionItem(
      id: 'approval-1',
      title: 'Approve command execution',
      subtitle: 'canDecide=true · canResume=false',
      kind: 'approval',
    ),
    AgentRuntimeActionItem(
      id: 'registry-1',
      title: 'Review command registry request',
      subtitle: 'canPreview=true · canDecide=true',
      kind: 'commandRegistry',
    ),
  ],
  controllerFacts: [
    AgentRuntimeFact(label: 'Selected session', value: 'session-a'),
    AgentRuntimeFact(label: 'Connection', value: 'streaming'),
  ],
  outputLog: ['projectionSnapshot watermark=42', 'controllerState streaming'],
  pendingRequestCount: 0,
);

const mockAgentRuntimeError = AgentRuntimeControlTowerData(
  connectionState: 'failed',
  baseUrl: 'http://127.0.0.1:8765',
  statusLabel: 'Runtime unavailable',
  watermarkLabel: '—',
  sessions: [],
  timeline: [],
  actions: [],
  controllerFacts: [
    AgentRuntimeFact(label: 'Error code', value: 'unavailable'),
  ],
  outputLog: ['AgentRuntimeOutputSignal.error'],
  pendingRequestCount: 0,
  errorMessage: 'runtime server HTTP sync failed',
);

const mockAgentRuntimeEmpty = AgentRuntimeControlTowerData(
  connectionState: 'streaming',
  baseUrl: 'http://127.0.0.1:8765',
  statusLabel: 'Runtime connected · no sessions',
  watermarkLabel: '7',
  sessions: [],
  timeline: [],
  actions: [],
  controllerFacts: [
    AgentRuntimeFact(label: 'Connection', value: 'streaming'),
    AgentRuntimeFact(label: 'Action queue', value: 'empty'),
  ],
  outputLog: ['projectionSnapshot watermark=7'],
  pendingRequestCount: 0,
);
