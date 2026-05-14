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
    unawaited(_restoreDrawerHeight());
  }

  static const _usernamePrefix = 'terminal.username.';
  static const _drawerHeightPreferenceKey = 'terminal.drawerHeight';
  static const double minDrawerHeight = 180;
  static const double maxDrawerHeight = 560;
  static const double defaultDrawerHeight = 284;

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
  bool _isDrawerVisible = false;
  bool _isConnecting = false;
  double _drawerHeight = defaultDrawerHeight;
  int _cols = 100;
  int _rows = 24;

  bool get isAvailable => !kIsWeb && defaultTargetPlatform == TargetPlatform.macOS;
  bool get isOpen => _isOpen;
  bool get isDrawerVisible => _isDrawerVisible;
  bool get isConnecting => _isConnecting;
  double get drawerHeight => _drawerHeight;
  String? get host => _host;
  String? get username => _username;
  String? get status => _status;

  @visibleForTesting
  void markConnectedForTest({
    required String sessionId,
    required String host,
    required String username,
  }) {
    _sessionId = sessionId;
    _host = host;
    _username = username;
    _pendingRequestId = null;
    _isConnecting = false;
    _isOpen = true;
    _isDrawerVisible = true;
    _status = 'Connected';
    notifyListeners();
  }

  Future<void> _restoreDrawerHeight() async {
    if (!isAvailable) {
      return;
    }
    final prefs = await SharedPreferences.getInstance();
    final stored = prefs.getDouble(_drawerHeightPreferenceKey);
    if (stored == null) {
      return;
    }
    _drawerHeight = stored.clamp(minDrawerHeight, maxDrawerHeight).toDouble();
    notifyListeners();
  }

  void showDrawer() {
    if (!isAvailable) {
      return;
    }
    _isDrawerVisible = true;
    notifyListeners();
  }

  void hideDrawer() {
    _isDrawerVisible = false;
    notifyListeners();
  }

  void setDrawerHeight(double height) {
    final next = height.clamp(minDrawerHeight, maxDrawerHeight).toDouble();
    if (next == _drawerHeight) {
      return;
    }
    _drawerHeight = next;
    notifyListeners();
  }

  Future<void> persistDrawerHeight() async {
    if (!isAvailable) {
      return;
    }
    final prefs = await SharedPreferences.getInstance();
    await prefs.setDouble(_drawerHeightPreferenceKey, _drawerHeight);
  }

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
    _isDrawerVisible = true;
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
    _isDrawerVisible = false;
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
    _isDrawerVisible = false;
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
        _isDrawerVisible = true;
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
        _isOpen = false;
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
    required this.host,
  });

  final IntegratedTerminalController controller;
  final String host;

  @override
  State<IntegratedTerminalDrawer> createState() => _IntegratedTerminalDrawerState();
}

class _IntegratedTerminalDrawerState extends State<IntegratedTerminalDrawer> {
  final TextEditingController _usernameController = TextEditingController();
  final FocusNode _usernameFocusNode = FocusNode();
  bool _didRequestInitialFocus = false;

  @override
  void initState() {
    super.initState();
    widget.controller.addListener(_syncFromController);
    widget.controller.terminal.write('Robdex SSH terminal\r\n');
    unawaited(_loadUsername(widget.host));
  }

  @override
  void didUpdateWidget(IntegratedTerminalDrawer oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.host != oldWidget.host && _usernameController.text.trim().isEmpty) {
      unawaited(_loadUsername(widget.host));
    }
  }

  @override
  void dispose() {
    widget.controller.removeListener(_syncFromController);
    _usernameFocusNode.dispose();
    _usernameController.dispose();
    super.dispose();
  }

  void _syncFromController() {
    if (!mounted) {
      return;
    }
    if (widget.controller.isDrawerVisible && !_didRequestInitialFocus) {
      _didRequestInitialFocus = true;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && widget.controller.isDrawerVisible && widget.controller.host == null) {
          _usernameFocusNode.requestFocus();
        }
      });
    } else if (!widget.controller.isDrawerVisible) {
      _didRequestInitialFocus = false;
    }
    setState(() {});
  }

  Future<void> _open() async {
    await widget.controller.open(
      host: widget.host,
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
        return AnimatedSize(
          duration: const Duration(milliseconds: 180),
          curve: Curves.easeOutCubic,
          alignment: Alignment.bottomCenter,
          child: controller.isDrawerVisible
              ? SizedBox(
                  height: controller.drawerHeight,
                  child: _TerminalDrawerBody(
                    controller: controller,
                    host: widget.host,
                    usernameController: _usernameController,
                    usernameFocusNode: _usernameFocusNode,
                    onOpen: _open,
                  ),
                )
              : const SizedBox.shrink(),
        );
      },
    );
  }
}

class _TerminalDrawerBody extends StatelessWidget {
  const _TerminalDrawerBody({
    required this.controller,
    required this.host,
    required this.usernameController,
    required this.usernameFocusNode,
    required this.onOpen,
  });

  final IntegratedTerminalController controller;
  final String host;
  final TextEditingController usernameController;
  final FocusNode usernameFocusNode;
  final Future<void> Function() onOpen;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: const BoxDecoration(
        color: Color(0xFF070A0F),
        border: Border(top: BorderSide(color: Color(0xFF30343B))),
      ),
      child: Column(
        children: [
          MouseRegion(
            cursor: SystemMouseCursors.resizeUpDown,
            child: GestureDetector(
              key: const ValueKey('semantic.terminal.resizeHandle'),
              behavior: HitTestBehavior.opaque,
              onVerticalDragUpdate: (details) {
                controller.setDrawerHeight(controller.drawerHeight - details.delta.dy);
              },
              onVerticalDragEnd: (_) {
                unawaited(controller.persistDrawerHeight());
              },
              child: Semantics(
                label: 'Resize terminal drawer',
                child: SizedBox(
                  height: 12,
                  child: Center(
                    child: Container(
                      width: 48,
                      height: 3,
                      decoration: BoxDecoration(
                        color: const Color(0xFF59606A),
                        borderRadius: BorderRadius.circular(999),
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
          if (!controller.isOpen) ...[
            Padding(
              padding: const EdgeInsets.fromLTRB(12, 0, 8, 8),
              child: Row(
                children: [
                  const Icon(Icons.terminal, size: 16),
                  const SizedBox(width: 10),
                  Flexible(
                    flex: 4,
                    child: ConstrainedBox(
                      constraints: const BoxConstraints(maxWidth: 260),
                      child: InputDecorator(
                        decoration: const InputDecoration(
                          isDense: true,
                          labelText: 'Bridge host',
                        ),
                        child: Text(
                          host,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                    ),
                  ),
                  const SizedBox(width: 8),
                  Flexible(
                    flex: 2,
                    child: ConstrainedBox(
                      constraints: const BoxConstraints(maxWidth: 160),
                      child: TextField(
                        controller: usernameController,
                        focusNode: usernameFocusNode,
                        enabled: !controller.isConnecting && controller.host == null,
                        onSubmitted: (_) => onOpen(),
                        decoration: const InputDecoration(
                          isDense: true,
                          labelText: 'Username',
                        ),
                      ),
                    ),
                  ),
                  const SizedBox(width: 8),
                  FilledButton(
                    onPressed: controller.isConnecting || controller.host != null
                        ? null
                        : onOpen,
                    child: const Text('Connect'),
                  ),
                  const SizedBox(width: 8),
                  IconButton(
                    onPressed: controller.hideDrawer,
                    tooltip: 'Hide terminal',
                    icon: const Icon(Icons.close, size: 18),
                  ),
                  const SizedBox(width: 8),
                  Flexible(
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
          ] else ...[
            Padding(
              padding: const EdgeInsets.fromLTRB(12, 0, 8, 8),
              child: Row(
                children: [
                  const Icon(Icons.terminal, size: 16),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Text(
                      controller.host == null
                          ? 'Connected'
                          : 'Connected to ${controller.username?.isNotEmpty == true ? '${controller.username}@' : ''}${controller.host}',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  const SizedBox(width: 8),
                  IconButton(
                    onPressed: controller.close,
                    tooltip: 'Close terminal',
                    icon: const Icon(Icons.close, size: 18),
                  ),
                  if ((controller.status ?? '').isNotEmpty) ...[
                    const SizedBox(width: 8),
                    Text(
                      controller.status!,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: Theme.of(context).textTheme.labelSmall?.copyWith(
                            color: Colors.white.withValues(alpha: 0.72),
                          ),
                    ),
                  ],
                ],
              ),
            ),
          ],
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
    );
  }
}
