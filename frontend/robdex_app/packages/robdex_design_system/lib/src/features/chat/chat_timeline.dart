import 'dart:async';
import 'dart:convert';
import 'package:flutter_markdown_plus/flutter_markdown_plus.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:highlight/highlight.dart' as hl;

import '../../core/formatters/timestamps.dart';
import '../../core/models/workbench_models.dart';
import '../composer/composer_panel.dart';
import '../inspector/inspector_panel.dart';
import '../requirements/requirement_set_form.dart';

const smartRadius = 6.0;

class ChatTimeline extends StatefulWidget {
  const ChatTimeline({
    super.key,
    required this.threadId,
    required this.entries,
    required this.title,
    required this.contextWindowRemainingPercent,
    required this.onSend,
    required this.onInterrupt,
    required this.composerEnabled,
    required this.isRunning,
    this.selection,
    this.availableModels = const [],
    this.onSettingsChanged,
    this.onCompactThread,
    this.showComposer = true,
    this.headerControls,
    this.overlay,
    this.leading,
    this.loadRequirementComposables,
    this.setThreadRequirements,
    this.uploadImageBytes,
    this.loadFullSizeImage,
    this.onOpenLink,
    this.onTerminateCommandExecution,
    this.requirementReview,
    this.onOpenThread,
    this.terminalAvailable = false,
    this.onTerminalPressed,
    this.composerDisabledHint,
    this.composerPlaceholder,
    this.composerStatusMessage,
  });

  final String? threadId;
  final List<ChatEntry> entries;
  final String title;
  final int? contextWindowRemainingPercent;
  final ValueChanged<ComposerSubmission> onSend;
  final VoidCallback onInterrupt;
  final bool composerEnabled;
  final bool isRunning;
  final WorkspaceSelection? selection;
  final List<ModelItem> availableModels;
  final ValueChanged<ThreadSettingsDraft>? onSettingsChanged;
  final VoidCallback? onCompactThread;
  final bool showComposer;
  final Widget? headerControls;
  final Widget? overlay;
  final Widget? leading;
  final RequirementComposableLoader? loadRequirementComposables;
  final Future<void> Function(String requirementSetJson)? setThreadRequirements;
  final ImageBytesUploader? uploadImageBytes;
  final FullSizeImageLoader? loadFullSizeImage;
  final ValueChanged<String>? onOpenLink;
  final ValueChanged<String>? onTerminateCommandExecution;
  final RequirementReviewSummary? requirementReview;
  final ValueChanged<String>? onOpenThread;
  final bool terminalAvailable;
  final VoidCallback? onTerminalPressed;
  final String? composerDisabledHint;
  final String? composerPlaceholder;
  final String? composerStatusMessage;

  @override
  State<ChatTimeline> createState() => _ChatTimelineState();
}

class _ChatTimelineState extends State<ChatTimeline> {
  late final _TimelineAutoScrollController _autoScrollController;
  final Set<String> _expandedEntryKeys = <String>{};

  @override
  void initState() {
    super.initState();
    _autoScrollController = _TimelineAutoScrollController();
  }

  @override
  void dispose() {
    _autoScrollController.dispose();
    super.dispose();
  }

  @override
  void didUpdateWidget(covariant ChatTimeline oldWidget) {
    super.didUpdateWidget(oldWidget);

    final currentKeys = widget.entries.map(_entryStorageKey).toSet();
    _expandedEntryKeys.removeWhere((key) => !currentKeys.contains(key));

    final threadChanged = widget.threadId != oldWidget.threadId;
    final entriesChanged =
        widget.entries.length != oldWidget.entries.length ||
        !_sameEntryIdentity(widget.entries, oldWidget.entries);

    if (!threadChanged && !entriesChanged) {
      return;
    }

    if (threadChanged) {
      WidgetsBinding.instance.scheduleFrameCallback((_) {
        if (mounted) {
          _autoScrollController.jumpToAnchor();
        }
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        DecoratedBox(
          decoration: BoxDecoration(
            gradient: LinearGradient(
              begin: Alignment.topCenter,
              end: Alignment.bottomCenter,
              colors: [
                const Color(0xFF171C22).withValues(alpha: 0.5),
                const Color(0xFF171C22).withValues(alpha: 0.18),
                const Color(0xFF171C22).withValues(alpha: 0.0),
              ],
              stops: const [0.0, 0.52, 1.0],
            ),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                widget.title,
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                style: theme.textTheme.bodyLarge?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
              const SizedBox(height: 8),
              Row(
                crossAxisAlignment: CrossAxisAlignment.center,
                children: [
                  if (widget.leading != null) widget.leading!,
                  const Spacer(),
                  if (widget.headerControls != null)
                    DefaultTextStyle.merge(
                      style:
                          theme.textTheme.labelSmall?.copyWith(
                            color: theme.colorScheme.onSurface.withValues(
                              alpha: 0.74,
                            ),
                          ) ??
                          const TextStyle(),
                      child: widget.headerControls!,
                    ),
                ],
              ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        Divider(
          height: 1,
          color: theme.colorScheme.outline.withValues(alpha: 0.64),
        ),
        if (widget.requirementReview != null) ...[
          const SizedBox(height: 10),
          _RequirementsReviewInlineBanner(
            summary: widget.requirementReview!,
            selectedThreadRole: widget.selection?.threadRole,
            onOpenThread: widget.onOpenThread,
          ),
        ],
        const SizedBox(height: 18),
        Expanded(
          child: Stack(
            children: [
              _TimelineAutoScroller<String>(
                controller: _autoScrollController,
                lengthIdentifier: _timelineLengthIdentifier(widget.entries),
                anchorThreshold: 50,
                builder: (context, scrollController) {
                  return ListView.separated(
                    key: PageStorageKey<String>(
                      'chat-timeline-${widget.threadId ?? 'none'}',
                    ),
                    controller: scrollController,
                    itemCount: widget.entries.length,
                    separatorBuilder: (_, _) => const SizedBox(height: 6),
                    itemBuilder: (context, index) {
                      final entry = widget.entries[index];
                      final entryKey = _entryStorageKey(entry);
                      return _ChatBubble(
                        key: ValueKey(entryKey),
                        entry: entry,
                        expanded: _expandedEntryKeys.contains(entryKey),
                        onExpandedChanged: (expanded) {
                          setState(() {
                            if (expanded) {
                              _expandedEntryKeys.add(entryKey);
                            } else {
                              _expandedEntryKeys.remove(entryKey);
                            }
                          });
                        },
                        onTerminateCommandExecution:
                            widget.onTerminateCommandExecution,
                        onOpenLink: widget.onOpenLink,
                        loadFullSizeImage: widget.loadFullSizeImage,
                        onSend: widget.onSend,
                      );
                    },
                  );
                },
              ),
              if (widget.overlay != null)
                Positioned(top: 0, left: 0, right: 0, child: widget.overlay!),
            ],
          ),
        ),
        if (widget.showComposer) ...[
          const SizedBox(height: 10),
          ComposerPanel(
            enabled: widget.composerEnabled,
            isRunning: widget.isRunning,
            selection: widget.selection ?? _emptySelection,
            availableModels: widget.availableModels,
            onSettingsChanged: widget.onSettingsChanged ?? (_) {},
            onCompactThread: widget.onCompactThread ?? () {},
            requirementReview: widget.requirementReview,
            loadRequirementComposables: widget.loadRequirementComposables,
            setThreadRequirements: widget.setThreadRequirements,
            uploadImageBytes: widget.uploadImageBytes,
            contextWindowRemainingPercent: widget.contextWindowRemainingPercent,
            terminalAvailable: widget.terminalAvailable,
            onTerminalPressed: widget.onTerminalPressed,
            disabledHint: widget.composerDisabledHint ?? 'Select a thread to enable the composer.',
            placeholder: widget.composerPlaceholder ?? 'Message selected thread...',
            statusMessage: widget.composerStatusMessage,
            onSend: widget.onSend,
            onInterrupt: widget.onInterrupt,
          ),
        ],
      ],
    );
  }
}


const WorkspaceSelection _emptySelection = WorkspaceSelection(
  projectId: null,
  projectRootPath: null,
  projectOrchestratorThreadId: null,
  projectOrchestratorName: null,
  threadId: null,
  threadRole: null,
  projectName: 'No Project',
  threadName: 'No Thread Selected',
  connectionLabel: 'Bridge Unknown',
);

typedef _TimelineScrollWidgetBuilder =
    Widget Function(BuildContext context, ScrollController scrollController);

class _TimelineAutoScrollController extends ChangeNotifier
    implements ValueListenable<bool> {
  _TimelineAutoScrollController({ScrollController? scrollController})
    : scrollController = scrollController ?? ScrollController(),
      _ownsScrollController = scrollController == null;

  final bool _ownsScrollController;
  final ScrollController scrollController;
  bool _anchored = false;

  bool get anchored => _anchored;

  @override
  bool get value => anchored;

  set anchored(bool value) {
    if (_anchored == value) {
      return;
    }
    _anchored = value;
    notifyListeners();
  }

  void updateAnchoredFromMetrics(ScrollMetrics metrics, double threshold) {
    anchored = metrics.maxScrollExtent - metrics.pixels <= threshold;
  }

  Future<void> jumpToAnchor() =>
      _goToLazyAnchor(scrollController.position.jumpTo);

  Future<void> _goToLazyAnchor(
    FutureOr<void> Function(double position) move,
  ) async {
    anchored = true;
    while (true) {
      if (!scrollController.hasClients) {
        return;
      }
      final targetPosition = scrollController.position.maxScrollExtent;
      await move(targetPosition);
      await WidgetsBinding.instance.endOfFrame;
      if (!scrollController.hasClients) {
        return;
      }
      final position = scrollController.position;
      if (position.pixels == position.maxScrollExtent ||
          position.pixels != targetPosition) {
        break;
      }
    }
  }

  @override
  void dispose() {
    if (_ownsScrollController) {
      scrollController.dispose();
    }
    super.dispose();
  }
}

class _TimelineAutoScroller<T> extends StatefulWidget {
  const _TimelineAutoScroller({
    required this.controller,
    required this.lengthIdentifier,
    required this.anchorThreshold,
    required this.builder,
  });

  final _TimelineAutoScrollController controller;
  final T lengthIdentifier;
  final double anchorThreshold;
  final _TimelineScrollWidgetBuilder builder;

  @override
  State<_TimelineAutoScroller<T>> createState() =>
      _TimelineAutoScrollerState<T>();
}

class _TimelineAutoScrollerState<T> extends State<_TimelineAutoScroller<T>> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.scheduleFrameCallback((_) {
      if (mounted) {
        widget.controller.jumpToAnchor();
      }
    });
  }

  @override
  void didUpdateWidget(covariant _TimelineAutoScroller<T> oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.lengthIdentifier != oldWidget.lengthIdentifier &&
        widget.controller.anchored) {
      WidgetsBinding.instance.scheduleFrameCallback((_) {
        if (mounted) {
          widget.controller.jumpToAnchor();
        }
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return NotificationListener<SizeChangedLayoutNotification>(
      onNotification: (_) {
        if (widget.controller.anchored) {
          WidgetsBinding.instance.scheduleFrameCallback((_) {
            if (mounted) {
              widget.controller.jumpToAnchor();
            }
          });
        }
        return false;
      },
      child: NotificationListener<ScrollNotification>(
        onNotification: (notification) {
          if (notification.depth != 0) {
            return false;
          }
          if (notification is ScrollEndNotification ||
              notification is UserScrollNotification &&
                  notification.direction == ScrollDirection.idle) {
            widget.controller.updateAnchoredFromMetrics(
              notification.metrics,
              widget.anchorThreshold,
            );
          }
          return false;
        },
        child: SizeChangedLayoutNotifier(
          child: widget.builder(context, widget.controller.scrollController),
        ),
      ),
    );
  }
}

class _RequirementsReviewInlineBanner extends StatelessWidget {
  const _RequirementsReviewInlineBanner({
    required this.summary,
    required this.selectedThreadRole,
    required this.onOpenThread,
  });

  final RequirementReviewSummary summary;
  final String? selectedThreadRole;
  final ValueChanged<String>? onOpenThread;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final reviewerThreadId = summary.reviewerThreadId;
    final parentThreadId = summary.parentThreadId;
    final isReviewerThread =
        selectedThreadRole == 'requirements-reviewer' ||
        selectedThreadRole == 'requirementsReviewer';
    final isWaiverRequired = summary.status == 'waiverRequired';
    final targetThreadId = isReviewerThread ? parentThreadId : reviewerThreadId;
    final buttonLabel = isReviewerThread
        ? 'Back to source thread'
        : 'Open review thread';
    final statusText = isReviewerThread
        ? 'Nested requirements reviewer'
        : isWaiverRequired
        ? 'Human waiver required'
        : 'Requirements ${summary.displayStatus.toLowerCase()}';
    final counts = isWaiverRequired
        ? 'Waiver needed · ${summary.activeRequirementCount} active'
        : summary.verdicts.isEmpty
        ? '${summary.activeRequirementCount} active'
        : '${summary.passedCount} passed · ${summary.failedCount} failed · ${summary.blockedCount} blocked';
    final accentColor = isWaiverRequired
        ? Colors.amber.shade700
        : theme.colorScheme.primary;

    return Semantics(
      key: const ValueKey('semantic.requirementsReview.inline'),
      container: true,
      label: statusText,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: isWaiverRequired
              ? const Color(0xFF231C09)
              : const Color(0xFF111923),
          borderRadius: BorderRadius.circular(smartRadius),
          border: Border.all(
            color: isWaiverRequired
                ? accentColor.withValues(alpha: 0.64)
                : theme.colorScheme.outline.withValues(alpha: 0.72),
          ),
        ),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 9),
          child: Row(
            children: [
              Icon(
                isReviewerThread
                    ? Icons.rate_review_outlined
                    : isWaiverRequired
                    ? Icons.policy_outlined
                    : Icons.rule_folder_outlined,
                color: accentColor,
                size: 16,
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      statusText,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: theme.textTheme.labelMedium?.copyWith(
                        color: theme.colorScheme.onSurface.withValues(
                          alpha: 0.88,
                        ),
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    const SizedBox(height: 2),
                    Text(
                      counts,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: theme.textTheme.labelSmall?.copyWith(
                        color: theme.colorScheme.onSurface.withValues(
                          alpha: 0.52,
                        ),
                      ),
                    ),
                  ],
                ),
              ),
              if (targetThreadId != null &&
                  targetThreadId.isNotEmpty &&
                  onOpenThread != null)
                TextButton.icon(
                  onPressed: () => onOpenThread!(targetThreadId),
                  icon: Icon(
                    isReviewerThread
                        ? Icons.arrow_back_rounded
                        : Icons.open_in_new_rounded,
                    size: 14,
                  ),
                  label: Text(buttonLabel),
                ),
            ],
          ),
        ),
      ),
    );
  }
}

bool _sameEntryIdentity(List<ChatEntry> a, List<ChatEntry> b) {
  if (a.length != b.length) {
    return false;
  }
  for (var i = 0; i < a.length; i += 1) {
    if (_entryStorageKey(a[i]) != _entryStorageKey(b[i]) ||
        a[i].body != b[i].body ||
        a[i].output != b[i].output ||
        a[i].status != b[i].status ||
        a[i].isStreaming != b[i].isStreaming) {
      return false;
    }
  }
  return true;
}

String _entryStorageKey(ChatEntry entry) {
  final stableId = entry.id.trim().isNotEmpty
      ? entry.id.trim()
      : '${entry.kind}|${entry.timestamp ?? 0}|${entry.processId ?? ''}|${entry.command ?? entry.body}';
  return stableId;
}

String _timelineLengthIdentifier(List<ChatEntry> entries) {
  if (entries.isEmpty) {
    return 'empty';
  }
  final last = entries.last;
  return [
    entries.length,
    _entryStorageKey(last),
    last.body.length,
    last.output?.length ?? 0,
    last.status ?? '',
    last.isStreaming,
  ].join('|');
}

class _ChatBubble extends StatelessWidget {
  const _ChatBubble({
    super.key,
    required this.entry,
    required this.expanded,
    required this.onExpandedChanged,
    this.onTerminateCommandExecution,
    this.onOpenLink,
    this.loadFullSizeImage,
    this.onSend,
  });

  final ChatEntry entry;
  final bool expanded;
  final ValueChanged<bool> onExpandedChanged;
  final ValueChanged<String>? onTerminateCommandExecution;
  final ValueChanged<String>? onOpenLink;
  final FullSizeImageLoader? loadFullSizeImage;
  final ValueChanged<ComposerSubmission>? onSend;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final timestampLabel = formatLocalTimeLabel(entry.timestamp);
    final isConversation =
        !entry.isTool &&
        (entry.author == 'User' ||
            entry.author == 'Assistant' ||
            entry.author == 'Operator');

    if (entry.hasPlanItems) {
      return _PlanUpdateCard(entry: entry, fallbackItems: const <PlanChecklistItem>[]);
    }

    if (!isConversation) {
      return _InlineEventRow(
        entry: entry,
        expanded: expanded,
        onExpandedChanged: onExpandedChanged,
        onTerminateCommandExecution: onTerminateCommandExecution,
        loadFullSizeImage: loadFullSizeImage,
      );
    }

    final semanticCard = entry.semanticCard;
    if (semanticCard != null && semanticCard.kind == 'plannerResponse') {
      return _PlannerResponseCard(
        entry: entry,
        card: semanticCard,
        onPick: (label) {
          onSend?.call(
            ComposerSubmission(
              text: 'I pick: $label',
              localImagePaths: const [],
              requirementSetJson: null,
            ),
          );
        },
      );
    }
    if (semanticCard != null) {
      return _SemanticMessageCard(entry: entry, card: semanticCard);
    }

    final isUser = entry.author == 'User' || entry.author == 'Operator';
    final isPending = entry.deliveryState == 'pending';
    final bubbleColor = isUser
        ? theme.colorScheme.primary.withValues(alpha: isPending ? 0.08 : 0.13)
        : theme.colorScheme.surface.withValues(alpha: 0.76);

    return Align(
      alignment: isUser ? Alignment.centerRight : Alignment.centerLeft,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 680),
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: bubbleColor,
            borderRadius: BorderRadius.circular(smartRadius),
            border: Border.all(
              color: isUser
                  ? theme.colorScheme.primary.withValues(
                      alpha: isPending ? 0.18 : 0.28,
                    )
                  : theme.colorScheme.outline.withValues(alpha: 0.72),
            ),
          ),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Expanded(
                      child: Wrap(
                        crossAxisAlignment: WrapCrossAlignment.center,
                        spacing: 6,
                        runSpacing: 2,
                        children: [
                          Text(
                            entry.displayLabel,
                            style: theme.textTheme.labelSmall?.copyWith(
                              fontWeight: FontWeight.w600,
                              color: isPending
                                  ? theme.colorScheme.onSurface.withValues(
                                      alpha: 0.68,
                                    )
                                  : theme.colorScheme.onSurface.withValues(
                                      alpha: 0.8,
                                    ),
                            ),
                          ),
                          Text(
                            timestampLabel,
                            style: theme.textTheme.labelSmall?.copyWith(
                              color: theme.colorScheme.onSurface.withValues(
                                alpha: 0.58,
                              ),
                            ),
                          ),
                          if (isPending)
                            Text(
                              'Sending...',
                              style: theme.textTheme.labelSmall?.copyWith(
                                color: theme.colorScheme.secondary,
                                fontStyle: FontStyle.italic,
                              ),
                            ),
                          if (entry.isStreaming)
                            const SizedBox(
                              width: 8,
                              height: 8,
                              child: CircularProgressIndicator(
                                strokeWidth: 1.5,
                              ),
                            ),
                        ],
                      ),
                    ),
                    Semantics(
                      key: ValueKey('semantic.chat.copy.${entry.id}'),
                      container: true,
                      button: true,
                      label: 'Copy message text',
                      child: ExcludeSemantics(
                        child: IconButton(
                          onPressed: () => _copyBubbleText(context, entry.body),
                          icon: const Icon(
                            Icons.content_copy_rounded,
                            size: 14,
                          ),
                          tooltip: 'Copy',
                          splashRadius: 14,
                          padding: EdgeInsets.zero,
                          constraints: const BoxConstraints.tightFor(
                            width: 22,
                            height: 22,
                          ),
                          visualDensity: VisualDensity.compact,
                        ),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 4),
                if (entry.author == 'Assistant' && entry.isStreaming)
                  Text(
                    key: ValueKey('chat.streamingPlainText.${entry.id}'),
                    entry.body,
                    style: theme.textTheme.bodyMedium?.copyWith(
                      color: theme.colorScheme.onSurface.withValues(alpha: 0.9),
                      height: 1.4,
                    ),
                  )
                else
                  MarkdownBody(
                    key: ValueKey('chat.markdownBody.${entry.id}'),
                    data: entry.body,
                    selectable: false,
                    onTapLink: (text, href, title) {
                      final target = href ?? text;
                      if (target.trim().isNotEmpty) {
                        onOpenLink?.call(target.trim());
                      }
                    },
                    styleSheet: _conversationMarkdownStyle(theme, isPending),
                    syntaxHighlighter: _ChatCodeSyntaxHighlighter(theme),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _PlanUpdateCard extends StatelessWidget {
  const _PlanUpdateCard({required this.entry, this.fallbackItems = const []});

  final ChatEntry entry;
  final List<PlanChecklistItem> fallbackItems;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final timestampLabel = formatLocalTimeLabel(entry.timestamp);
    final note = _planSummary(entry.body);
    final accent = entry.isStreaming
        ? Colors.amber.shade700
        : theme.colorScheme.primary;
    final items = entry.hasPlanItems ? entry.planItems : fallbackItems;

    return Align(
      alignment: Alignment.centerLeft,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 720),
        child: DecoratedBox(
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(smartRadius),
            color: Color.alphaBlend(
              accent.withValues(alpha: 0.07),
              theme.colorScheme.surface,
            ),
            border: Border.all(color: accent.withValues(alpha: 0.22)),
          ),
          child: Padding(
            padding: const EdgeInsets.fromLTRB(16, 14, 16, 16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Container(
                      width: 28,
                      height: 28,
                      decoration: BoxDecoration(
                        color: accent.withValues(alpha: 0.14),
                        borderRadius: BorderRadius.circular(9),
                      ),
                      child: Icon(
                        Icons.checklist_rounded,
                        size: 16,
                        color: accent,
                      ),
                    ),
                    const SizedBox(width: 10),
                    Expanded(
                      child: Text(
                        'Plan',
                        style: theme.textTheme.titleSmall?.copyWith(
                          fontWeight: FontWeight.w800,
                        ),
                      ),
                    ),
                    Text(
                      timestampLabel,
                      style: theme.textTheme.labelSmall?.copyWith(
                        color: theme.colorScheme.onSurface.withValues(
                          alpha: 0.62,
                        ),
                      ),
                    ),
                  ],
                ),
                if (note != null) ...[
                  const SizedBox(height: 12),
                  Text(
                    note,
                    style: theme.textTheme.bodySmall?.copyWith(
                      height: 1.4,
                      color: theme.colorScheme.onSurface.withValues(
                        alpha: 0.82,
                      ),
                    ),
                  ),
                ],
                const SizedBox(height: 14),
                ...items.asMap().entries.map(
                  (entry) => Padding(
                    padding: EdgeInsets.only(
                      bottom: entry.key == items.length - 1 ? 0 : 10,
                    ),
                    child: _PlanChecklistRow(item: entry.value),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _PlanChecklistRow extends StatelessWidget {
  const _PlanChecklistRow({required this.item});

  final PlanChecklistItem item;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final accent = item.completed
        ? Colors.green.shade700
        : item.isInProgress
        ? Colors.amber.shade800
        : theme.colorScheme.onSurface.withValues(alpha: 0.52);
    final icon = item.completed
        ? Icons.check_circle_rounded
        : item.isInProgress
        ? Icons.radio_button_checked_rounded
        : Icons.radio_button_unchecked_rounded;

    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.only(top: 1),
          child: _AnimatedStatusIcon(
            icon: icon,
            color: accent,
            identity: '${item.status}|${item.completed}',
            size: 18,
          ),
        ),
        const SizedBox(width: 10),
        Expanded(
          child: TweenAnimationBuilder<Color?>(
            tween: ColorTween(
              end: item.completed
                  ? theme.colorScheme.onSurface.withValues(alpha: 0.7)
                  : theme.colorScheme.onSurface.withValues(alpha: 0.92),
            ),
            duration: const Duration(milliseconds: 150),
            curve: Curves.easeOutCubic,
            builder: (context, animatedColor, child) {
              return Text(
                item.text,
                style: theme.textTheme.bodyMedium?.copyWith(
                  height: 1.34,
                  color: animatedColor,
                  decoration: item.completed
                      ? TextDecoration.lineThrough
                      : null,
                  decorationColor: accent.withValues(alpha: 0.6),
                ),
              );
            },
          ),
        ),
      ],
    );
  }
}

class _SemanticMessageCard extends StatelessWidget {
  const _SemanticMessageCard({required this.entry, required this.card});

  final ChatEntry entry;
  final ChatSemanticCard card;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final timestampLabel = formatLocalTimeLabel(entry.timestamp);
    final accent = _semanticToneColor(theme, card.tone);
    return Align(
      alignment: Alignment.centerLeft,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 760),
        child: DecoratedBox(
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(smartRadius),
            color: Color.alphaBlend(
              accent.withValues(alpha: 0.08),
              theme.colorScheme.surface,
            ),
            border: Border.all(color: accent.withValues(alpha: 0.28)),
          ),
          child: Padding(
            padding: const EdgeInsets.fromLTRB(14, 12, 14, 14),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Icon(_semanticIcon(card.icon), color: accent, size: 18),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        card.title,
                        style: theme.textTheme.titleSmall?.copyWith(
                          fontWeight: FontWeight.w800,
                        ),
                      ),
                    ),
                    if (card.statusLabel != null) ...[
                      Container(
                        margin: const EdgeInsets.only(right: 8),
                        padding: const EdgeInsets.symmetric(
                          horizontal: 7,
                          vertical: 3,
                        ),
                        decoration: BoxDecoration(
                          color: accent.withValues(alpha: 0.1),
                          borderRadius: BorderRadius.circular(999),
                          border: Border.all(
                            color: accent.withValues(alpha: 0.24),
                          ),
                        ),
                        child: Text(
                          card.statusLabel!,
                          style: theme.textTheme.labelSmall?.copyWith(
                            color: accent,
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                      ),
                    ],
                    Text(
                      timestampLabel,
                      style: theme.textTheme.labelSmall?.copyWith(
                        color: theme.colorScheme.onSurface.withValues(
                          alpha: 0.58,
                        ),
                      ),
                    ),
                  ],
                ),
                if (card.summary.trim().isNotEmpty) ...[
                  const SizedBox(height: 10),
                  Text(
                    card.summary.trim(),
                    style: theme.textTheme.bodySmall?.copyWith(
                      height: 1.36,
                      color: theme.colorScheme.onSurface.withValues(
                        alpha: 0.82,
                      ),
                    ),
                  ),
                ],
                if (card.rows.isNotEmpty) const SizedBox(height: 12),
                ...card.rows.map((row) => _SemanticMessageRow(row: row)),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _SemanticMessageRow extends StatelessWidget {
  const _SemanticMessageRow({required this.row});

  final ChatSemanticRow row;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final rowColor = _semanticToneColor(theme, row.tone);
    return Padding(
      padding: const EdgeInsets.only(bottom: 10),
      child: DecoratedBox(
        decoration: BoxDecoration(
          border: Border(left: BorderSide(color: rowColor, width: 3)),
        ),
        child: Padding(
          padding: const EdgeInsets.only(left: 10),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  Icon(_semanticIcon(row.icon), color: rowColor, size: 16),
                  const SizedBox(width: 7),
                  Expanded(
                    child: Text(
                      row.title,
                      style: theme.textTheme.labelLarge?.copyWith(
                        fontWeight: FontWeight.w800,
                      ),
                    ),
                  ),
                  if (row.trailingLabel != null)
                    Text(
                      row.trailingLabel!,
                      style: theme.textTheme.labelSmall?.copyWith(
                        color: rowColor,
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                ],
              ),
              if (row.summary.trim().isNotEmpty) ...[
                const SizedBox(height: 5),
                Text(
                  row.summary.trim(),
                  style: theme.textTheme.bodySmall?.copyWith(
                    height: 1.34,
                    color: theme.colorScheme.onSurface.withValues(alpha: 0.78),
                  ),
                ),
              ],
              if (row.detail?.trim().isNotEmpty == true) ...[
                const SizedBox(height: 5),
                Text(
                  row.detail!.trim(),
                  style: theme.textTheme.bodySmall?.copyWith(
                    height: 1.32,
                    color: theme.colorScheme.onSurface.withValues(alpha: 0.68),
                  ),
                ),
              ],
              if (row.bullets.isNotEmpty) ...[
                const SizedBox(height: 6),
                for (final item in row.bullets)
                  Padding(
                    padding: const EdgeInsets.only(bottom: 3),
                    child: Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          '• ',
                          style: theme.textTheme.bodySmall?.copyWith(
                            color: theme.colorScheme.onSurface.withValues(alpha: 0.56),
                          ),
                        ),
                        Expanded(
                          child: Text(
                            item,
                            style: theme.textTheme.bodySmall?.copyWith(
                              height: 1.32,
                              color: theme.colorScheme.onSurface.withValues(alpha: 0.68),
                            ),
                          ),
                        ),
                      ],
                    ),
                  ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

class _AnimatedStatusIcon extends StatelessWidget {
  const _AnimatedStatusIcon({
    required this.icon,
    required this.color,
    required this.identity,
    this.size = 16,
  });

  final IconData icon;
  final Color color;
  final Object identity;
  final double size;

  @override
  Widget build(BuildContext context) {
    return TweenAnimationBuilder<Color?>(
      tween: ColorTween(end: color),
      duration: const Duration(milliseconds: 150),
      curve: Curves.easeOutCubic,
      builder: (context, animatedColor, child) {
        return AnimatedSwitcher(
          duration: const Duration(milliseconds: 140),
          switchInCurve: Curves.easeOutCubic,
          switchOutCurve: Curves.easeOutCubic,
          transitionBuilder: (child, animation) => FadeTransition(
            opacity: animation,
            child: ScaleTransition(scale: animation, child: child),
          ),
          child: Icon(
            icon,
            key: ValueKey(identity),
            size: size,
            color: animatedColor,
          ),
        );
      },
    );
  }
}

class _AnimatedStatusDot extends StatelessWidget {
  const _AnimatedStatusDot({required this.color});

  final Color color;

  @override
  Widget build(BuildContext context) {
    return TweenAnimationBuilder<Color?>(
      tween: ColorTween(end: color),
      duration: const Duration(milliseconds: 150),
      curve: Curves.easeOutCubic,
      builder: (context, animatedColor, child) {
        return Container(
          width: 6,
          height: 6,
          margin: const EdgeInsets.only(top: 6),
          decoration: BoxDecoration(
            color: animatedColor,
            shape: BoxShape.circle,
          ),
        );
      },
    );
  }
}

void _copyBubbleText(BuildContext context, String text) {
  Clipboard.setData(ClipboardData(text: text));
  ScaffoldMessenger.of(context).hideCurrentSnackBar();
  ScaffoldMessenger.of(context).showSnackBar(
    const SnackBar(
      content: Text('Copied'),
      duration: Duration(milliseconds: 900),
    ),
  );
}

class _PlannerResponseCard extends StatelessWidget {
  const _PlannerResponseCard({
    required this.entry,
    required this.card,
    required this.onPick,
  });

  final ChatEntry entry;
  final ChatSemanticCard card;
  final ValueChanged<String> onPick;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final question = card.rows.isEmpty ? null : card.rows.first.title;

    return Align(
      alignment: Alignment.centerLeft,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 760),
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: theme.colorScheme.surface.withValues(alpha: 0.82),
            borderRadius: BorderRadius.circular(12),
            border: Border.all(
              color: theme.colorScheme.primary.withValues(alpha: 0.32),
            ),
          ),
          child: Padding(
            padding: const EdgeInsets.fromLTRB(12, 10, 12, 12),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Icon(
                      Icons.psychology_alt_outlined,
                      size: 16,
                      color: theme.colorScheme.primary,
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        card.title.trim().isNotEmpty ? card.title.trim() : 'Planner',
                        style: theme.textTheme.labelLarge?.copyWith(
                          fontWeight: FontWeight.w800,
                        ),
                      ),
                    ),
                    Text(
                      formatLocalTimeLabel(entry.timestamp),
                      style: theme.textTheme.labelSmall?.copyWith(
                        color: theme.colorScheme.onSurface.withValues(alpha: 0.52),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 8),
                SelectableText(
                  card.summary,
                  scrollPhysics: const NeverScrollableScrollPhysics(),
                  style: theme.textTheme.bodyMedium?.copyWith(height: 1.4),
                ),
                if (question != null && question.trim().isNotEmpty) ...[
                  const SizedBox(height: 12),
                  Text(
                    question.trim(),
                    style: theme.textTheme.labelLarge?.copyWith(
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                  const SizedBox(height: 8),
                  Wrap(
                    spacing: 8,
                    runSpacing: 8,
                    children: [
                      for (final option in card.plannerOptions)
                        _PlannerClarificationButton(
                          label: option.label,
                          description: option.description,
                          onPick: onPick,
                        ),
                    ],
                  ),
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _PlannerClarificationButton extends StatelessWidget {
  const _PlannerClarificationButton({
    required this.label,
    required this.description,
    required this.onPick,
  });

  final String label;
  final String description;
  final ValueChanged<String> onPick;

  @override
  Widget build(BuildContext context) {
    final trimmed = label.trim();
    if (trimmed.isEmpty) {
      return const SizedBox.shrink();
    }
    return Tooltip(
      message: description.trim().isEmpty ? trimmed : description.trim(),
      child: OutlinedButton(
        onPressed: () => onPick(trimmed),
        child: Text(trimmed),
      ),
    );
  }
}

IconData _semanticIcon(String icon) {
  return switch (icon) {
    'factCheck' => Icons.fact_check_rounded,
    'warning' => Icons.warning_amber_rounded,
    'build' => Icons.build_circle_outlined,
    'rule' => Icons.rule_outlined,
    'notes' => Icons.notes_rounded,
    'verified' => Icons.verified_rounded,
    'cancel' => Icons.cancel_rounded,
    'problem' => Icons.report_problem_outlined,
    'gavel' => Icons.gavel_rounded,
    'check' => Icons.check_circle_rounded,
    'remove' => Icons.remove_circle_outline,
    'dot' => Icons.radio_button_checked_rounded,
    'planner' => Icons.psychology_alt_outlined,
    'question' => Icons.help_outline_rounded,
    _ => Icons.rate_review_outlined,
  };
}

Color _semanticToneColor(ThemeData theme, String tone) {
  return switch (tone) {
    'success' => Colors.green.shade700,
    'danger' => theme.colorScheme.error,
    'warning' => Colors.amber.shade800,
    'muted' => theme.colorScheme.outline,
    'primary' => theme.colorScheme.primary,
    _ => theme.colorScheme.secondary,
  };
}

MarkdownStyleSheet _conversationMarkdownStyle(ThemeData theme, bool isPending) {
  final baseBody = theme.textTheme.bodySmall?.copyWith(
    height: 1.35,
    color: isPending
        ? theme.colorScheme.onSurface.withValues(alpha: 0.8)
        : null,
  );

  return MarkdownStyleSheet.fromTheme(theme).copyWith(
    p: baseBody,
    code: (theme.textTheme.bodySmall ?? const TextStyle()).copyWith(
      fontFamily: 'monospace',
      fontSize: (theme.textTheme.bodySmall?.fontSize ?? 12) * 0.94,
      height: 1.35,
      color: theme.colorScheme.onSurface,
      backgroundColor: theme.colorScheme.surfaceContainerHighest.withValues(
        alpha: 0.9,
      ),
    ),
    codeblockPadding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
    codeblockDecoration: BoxDecoration(
      color: theme.colorScheme.surfaceContainerHighest.withValues(alpha: 0.7),
      borderRadius: BorderRadius.circular(8),
      border: Border.all(
        color: theme.colorScheme.outline.withValues(alpha: 0.5),
      ),
    ),
    blockquote: theme.textTheme.bodySmall?.copyWith(
      height: 1.35,
      color: theme.colorScheme.onSurface.withValues(
        alpha: isPending ? 0.78 : 0.9,
      ),
    ),
    blockquotePadding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
    blockquoteDecoration: BoxDecoration(
      color: theme.colorScheme.surfaceContainerHighest.withValues(alpha: 0.6),
      borderRadius: BorderRadius.circular(6),
      border: Border(
        left: BorderSide(
          color: theme.colorScheme.primary.withValues(alpha: 0.55),
          width: 3,
        ),
      ),
    ),
    listBullet: theme.textTheme.bodySmall?.copyWith(height: 1.35),
  );
}

class _ChatCodeSyntaxHighlighter extends SyntaxHighlighter {
  _ChatCodeSyntaxHighlighter(this.theme);

  final ThemeData theme;

  @override
  TextSpan format(String source) {
    final baseStyle = (theme.textTheme.bodySmall ?? const TextStyle()).copyWith(
      fontFamily: 'monospace',
      fontSize: (theme.textTheme.bodySmall?.fontSize ?? 12) * 0.94,
      height: 1.45,
      color: theme.colorScheme.onSurface,
    );

    try {
      final result = hl.highlight.parse(source, autoDetection: true);
      final nodes = result.nodes;
      if (nodes == null || nodes.isEmpty) {
        return TextSpan(text: source, style: baseStyle);
      }
      return TextSpan(
        style: baseStyle,
        children: _highlightNodesToSpans(nodes, baseStyle, theme),
      );
    } catch (_) {
      return TextSpan(text: source, style: baseStyle);
    }
  }
}

List<InlineSpan> _highlightNodesToSpans(
  List<hl.Node> nodes,
  TextStyle baseStyle,
  ThemeData theme,
) {
  return nodes
      .map((node) => _highlightNodeToSpan(node, baseStyle, theme))
      .toList();
}

InlineSpan _highlightNodeToSpan(
  hl.Node node,
  TextStyle baseStyle,
  ThemeData theme,
) {
  final style = baseStyle.merge(_highlightStyleForClass(node.className, theme));
  if (node.children != null && node.children!.isNotEmpty) {
    return TextSpan(
      style: style,
      children: _highlightNodesToSpans(node.children!, style, theme),
    );
  }
  return TextSpan(text: node.value ?? '', style: style);
}

TextStyle? _highlightStyleForClass(String? className, ThemeData theme) {
  if (className == null || className.isEmpty) {
    return null;
  }

  final keywordColor = Colors.pink.shade300;
  final stringColor = Colors.green.shade300;
  final numberColor = Colors.orange.shade300;
  final commentColor = theme.colorScheme.onSurface.withValues(alpha: 0.58);
  final typeColor = Colors.cyan.shade300;
  final functionColor = Colors.blue.shade300;
  final literalColor = Colors.purple.shade200;
  final metaColor = Colors.amber.shade300;

  if (className.contains('comment')) {
    return TextStyle(color: commentColor, fontStyle: FontStyle.italic);
  }
  if (className.contains('string') || className.contains('regexp')) {
    return TextStyle(color: stringColor);
  }
  if (className.contains('number')) {
    return TextStyle(color: numberColor);
  }
  if (className.contains('keyword') ||
      className.contains('selector-tag') ||
      className.contains('built_in')) {
    return TextStyle(color: keywordColor, fontWeight: FontWeight.w600);
  }
  if (className.contains('literal') || className.contains('symbol')) {
    return TextStyle(color: literalColor);
  }
  if (className.contains('type') ||
      className.contains('class') ||
      className.contains('section')) {
    return TextStyle(color: typeColor);
  }
  if (className.contains('title') || className.contains('function')) {
    return TextStyle(color: functionColor);
  }
  if (className.contains('meta') || className.contains('doctag')) {
    return TextStyle(color: metaColor);
  }
  if (className.contains('attr') || className.contains('attribute')) {
    return TextStyle(color: Colors.tealAccent.shade200);
  }
  if (className.contains('subst') || className.contains('params')) {
    return TextStyle(color: theme.colorScheme.onSurface.withValues(alpha: 0.9));
  }

  return null;
}

class _InlineEventRow extends StatelessWidget {
  const _InlineEventRow({
    required this.entry,
    required this.expanded,
    required this.onExpandedChanged,
    this.onTerminateCommandExecution,
    this.loadFullSizeImage,
  });

  final ChatEntry entry;
  final bool expanded;
  final ValueChanged<bool> onExpandedChanged;
  final ValueChanged<String>? onTerminateCommandExecution;
  final FullSizeImageLoader? loadFullSizeImage;

  @override
  Widget build(BuildContext context) {
    switch (entry.kind) {
      case 'commandExecution':
        return _CommandEventRow(
          entry: entry,
          expanded: expanded,
          onExpandedChanged: onExpandedChanged,
          onTerminateCommandExecution: onTerminateCommandExecution,
        );
      case 'imageGeneration':
        return _ImageGenerationEventRow(
          entry: entry,
          loadFullSizeImage: loadFullSizeImage,
        );
      case 'imageView':
        return _ImageGenerationEventRow(
          entry: entry,
          loadFullSizeImage: loadFullSizeImage,
        );
      case 'mcpToolCall':
        return _ToolEventRow(
          entry: entry,
          expanded: expanded,
          onExpandedChanged: onExpandedChanged,
        );
      case 'fileChange':
        return _FileChangeEventRow(
          entry: entry,
          expanded: expanded,
          onExpandedChanged: onExpandedChanged,
        );
      default:
        return _GenericEventRow(
          entry: entry,
          expanded: expanded,
          onExpandedChanged: onExpandedChanged,
        );
    }
  }
}

class _ImageGenerationEventRow extends StatelessWidget {
  const _ImageGenerationEventRow({
    required this.entry,
    this.loadFullSizeImage,
  });

  final ChatEntry entry;
  final FullSizeImageLoader? loadFullSizeImage;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final timestampLabel = formatLocalTimeLabel(entry.timestamp);
    final path = entry.output?.trim();
    final showPath = path != null && path.isNotEmpty && !path.startsWith('agent-runtime-image://');

    return ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: 760),
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 4),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            _ImagePreview(
              entry: entry,
              loadFullSizeImage: loadFullSizeImage,
            ),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      Text(
                        'Image',
                        style: theme.textTheme.labelMedium?.copyWith(
                          fontWeight: FontWeight.w800,
                          color: theme.colorScheme.onSurface.withValues(
                            alpha: 0.9,
                          ),
                        ),
                      ),
                      const SizedBox(width: 8),
                      _AnimatedStatusIcon(
                        icon: _eventStatusIcon(entry),
                        color: _eventStatusColor(theme, entry),
                        identity: '${entry.status}|${entry.isStreaming}',
                      ),
                      const Spacer(),
                      Text(
                        timestampLabel,
                        style: theme.textTheme.labelSmall?.copyWith(
                          color: theme.colorScheme.onSurface.withValues(
                            alpha: 0.58,
                          ),
                        ),
                      ),
                    ],
                  ),
                  if (entry.command?.trim().isNotEmpty ?? false) ...[
                    const SizedBox(height: 6),
                    SelectableText(
                      entry.command!.trim(),
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: theme.colorScheme.onSurface.withValues(
                          alpha: 0.88,
                        ),
                      ),
                    ),
                  ],
                  if (showPath) ...[
                    const SizedBox(height: 6),
                    SelectableText(
                      path,
                      style: theme.textTheme.bodySmall?.copyWith(
                        fontFamily: 'monospace',
                        color: theme.colorScheme.onSurface.withValues(
                          alpha: 0.62,
                        ),
                      ),
                    ),
                  ],
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _ImagePreview extends StatelessWidget {
  const _ImagePreview({
    required this.entry,
    this.loadFullSizeImage,
  });

  final ChatEntry entry;
  final FullSizeImageLoader? loadFullSizeImage;

  @override
  Widget build(BuildContext context) {
    final path = entry.output?.trim();
    final canOpen = loadFullSizeImage != null && path != null && path.isNotEmpty;
    final encoded = entry.imagePreviewBase64?.trim();
    Widget preview;
    if (encoded != null && encoded.isNotEmpty) {
      try {
        final bytes = base64Decode(encoded);
        preview = ClipRRect(
          borderRadius: BorderRadius.circular(6),
          child: Image.memory(
            bytes,
            width: 100,
            height: 100,
            fit: BoxFit.cover,
            gaplessPlayback: true,
            errorBuilder: (_, _, _) => const _ImageUnavailable(),
          ),
        );
      } catch (_) {
        preview = const _ImageUnavailable();
      }
    } else {
      preview = const _ImageUnavailable();
    }

    if (!canOpen) {
      return preview;
    }

    return Semantics(
      button: true,
      label: 'Open full size image',
      child: Tooltip(
        message: 'Open full size image',
        child: InkWell(
          borderRadius: BorderRadius.circular(6),
          onTap: () => _showFullSizeImageDialog(
            context,
            path: path,
            loadFullSizeImage: loadFullSizeImage!,
          ),
          child: Stack(
            children: [
              preview,
              Positioned.fill(
                child: DecoratedBox(
                  decoration: BoxDecoration(
                    borderRadius: BorderRadius.circular(6),
                    border: Border.all(
                      color: Theme.of(context).colorScheme.primary.withValues(alpha: 0.26),
                    ),
                  ),
                ),
              ),
              Positioned(
                right: 6,
                bottom: 6,
                child: DecoratedBox(
                  decoration: BoxDecoration(
                    color: Colors.black.withValues(alpha: 0.52),
                    borderRadius: BorderRadius.circular(999),
                  ),
                  child: const Padding(
                    padding: EdgeInsets.all(4),
                    child: Icon(Icons.open_in_full_rounded, size: 14),
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

Future<void> _showFullSizeImageDialog(
  BuildContext context, {
  required String path,
  required FullSizeImageLoader loadFullSizeImage,
}) {
  final imageFuture = loadFullSizeImage(path);
  return showDialog<void>(
    context: context,
    builder: (context) => _FullSizeImageDialog(
      path: path,
      imageFuture: imageFuture,
    ),
  );
}

class _FullSizeImageDialog extends StatelessWidget {
  const _FullSizeImageDialog({
    required this.path,
    required this.imageFuture,
  });

  final String path;
  final Future<FullSizeImageData> imageFuture;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Dialog.fullscreen(
      backgroundColor: const Color(0xF2070B10),
      child: SafeArea(
        child: Column(
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 12, 10, 10),
              child: Row(
                children: [
                  Expanded(
                    child: SelectableText(
                      path,
                      maxLines: 2,
                      style: theme.textTheme.bodySmall?.copyWith(
                        fontFamily: 'monospace',
                        color: theme.colorScheme.onSurface.withValues(alpha: 0.72),
                      ),
                    ),
                  ),
                  IconButton(
                    tooltip: 'Close image',
                    onPressed: () => Navigator.of(context).pop(),
                    icon: const Icon(Icons.close_rounded),
                  ),
                ],
              ),
            ),
            Divider(
              height: 1,
              color: theme.colorScheme.outline.withValues(alpha: 0.32),
            ),
            Expanded(
              child: FutureBuilder<FullSizeImageData>(
                future: imageFuture,
                builder: (context, snapshot) {
                  if (snapshot.connectionState != ConnectionState.done) {
                    return const Center(child: CircularProgressIndicator());
                  }
                  if (snapshot.hasError) {
                    return Center(
                      child: Padding(
                        padding: const EdgeInsets.all(24),
                        child: Text(
                          'Could not load full size image: ${snapshot.error}',
                          textAlign: TextAlign.center,
                        ),
                      ),
                    );
                  }
                  final image = snapshot.data;
                  if (image == null || image.bytesBase64.isEmpty) {
                    return const Center(child: Text('No image data returned.'));
                  }
                  try {
                    final bytes = base64Decode(image.bytesBase64);
                    return InteractiveViewer(
                      minScale: 0.2,
                      maxScale: 8,
                      child: Center(
                        child: Image.memory(
                          bytes,
                          fit: BoxFit.contain,
                          gaplessPlayback: true,
                        ),
                      ),
                    );
                  } catch (error) {
                    return Center(child: Text('Could not decode image: $error'));
                  }
                },
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _ImageUnavailable extends StatelessWidget {
  const _ImageUnavailable();

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 100,
      height: 100,
      alignment: Alignment.center,
      color: Theme.of(context).colorScheme.surfaceContainerHighest,
      child: Text(
        'Image unavailable',
        textAlign: TextAlign.center,
        style: Theme.of(context).textTheme.labelSmall,
      ),
    );
  }
}

class _CommandEventRow extends StatelessWidget {
  const _CommandEventRow({
    required this.entry,
    required this.expanded,
    required this.onExpandedChanged,
    this.onTerminateCommandExecution,
  });

  final ChatEntry entry;
  final bool expanded;
  final ValueChanged<bool> onExpandedChanged;
  final ValueChanged<String>? onTerminateCommandExecution;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final statusColor = _eventStatusColor(theme, entry);
    final statusIcon = _eventStatusIcon(entry);
    final timestampLabel = formatLocalTimeLabel(entry.timestamp);
    final canExpand =
        _hasValue(entry.output) ||
        _hasValue(entry.body) ||
        _hasValue(entry.processId);
    final canTerminate =
        (entry.isStreaming || _isInProgressStatus(entry.status)) &&
        (entry.processId?.trim().isNotEmpty ?? false) &&
        entry.id.trim().isNotEmpty &&
        onTerminateCommandExecution != null;

    return ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: 760),
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 4),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Text(
                  'Command',
                  style: theme.textTheme.labelMedium?.copyWith(
                    fontWeight: FontWeight.w800,
                    color: theme.colorScheme.onSurface.withValues(alpha: 0.9),
                  ),
                ),
                const SizedBox(width: 8),
                _AnimatedStatusIcon(
                  icon: statusIcon,
                  color: statusColor,
                  identity: '${entry.status}|${entry.isStreaming}',
                ),
                if (canExpand) ...[
                  const SizedBox(width: 2),
                  InkWell(
                    borderRadius: BorderRadius.circular(999),
                    onTap: () => onExpandedChanged(!expanded),
                    child: Padding(
                      padding: const EdgeInsets.all(2),
                      child: Icon(
                        expanded
                            ? Icons.keyboard_arrow_up_rounded
                            : Icons.keyboard_arrow_down_rounded,
                        size: 18,
                        color: theme.colorScheme.onSurface.withValues(
                          alpha: 0.68,
                        ),
                      ),
                    ),
                  ),
                ],
                const Spacer(),
                Text(
                  timestampLabel,
                  style: theme.textTheme.labelSmall?.copyWith(
                    color: theme.colorScheme.onSurface.withValues(alpha: 0.58),
                  ),
                ),
                if (canTerminate) ...[
                  const SizedBox(width: 6),
                  Semantics(
                    key: ValueKey(
                      'semantic.command.terminate.${entry.processId}',
                    ),
                    container: true,
                    button: true,
                    label: 'Terminate command process ${entry.processId}',
                    child: ExcludeSemantics(
                      child: IconButton(
                        onPressed: () =>
                            onTerminateCommandExecution?.call(entry.processId!),
                        icon: const Icon(Icons.stop_circle_outlined, size: 16),
                        tooltip: 'Terminate command',
                        splashRadius: 14,
                        padding: EdgeInsets.zero,
                        constraints: const BoxConstraints.tightFor(
                          width: 22,
                          height: 22,
                        ),
                        visualDensity: VisualDensity.compact,
                        color: theme.colorScheme.error,
                      ),
                    ),
                  ),
                ],
              ],
            ),
            const SizedBox(height: 6),
            if (expanded)
              Text(
                entry.command?.trim().isNotEmpty == true
                    ? entry.command!.trim()
                    : entry.body.trim(),
                style: theme.textTheme.bodySmall?.copyWith(
                  fontFamily: 'monospace',
                  height: 1.38,
                  color: theme.colorScheme.onSurface.withValues(alpha: 0.88),
                ),
              )
            else
              Text(
                _compactPreview(
                  entry.command?.trim().isNotEmpty == true
                      ? entry.command!.trim()
                      : entry.body.trim(),
                ),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: theme.textTheme.bodySmall?.copyWith(
                  fontFamily: 'monospace',
                  height: 1.38,
                  color: theme.colorScheme.onSurface.withValues(alpha: 0.88),
                ),
              ),
            if (expanded) ...[
              if (_hasValue(entry.processId)) ...[
                const SizedBox(height: 8),
                _EventSection(
                  label: 'PID',
                  value: entry.processId!,
                  mono: true,
                ),
              ],
              if (_hasValue(entry.output)) ...[
                const SizedBox(height: 8),
                _EventSection(
                  label: 'Output',
                  value: entry.output!,
                  mono: true,
                ),
              ],
            ],
          ],
        ),
      ),
    );
  }
}

class _ToolEventRow extends StatelessWidget {
  const _ToolEventRow({
    required this.entry,
    required this.expanded,
    required this.onExpandedChanged,
  });

  final ChatEntry entry;
  final bool expanded;
  final ValueChanged<bool> onExpandedChanged;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final statusColor = _eventStatusColor(theme, entry);
    final timestampLabel = formatLocalTimeLabel(entry.timestamp);
    final canExpand =
        _hasValue(entry.output) ||
        _needsExpansion(entry.command) ||
        _needsExpansion(entry.body);
    final title = entry.subtitle?.trim().isNotEmpty == true
        ? entry.subtitle!.trim()
        : 'Tool';
    final preview = entry.command?.trim().isNotEmpty == true
        ? entry.command!.trim()
        : entry.body.trim();

    return ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: 760),
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 4),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                _AnimatedStatusIcon(
                  icon: Icons.extension_outlined,
                  color: statusColor,
                  identity: '${entry.status}|${entry.isStreaming}',
                  size: 15,
                ),
                const SizedBox(width: 6),
                Flexible(
                  child: Text(
                    title,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: theme.textTheme.labelMedium?.copyWith(
                      fontWeight: FontWeight.w700,
                      color: theme.colorScheme.onSurface.withValues(
                        alpha: 0.88,
                      ),
                    ),
                  ),
                ),
                if (canExpand) ...[
                  const SizedBox(width: 2),
                  InkWell(
                    borderRadius: BorderRadius.circular(999),
                    onTap: () => onExpandedChanged(!expanded),
                    child: Padding(
                      padding: const EdgeInsets.all(2),
                      child: Icon(
                        expanded
                            ? Icons.keyboard_arrow_up_rounded
                            : Icons.keyboard_arrow_down_rounded,
                        size: 18,
                        color: theme.colorScheme.onSurface.withValues(
                          alpha: 0.68,
                        ),
                      ),
                    ),
                  ),
                ],
                const Spacer(),
                Text(
                  timestampLabel,
                  style: theme.textTheme.labelSmall?.copyWith(
                    color: theme.colorScheme.onSurface.withValues(alpha: 0.58),
                  ),
                ),
              ],
            ),
            if (preview.isNotEmpty) ...[
              const SizedBox(height: 5),
              Text(
                expanded ? preview : _compactPreview(preview),
                style: theme.textTheme.bodySmall?.copyWith(
                  fontFamily: 'monospace',
                  height: 1.34,
                  color: theme.colorScheme.onSurface.withValues(alpha: 0.72),
                ),
              ),
            ],
            if (expanded && _hasValue(entry.output)) ...[
              const SizedBox(height: 8),
              _EventSection(label: 'Output', value: entry.output!, mono: true),
            ],
          ],
        ),
      ),
    );
  }
}

class _FileChangeEventRow extends StatelessWidget {
  const _FileChangeEventRow({
    required this.entry,
    required this.expanded,
    required this.onExpandedChanged,
  });

  final ChatEntry entry;
  final bool expanded;
  final ValueChanged<bool> onExpandedChanged;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final timestampLabel = formatLocalTimeLabel(entry.timestamp);
    final hasUnifiedDiff =
        _hasValue(entry.output) && _looksLikeUnifiedDiff(entry.output!);
    final canExpand =
        !hasUnifiedDiff &&
        (_hasValue(entry.output) || _hasValue(entry.command));
    final summary = entry.command?.trim().isNotEmpty == true
        ? entry.command!.trim()
        : entry.body.trim();

    return ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: 760),
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 5),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Icon(
              Icons.edit_note_rounded,
              size: 16,
              color: Theme.of(context).colorScheme.outline,
            ),
            const SizedBox(width: 8),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      Text(
                        'Files',
                        style: theme.textTheme.labelMedium?.copyWith(
                          fontWeight: FontWeight.w800,
                        ),
                      ),
                      if (canExpand) ...[
                        const SizedBox(width: 2),
                        InkWell(
                          borderRadius: BorderRadius.circular(999),
                          onTap: () => onExpandedChanged(!expanded),
                          child: Padding(
                            padding: const EdgeInsets.all(2),
                            child: Icon(
                              expanded
                                  ? Icons.keyboard_arrow_up_rounded
                                  : Icons.keyboard_arrow_down_rounded,
                              size: 18,
                              color: theme.colorScheme.onSurface.withValues(
                                alpha: 0.68,
                              ),
                            ),
                          ),
                        ),
                      ],
                      const Spacer(),
                      Text(timestampLabel, style: theme.textTheme.labelSmall),
                    ],
                  ),
                  if (summary.isNotEmpty) ...[
                    const SizedBox(height: 5),
                    Text(
                      expanded ? summary : _compactPreview(summary),
                      style: theme.textTheme.bodySmall?.copyWith(
                        height: 1.34,
                        color: theme.colorScheme.onSurface.withValues(
                          alpha: 0.76,
                        ),
                      ),
                    ),
                  ],
                  if (hasUnifiedDiff) ...[
                    const SizedBox(height: 8),
                    _DiffEventSection(value: entry.output!),
                  ] else if (expanded && _hasValue(entry.output)) ...[
                    const SizedBox(height: 8),
                    _EventSection(
                      label: 'Diff',
                      value: entry.output!,
                      mono: true,
                    ),
                  ],
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _GenericEventRow extends StatelessWidget {
  const _GenericEventRow({
    required this.entry,
    required this.expanded,
    required this.onExpandedChanged,
  });

  final ChatEntry entry;
  final bool expanded;
  final ValueChanged<bool> onExpandedChanged;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final timestampLabel = formatLocalTimeLabel(entry.timestamp);
    final canExpand =
        _needsExpansion(entry.body) ||
        _hasValue(entry.output) ||
        _hasValue(entry.command);
    final label = _genericEventTitle(entry);
    final summary = _hasValue(entry.body)
        ? entry.body.trim()
        : _hasValue(entry.command)
        ? entry.command!.trim()
        : entry.displayLabel;

    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _AnimatedStatusDot(color: _eventStatusColor(theme, entry)),
          const SizedBox(width: 8),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Text(
                      label,
                      style: theme.textTheme.labelMedium?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    if (canExpand) ...[
                      const SizedBox(width: 2),
                      InkWell(
                        borderRadius: BorderRadius.circular(999),
                        onTap: () => onExpandedChanged(!expanded),
                        child: Padding(
                          padding: const EdgeInsets.all(2),
                          child: Icon(
                            expanded
                                ? Icons.keyboard_arrow_up_rounded
                                : Icons.keyboard_arrow_down_rounded,
                            size: 18,
                            color: theme.colorScheme.onSurface.withValues(
                              alpha: 0.68,
                            ),
                          ),
                        ),
                      ),
                    ],
                    const Spacer(),
                    Text(timestampLabel, style: theme.textTheme.labelSmall),
                  ],
                ),
                if (summary.isNotEmpty) ...[
                  const SizedBox(height: 4),
                  Text(
                    expanded ? summary : _compactPreview(summary),
                    style: theme.textTheme.bodySmall?.copyWith(
                      height: 1.34,
                      color: theme.colorScheme.onSurface.withValues(
                        alpha: 0.72,
                      ),
                    ),
                  ),
                ],
                if (expanded && _hasValue(entry.output)) ...[
                  const SizedBox(height: 8),
                  _EventSection(
                    label: 'Output',
                    value: entry.output!,
                    mono: true,
                  ),
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _DiffEventSection extends StatelessWidget {
  const _DiffEventSection({required this.value});

  final String value;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final textStyle = (theme.textTheme.bodySmall ?? const TextStyle()).copyWith(
      fontFamily: 'monospace',
      height: 1.4,
      color: theme.colorScheme.onSurface.withValues(alpha: 0.84),
    );
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(10),
        color: theme.colorScheme.surfaceContainerHighest.withValues(
          alpha: 0.18,
        ),
        border: Border.all(
          color: theme.colorScheme.outline.withValues(alpha: 0.18),
        ),
      ),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(10),
        child: Text.rich(
          TextSpan(
            style: textStyle,
            children: _diffTextSpans(
              value.replaceAll('\r\n', '\n'),
              theme,
              textStyle,
            ),
          ),
          softWrap: true,
        ),
      ),
    );
  }
}

enum _DiffLineKind { header, hunk, added, removed, context }

List<InlineSpan> _diffTextSpans(
  String value,
  ThemeData theme,
  TextStyle baseStyle,
) {
  final lines = value.split('\n');
  return lines
      .asMap()
      .entries
      .map((entry) {
        final line = entry.value;
        final kind = _classifyDiffLine(line);
        final color = switch (kind) {
          _DiffLineKind.added => Colors.green.shade100,
          _DiffLineKind.removed => Colors.red.shade100,
          _DiffLineKind.hunk => theme.colorScheme.primary.withValues(
            alpha: 0.94,
          ),
          _DiffLineKind.header => theme.colorScheme.onSurface.withValues(
            alpha: 0.62,
          ),
          _DiffLineKind.context => theme.colorScheme.onSurface.withValues(
            alpha: 0.82,
          ),
        };
        final backgroundColor = switch (kind) {
          _DiffLineKind.added => Colors.green.shade900.withValues(alpha: 0.18),
          _DiffLineKind.removed => Colors.red.shade900.withValues(alpha: 0.16),
          _DiffLineKind.hunk => theme.colorScheme.primary.withValues(
            alpha: 0.12,
          ),
          _DiffLineKind.header =>
            theme.colorScheme.surfaceContainerHighest.withValues(alpha: 0.42),
          _DiffLineKind.context => null,
        };
        final suffix = entry.key == lines.length - 1 ? '' : '\n';
        return TextSpan(
          text: '$line$suffix',
          style: baseStyle.copyWith(
            color: color,
            backgroundColor: backgroundColor,
            fontWeight: kind == _DiffLineKind.hunk ? FontWeight.w700 : null,
          ),
        );
      })
      .toList(growable: false);
}

_DiffLineKind _classifyDiffLine(String line) {
  if (line.startsWith('diff --git') ||
      line.startsWith('index ') ||
      line.startsWith('--- ') ||
      line.startsWith('+++ ')) {
    return _DiffLineKind.header;
  }
  if (line.startsWith('@@')) {
    return _DiffLineKind.hunk;
  }
  if (line.startsWith('+')) {
    return _DiffLineKind.added;
  }
  if (line.startsWith('-')) {
    return _DiffLineKind.removed;
  }
  return _DiffLineKind.context;
}

bool _looksLikeUnifiedDiff(String value) {
  final normalized = value.replaceAll('\r\n', '\n');
  return normalized.contains('\n@@') ||
      normalized.startsWith('@@') ||
      normalized.contains('\ndiff --git ') ||
      normalized.startsWith('diff --git ') ||
      (normalized.contains('\n--- ') && normalized.contains('\n+++ '));
}

class _EventSection extends StatelessWidget {
  const _EventSection({
    required this.label,
    required this.value,
    this.mono = false,
  });

  final String label;
  final String value;
  final bool mono;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.fromLTRB(10, 9, 10, 10),
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(12),
        color: theme.colorScheme.surfaceContainerHighest.withValues(
          alpha: 0.28,
        ),
        border: Border.all(
          color: theme.colorScheme.outline.withValues(alpha: 0.18),
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            label,
            style: theme.textTheme.labelSmall?.copyWith(
              fontWeight: FontWeight.w700,
              color: theme.colorScheme.onSurface.withValues(alpha: 0.64),
            ),
          ),
          const SizedBox(height: 6),
          Text(
            value,
            style: theme.textTheme.bodySmall?.copyWith(
              fontFamily: mono ? 'monospace' : null,
              height: 1.36,
              color: theme.colorScheme.onSurface.withValues(alpha: 0.84),
            ),
          ),
        ],
      ),
    );
  }
}

bool _hasValue(String? value) => value != null && value.trim().isNotEmpty;

Color _eventStatusColor(ThemeData theme, ChatEntry? entry) {
  if (entry?.displayLabel == 'Diff') {
    return theme.colorScheme.outline;
  }
  final status = entry?.status?.toLowerCase();
  if (status == null || status.isEmpty) {
    return theme.colorScheme.secondary;
  }
  if (status.contains('fail') ||
      status.contains('error') ||
      status.contains('reject')) {
    return theme.colorScheme.error;
  }
  if (status.contains('progress') ||
      status.contains('pending') ||
      status.contains('running')) {
    return Colors.amber.shade700;
  }
  if (status.contains('complete') ||
      status.contains('success') ||
      status.contains('approved')) {
    return Colors.green.shade700;
  }
  return theme.colorScheme.secondary;
}

IconData _eventStatusIcon(ChatEntry entry) {
  final status = entry.status?.toLowerCase();
  if (status == null || status.isEmpty) {
    return entry.isStreaming
        ? Icons.schedule_rounded
        : Icons.check_circle_rounded;
  }
  if (status.contains('fail') ||
      status.contains('error') ||
      status.contains('reject')) {
    return Icons.cancel_rounded;
  }
  if (status.contains('progress') ||
      status.contains('pending') ||
      status.contains('running')) {
    return Icons.schedule_rounded;
  }
  if (status.contains('complete') ||
      status.contains('success') ||
      status.contains('approved')) {
    return Icons.check_circle_rounded;
  }
  return Icons.radio_button_checked_rounded;
}

String _genericEventTitle(ChatEntry entry) {
  switch (entry.kind) {
    case 'requestCompaction':
      return 'Compaction';
    case 'approvalRequest':
      return 'Approval';
    default:
      final label = entry.displayLabel.trim();
      if (label.isNotEmpty) {
        return label;
      }
      final kind = entry.kind?.trim();
      if (kind == null || kind.isEmpty) {
        return 'Event';
      }
      return kind
          .replaceAllMapped(RegExp(r'([a-z])([A-Z])'), (m) => '${m[1]} ${m[2]}')
          .replaceAll('_', ' ')
          .split(' ')
          .where((part) => part.isNotEmpty)
          .map((part) => part[0].toUpperCase() + part.substring(1))
          .join(' ');
  }
}

bool _needsExpansion(String? value) {
  if (!_hasValue(value)) {
    return false;
  }
  final trimmed = value!.trim();
  return trimmed.contains('\n') || trimmed.length > 160;
}

bool _isInProgressStatus(String? status) {
  final normalized = status?.toLowerCase();
  if (normalized == null || normalized.isEmpty) {
    return false;
  }
  return normalized.contains('progress') ||
      normalized.contains('pending') ||
      normalized.contains('running');
}

String _compactPreview(String value) {
  final normalized = value
      .trim()
      .replaceAll('\r\n', '\n')
      .replaceAll('\n', '  ');
  if (normalized.length <= 160) {
    return normalized;
  }
  return '${normalized.substring(0, 157)}...';
}

String? _planSummary(String body) {
  final lines = body
      .replaceAll('\r\n', '\n')
      .split('\n')
      .map((line) => line.trim())
      .where((line) => line.isNotEmpty)
      .where(
        (line) =>
            !RegExp(r'^\[(pending|in_progress|completed)\]\s+').hasMatch(line),
      )
      .toList(growable: false);
  if (lines.isEmpty) {
    return null;
  }
  return lines
      .asMap()
      .entries
      .map((entry) {
        if (entry.key > 0) {
          return entry.value;
        }
        return entry.value.replaceFirst(
          RegExp(r'^summary\s*:?\s*', caseSensitive: false),
          '',
        );
      })
      .where((line) => line.isNotEmpty)
      .join(' ');
}
