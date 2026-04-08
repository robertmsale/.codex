import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

class ComposerPanel extends StatefulWidget {
  const ComposerPanel({
    super.key,
    required this.enabled,
    required this.isRunning,
    required this.onSend,
    required this.onInterrupt,
  });

  final bool enabled;
  final bool isRunning;
  final ValueChanged<String> onSend;
  final VoidCallback onInterrupt;

  @override
  State<ComposerPanel> createState() => _ComposerPanelState();
}

class _ComposerPanelState extends State<ComposerPanel> {
  late final TextEditingController _controller;
  final FocusNode _focusNode = FocusNode();
  bool _hasDraftText = false;

  @override
  void initState() {
    super.initState();
    _controller = TextEditingController();
    _controller.addListener(_handleDraftChanged);
  }

  @override
  void dispose() {
    _controller.removeListener(_handleDraftChanged);
    _controller.dispose();
    _focusNode.dispose();
    super.dispose();
  }

  void _handleDraftChanged() {
    final next = _controller.text.trim().isNotEmpty;
    if (next != _hasDraftText) {
      setState(() {
        _hasDraftText = next;
      });
    }
  }

  void _submit() {
    final text = _controller.text.trim();
    if (text.isEmpty || !widget.enabled) {
      return;
    }
    widget.onSend(text);
    _controller.clear();
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final isDesktopPlatform = switch (defaultTargetPlatform) {
      TargetPlatform.macOS || TargetPlatform.linux || TargetPlatform.windows => true,
      _ => false,
    };
    final showsInterrupt = widget.isRunning && !_hasDraftText;
    final actionBackground = showsInterrupt
        ? theme.colorScheme.error
        : theme.colorScheme.primary;
    final actionForeground = showsInterrupt
        ? theme.colorScheme.onError
        : const Color(0xFF08111A);

    return DecoratedBox(
      decoration: BoxDecoration(
        border: Border(
          top: BorderSide(color: theme.colorScheme.outline),
        ),
      ),
      child: Padding(
        padding: const EdgeInsets.only(top: 8),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                Expanded(
                  child: Focus(
                    onKeyEvent: (node, event) {
                      if (!isDesktopPlatform || !widget.enabled) {
                        return KeyEventResult.ignored;
                      }
                      if (event is! KeyDownEvent) {
                        return KeyEventResult.ignored;
                      }
                      final isEnter =
                          event.logicalKey == LogicalKeyboardKey.enter ||
                          event.logicalKey == LogicalKeyboardKey.numpadEnter;
                      if (!isEnter) {
                        return KeyEventResult.ignored;
                      }
                      if (HardwareKeyboard.instance.isShiftPressed) {
                        return KeyEventResult.ignored;
                      }
                      _submit();
                      return KeyEventResult.handled;
                    },
                    child: TextField(
                      controller: _controller,
                      focusNode: _focusNode,
                      enabled: widget.enabled,
                      minLines: 1,
                      maxLines: 4,
                      style: theme.textTheme.bodySmall?.copyWith(
                        fontSize: 11,
                        height: 1.3,
                      ),
                      onSubmitted: (_) {
                        if (!isDesktopPlatform) {
                          return;
                        }
                        _submit();
                      },
                      decoration: InputDecoration(
                        hintText: 'Send a message to the selected thread',
                        hintStyle: theme.textTheme.bodySmall?.copyWith(
                          fontSize: 11,
                          color: theme.colorScheme.onSurface.withValues(alpha: 0.48),
                        ),
                      ),
                    ),
                  ),
                ),
                const SizedBox(width: 8),
                IconButton(
                  onPressed: widget.enabled
                      ? (showsInterrupt ? widget.onInterrupt : _submit)
                      : null,
                  icon: Icon(
                    showsInterrupt
                        ? Icons.stop_rounded
                        : Icons.arrow_upward_rounded,
                    size: 18,
                  ),
                  style: IconButton.styleFrom(
                    backgroundColor: actionBackground,
                    foregroundColor: actionForeground,
                    disabledBackgroundColor:
                        theme.colorScheme.surfaceContainerHighest.withValues(alpha: 0.55),
                    disabledForegroundColor:
                        theme.colorScheme.onSurface.withValues(alpha: 0.4),
                    minimumSize: const Size(42, 42),
                    shape: const CircleBorder(),
                  ),
                  tooltip: showsInterrupt ? 'Interrupt' : 'Send',
                ),
              ],
            ),
            if (!widget.enabled) ...[
              const SizedBox(height: 10),
              Text(
                'Select a thread to enable the composer.',
                style: theme.textTheme.labelSmall?.copyWith(
                  color: theme.colorScheme.secondary,
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}
