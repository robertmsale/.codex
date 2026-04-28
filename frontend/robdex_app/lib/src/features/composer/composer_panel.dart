import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:desktop_drop/desktop_drop.dart';
import 'package:file_selector/file_selector.dart';
import 'package:screen_capturer/screen_capturer.dart';

class ComposerSubmission {
  const ComposerSubmission({
    required this.text,
    required this.localImagePaths,
  });

  final String text;
  final List<String> localImagePaths;
}

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
  final ValueChanged<ComposerSubmission> onSend;
  final VoidCallback onInterrupt;

  @override
  State<ComposerPanel> createState() => _ComposerPanelState();
}

class _ComposerPanelState extends State<ComposerPanel> {
  static const _sendTransitionAsset = 'assets/animations/send-transition.gif';
  static const _sendTransitionDuration = Duration(milliseconds: 2500);

  late final TextEditingController _controller;
  final FocusNode _focusNode = FocusNode();
  final List<String> _localImagePaths = <String>[];
  bool _hasDraftText = false;
  bool _isDesktopDragging = false;
  bool _isPickingImages = false;
  bool _isCapturingScreenshot = false;
  bool _isShowingSendTransition = false;
  String? _attachmentError;
  int _sendTransitionSerial = 0;

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
    if ((text.isEmpty && _localImagePaths.isEmpty) || !widget.enabled) {
      return;
    }
    _showSendTransition();
    widget.onSend(
      ComposerSubmission(
        text: text,
        localImagePaths: List<String>.unmodifiable(_localImagePaths),
      ),
    );
    _controller.clear();
    setState(() {
      _localImagePaths.clear();
    });
  }

  void _showSendTransition() {
    final serial = _sendTransitionSerial + 1;
    setState(() {
      _sendTransitionSerial = serial;
      _isShowingSendTransition = true;
    });
    Future<void>.delayed(_sendTransitionDuration, () {
      if (!mounted || _sendTransitionSerial != serial) {
        return;
      }
      setState(() {
        _isShowingSendTransition = false;
      });
    });
  }

  bool _isImagePath(String path) {
    final lower = path.toLowerCase();
    return lower.endsWith('.png') ||
        lower.endsWith('.jpg') ||
        lower.endsWith('.jpeg') ||
        lower.endsWith('.gif') ||
        lower.endsWith('.webp') ||
        lower.endsWith('.bmp') ||
        lower.endsWith('.heic') ||
        lower.endsWith('.heif');
  }

  void _addDroppedFiles(List<DropItem> files) {
    if (!widget.enabled) {
      return;
    }
    final next = files
        .map((file) => file.path)
        .where((path) => path.isNotEmpty && _isImagePath(path))
        .toList(growable: false);
    _appendImagePaths(next);
  }

  void _appendImagePaths(List<String> paths) {
    if (paths.isEmpty) {
      return;
    }
    final next = paths
        .where((path) => path.isNotEmpty && _isImagePath(path))
        .toList(growable: false);
    if (next.isEmpty) {
      return;
    }
    setState(() {
      for (final path in next) {
        if (!_localImagePaths.contains(path)) {
          _localImagePaths.add(path);
        }
      }
    });
  }

  Future<void> _pickImages() async {
    if (!widget.enabled || _isPickingImages) {
      return;
    }
    setState(() {
      _isPickingImages = true;
    });
    try {
      final files = await openFiles(
        acceptedTypeGroups: const <XTypeGroup>[
          XTypeGroup(
            label: 'Images',
            extensions: <String>[
              'png',
              'jpg',
              'jpeg',
              'gif',
              'webp',
              'bmp',
              'heic',
              'heif',
            ],
          ),
        ],
      );
      if (!mounted || files.isEmpty) {
        return;
      }
      _appendImagePaths(
        files
            .map((file) => file.path)
            .whereType<String>()
            .toList(growable: false),
      );
    } finally {
      if (mounted) {
        setState(() {
          _isPickingImages = false;
        });
      }
    }
  }

  Future<void> _captureScreenshot() async {
    if (!widget.enabled || _isCapturingScreenshot) {
      return;
    }
    setState(() {
      _isCapturingScreenshot = true;
      _attachmentError = null;
    });
    var requestedAccess = false;
    try {
      if (defaultTargetPlatform == TargetPlatform.macOS) {
        final hasAccess = await screenCapturer.isAccessAllowed();
        if (!hasAccess) {
          requestedAccess = true;
          await screenCapturer.requestAccess();
        }
      }

      final directory = Directory(
        '${Directory.systemTemp.path}/robdex/screenshots',
      );
      if (!directory.existsSync()) {
        directory.createSync(recursive: true);
      }
      final imagePath =
          '${directory.path}/screenshot-${DateTime.now().millisecondsSinceEpoch}.png';

      final captured = await screenCapturer.capture(
        mode: CaptureMode.region,
        imagePath: imagePath,
        copyToClipboard: false,
      );
      if (!mounted) {
        return;
      }

      final capturedPath = captured?.imagePath ?? imagePath;
      final file = File(capturedPath);
      if (captured == null || !file.existsSync()) {
        if (requestedAccess) {
          setState(() {
            _attachmentError =
                'Allow Screen Recording in System Settings, then try again.';
          });
        }
        return;
      }

      _appendImagePaths(<String>[capturedPath]);
      _focusNode.requestFocus();
    } on PlatformException {
      if (!mounted) {
        return;
      }
      setState(() {
        _attachmentError =
            'Allow Screen Recording in System Settings, then try again.';
      });
    } catch (_) {
      if (!mounted) {
        return;
      }
      setState(() {
        _attachmentError = 'Could not capture screenshot.';
      });
    } finally {
      if (mounted) {
        setState(() {
          _isCapturingScreenshot = false;
        });
      }
    }
  }

  String _fileNameFor(String path) {
    final normalized = path.replaceAll('\\', '/');
    final lastSlash = normalized.lastIndexOf('/');
    return lastSlash >= 0 ? normalized.substring(lastSlash + 1) : normalized;
  }

  void _removeImageAt(int index) {
    setState(() {
      _localImagePaths.removeAt(index);
    });
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final isDesktopPlatform = switch (defaultTargetPlatform) {
      TargetPlatform.macOS || TargetPlatform.linux || TargetPlatform.windows => true,
      _ => false,
    };
    final supportsScreenshots = defaultTargetPlatform == TargetPlatform.macOS;
    final showsInterrupt =
        widget.isRunning && !_hasDraftText && !_isShowingSendTransition;
    final showsSendTransition = _isShowingSendTransition;
    final actionBackground = showsInterrupt
        ? theme.colorScheme.error
        : theme.colorScheme.primary;
    final actionForeground = showsInterrupt
        ? theme.colorScheme.onError
        : const Color(0xFF08111A);
    const actionButtonSize = 42.0;

    final panel = DecoratedBox(
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
            if (_localImagePaths.isNotEmpty) ...[
              Wrap(
                spacing: 8,
                runSpacing: 8,
                children: List<Widget>.generate(_localImagePaths.length, (index) {
                  final path = _localImagePaths[index];
                  return DecoratedBox(
                    decoration: BoxDecoration(
                      color: theme.colorScheme.surface.withValues(alpha: 0.72),
                      borderRadius: BorderRadius.circular(14),
                      border: Border.all(
                        color: theme.colorScheme.outline.withValues(alpha: 0.28),
                      ),
                    ),
                    child: Padding(
                      padding: const EdgeInsets.only(left: 10, right: 4, top: 4, bottom: 4),
                      child: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Icon(
                            Icons.image_outlined,
                            size: 15,
                            color: theme.colorScheme.secondary,
                          ),
                          const SizedBox(width: 8),
                          ConstrainedBox(
                            constraints: const BoxConstraints(maxWidth: 180),
                            child: Text(
                              _fileNameFor(path),
                              overflow: TextOverflow.ellipsis,
                              style: theme.textTheme.labelSmall?.copyWith(
                                color: theme.colorScheme.onSurface.withValues(alpha: 0.86),
                              ),
                            ),
                          ),
                          const SizedBox(width: 2),
                          IconButton(
                            onPressed: () => _removeImageAt(index),
                            tooltip: 'Remove',
                            icon: const Icon(Icons.close_rounded, size: 14),
                            visualDensity: VisualDensity.compact,
                            splashRadius: 14,
                          ),
                        ],
                      ),
                    ),
                  );
                }),
              ),
              const SizedBox(height: 10),
            ],
            Row(
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                IconButton(
                  onPressed: widget.enabled && !_isPickingImages ? _pickImages : null,
                  tooltip: 'Add images',
                  icon: _isPickingImages
                      ? SizedBox(
                          width: 16,
                          height: 16,
                          child: CircularProgressIndicator(
                            strokeWidth: 1.8,
                            color: theme.colorScheme.primary,
                          ),
                        )
                      : const Icon(Icons.add_photo_alternate_outlined),
                  visualDensity: VisualDensity.compact,
                ),
                if (supportsScreenshots) ...[
                  const SizedBox(width: 2),
                  IconButton(
                    onPressed: widget.enabled && !_isCapturingScreenshot
                        ? _captureScreenshot
                        : null,
                    tooltip: _isCapturingScreenshot
                        ? 'Select an area to capture'
                        : 'Capture screenshot',
                    icon: _isCapturingScreenshot
                        ? SizedBox(
                            width: 16,
                            height: 16,
                            child: CircularProgressIndicator(
                              strokeWidth: 1.8,
                              color: theme.colorScheme.primary,
                            ),
                          )
                        : const Icon(Icons.screenshot_monitor_outlined),
                    visualDensity: VisualDensity.compact,
                  ),
                ],
                const SizedBox(width: 8),
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
                      ? (showsSendTransition
                            ? () {}
                            : showsInterrupt
                                ? widget.onInterrupt
                                : _submit)
                      : null,
                  icon: AnimatedSwitcher(
                    duration: const Duration(milliseconds: 120),
                    child: showsSendTransition
                        ? ClipOval(
                            key: ValueKey('send-transition-$_sendTransitionSerial'),
                            child: Image.asset(
                              _sendTransitionAsset,
                              width: 28,
                              height: 28,
                              fit: BoxFit.cover,
                              gaplessPlayback: false,
                            ),
                          )
                        : Icon(
                            showsInterrupt
                                ? Icons.stop_rounded
                                : Icons.arrow_upward_rounded,
                            key: ValueKey(showsInterrupt ? 'stop' : 'send'),
                            size: 18,
                          ),
                  ),
                  style: IconButton.styleFrom(
                    backgroundColor: actionBackground,
                    foregroundColor: actionForeground,
                    disabledBackgroundColor:
                        theme.colorScheme.surfaceContainerHighest.withValues(alpha: 0.55),
                    disabledForegroundColor:
                        theme.colorScheme.onSurface.withValues(alpha: 0.4),
                    minimumSize: const Size.square(actionButtonSize),
                    fixedSize: const Size.square(actionButtonSize),
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
            if (_attachmentError != null) ...[
              const SizedBox(height: 10),
              Text(
                _attachmentError!,
                style: theme.textTheme.labelSmall?.copyWith(
                  color: theme.colorScheme.secondary,
                ),
              ),
            ],
          ],
        ),
      ),
    );

    if (!isDesktopPlatform) {
      return panel;
    }

    return DropTarget(
      onDragEntered: (_) {
        if (!_isDesktopDragging) {
          setState(() {
            _isDesktopDragging = true;
          });
        }
      },
      onDragExited: (_) {
        if (_isDesktopDragging) {
          setState(() {
            _isDesktopDragging = false;
          });
        }
      },
      onDragDone: (detail) {
        setState(() {
          _isDesktopDragging = false;
        });
        _addDroppedFiles(detail.files);
      },
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 120),
        curve: Curves.easeOut,
        decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(18),
          color: _isDesktopDragging
              ? theme.colorScheme.primary.withValues(alpha: 0.08)
              : Colors.transparent,
          border: _isDesktopDragging
              ? Border.all(
                  color: theme.colorScheme.primary.withValues(alpha: 0.34),
                )
              : null,
        ),
        padding: _isDesktopDragging ? const EdgeInsets.all(10) : EdgeInsets.zero,
        child: panel,
      ),
    );
  }
}
