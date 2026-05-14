import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:desktop_drop/desktop_drop.dart';
import 'package:file_selector/file_selector.dart';
import 'package:http/http.dart' as http;

import '../../core/models/workbench_models.dart';
import '../inspector/inspector_panel.dart';
import '../requirements/requirement_set_form.dart';
import 'screenshot_capture.dart';

class ComposerSubmission {
  const ComposerSubmission({
    required this.text,
    required this.localImagePaths,
    required this.requirementSetJson,
  });

  final String text;
  final List<String> localImagePaths;
  final String? requirementSetJson;
}

class ComposerPanel extends StatefulWidget {
  const ComposerPanel({
    super.key,
    required this.enabled,
    required this.isRunning,
    required this.onSend,
    required this.onInterrupt,
    required this.selection,
    required this.availableModels,
    required this.onSettingsChanged,
    this.bridgeBaseUri,
    this.terminalAvailable = false,
    this.onTerminalPressed,
  });

  final bool enabled;
  final bool isRunning;
  final ValueChanged<ComposerSubmission> onSend;
  final VoidCallback onInterrupt;
  final WorkspaceSelection selection;
  final List<ModelItem> availableModels;
  final ValueChanged<ThreadSettingsDraft> onSettingsChanged;
  final Uri? bridgeBaseUri;
  final bool terminalAvailable;
  final VoidCallback? onTerminalPressed;

  @override
  State<ComposerPanel> createState() => _ComposerPanelState();
}

class _ComposerPanelState extends State<ComposerPanel> {
  static const _sendTransitionAsset = 'assets/animations/send-transition.gif';
  static const _assetPackage = 'robdex_design_system';
  static const _sendTransitionDuration = Duration(milliseconds: 2500);

  late final TextEditingController _controller;
  final FocusNode _focusNode = FocusNode();
  final List<String> _localImagePaths = <String>[];
  bool _hasDraftText = false;
  bool _isDesktopDragging = false;
  bool _isPickingImages = false;
  bool _isCapturingScreenshot = false;
  bool _isShowingSendTransition = false;
  String? _requirementSetJson;
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
    setState(() {
      _hasDraftText = next;
    });
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
        requirementSetJson: _requirementSetJson,
      ),
    );
    _controller.clear();
    setState(() {
      _localImagePaths.clear();
      _requirementSetJson = null;
    });
  }

  Future<void> _editRequirements() async {
    final result = await showRequirementSetFormDialog(
      context,
      initialJson: _requirementSetJson,
      title: 'Attach Requirements',
      actionLabel: 'Attach',
      helperText:
          'These requirements apply to the next new turn only. They cannot be attached while the thread is running.',
      bridgeBaseUri: widget.bridgeBaseUri,
    );
    if (!mounted || result == null) {
      return;
    }
    setState(() {
      _requirementSetJson = result.trim().isEmpty ? null : result.trim();
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
      _attachmentError = null;
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
      if (kIsWeb) {
        await _uploadWebImages(files);
      } else {
        _appendImagePaths(
          files
              .map((file) => file.path)
              .whereType<String>()
              .toList(growable: false),
        );
      }
    } catch (_) {
      if (mounted) {
        setState(() {
          _attachmentError = 'Could not upload image attachment.';
        });
      }
    } finally {
      if (mounted) {
        setState(() {
          _isPickingImages = false;
        });
      }
    }
  }

  Future<void> _uploadWebImages(List<XFile> files) async {
    final baseUri = widget.bridgeBaseUri;
    if (baseUri == null) {
      setState(() {
        _attachmentError = 'Bridge URL is unavailable for image upload.';
      });
      return;
    }

    final uploaded = <String>[];
    for (final file in files) {
      final filename = file.name.trim().isEmpty ? 'image' : file.name.trim();
      final uploadUri = baseUri.resolve('/uploads/images/instant').replace(
        queryParameters: {'filename': filename},
      );
      final bytes = await file.readAsBytes();
      final response = await http.post(
        uploadUri,
        headers: {'content-type': _contentTypeFor(filename)},
        body: bytes,
      );
      if (response.statusCode < 200 || response.statusCode >= 300) {
        throw StateError('Image upload failed with ${response.statusCode}');
      }
      final payload = jsonDecode(response.body);
      final savedPath = payload is Map<String, dynamic> ? payload['path'] as String? : null;
      if (savedPath == null || savedPath.trim().isEmpty) {
        throw StateError('Image upload response missing path');
      }
      uploaded.add(savedPath);
    }

    _appendImagePaths(uploaded);
  }

  String _contentTypeFor(String path) {
    final lower = path.toLowerCase();
    if (lower.endsWith('.jpg') || lower.endsWith('.jpeg')) {
      return 'image/jpeg';
    }
    if (lower.endsWith('.gif')) {
      return 'image/gif';
    }
    if (lower.endsWith('.webp')) {
      return 'image/webp';
    }
    if (lower.endsWith('.bmp')) {
      return 'image/bmp';
    }
    return 'image/png';
  }

  Future<void> _captureScreenshot() async {
    if (!widget.enabled || _isCapturingScreenshot) {
      return;
    }
    setState(() {
      _isCapturingScreenshot = true;
      _attachmentError = null;
    });
    try {
      final capturedPath = await captureRobdexScreenshot();
      if (!mounted) {
        return;
      }
      if (capturedPath == null) {
        setState(() {
          _attachmentError =
              'Allow Screen Recording in System Settings, then try again.';
        });
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

  Uri? _thumbnailUriFor(String path) {
    final baseUri = widget.bridgeBaseUri;
    if (baseUri == null || path.trim().isEmpty) {
      return null;
    }
    return baseUri.resolve('/images/thumbnail').replace(
      queryParameters: {'saved_path': path},
    );
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
    final supportsPathAttachments = !kIsWeb;
    final supportsImagePicker = supportsPathAttachments || widget.bridgeBaseUri != null;
    final supportsScreenshots = defaultTargetPlatform == TargetPlatform.macOS;
    final showsInterrupt =
        widget.isRunning && !_hasDraftText && !_isShowingSendTransition;
    final showsSendTransition = _isShowingSendTransition;
    final actionBackground = showsInterrupt
        ? const Color(0xFFE9EAEC)
        : theme.colorScheme.primary;
    final actionForeground = showsInterrupt
        ? const Color(0xFF12151A)
        : const Color(0xFF08111A);
    const actionButtonSize = 42.0;

    final panel = DecoratedBox(
      decoration: BoxDecoration(
        color: theme.colorScheme.surface.withValues(alpha: 0.38),
        borderRadius: BorderRadius.circular(18),
        border: Border.all(
          color: theme.colorScheme.outline.withValues(alpha: 0.62),
        ),
      ),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(12, 10, 12, 10),
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
                          Builder(
                            builder: (context) {
                              final thumbnailUri = kIsWeb ? _thumbnailUriFor(path) : null;
                              if (thumbnailUri == null) {
                                return Icon(
                                  Icons.image_outlined,
                                  size: 15,
                                  color: theme.colorScheme.secondary,
                                );
                              }
                              return ClipRRect(
                                borderRadius: BorderRadius.circular(6),
                                child: Image.network(
                                  thumbnailUri.toString(),
                                  width: 22,
                                  height: 22,
                                  fit: BoxFit.cover,
                                  errorBuilder: (_, _, _) => Icon(
                                    Icons.image_outlined,
                                    size: 15,
                                    color: theme.colorScheme.secondary,
                                  ),
                                ),
                              );
                            },
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
            Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Focus(
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
                    child: Semantics(
                      key: const ValueKey('semantic.composer.messageInput'),
                      container: true,
                      textField: true,
                      enabled: widget.enabled,
                      label: 'Chat message input',
                      value: _controller.text,
                      child: ExcludeSemantics(
                        child: TextField(
                          controller: _controller,
                          focusNode: _focusNode,
                          enabled: widget.enabled,
                          minLines: 2,
                          maxLines: 5,
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
                            border: InputBorder.none,
                            enabledBorder: InputBorder.none,
                            focusedBorder: InputBorder.none,
                            disabledBorder: InputBorder.none,
                            filled: false,
                            isDense: true,
                            hintText: 'Message selected thread...',
                            hintStyle: theme.textTheme.bodySmall?.copyWith(
                              fontSize: 11,
                              color: theme.colorScheme.onSurface.withValues(alpha: 0.48),
                            ),
                            contentPadding: const EdgeInsets.symmetric(
                              horizontal: 12,
                              vertical: 10,
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                const SizedBox(height: 8),
                Row(
                  children: [
                    Semantics(
                      key: const ValueKey('semantic.composer.addMenu'),
                      container: true,
                      button: true,
                      enabled: widget.enabled,
                      label: 'Open composer attachment menu',
                      child: ExcludeSemantics(
                        child: PopupMenuButton<String>(
                          enabled: widget.enabled,
                          tooltip: 'Add',
                          icon: const Icon(Icons.add_rounded),
                          onSelected: (value) {
                            switch (value) {
                              case 'image':
                                _pickImages();
                                break;
                              case 'screenshot':
                                _captureScreenshot();
                                break;
                              case 'requirements':
                                _editRequirements();
                                break;
                            }
                          },
                          itemBuilder: (context) => [
                            if (supportsImagePicker)
                              const PopupMenuItem(
                                value: 'image',
                                child: ListTile(
                                  leading: Icon(Icons.add_photo_alternate_outlined),
                                  title: Text('Add image'),
                                  dense: true,
                                ),
                              ),
                            if (supportsPathAttachments && supportsScreenshots)
                              const PopupMenuItem(
                                value: 'screenshot',
                                child: ListTile(
                                  leading: Icon(Icons.screenshot_monitor_outlined),
                                  title: Text('Take screenshot'),
                                  dense: true,
                                ),
                              ),
                            PopupMenuItem(
                              value: 'requirements',
                              enabled: !widget.isRunning,
                              child: ListTile(
                                leading: Icon(
                                  _requirementSetJson == null
                                      ? Icons.rule_outlined
                                      : Icons.rule_rounded,
                                ),
                                title: Text(
                                  _requirementSetJson == null
                                      ? 'Add requirements'
                                      : 'Edit requirements',
                                ),
                                dense: true,
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                    const SizedBox(width: 10),
                    Expanded(
                      child: _ComposerSettingsControls(
                        enabled: widget.enabled && widget.selection.threadId != null,
                        selection: widget.selection,
                        availableModels: widget.availableModels,
                        onSettingsChanged: widget.onSettingsChanged,
                        terminalAvailable: widget.terminalAvailable,
                        onTerminalPressed: widget.onTerminalPressed,
                      ),
                    ),
                    Semantics(
                      key: ValueKey(
                        showsInterrupt
                            ? 'semantic.composer.interrupt'
                            : 'semantic.composer.send',
                      ),
                      container: true,
                      button: true,
                      enabled: widget.enabled,
                      label: showsInterrupt ? 'Interrupt running thread' : 'Send message',
                      child: ExcludeSemantics(
                        child: IconButton(
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
                                      package: _assetPackage,
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
                      ),
                    ),
                  ],
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

    if (!isDesktopPlatform || !supportsPathAttachments) {
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

class _ComposerSettingsControls extends StatelessWidget {
  const _ComposerSettingsControls({
    required this.enabled,
    required this.selection,
    required this.availableModels,
    required this.onSettingsChanged,
    required this.terminalAvailable,
    required this.onTerminalPressed,
  });

  final bool enabled;
  final WorkspaceSelection selection;
  final List<ModelItem> availableModels;
  final ValueChanged<ThreadSettingsDraft> onSettingsChanged;
  final bool terminalAvailable;
  final VoidCallback? onTerminalPressed;

  @override
  Widget build(BuildContext context) {
    final modelItems = <PopupMenuEntry<String>>[
      PopupMenuItem(
        value: '',
        child: Text(_modelLabel(selection.effectiveModel, availableModels)),
      ),
      ...availableModels.map(
        (model) => PopupMenuItem(
          value: model.id,
          child: Text((model.name?.isEmpty ?? true) ? model.id : model.name!),
        ),
      ),
    ];

    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      child: Row(
        children: [
          _ComposerDropdownControl(
            enabled: enabled,
            label: 'Model',
            value: _shortModelLabel(selection.model ?? selection.effectiveModel),
            icon: Icons.smart_toy_outlined,
            maxWidth: 190,
            items: modelItems,
            onSelected: (value) => onSettingsChanged(_draft(modelId: value)),
          ),
          _ComposerDropdownControl(
            enabled: enabled,
            label: 'Reasoning',
            icon: Icons.signal_cellular_alt_rounded,
            customIcon: _ReasoningBars(
              effort: selection.reasoningEffort ??
                  selection.effectiveReasoningEffort ??
                  '',
            ),
            items: [
              PopupMenuItem(
                value: '',
                child: _ReasoningMenuRow(effort: '', label: 'System'),
              ),
              PopupMenuItem(
                value: 'low',
                child: _ReasoningMenuRow(effort: 'low', label: 'Low'),
              ),
              PopupMenuItem(
                value: 'medium',
                child: _ReasoningMenuRow(effort: 'medium', label: 'Medium'),
              ),
              PopupMenuItem(
                value: 'high',
                child: _ReasoningMenuRow(effort: 'high', label: 'High'),
              ),
            ],
            onSelected: (value) => onSettingsChanged(_draft(reasoningEffort: value)),
          ),
          _ComposerDropdownControl(
            enabled: enabled,
            label: 'Service tier',
            icon: Icons.pets_rounded,
            glyph: '🐢',
            items: const [
              PopupMenuItem(value: '', child: Text('(System)')),
              PopupMenuItem(value: 'fast', child: Text('fast')),
              PopupMenuItem(value: 'flex', child: Text('flex')),
            ],
            onSelected: (value) => onSettingsChanged(_draft(serviceTier: value)),
          ),
          _ComposerDropdownControl(
            enabled: enabled,
            label: 'Role',
            icon: Icons.engineering_rounded,
            glyph: '👷',
            items: const [
              PopupMenuItem(value: 'worker', child: Text('worker')),
              PopupMenuItem(value: 'designer', child: Text('designer')),
              PopupMenuItem(value: 'qa', child: Text('qa')),
              PopupMenuItem(value: 'operator', child: Text('operator')),
              PopupMenuItem(value: 'orchestrator', child: Text('orchestrator')),
              PopupMenuItem(value: 'hidden', child: Text('hidden')),
            ],
            onSelected: (value) => onSettingsChanged(_draft(role: value)),
          ),
          _ComposerDropdownControl(
            enabled: enabled,
            label: 'Sandbox',
            icon: Icons.shield_outlined,
            maxWidth: 52,
            items: const [
              PopupMenuItem(value: '', child: Text('(System)')),
              PopupMenuItem(value: 'workspace-write', child: Text('workspace-write')),
              PopupMenuItem(value: 'danger-full-access', child: Text('danger-full-access')),
            ],
            onSelected: (value) => onSettingsChanged(_draft(sandboxMode: value)),
          ),
          _ComposerDropdownControl(
            key: const ValueKey('semantic.composer.networkDropdown'),
            enabled: enabled,
            label: 'Network',
            icon: (selection.networkAccess ?? selection.effectiveNetworkAccess ?? false)
                ? Icons.wifi_rounded
                : Icons.wifi_off_rounded,
            items: const [
              PopupMenuItem(value: 'default', child: Text('(System)')),
              PopupMenuItem(value: 'enabled', child: Text('Enabled')),
              PopupMenuItem(value: 'disabled', child: Text('Disabled')),
            ],
            onSelected: (value) => onSettingsChanged(_draft(networkAccessMode: value)),
          ),
          if (terminalAvailable)
            Padding(
              padding: const EdgeInsets.only(right: 6),
              child: Semantics(
                key: const ValueKey('semantic.composer.terminal'),
                container: true,
                button: true,
                enabled: enabled && onTerminalPressed != null,
                label: 'Open integrated terminal',
                child: ExcludeSemantics(
                  child: IconButton(
                    onPressed: enabled ? onTerminalPressed : null,
                    icon: const Icon(Icons.terminal_rounded, size: 15),
                    tooltip: 'Open terminal',
                    style: IconButton.styleFrom(
                      minimumSize: const Size.square(31),
                      fixedSize: const Size.square(31),
                      padding: EdgeInsets.zero,
                      backgroundColor: Theme.of(context)
                          .colorScheme
                          .surface
                          .withValues(alpha: 0.26),
                      foregroundColor: Theme.of(context)
                          .colorScheme
                          .onSurface
                          .withValues(alpha: enabled ? 0.82 : 0.32),
                      disabledForegroundColor: Theme.of(context)
                          .colorScheme
                          .onSurface
                          .withValues(alpha: 0.32),
                      shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(8),
                        side: BorderSide(
                          color: Theme.of(context)
                              .colorScheme
                              .outline
                              .withValues(alpha: 0.34),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }

  ThreadSettingsDraft _draft({
    String? role,
    String? approvalPolicy,
    String? sandboxMode,
    String? networkAccessMode,
    String? modelId,
    String? reasoningEffort,
    String? serviceTier,
  }) {
    return ThreadSettingsDraft(
      role: role ?? (selection.threadRole ?? 'worker'),
      approvalPolicy: approvalPolicy ?? (selection.approvalPolicy ?? ''),
      sandboxMode: sandboxMode ?? (selection.sandboxMode ?? ''),
      networkAccessMode: networkAccessMode ??
          (selection.networkAccess == null
              ? 'default'
              : (selection.networkAccess! ? 'enabled' : 'disabled')),
      modelId: modelId ?? (selection.model ?? ''),
      reasoningEffort: reasoningEffort ?? (selection.reasoningEffort ?? ''),
      serviceTier: serviceTier ?? (selection.serviceTier ?? ''),
    );
  }

  String _modelLabel(String? modelId, List<ModelItem> models) {
    final id = modelId?.trim();
    if (id == null || id.isEmpty) {
      return '(System)';
    }
    for (final model in models) {
      if (model.id == id) {
        return (model.name?.isEmpty ?? true) ? model.id : model.name!;
      }
    }
    return id;
  }

  String _shortModelLabel(String? modelId) {
    final id = modelId?.trim();
    if (id == null || id.isEmpty) {
      return 'Model';
    }
    return id.toUpperCase().replaceFirst('GPT-', 'GPT-');
  }
}

class _ComposerDropdownControl extends StatelessWidget {
  const _ComposerDropdownControl({
    super.key,
    required this.enabled,
    required this.label,
    required this.icon,
    required this.items,
    required this.onSelected,
    this.value,
    this.glyph,
    this.customIcon,
    this.maxWidth,
  });

  final bool enabled;
  final String label;
  final String? value;
  final String? glyph;
  final Widget? customIcon;
  final IconData icon;
  final double? maxWidth;
  final List<PopupMenuEntry<String>> items;
  final ValueChanged<String> onSelected;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.only(right: 6),
      child: Semantics(
        container: true,
        button: true,
        enabled: enabled,
        label: label,
        child: ExcludeSemantics(
          child: PopupMenuButton<String>(
            enabled: enabled,
            tooltip: label,
            onSelected: onSelected,
            itemBuilder: (context) => items,
            child: ConstrainedBox(
              constraints: BoxConstraints(maxWidth: maxWidth ?? double.infinity),
              child: DecoratedBox(
              decoration: BoxDecoration(
                color: theme.colorScheme.surface.withValues(alpha: 0.26),
                borderRadius: BorderRadius.circular(8),
                border: Border.all(
                  color: theme.colorScheme.outline.withValues(alpha: 0.34),
                ),
              ),
              child: Padding(
                padding: EdgeInsets.symmetric(
                  horizontal: value == null ? 8 : 10,
                  vertical: 6,
                ),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    if (customIcon != null)
                      customIcon!
                    else if (glyph == null)
                      Icon(
                        icon,
                        size: 13,
                        color: theme.colorScheme.onSurface
                            .withValues(alpha: enabled ? 0.82 : 0.32),
                      )
                    else
                      Text(
                        glyph!,
                        style: TextStyle(
                          fontSize: 12,
                          color: theme.colorScheme.onSurface
                              .withValues(alpha: enabled ? 0.86 : 0.32),
                        ),
                      ),
                    if (value != null) ...[
                      const SizedBox(width: 6),
                      Flexible(
                        child: Text(
                          value!,
                          overflow: TextOverflow.ellipsis,
                          softWrap: false,
                          style: theme.textTheme.labelSmall?.copyWith(
                            fontWeight: FontWeight.w800,
                            color: theme.colorScheme.onSurface
                                .withValues(alpha: enabled ? 0.88 : 0.36),
                          ),
                        ),
                      ),
                    ],
                    const SizedBox(width: 6),
                    Icon(
                      Icons.keyboard_arrow_down_rounded,
                      size: 13,
                      color: theme.colorScheme.onSurface
                          .withValues(alpha: enabled ? 0.62 : 0.26),
                    ),
                  ],
                ),
              ),
            ),
            ),
          ),
        ),
      ),
    );
  }
}

class _ReasoningMenuRow extends StatelessWidget {
  const _ReasoningMenuRow({required this.effort, required this.label});

  final String effort;
  final String label;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        _ReasoningBars(effort: effort),
        const SizedBox(width: 12),
        Text(
          label,
          style: theme.textTheme.bodyMedium?.copyWith(
            fontWeight: FontWeight.w600,
          ),
        ),
      ],
    );
  }
}

class _ReasoningBars extends StatelessWidget {
  const _ReasoningBars({required this.effort});

  final String effort;

  @override
  Widget build(BuildContext context) {
    final level = switch (effort) {
      'high' => 3,
      'medium' => 2,
      'low' => 1,
      _ => 0,
    };
    final activeColor = level == 0
        ? const Color(0xFFFFB238)
        : const Color(0xFF4FC36A);
    final inactiveColor = activeColor.withValues(alpha: 0.28);
    return SizedBox(
      width: 16,
      height: 14,
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: List.generate(3, (index) {
          final bar = index + 1;
          return Padding(
            padding: const EdgeInsets.only(right: 2),
            child: DecoratedBox(
              decoration: BoxDecoration(
                color: level == 0 || bar <= level ? activeColor : inactiveColor,
                borderRadius: BorderRadius.circular(2),
              ),
              child: SizedBox(
                width: 3,
                height: 5 + (index * 4),
              ),
            ),
          );
        }),
      ),
    );
  }
}
