import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:rinf/rinf.dart';
import 'package:robdex_design_system/robdex_design_system.dart';

import '../bindings/bindings.dart';

class AgentRuntimeControlTowerController extends ChangeNotifier {
  StreamSubscription<RustSignalPack<AgentRuntimeOutputSignal>>? _subscription;
  final Set<String> _pendingRequestIds = <String>{};
  int _serial = 0;
  String _baseUrl = 'http://127.0.0.1:8765';
  AgentRuntimeControlTowerData? _viewModel;

  AgentRuntimeControlTowerController() {
    _subscription = AgentRuntimeOutputSignal.rustSignalStream.listen(
      _handleOutput,
      onError: (Object _) {
        notifyListeners();
      },
    );
  }

  AgentRuntimeControlTowerData get data {
    return (_viewModel ?? _disconnectedViewModel).copyWith(
      pendingRequestCount: _pendingRequestIds.length,
    );
  }

  void connect(String baseUrl) {
    _baseUrl = baseUrl.trim().isEmpty ? _baseUrl : baseUrl.trim();
    _send('connect', {
      'type': 'connect',
      'payload': {
        'baseUrl': _baseUrl,
        'selectedSessionId': null,
      },
    });
  }

  void pollStreamOnce() {
    _send('poll', {'type': 'pollStreamOnce'});
  }

  void disconnect() {
    _send('disconnect', {'type': 'disconnect'});
  }

  void _send(String prefix, Map<String, dynamic> intent) {
    _serial += 1;
    final requestId = 'agent-runtime-$prefix-$_serial';
    _pendingRequestIds.add(requestId);
    AgentRuntimeRequestSignal(
      requestId: requestId,
      packetJson: jsonEncode({
        'packetId': requestId,
        'intent': intent,
      }),
    ).sendSignalToRust();
    notifyListeners();
  }

  void _handleOutput(RustSignalPack<AgentRuntimeOutputSignal> pack) {
    final signal = pack.message;
    _pendingRequestIds.remove(signal.requestId);
    final decoded = jsonDecode(signal.outputJson) as Map<String, dynamic>;
    final output = decoded['output'] as Map<String, dynamic>? ?? const {};
    final type = output['type'] as String? ?? 'unknown';
    final payload = output['payload'];
    if (type == 'controlTowerView' && payload is Map<String, dynamic>) {
      final viewModel = payload['viewModel'];
      if (viewModel is Map) {
        _viewModel = AgentRuntimeControlTowerData.fromJson(
          Map<String, dynamic>.from(viewModel),
        );
      }
    }
    notifyListeners();
  }

  AgentRuntimeControlTowerData get _disconnectedViewModel => AgentRuntimeControlTowerData(
        connectionState: 'disconnected',
        connectionTone: 'muted',
        baseUrl: _baseUrl,
        statusLabel: 'No projection packet',
        watermarkLabel: '—',
        statusBadges: const [
          AgentRuntimeStatusBadge(label: 'Connection', value: 'disconnected', tone: 'muted'),
        ],
        selectedSessionLabel: 'none selected',
        sessionsTitle: 'Sessions',
        sessionsSubtitle: '',
        timelineTitle: 'Selected session stream',
        timelineSubtitle: '',
        actionsTitle: 'Action queue',
        actionsSubtitle: '',
        detailTitle: 'Controller detail',
        detailSubtitle: '',
        sessionsEmptyTitle: 'No sessions',
        sessionsEmptyText: 'Connect to hydrate runtime sessions.',
        timelineEmptyTitle: 'No timeline',
        timelineEmptyText: 'Select a session to inspect its event stream.',
        actionsEmptyTitle: 'No action queue',
        actionsEmptyText: 'No runtime action queue is loaded.',
        sessions: const [],
        timeline: const [],
        actions: const [],
        controllerFacts: const [],
        outputLog: const [],
        pendingRequestCount: _pendingRequestIds.length,
      );

  @override
  void dispose() {
    _subscription?.cancel();
    super.dispose();
  }
}
