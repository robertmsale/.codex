import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:rinf/rinf.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:xterm/xterm.dart';

import '../bindings/bindings.dart';

class IntegratedTerminalController extends ChangeNotifier {
  IntegratedTerminalController() {
    _subscription = TerminalEventSignal.rustSignalStream.listen(_handleEvent);
  }

  static const _usernamePrefix = 'terminal.username.';

  final Terminal terminal = Terminal(maxLines: 5000);
  StreamSubscription<RustSignalPack<TerminalEventSignal>>? _subscription;
  String? _sessionId;
  String? _host;
  String? _username;
  String? _status;
  String? _pendingRequestId;
  int _nextRequestId = 1;
  final Set<String> _cancelledRequestIds = <String>{};
  bool _isOpen = false;
  bool _isConnecting = false;
  int _cols = 100;
  int _rows = 24;

  bool get isAvailable => !kIsWeb && defaultTargetPlatform == TargetPlatform.macOS;
  bool get isOpen => _isOpen;
  bool get isConnecting => _isConnecting;
  String? get host => _host;
  String? get username => _username;
  String? get status => _status;

  Future<String> usernameForHost(String host) async {
    final prefs = await SharedPreferences.getInstance();
    return prefs.getString('$_usernamePrefix${host.trim()}') ?? '';
  }

  Future<void> open({
    required String host,
    required String username,
  }) async {
    if (!isAvailable) {
      return;
    }
    final cleanHost = host.trim();
    final cleanUsername = username.trim();
    if (cleanHost.isEmpty) {
      return;
    }
    await close();
    final prefs = await SharedPreferences.getInstance();
    if (cleanUsername.isNotEmpty) {
      await prefs.setString('$_usernamePrefix$cleanHost', cleanUsername);
    }
    _host = cleanHost;
    _username = cleanUsername;
    final requestId = 'terminal-open-${_nextRequestId++}';
    _pendingRequestId = requestId;
    _cancelledRequestIds.remove(requestId);
    _status = 'Connecting';
    _isConnecting = true;
    _isOpen = true;
    terminal.write('\x1b[2J\x1b[H');
    terminal.write('\r\nConnecting to $cleanHost...\r\n');
    terminal.onOutput = _sendInput;
    terminal.onResize = (width, height, _, _) {
      _cols = width;
      _rows = height;
      final sessionId = _sessionId;
      if (sessionId == null) {
        return;
      }
      TerminalResizeSignal(
        sessionId: sessionId,
        cols: width,
        rows: height,
      ).sendSignalToRust();
    };
    TerminalOpenSignal(
      requestId: requestId,
      host: cleanHost,
      username: cleanUsername,
      cols: _cols,
      rows: _rows,
    ).sendSignalToRust();
    notifyListeners();
  }

  Future<void> close() async {
    final sessionId = _sessionId;
    if (sessionId != null) {
      TerminalCloseSignal(sessionId: sessionId).sendSignalToRust();
    } else {
      final pendingRequestId = _pendingRequestId;
      if (pendingRequestId != null) {
        _cancelledRequestIds.add(pendingRequestId);
      }
    }
    _sessionId = null;
    _pendingRequestId = null;
    _host = null;
    _username = null;
    _isConnecting = false;
    _isOpen = false;
    _status = null;
    terminal.onOutput = null;
    terminal.onResize = null;
    notifyListeners();
  }

  void closeAll() {
    if (!isAvailable) {
      return;
    }
    TerminalCloseAllSignal().sendSignalToRust();
    _sessionId = null;
    final pendingRequestId = _pendingRequestId;
    if (pendingRequestId != null) {
      _cancelledRequestIds.add(pendingRequestId);
    }
    _pendingRequestId = null;
    _host = null;
    _username = null;
    _isConnecting = false;
    _isOpen = false;
    _status = null;
    terminal.onOutput = null;
    terminal.onResize = null;
    notifyListeners();
  }

  void _sendInput(String data) {
    final sessionId = _sessionId;
    if (sessionId == null || data.isEmpty) {
      return;
    }
    TerminalInputSignal(sessionId: sessionId, data: data).sendSignalToRust();
  }

  void _handleEvent(RustSignalPack<TerminalEventSignal> pack) {
    final event = pack.message;
    final knownSession = _sessionId;
    if (knownSession != null && event.sessionId != knownSession) {
      return;
    }
    final pendingRequestId = _pendingRequestId;
    if (event.requestId.isNotEmpty &&
        pendingRequestId != null &&
        event.requestId != pendingRequestId &&
        !_cancelledRequestIds.contains(event.requestId)) {
      return;
    }
    switch (event.kind) {
      case 'opened':
        if (_cancelledRequestIds.remove(event.requestId)) {
          TerminalCloseSignal(sessionId: event.sessionId).sendSignalToRust();
          return;
        }
        _pendingRequestId = null;
        _sessionId = event.sessionId;
        _host = event.host;
        _username = event.username;
        _isConnecting = false;
        _isOpen = true;
        _status = 'Connected';
        break;
      case 'output':
        terminal.write(event.data);
        break;
      case 'error':
        if (_cancelledRequestIds.remove(event.requestId)) {
          return;
        }
        _pendingRequestId = null;
        terminal.write('\r\n${event.data}\r\n');
        _status = event.data;
        _isConnecting = false;
        break;
      case 'closed':
        _cancelledRequestIds.remove(event.requestId);
        _pendingRequestId = null;
        terminal.write('\r\n[connection closed]\r\n');
        _sessionId = null;
        _host = null;
        _username = null;
        _isConnecting = false;
        _status = 'Closed';
        break;
    }
    notifyListeners();
  }

  @override
  void dispose() {
    closeAll();
    _subscription?.cancel();
    super.dispose();
  }
}

class IntegratedTerminalDrawer extends StatefulWidget {
  const IntegratedTerminalDrawer({
    super.key,
    required this.controller,
  });

  final IntegratedTerminalController controller;

  @override
  State<IntegratedTerminalDrawer> createState() => _IntegratedTerminalDrawerState();
}

class _IntegratedTerminalDrawerState extends State<IntegratedTerminalDrawer> {
  final TextEditingController _hostController = TextEditingController();
  final TextEditingController _usernameController = TextEditingController();
  final FocusNode _hostFocusNode = FocusNode();
  double _height = 284;

  @override
  void initState() {
    super.initState();
    widget.controller.addListener(_syncFromController);
    widget.controller.terminal.write('Robdex SSH terminal\r\n');
  }

  @override
  void dispose() {
    widget.controller.removeListener(_syncFromController);
    _hostFocusNode.dispose();
    _hostController.dispose();
    _usernameController.dispose();
    super.dispose();
  }

  void _syncFromController() {
    if (!mounted) {
      return;
    }
    setState(() {});
  }

  Future<void> _open() async {
    await widget.controller.open(
      host: _hostController.text,
      username: _usernameController.text,
    );
  }

  Future<void> _loadUsername(String value) async {
    final username = await widget.controller.usernameForHost(value);
    if (!mounted || _usernameController.text.trim().isNotEmpty) {
      return;
    }
    _usernameController.text = username;
  }

  @override
  Widget build(BuildContext context) {
    final controller = widget.controller;
    if (!controller.isAvailable) {
      return const SizedBox.shrink();
    }
    return AnimatedBuilder(
      animation: controller,
      builder: (context, _) {
        if (!controller.isOpen && !_hostFocusNode.hasFocus) {
          return Align(
            alignment: Alignment.centerRight,
            child: Padding(
              padding: const EdgeInsets.only(top: 10),
              child: OutlinedButton.icon(
                onPressed: () {
                  setState(() {
                    _hostFocusNode.requestFocus();
                  });
                },
                icon: const Icon(Icons.terminal, size: 16),
                label: const Text('Terminal'),
              ),
            ),
          );
        }
        return SizedBox(
          height: _height,
          child: DecoratedBox(
            decoration: const BoxDecoration(
              color: Color(0xFF070A0F),
              border: Border(top: BorderSide(color: Color(0xFF30343B))),
            ),
            child: Column(
              children: [
                MouseRegion(
                  cursor: SystemMouseCursors.resizeUpDown,
                  child: GestureDetector(
                    behavior: HitTestBehavior.opaque,
                    onVerticalDragUpdate: (details) {
                      setState(() {
                        _height = (_height - details.delta.dy).clamp(180, 520);
                      });
                    },
                    child: const SizedBox(height: 8),
                  ),
                ),
                Padding(
                  padding: const EdgeInsets.fromLTRB(12, 0, 8, 8),
                  child: Row(
                    children: [
                      const Icon(Icons.terminal, size: 16),
                      const SizedBox(width: 10),
                      SizedBox(
                        width: 220,
                        child: TextField(
                          controller: _hostController,
                          focusNode: _hostFocusNode,
                          enabled: !controller.isConnecting && controller.host == null,
                          onChanged: _loadUsername,
                          onSubmitted: (_) => _open(),
                          decoration: const InputDecoration(
                            isDense: true,
                            labelText: 'Host',
                          ),
                        ),
                      ),
                      const SizedBox(width: 8),
                      SizedBox(
                        width: 160,
                        child: TextField(
                          controller: _usernameController,
                          enabled: !controller.isConnecting && controller.host == null,
                          onSubmitted: (_) => _open(),
                          decoration: const InputDecoration(
                            isDense: true,
                            labelText: 'Username',
                          ),
                        ),
                      ),
                      const SizedBox(width: 8),
                      FilledButton(
                        onPressed: controller.isConnecting || controller.host != null
                            ? null
                            : _open,
                        child: const Text('Connect'),
                      ),
                      const SizedBox(width: 8),
                      IconButton(
                        onPressed: controller.isOpen ? controller.close : null,
                        tooltip: 'Close terminal',
                        icon: const Icon(Icons.close, size: 18),
                      ),
                      const SizedBox(width: 8),
                      Expanded(
                        child: Text(
                          controller.status ?? '',
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          textAlign: TextAlign.right,
                        ),
                      ),
                    ],
                  ),
                ),
                Expanded(
                  child: TerminalView(
                    controller.terminal,
                    autofocus: controller.isOpen,
                    backgroundOpacity: 1,
                    theme: TerminalThemes.defaultTheme,
                    textStyle: const TerminalStyle(fontSize: 13),
                  ),
                ),
              ],
            ),
          ),
        );
      },
    );
  }
}
