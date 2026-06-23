import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:desktop_drop/desktop_drop.dart';
import 'package:file_selector/file_selector.dart';

import '../../core/models/workbench_models.dart';
import '../inspector/inspector_panel.dart';
import '../requirements/requirement_set_form.dart';
import 'screenshot_capture.dart';
import 'slash_commands.dart';

const _handoffPrompt =
    'Please give yourself a warm handoff for a new agent who may resume this thread. '
    'Pass along your current identity, role, responsibilities, active objective, '
    'recent important work, files or systems touched, current state, known blockers, '
    'validation or review status, live-state cautions, and the next best actions. '
    'If there is no active work to resume, summarize the recent context and anything '
    'the next agent should be aware of.';

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
    required this.onCompactThread,
    this.requirementReview,
    this.loadRequirementComposables,
    this.setThreadRequirements,
    this.uploadImageBytes,
    this.contextWindowRemainingPercent,
    this.terminalAvailable = false,
    this.onTerminalPressed,
    this.disabledHint = 'Select a thread to enable the composer.',
    this.placeholder = 'Message selected thread...',
    this.statusMessage,
  });

  final bool enabled;
  final bool isRunning;
  final ValueChanged<ComposerSubmission> onSend;
  final VoidCallback onInterrupt;
  final WorkspaceSelection selection;
  final List<ModelItem> availableModels;
  final ValueChanged<ThreadSettingsDraft> onSettingsChanged;
  final VoidCallback onCompactThread;
  final RequirementReviewSummary? requirementReview;
  final RequirementComposableLoader? loadRequirementComposables;
  final Future<void> Function(String requirementSetJson)? setThreadRequirements;
  final ImageBytesUploader? uploadImageBytes;
  final int? contextWindowRemainingPercent;
  final bool terminalAvailable;
  final VoidCallback? onTerminalPressed;
  final String disabledHint;
  final String placeholder;
  final String? statusMessage;

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
  final Map<String, Uint8List> _localImagePreviewBytes = <String, Uint8List>{};
  bool _hasDraftText = false;
  bool _isDesktopDragging = false;
  bool _isPickingImages = false;
  bool _isCapturingScreenshot = false;
  bool _isShowingSendTransition = false;
  String? _attachmentError;
  String? _dismissedSlashText;
  OverlayEntry? _slashFeedbackOverlay;
  int _selectedSlashIndex = 0;
  int _slashFeedbackSerial = 0;
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
    _slashFeedbackOverlay?.remove();
    _controller.dispose();
    _focusNode.dispose();
    super.dispose();
  }

  void _handleDraftChanged() {
    final next = _controller.text.trim().isNotEmpty;
    setState(() {
      _hasDraftText = next;
      if (_dismissedSlashText != _controller.text) {
        _dismissedSlashText = null;
      }
      final suggestions = _slashSuggestionState;
      if (suggestions == null || _selectedSlashIndex >= suggestions.options.length) {
        _selectedSlashIndex = 0;
      }
    });
  }

  void _submit() {
    final rawText = _controller.text;
    final text = rawText.trim();
    if ((text.isEmpty && _localImagePaths.isEmpty) || !widget.enabled) {
      return;
    }
    if (_tryExecuteSlashCommand(rawText)) {
      return;
    }
    _showSendTransition();
    widget.onSend(
      ComposerSubmission(
        text: text,
        localImagePaths: List<String>.unmodifiable(_localImagePaths),
        requirementSetJson: null,
      ),
    );
    _controller.clear();
    setState(() {
      _localImagePaths.clear();
      _localImagePreviewBytes.clear();
    });
  }

  SlashCommandSuggestionState? get _slashSuggestionState {
    final text = _controller.text;
    if (text == _dismissedSlashText) {
      return null;
    }
    return slashCommandSuggestions(
      text,
      selection: widget.selection,
      availableModels: widget.availableModels,
    );
  }

  bool _tryExecuteSlashCommand(String text) {
    final command = parseCompleteSlashCommand(
      text,
      selection: widget.selection,
      availableModels: widget.availableModels,
    );
    if (command == null) {
      return false;
    }
    if (widget.selection.threadId == null || !widget.enabled) {
      _showSlashFeedback('Select a thread first');
      return true;
    }
    switch (command.kind) {
      case SlashCommandKind.model:
        widget.onSettingsChanged(_settingsDraft(modelId: command.argument ?? ''));
        _finishSlashCommand('Model set to ${command.argument}');
        return true;
      case SlashCommandKind.reasoning:
        widget.onSettingsChanged(_settingsDraft(reasoningEffort: command.argument ?? ''));
        _finishSlashCommand('Reasoning set to ${command.argument}');
        return true;
      case SlashCommandKind.role:
        widget.onSettingsChanged(_settingsDraft(role: command.argument ?? 'worker'));
        _finishSlashCommand('Role set to ${command.argument}');
        return true;
      case SlashCommandKind.sandbox:
        widget.onSettingsChanged(_settingsDraft(sandboxMode: command.argument ?? ''));
        _finishSlashCommand('Sandbox set to ${command.argument}');
        return true;
      case SlashCommandKind.approval:
        widget.onSettingsChanged(_settingsDraft(approvalPolicy: command.argument ?? ''));
        _finishSlashCommand('Approval set to ${command.argument}');
        return true;
      case SlashCommandKind.compact:
        widget.onCompactThread();
        _finishSlashCommand('Compaction requested');
        return true;
      case SlashCommandKind.handoff:
        _controller.value = TextEditingValue(
          text: _handoffPrompt,
          selection: const TextSelection.collapsed(offset: _handoffPrompt.length),
        );
        setState(() {
          _dismissedSlashText = null;
          _selectedSlashIndex = 0;
        });
        _showSlashFeedback('Handoff prompt inserted');
        return true;
    }
  }

  void _finishSlashCommand(String message) {
    _controller.clear();
    setState(() {
      _dismissedSlashText = null;
      _selectedSlashIndex = 0;
    });
    _showSlashFeedback(message);
  }

  void _showSlashFeedback(String message) {
    final serial = _slashFeedbackSerial + 1;
    _slashFeedbackSerial = serial;
    _slashFeedbackOverlay?.remove();
    _slashFeedbackOverlay = OverlayEntry(
      builder: (context) => Positioned(
        left: 24,
        right: 24,
        bottom: 130,
        child: IgnorePointer(
          child: _SlashFeedbackToast(message: message),
        ),
      ),
    );
    Overlay.of(context).insert(_slashFeedbackOverlay!);
    Future<void>.delayed(const Duration(milliseconds: 1600), () {
      if (!mounted || _slashFeedbackSerial != serial) {
        return;
      }
      _slashFeedbackOverlay?.remove();
      _slashFeedbackOverlay = null;
    });
  }

  ThreadSettingsDraft _settingsDraft({
    String? role,
    String? approvalPolicy,
    String? sandboxMode,
    String? networkAccessMode,
    String? modelId,
    String? reasoningEffort,
    String? serviceTier,
  }) {
    return ThreadSettingsDraft(
      role: role ?? (widget.selection.threadRole ?? 'worker'),
      approvalPolicy: approvalPolicy ?? (widget.selection.approvalPolicy ?? ''),
      sandboxMode: sandboxMode ?? (widget.selection.sandboxMode ?? ''),
      networkAccessMode: networkAccessMode ??
          (widget.selection.networkAccess == null
              ? 'default'
              : (widget.selection.networkAccess! ? 'enabled' : 'disabled')),
      modelId: modelId ?? (widget.selection.model ?? ''),
      reasoningEffort: reasoningEffort ?? (widget.selection.reasoningEffort ?? ''),
      serviceTier: serviceTier ?? (widget.selection.serviceTier ?? ''),
    );
  }

  void _completeSlashSelection(SlashCommandSuggestionState suggestions, int index) {
    if (suggestions.options.isEmpty) {
      return;
    }
    final option = suggestions.options[index.clamp(0, suggestions.options.length - 1).toInt()];
    if (suggestions.command == null || !_controller.text.contains(' ')) {
      final definition = slashCommandDefinitions.firstWhere(
        (definition) => definition.name == option.value,
      );
      _controller.value = TextEditingValue(
        text: definition.requiresArgument ? '/${option.value} ' : '/${option.value}',
        selection: TextSelection.collapsed(
          offset: definition.requiresArgument ? option.value.length + 2 : option.value.length + 1,
        ),
      );
      if (!definition.requiresArgument) {
        _tryExecuteSlashCommand(_controller.text);
      }
      return;
    }
    final nextText = '/${suggestions.command!.name} ${option.value}';
    _controller.value = TextEditingValue(
      text: nextText,
      selection: TextSelection.collapsed(offset: nextText.length),
    );
  }

  KeyEventResult _handleComposerKey(
    KeyEvent event, {
    required bool isDesktopPlatform,
  }) {
    if (!isDesktopPlatform || !widget.enabled || event is! KeyDownEvent) {
      return KeyEventResult.ignored;
    }
    final suggestions = _slashSuggestionState;
    if (suggestions != null) {
      if (event.logicalKey == LogicalKeyboardKey.escape) {
        setState(() {
          _dismissedSlashText = _controller.text;
          _selectedSlashIndex = 0;
        });
        return KeyEventResult.handled;
      }
      if (event.logicalKey == LogicalKeyboardKey.arrowDown && suggestions.options.isNotEmpty) {
        setState(() {
          _selectedSlashIndex = (_selectedSlashIndex + 1) % suggestions.options.length;
        });
        return KeyEventResult.handled;
      }
      if (event.logicalKey == LogicalKeyboardKey.arrowUp && suggestions.options.isNotEmpty) {
        setState(() {
          _selectedSlashIndex =
              (_selectedSlashIndex - 1 + suggestions.options.length) % suggestions.options.length;
        });
        return KeyEventResult.handled;
      }
      if (event.logicalKey == LogicalKeyboardKey.tab) {
        _completeSlashSelection(suggestions, _selectedSlashIndex);
        return KeyEventResult.handled;
      }
    }
    final isEnter =
        event.logicalKey == LogicalKeyboardKey.enter ||
        event.logicalKey == LogicalKeyboardKey.numpadEnter;
    if (!isEnter || HardwareKeyboard.instance.isShiftPressed) {
      return KeyEventResult.ignored;
    }
    if (suggestions != null) {
      if (parseCompleteSlashCommand(
            _controller.text.trim(),
            selection: widget.selection,
            availableModels: widget.availableModels,
          ) !=
          null) {
        _submit();
      } else {
        _completeSlashSelection(suggestions, _selectedSlashIndex);
      }
      return KeyEventResult.handled;
    }
    _submit();
    return KeyEventResult.handled;
  }

  Future<void> _editRequirements() async {
    final storedRequirements = widget.requirementReview?.storedRequirementCount ?? 0;
    final hasStoredRequirements = storedRequirements > 0;
    final requirementsActive = widget.requirementReview?.requirementSetActive ?? false;
    final initialJson = hasStoredRequirements && widget.requirementReview != null
        ? requirementSetJsonFromReviewSummary(widget.requirementReview!)
        : null;
    final result = await showRequirementSetFormDialog(
      context,
      initialJson: initialJson,
      title: hasStoredRequirements ? 'Replace Requirements' : 'Set Requirements',
      actionLabel: hasStoredRequirements ? 'Replace' : 'Set',
      helperText: hasStoredRequirements
          ? 'Replace, activate, deactivate, or clear the stored Requirements for this thread.'
          : 'Set active Requirements for this thread without sending a message.',
      showActivationToggle: hasStoredRequirements,
      requirementsActive: requirementsActive,
      recipientThreadId: widget.selection.threadId,
      projectPath: widget.selection.projectRootPath,
      loadComposableItems: widget.loadRequirementComposables,
      uploadImageBytes: widget.uploadImageBytes,
    );
    if (!mounted || result == null) {
      return;
    }
    await _setThreadRequirements(result.trim());
  }

  Future<void> _setThreadRequirements(String requirementSetJson) async {
    final sourceId = widget.selection.threadId;
    final setRequirements = widget.setThreadRequirements;
    if (sourceId == null || setRequirements == null) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Select a bridge-backed thread first.')),
      );
      return;
    }
    try {
      await setRequirements(requirementSetJson);
      if (!mounted) {
        return;
      }
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Requirements updated.')),
      );
    } catch (error) {
      if (!mounted) {
        return;
      }
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Set requirements failed: $error')),
      );
    }
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

  void _appendImagePaths(
    List<String> paths, {
    Map<String, Uint8List> previewBytes = const <String, Uint8List>{},
  }) {
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
        final bytes = previewBytes[path];
        if (bytes != null) {
          _localImagePreviewBytes[path] = bytes;
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
        final paths = <String>[];
        final previews = <String, Uint8List>{};
        for (final file in files) {
          final path = file.path;
          if (path.isEmpty) {
            continue;
          }
          paths.add(path);
          try {
            previews[path] = await file.readAsBytes();
          } catch (_) {
            // Keep the attachment usable even when preview bytes are unavailable.
          }
        }
        _appendImagePaths(paths, previewBytes: previews);
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
    final upload = widget.uploadImageBytes;
    if (upload == null) {
      setState(() {
        _attachmentError = 'Image upload is unavailable.';
      });
      return;
    }

    final uploaded = <String>[];
    final previews = <String, Uint8List>{};
    for (final file in files) {
      final filename = file.name.trim().isEmpty ? 'image' : file.name.trim();
      final bytes = await file.readAsBytes();
      final savedPath = await upload(
        filename: filename,
        contentType: _contentTypeFor(filename),
        bytes: bytes,
      );
      uploaded.add(savedPath);
      previews[savedPath] = bytes;
    }

    _appendImagePaths(uploaded, previewBytes: previews);
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

      final previews = <String, Uint8List>{};
      try {
        previews[capturedPath] = await XFile(capturedPath).readAsBytes();
      } catch (_) {
        // Keep the captured attachment usable even when preview bytes are unavailable.
      }
      _appendImagePaths(<String>[capturedPath], previewBytes: previews);
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
      final removed = _localImagePaths.removeAt(index);
      _localImagePreviewBytes.remove(removed);
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
    final supportsImagePicker = supportsPathAttachments || widget.uploadImageBytes != null;
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

    final suggestions = _slashSuggestionState;
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
                  final previewBytes = _localImagePreviewBytes[path];
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
                          _AttachmentPreview(
                            bytes: previewBytes,
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
            Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                CallbackShortcuts(
                  bindings: <ShortcutActivator, VoidCallback>{
                    if (suggestions != null) ...{
                      const SingleActivator(LogicalKeyboardKey.tab): () {
                        final current = _slashSuggestionState;
                        if (current != null) {
                          _completeSlashSelection(current, _selectedSlashIndex);
                        }
                      },
                      const SingleActivator(LogicalKeyboardKey.arrowDown): () {
                        final current = _slashSuggestionState;
                        if (current != null && current.options.isNotEmpty) {
                          setState(() {
                            _selectedSlashIndex =
                                (_selectedSlashIndex + 1) % current.options.length;
                          });
                        }
                      },
                      const SingleActivator(LogicalKeyboardKey.arrowUp): () {
                        final current = _slashSuggestionState;
                        if (current != null && current.options.isNotEmpty) {
                          setState(() {
                            _selectedSlashIndex =
                                (_selectedSlashIndex - 1 + current.options.length) %
                                    current.options.length;
                          });
                        }
                      },
                      const SingleActivator(LogicalKeyboardKey.escape): () {
                        if (_slashSuggestionState != null) {
                          setState(() {
                            _dismissedSlashText = _controller.text;
                            _selectedSlashIndex = 0;
                          });
                        }
                      },
                    },
                    const SingleActivator(LogicalKeyboardKey.enter): () {
                      _submit();
                    },
                    const SingleActivator(LogicalKeyboardKey.numpadEnter): () {
                      _submit();
                    },
                  },
                  child: Focus(
                    onKeyEvent: (node, event) => _handleComposerKey(
                      event,
                      isDesktopPlatform: isDesktopPlatform,
                    ),
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
                            hintText: widget.placeholder,
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
                                break; // TODO: Add warm handoff button here and use _handoffPrompt 
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
                                  (widget.requirementReview?.storedRequirementCount ?? 0) == 0
                                      ? Icons.rule_outlined
                                      : Icons.rule_rounded,
                                ),
                                title: Text(
                                  (widget.requirementReview?.storedRequirementCount ?? 0) == 0
                                      ? 'Add requirements'
                                      : 'Replace Requirements',
                                ),
                                dense: true,
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                    const SizedBox(width: 10),
                    _ContextWindowDonut(
                      percentRemaining: widget.contextWindowRemainingPercent,
                    ),
                    const SizedBox(width: 10),
	                    Expanded(
	                      child: _ComposerSettingsControls(
	                        enabled: widget.enabled && widget.selection.threadId != null,
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
                widget.disabledHint,
                style: theme.textTheme.labelSmall?.copyWith(
                  color: theme.colorScheme.secondary,
                ),
              ),
            ],
            if (widget.enabled && widget.statusMessage != null && widget.statusMessage!.isNotEmpty) ...[
              const SizedBox(height: 10),
              Text(
                widget.statusMessage!,
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

    final stackedPanel = Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (suggestions != null) ...[
          _SlashCommandMenu(
            suggestions: suggestions,
            selectedIndex: _selectedSlashIndex,
            onSelected: (index) {
              _completeSlashSelection(suggestions, index);
              final text = _controller.text;
              if (parseCompleteSlashCommand(
                    text,
                    selection: widget.selection,
                    availableModels: widget.availableModels,
                  ) !=
                  null) {
                _tryExecuteSlashCommand(text);
              }
            },
          ),
          const SizedBox(height: 6),
        ],
        panel,
      ],
    );

	    if (!isDesktopPlatform || !supportsPathAttachments) {
	      return stackedPanel;
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
	        child: stackedPanel,
	      ),
	    );
	  }
}

class _AttachmentPreview extends StatelessWidget {
  const _AttachmentPreview({required this.bytes, required this.color});

  final Uint8List? bytes;
  final Color color;

  @override
  Widget build(BuildContext context) {
    final imageBytes = bytes;
    if (imageBytes == null || imageBytes.isEmpty) {
      return Icon(Icons.image_outlined, size: 15, color: color);
    }
    return ClipRRect(
      borderRadius: BorderRadius.circular(6),
      child: Image.memory(
        imageBytes,
        width: 24,
        height: 24,
        fit: BoxFit.cover,
        gaplessPlayback: true,
        errorBuilder: (_, _, _) =>
            Icon(Icons.image_outlined, size: 15, color: color),
      ),
    );
  }
}

class _ContextWindowDonut extends StatelessWidget {
  const _ContextWindowDonut({required this.percentRemaining});

  final int? percentRemaining;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final raw = percentRemaining;
    final int? clamped = raw?.clamp(0, 100).toInt();
    final fraction = clamped == null ? 0.0 : clamped / 100.0;
    final color = switch (clamped) {
      null => theme.colorScheme.outline,
      <= 15 => theme.colorScheme.error,
      <= 35 => Colors.amber.shade800,
      _ => theme.colorScheme.secondary,
    };
    final label = clamped == null
        ? 'Context remaining unavailable'
        : '$clamped percent context remaining';
    return Tooltip(
      message: clamped == null ? 'Context remaining unavailable' : '$clamped% remaining',
      child: Semantics(
        label: label,
        image: true,
        child: SizedBox.square(
          dimension: 28,
          child: CustomPaint(
            painter: _ContextWindowDonutPainter(
              fraction: fraction,
              color: color,
              trackColor: theme.colorScheme.outline.withValues(alpha: 0.22),
            ),
            child: Center(
              child: Container(
                width: 6,
                height: 6,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: color.withValues(alpha: clamped == null ? 0.38 : 0.95),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _ContextWindowDonutPainter extends CustomPainter {
  const _ContextWindowDonutPainter({
    required this.fraction,
    required this.color,
    required this.trackColor,
  });

  final double fraction;
  final Color color;
  final Color trackColor;

  @override
  void paint(Canvas canvas, Size size) {
    final center = Offset(size.width / 2, size.height / 2);
    final radius = (size.shortestSide - 4) / 2;
    final stroke = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 4
      ..strokeCap = StrokeCap.round;
    canvas.drawCircle(center, radius, stroke..color = trackColor);
    if (fraction <= 0) {
      return;
    }
    canvas.drawArc(
      Rect.fromCircle(center: center, radius: radius),
      -1.5708,
      fraction * 6.28318,
      false,
      stroke..color = color,
    );
  }

  @override
  bool shouldRepaint(covariant _ContextWindowDonutPainter oldDelegate) {
    return oldDelegate.fraction != fraction ||
        oldDelegate.color != color ||
        oldDelegate.trackColor != trackColor;
  }
}

class _ComposerSettingsControls extends StatelessWidget {
  const _ComposerSettingsControls({
    required this.enabled,
    required this.terminalAvailable,
    required this.onTerminalPressed,
  });

  final bool enabled;
  final bool terminalAvailable;
  final VoidCallback? onTerminalPressed;

  @override
  Widget build(BuildContext context) {
    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      child: Row(
        children: [
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
}

class _SlashCommandMenu extends StatelessWidget {
  const _SlashCommandMenu({
    required this.suggestions,
    required this.selectedIndex,
    required this.onSelected,
  });

  final SlashCommandSuggestionState suggestions;
  final int selectedIndex;
  final ValueChanged<int> onSelected;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final options = suggestions.options.take(6).toList(growable: false);
    return Align(
      alignment: Alignment.bottomLeft,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 360),
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: theme.colorScheme.surface.withValues(alpha: 0.96),
            borderRadius: BorderRadius.circular(12),
            border: Border.all(
              color: theme.colorScheme.outline.withValues(alpha: 0.42),
            ),
            boxShadow: [
              BoxShadow(
                color: Colors.black.withValues(alpha: 0.22),
                blurRadius: 18,
                offset: const Offset(0, 10),
              ),
            ],
          ),
          child: Padding(
            padding: const EdgeInsets.symmetric(vertical: 6),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                for (var index = 0; index < options.length; index++)
                  _SlashCommandMenuRow(
                    option: options[index],
                    selected: index == selectedIndex,
                    onTap: () => onSelected(index),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _SlashCommandMenuRow extends StatelessWidget {
  const _SlashCommandMenuRow({
    required this.option,
    required this.selected,
    required this.onTap,
  });

  final SlashCommandOption option;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return InkWell(
      onTap: onTap,
      child: Container(
        key: ValueKey('slash.option.${option.value}'),
        color: selected
            ? theme.colorScheme.primary.withValues(alpha: 0.14)
            : Colors.transparent,
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 7),
        child: Row(
          children: [
            Expanded(
              child: Text(
                option.label,
                overflow: TextOverflow.ellipsis,
                style: theme.textTheme.labelMedium?.copyWith(
                  fontWeight: FontWeight.w800,
                  color: theme.colorScheme.onSurface.withValues(alpha: 0.9),
                ),
              ),
            ),
            if (option.current)
              DecoratedBox(
                decoration: BoxDecoration(
                  color: theme.colorScheme.secondary.withValues(alpha: 0.16),
                  borderRadius: BorderRadius.circular(999),
                ),
                child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 2),
                  child: Text(
                    'CURRENT',
                    style: theme.textTheme.labelSmall?.copyWith(
                      fontSize: 9,
                      fontWeight: FontWeight.w900,
                      color: theme.colorScheme.secondary,
                    ),
                  ),
                ),
              ),
          ],
        ),
      ),
    );
  }
}

class _SlashFeedbackToast extends StatelessWidget {
  const _SlashFeedbackToast({required this.message});

  final String message;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Align(
      alignment: Alignment.bottomLeft,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: theme.colorScheme.inverseSurface.withValues(alpha: 0.94),
          borderRadius: BorderRadius.circular(999),
          boxShadow: [
            BoxShadow(
              color: Colors.black.withValues(alpha: 0.18),
              blurRadius: 14,
              offset: const Offset(0, 8),
            ),
          ],
        ),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 7),
          child: Text(
            message,
            key: const ValueKey('slash.feedback'),
            style: theme.textTheme.labelSmall?.copyWith(
              color: theme.colorScheme.onInverseSurface,
              fontWeight: FontWeight.w800,
            ),
          ),
        ),
      ),
    );
  }
}
