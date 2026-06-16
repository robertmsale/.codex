import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:rinf/rinf.dart';
import 'package:robdex_design_system/robdex_design_system.dart';

import '../bindings/bindings.dart';

class AgentRuntimeControlTowerController extends ChangeNotifier {
  StreamSubscription<RustSignalPack<AgentRuntimeOutputSignal>>? _subscription;
  final Set<String> _pendingRequestIds = <String>{};
  final List<String> _outputLog = <String>[];
  int _serial = 0;
  String _baseUrl = 'http://127.0.0.1:8765';
  Map<String, dynamic>? _projection;
  Map<String, dynamic>? _controllerState;
  String? _errorMessage;

  AgentRuntimeControlTowerController() {
    _subscription = AgentRuntimeOutputSignal.rustSignalStream.listen(
      _handleOutput,
      onError: (Object error) {
        _errorMessage = 'signal error: $error';
        notifyListeners();
      },
    );
  }

  AgentRuntimeControlTowerData get data {
    final projection = _projection;
    final controller = _controllerState;
    final sessions = (projection?['sessions'] as List<dynamic>? ?? const [])
        .whereType<Map<String, dynamic>>()
        .map(_sessionItem)
        .toList(growable: false);
    final timeline = (projection?['timeline'] as List<dynamic>? ?? const [])
        .whereType<Map<String, dynamic>>()
        .map(_timelineItem)
        .toList(growable: false);
    final approvals = (projection?['pendingApprovals'] as List<dynamic>? ?? const [])
        .whereType<Map<String, dynamic>>()
        .map(_approvalAction)
        .toList(growable: false);
    final registry = (projection?['commandRegistryRequests'] as List<dynamic>? ?? const [])
        .whereType<Map<String, dynamic>>()
        .map(_registryAction)
        .toList(growable: false);
    return AgentRuntimeControlTowerData(
      connectionState: _renderConnectionState(),
      baseUrl: _baseUrl,
      statusLabel: _statusLabel(projection),
      watermarkLabel: '${projection?['watermark'] ?? '—'}',
      sessions: sessions,
      timeline: timeline,
      actions: [...approvals, ...registry],
      controllerFacts: [
        AgentRuntimeFact(
          label: 'Controller',
          value: '${controller?['connectionState'] ?? _renderConnectionState()}',
        ),
        AgentRuntimeFact(
          label: 'Selected session',
          value: '${controller?['selectedSessionId'] ?? 'none'}',
        ),
        AgentRuntimeFact(
          label: 'Pending rehydrate',
          value: '${controller?['pendingRehydrate'] ?? false}',
        ),
      ],
      outputLog: _outputLog.take(8).toList(growable: false),
      pendingRequestCount: _pendingRequestIds.length,
      errorMessage: _errorMessage,
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
    _outputLog.insert(0, '$type · ${signal.requestId}');
    if (type == 'projectionSnapshot' && payload is Map<String, dynamic>) {
      _projection = Map<String, dynamic>.from(payload['projection'] as Map);
      _errorMessage = null;
    } else if (type == 'controllerState' && payload is Map<String, dynamic>) {
      _controllerState = Map<String, dynamic>.from(payload['controllerState'] as Map);
    } else if (type == 'streamOutcome' && payload is Map<String, dynamic>) {
      if (payload['projection'] is Map) {
        _projection = Map<String, dynamic>.from(payload['projection'] as Map);
      }
      if (payload['controllerState'] is Map) {
        _controllerState = Map<String, dynamic>.from(payload['controllerState'] as Map);
      }
    } else if (type == 'error' && payload is Map<String, dynamic>) {
      final error = payload['error'] as Map<String, dynamic>? ?? const {};
      final body = error['error'] as Map<String, dynamic>? ?? const {};
      _errorMessage = '${body['code'] ?? 'error'}: ${body['message'] ?? 'runtime transport failed'}';
    }
    notifyListeners();
  }

  String _renderConnectionState() {
    final rustState = _controllerState?['connectionState'];
    if (rustState is String && rustState.trim().isNotEmpty) {
      return rustState;
    }
    if (_pendingRequestIds.isNotEmpty) {
      return 'requestPending';
    }
    if (_errorMessage != null) {
      return 'errorPacket';
    }
    if (_projection != null) {
      return 'projectionPacket';
    }
    return 'disconnected';
  }

  static String _statusLabel(Map<String, dynamic>? projection) {
    final status = projection?['serverStatus'];
    if (status is Map<String, dynamic>) {
      return '${status['status'] ?? 'runtime'} · ${status['database'] ?? 'database unknown'}';
    }
    return 'No projection packet';
  }

  static AgentRuntimeSessionItem _sessionItem(Map<String, dynamic> json) {
    final id = '${json['id'] ?? 'session'}';
    final title = '${json['title'] ?? json['name'] ?? id}';
    final role = json['roleId'] ?? json['roleVersion'] ?? 'runtime role';
    return AgentRuntimeSessionItem(
      id: id,
      title: title,
      status: '${json['status'] ?? 'unknown'}',
      subtitle: '$role · ${json['workdir'] ?? ''}',
    );
  }

  static AgentRuntimeTimelineItem _timelineItem(Map<String, dynamic> json) {
    final id = '${json['id'] ?? json['sequence'] ?? 'event'}';
    return AgentRuntimeTimelineItem(
      id: id,
      title: '${json['eventType'] ?? json['entityType'] ?? 'event'}',
      subtitle: '${json['summary'] ?? json['entityId'] ?? ''}',
      status: '${json['status'] ?? json['sequence'] ?? ''}',
    );
  }

  static AgentRuntimeActionItem _approvalAction(Map<String, dynamic> json) {
    final id = '${json['id'] ?? 'approval'}';
    return AgentRuntimeActionItem(
      id: id,
      title: '${json['actionName'] ?? 'Approval'}',
      subtitle: 'canDecide=${json['canDecide']} · canResume=${json['canResume']}',
      kind: 'approval',
    );
  }

  static AgentRuntimeActionItem _registryAction(Map<String, dynamic> json) {
    final id = '${json['id'] ?? 'request'}';
    return AgentRuntimeActionItem(
      id: id,
      title: '${json['actionId'] ?? 'Command request'}',
      subtitle: 'canPreview=${json['canPreview']} · canApply=${json['canApply']}',
      kind: 'commandRegistry',
    );
  }

  @override
  void dispose() {
    _subscription?.cancel();
    super.dispose();
  }
}
