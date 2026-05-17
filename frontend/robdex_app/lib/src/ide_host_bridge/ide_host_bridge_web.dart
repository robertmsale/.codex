import 'dart:js_interop';
import 'dart:js_interop_unsafe';

JSObject? _vscodeApi;

void openMentionedFile(String target) {
  final trimmed = target.trim();
  if (trimmed.isEmpty) {
    return;
  }
  _postDiagnostic('openMentionedFile requested: $trimmed');
  final api = _vscodeApi ?? _acquireVsCodeApi();
  if (api == null) {
    _postDiagnostic('VS Code API unavailable for openMentionedFile.');
    return;
  }
  _vscodeApi = api;
  api.callMethod(
    'postMessage'.toJS,
    {
      'type': 'robdex.openMentionedFile',
      'target': trimmed,
    }.jsify(),
  );
}

void _postDiagnostic(String message) {
  final api = _vscodeApi ?? _acquireVsCodeApi();
  if (api == null) {
    return;
  }
  _vscodeApi = api;
  api.callMethod(
    'postMessage'.toJS,
    {
      'type': 'robdex.diagnostic',
      'level': 'info',
      'message': message,
    }.jsify(),
  );
}

Uri? configuredBridgeBaseUri() {
  final fromQuery = Uri.base.queryParameters['bridgeBaseUrl'];
  final fromHost = _stringGlobal('__ROBDEX_BRIDGE_BASE_URL__');
  final value = fromQuery?.trim().isNotEmpty == true ? fromQuery : fromHost;
  if (value == null || value.trim().isEmpty) {
    return null;
  }
  final parsed = Uri.tryParse(value.trim());
  if (parsed == null || parsed.scheme.isEmpty || parsed.host.isEmpty) {
    return null;
  }
  return parsed;
}

JSObject? _acquireVsCodeApi() {
  try {
    return _acquireVsCodeApiExternal();
  } catch (_) {
    return null;
  }
}

String? _stringGlobal(String name) {
  try {
    if (name == '__ROBDEX_BRIDGE_BASE_URL__') {
      return _robdexBridgeBaseUrl?.toDart;
    }
    return null;
  } catch (_) {
    return null;
  }
}

@JS('acquireVsCodeApi')
external JSObject _acquireVsCodeApiExternal();

@JS('__ROBDEX_BRIDGE_BASE_URL__')
external JSString? get _robdexBridgeBaseUrl;
