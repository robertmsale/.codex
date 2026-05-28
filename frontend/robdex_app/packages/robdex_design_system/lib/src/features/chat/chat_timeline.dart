import 'dart:convert';

import 'package:flutter_markdown_plus/flutter_markdown_plus.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:highlight/highlight.dart' as hl;

import '../../core/formatters/timestamps.dart';
import '../../core/models/workbench_models.dart';
import '../composer/composer_panel.dart';
import '../inspector/inspector_panel.dart';

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
    this.bridgeBaseUri,
    this.onOpenLink,
    this.onTerminateCommandExecution,
    this.requirementReview,
    this.onOpenThread,
    this.terminalAvailable = false,
    this.onTerminalPressed,
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
  final Uri? bridgeBaseUri;
  final ValueChanged<String>? onOpenLink;
  final ValueChanged<String>? onTerminateCommandExecution;
  final RequirementReviewSummary? requirementReview;
  final ValueChanged<String>? onOpenThread;
  final bool terminalAvailable;
  final VoidCallback? onTerminalPressed;

  @override
  State<ChatTimeline> createState() => _ChatTimelineState();
}

class _ChatTimelineState extends State<ChatTimeline> {
  late final ScrollController _scrollController;
  final Set<String> _expandedEntryKeys = <String>{};
  bool _stickToBottom = true;
  bool _userScrollActive = false;

  @override
  void initState() {
    super.initState();
    _scrollController = ScrollController();
  }

  @override
  void dispose() {
    _scrollController.dispose();
    super.dispose();
  }

  @override
  void didUpdateWidget(covariant ChatTimeline oldWidget) {
    super.didUpdateWidget(oldWidget);

    final currentKeys = widget.entries
        .map(_entryStorageKey)
        .toSet();
    _expandedEntryKeys.removeWhere((key) => !currentKeys.contains(key));

    final threadChanged = widget.threadId != oldWidget.threadId;
    final entriesChanged = widget.entries.length != oldWidget.entries.length ||
        !_sameEntryIdentity(widget.entries, oldWidget.entries);

    if (!threadChanged && !entriesChanged) {
      return;
    }

    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_scrollController.hasClients) {
        return;
      }
      final position = _scrollController.position;
      if (threadChanged) {
        final target = position.maxScrollExtent.clamp(
          position.minScrollExtent,
          position.maxScrollExtent,
        );
        if ((position.pixels - target).abs() > 1) {
          _scrollController.jumpTo(target);
        }
        _stickToBottom = true;
        _userScrollActive = false;
        return;
      }
      if (_stickToBottom && !_userScrollActive) {
        final target = position.maxScrollExtent.clamp(
          position.minScrollExtent,
          position.maxScrollExtent,
        );
        if ((position.pixels - target).abs() > 1) {
          _scrollController.jumpTo(target);
        }
        return;
      }
    });
  }

  bool _isNearBottom() {
    if (!_scrollController.hasClients) {
      return true;
    }
    final position = _scrollController.position;
    return position.maxScrollExtent - position.pixels < 36;
  }

  bool _handleScrollNotification(ScrollNotification notification) {
    if (!_scrollController.hasClients) {
      return false;
    }
    if (notification is ScrollStartNotification &&
        notification.dragDetails != null) {
      _userScrollActive = true;
      _stickToBottom = false;
      return false;
    }
    if (notification is ScrollUpdateNotification &&
        notification.dragDetails != null) {
      if (!_userScrollActive || _stickToBottom) {
        setState(() {
          _userScrollActive = true;
          _stickToBottom = false;
        });
      }
      return false;
    }
    if (notification is ScrollEndNotification && _userScrollActive) {
      final shouldStick = _isNearBottom();
      setState(() {
        _userScrollActive = false;
        _stickToBottom = shouldStick;
      });
      return false;
    }
    if (notification is UserScrollNotification &&
        notification.direction == ScrollDirection.idle &&
        _userScrollActive) {
      final shouldStick = _isNearBottom();
      setState(() {
        _userScrollActive = false;
        _stickToBottom = shouldStick;
      });
    }
    return false;
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            if (widget.leading != null) ...[
              Padding(
                padding: const EdgeInsets.only(top: 2),
                child: widget.leading!,
              ),
              const SizedBox(width: 10),
            ],
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    widget.title,
                    style: theme.textTheme.bodyLarge?.copyWith(
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                  const SizedBox(height: 2),
                  Text(
                    widget.contextWindowRemainingPercent == null
                        ? '--% remaining'
                        : '${widget.contextWindowRemainingPercent!}% remaining',
                    style: theme.textTheme.labelSmall?.copyWith(
                      color: theme.colorScheme.onSurface.withValues(alpha: 0.52),
                    ),
                  ),
                ],
              ),
            ),
            if (widget.headerControls != null) ...[
              const SizedBox(width: 18),
              Flexible(
                flex: 0,
                child: DefaultTextStyle.merge(
                  style: theme.textTheme.labelSmall?.copyWith(
                        color: theme.colorScheme.onSurface.withValues(alpha: 0.74),
                      ) ??
                      const TextStyle(),
                  child: widget.headerControls!,
                ),
              ),
            ],
          ],
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
              NotificationListener<ScrollNotification>(
                onNotification: _handleScrollNotification,
                child: SelectionArea(
                  child: ListView.separated(
                    key: PageStorageKey<String>('chat-timeline-${widget.threadId ?? 'none'}'),
                    controller: _scrollController,
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
                        bridgeBaseUri: widget.bridgeBaseUri,
                        onOpenLink: widget.onOpenLink,
                      );
                    },
                  ),
                ),
              ),
              if (widget.overlay != null)
                Positioned(
                  top: 0,
                  left: 0,
                  right: 0,
                  child: widget.overlay!,
                ),
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
            bridgeBaseUri: widget.bridgeBaseUri,
            terminalAvailable: widget.terminalAvailable,
            onTerminalPressed: widget.onTerminalPressed,
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
    final isReviewerThread = selectedThreadRole == 'requirements-reviewer' ||
        selectedThreadRole == 'requirementsReviewer';
    final isWaiverRequired = summary.status == 'waiverRequired';
    final targetThreadId = isReviewerThread ? parentThreadId : reviewerThreadId;
    final buttonLabel = isReviewerThread ? 'Back to source thread' : 'Open review thread';
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
    final accentColor = isWaiverRequired ? Colors.amber.shade700 : theme.colorScheme.primary;

    return Semantics(
      key: const ValueKey('semantic.requirementsReview.inline'),
      container: true,
      label: statusText,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: isWaiverRequired ? const Color(0xFF231C09) : const Color(0xFF111923),
          borderRadius: BorderRadius.circular(12),
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
                        color: theme.colorScheme.onSurface.withValues(alpha: 0.88),
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    const SizedBox(height: 2),
                    Text(
                      counts,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: theme.textTheme.labelSmall?.copyWith(
                        color: theme.colorScheme.onSurface.withValues(alpha: 0.52),
                      ),
                    ),
                  ],
                ),
              ),
              if (targetThreadId != null && targetThreadId.isNotEmpty && onOpenThread != null)
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

class _ChatBubble extends StatelessWidget {
  const _ChatBubble({
    super.key,
    required this.entry,
    required this.expanded,
    required this.onExpandedChanged,
    this.onTerminateCommandExecution,
    this.bridgeBaseUri,
    this.onOpenLink,
  });

  final ChatEntry entry;
  final bool expanded;
  final ValueChanged<bool> onExpandedChanged;
  final ValueChanged<String>? onTerminateCommandExecution;
  final Uri? bridgeBaseUri;
  final ValueChanged<String>? onOpenLink;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final timestampLabel = formatLocalTimeLabel(entry.timestamp);
    final isConversation = !entry.isTool &&
        (entry.author == 'User' || entry.author == 'Assistant' || entry.author == 'Operator');

    final fallbackPlanItems = entry.hasPlanItems ? const <PlanChecklistItem>[] : _planItemsFromBody(entry.body);
    if (entry.hasPlanItems || fallbackPlanItems.isNotEmpty) {
      return _PlanUpdateCard(
        entry: entry,
        fallbackItems: fallbackPlanItems,
      );
    }

    if (!isConversation) {
      return _InlineEventRow(
        entry: entry,
        expanded: expanded,
        onExpandedChanged: onExpandedChanged,
        onTerminateCommandExecution: onTerminateCommandExecution,
        bridgeBaseUri: bridgeBaseUri,
      );
    }

    final requirementsVerdictPayload = _requirementsVerdictPayloadFromBody(entry);
    if (requirementsVerdictPayload != null) {
      return _RequirementsVerdictCard(
        entry: entry,
        payload: requirementsVerdictPayload,
      );
    }

    final requirementsPayload = _requirementsClaimPayloadFromBody(entry);
    if (requirementsPayload != null) {
      return _RequirementsClaimCard(
        entry: entry,
        payload: requirementsPayload,
      );
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
            borderRadius: BorderRadius.circular(12),
            border: Border.all(
              color: isUser
                  ? theme.colorScheme.primary.withValues(alpha: isPending ? 0.18 : 0.28)
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
                                  ? theme.colorScheme.onSurface.withValues(alpha: 0.68)
                                  : theme.colorScheme.onSurface.withValues(alpha: 0.8),
                            ),
                          ),
                          Text(
                            timestampLabel,
                            style: theme.textTheme.labelSmall?.copyWith(
                              color: theme.colorScheme.onSurface.withValues(alpha: 0.58),
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
                              child: CircularProgressIndicator(strokeWidth: 1.5),
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
                          icon: const Icon(Icons.content_copy_rounded, size: 14),
                          tooltip: 'Copy',
                          splashRadius: 14,
                          padding: EdgeInsets.zero,
                          constraints: const BoxConstraints.tightFor(width: 22, height: 22),
                          visualDensity: VisualDensity.compact,
                        ),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 4),
                MarkdownBody(
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
  const _PlanUpdateCard({
    required this.entry,
    this.fallbackItems = const [],
  });

  final ChatEntry entry;
  final List<PlanChecklistItem> fallbackItems;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final timestampLabel = formatLocalTimeLabel(entry.timestamp);
    final note = _planSummary(entry.body);
    final accent = entry.isStreaming ? Colors.amber.shade700 : theme.colorScheme.primary;
    final items = entry.hasPlanItems ? entry.planItems : fallbackItems;

    return Align(
      alignment: Alignment.centerLeft,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 720),
        child: DecoratedBox(
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(20),
            color: Color.alphaBlend(
              accent.withValues(alpha: 0.07),
              theme.colorScheme.surface,
            ),
            border: Border.all(
              color: accent.withValues(alpha: 0.22),
            ),
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
                        color: theme.colorScheme.onSurface.withValues(alpha: 0.62),
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
                      color: theme.colorScheme.onSurface.withValues(alpha: 0.82),
                    ),
                  ),
                ],
                const SizedBox(height: 14),
                ...items.asMap().entries.map(
                  (entry) => Padding(
                    padding: EdgeInsets.only(bottom: entry.key == items.length - 1 ? 0 : 10),
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
  const _PlanChecklistRow({
    required this.item,
  });

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
                  decoration: item.completed ? TextDecoration.lineThrough : null,
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

class _RequirementsClaimCard extends StatelessWidget {
  const _RequirementsClaimCard({
    required this.entry,
    required this.payload,
  });

  final ChatEntry entry;
  final Map<String, dynamic> payload;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final timestampLabel = formatLocalTimeLabel(entry.timestamp);
    final disposition = payload['finalDisposition'] as String? ?? 'unknown';
    final summary = payload['summary'] as String? ?? '';
    final nestedRequirements = payload['requirements'];
    final requirementEntries = nestedRequirements is Map<String, dynamic>
        ? nestedRequirements.entries
            .where((entry) => entry.value is Map<String, dynamic>)
            .toList(growable: false)
        : payload.entries
            .where((entry) =>
                entry.key != 'summary' &&
                entry.key != 'finalDisposition' &&
                entry.key != 'requirements' &&
                entry.value is Map<String, dynamic>)
            .toList(growable: false);
    final isCommentaryPacket = payload.containsKey('requirements') && nestedRequirements == null;
    final accent = isCommentaryPacket
        ? theme.colorScheme.secondary
        : switch (disposition) {
            'readyForRequirementsReview' => Colors.green.shade700,
            'blockedNeedsOwnerAction' => Colors.amber.shade800,
            'continueWorkNeeded' => theme.colorScheme.error,
            _ => requirementEntries.isEmpty ? theme.colorScheme.secondary : Colors.green.shade700,
          };
    final title = isCommentaryPacket
        ? 'Requirements Commentary'
        : switch (disposition) {
            'readyForRequirementsReview' => 'Requirements Claim',
            'blockedNeedsOwnerAction' => 'Requirements Blocked',
            'continueWorkNeeded' => 'Requirements Need Work',
            _ => requirementEntries.isEmpty ? 'Requirements Output' : 'Requirements Claim',
          };
    final icon = isCommentaryPacket
        ? Icons.notes_rounded
        : switch (disposition) {
            'readyForRequirementsReview' => _requirementsDispositionIcon(disposition),
            'blockedNeedsOwnerAction' => _requirementsDispositionIcon(disposition),
            'continueWorkNeeded' => _requirementsDispositionIcon(disposition),
            _ => requirementEntries.isEmpty ? Icons.rule_outlined : Icons.fact_check_rounded,
          };
    final statusLabel = isCommentaryPacket
        ? 'commentary'
        : requirementEntries.isEmpty
            ? null
            : '${requirementEntries.length} ${requirementEntries.length == 1 ? 'claim' : 'claims'}';

    return Align(
      alignment: Alignment.centerLeft,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 760),
        child: DecoratedBox(
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(16),
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
                    Icon(icon, color: accent, size: 18),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        title,
                        style: theme.textTheme.titleSmall?.copyWith(
                          fontWeight: FontWeight.w800,
                        ),
                      ),
                    ),
                    if (statusLabel != null) ...[
                      Container(
                        margin: const EdgeInsets.only(right: 8),
                        padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 3),
                        decoration: BoxDecoration(
                          color: accent.withValues(alpha: 0.1),
                          borderRadius: BorderRadius.circular(999),
                          border: Border.all(color: accent.withValues(alpha: 0.24)),
                        ),
                        child: Text(
                          statusLabel,
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
                        color: theme.colorScheme.onSurface.withValues(alpha: 0.58),
                      ),
                    ),
                  ],
                ),
                if (summary.trim().isNotEmpty) ...[
                  const SizedBox(height: 10),
                  Text(
                    summary.trim(),
                    style: theme.textTheme.bodySmall?.copyWith(
                      height: 1.36,
                      color: theme.colorScheme.onSurface.withValues(alpha: 0.82),
                    ),
                  ),
                ],
                if (requirementEntries.isNotEmpty) const SizedBox(height: 12),
                ...requirementEntries.map((entry) {
                  final value = entry.value as Map<String, dynamic>;
                  final claim = value['claim'] as String? ?? 'unknown';
                  final risk = value['risk'] as String? ?? 'unknown';
                  final justification = value['justification'] as String? ?? '';
                  final evidence = (value['evidence'] as List<dynamic>? ?? const [])
                      .whereType<String>()
                      .toList(growable: false);
                  final claimColor = _requirementsClaimColor(theme, claim);
                  return Padding(
                    padding: const EdgeInsets.only(bottom: 10),
                    child: DecoratedBox(
                      decoration: BoxDecoration(
                        border: Border(
                          left: BorderSide(color: claimColor, width: 3),
                        ),
                      ),
                      child: Padding(
                        padding: const EdgeInsets.only(left: 10),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Row(
                              children: [
                                Icon(_requirementsClaimIcon(claim), color: claimColor, size: 16),
                                const SizedBox(width: 7),
                                Expanded(
                                  child: Text(
                                    entry.key,
                                    style: theme.textTheme.labelLarge?.copyWith(
                                      fontWeight: FontWeight.w800,
                                    ),
                                  ),
                                ),
                                Text(
                                  '${_titleCaseClaim(claim)} · risk $risk',
                                  style: theme.textTheme.labelSmall?.copyWith(
                                    color: claimColor,
                                    fontWeight: FontWeight.w700,
                                  ),
                                ),
                              ],
                            ),
                            if (justification.trim().isNotEmpty) ...[
                              const SizedBox(height: 5),
                              Text(
                                justification.trim(),
                                style: theme.textTheme.bodySmall?.copyWith(
                                  height: 1.34,
                                  color: theme.colorScheme.onSurface.withValues(alpha: 0.78),
                                ),
                              ),
                            ],
                            if (evidence.isNotEmpty) ...[
                              const SizedBox(height: 6),
                              for (final item in evidence.take(4))
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
                }),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _RequirementsVerdictCard extends StatelessWidget {
  const _RequirementsVerdictCard({
    required this.entry,
    required this.payload,
  });

  final ChatEntry entry;
  final Map<String, dynamic> payload;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final timestampLabel = formatLocalTimeLabel(entry.timestamp);
    final overall = payload['overallVerdict'] as String? ?? 'unknown';
    final route = payload['route'] is Map<String, dynamic>
        ? payload['route'] as Map<String, dynamic>
        : const <String, dynamic>{};
    final routeMessage = route['message'] as String? ?? '';
    final requirementEntries = payload.entries
        .where((entry) =>
            entry.key != 'overallVerdict' &&
            entry.key != 'route' &&
            entry.value is Map<String, dynamic>)
        .toList(growable: false);
    final accent = _requirementsVerdictColor(theme, overall);
    final title = switch (overall) {
      'pass' => 'Requirements Review Passed',
      'fail' => 'Requirements Review Failed',
      'acceptedBlocked' => 'Requirements Review Accepted Blocker',
      'rejectedBlocked' => 'Requirements Review Rejected Blocker',
      'needsHumanWaiver' => 'Requirements Review Needs Waiver',
      _ => 'Requirements Review',
    };

    return Align(
      alignment: Alignment.centerLeft,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 760),
        child: DecoratedBox(
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(16),
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
                    Icon(_requirementsVerdictIcon(overall), color: accent, size: 18),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        title,
                        style: theme.textTheme.titleSmall?.copyWith(
                          fontWeight: FontWeight.w800,
                        ),
                      ),
                    ),
                    Text(
                      timestampLabel,
                      style: theme.textTheme.labelSmall?.copyWith(
                        color: theme.colorScheme.onSurface.withValues(alpha: 0.58),
                      ),
                    ),
                  ],
                ),
                if (routeMessage.trim().isNotEmpty) ...[
                  const SizedBox(height: 10),
                  Text(
                    routeMessage.trim(),
                    style: theme.textTheme.bodySmall?.copyWith(
                      height: 1.36,
                      color: theme.colorScheme.onSurface.withValues(alpha: 0.82),
                    ),
                  ),
                ],
                const SizedBox(height: 12),
                ...requirementEntries.map((entry) {
                  final value = entry.value as Map<String, dynamic>;
                  final verdict = value['verdict'] as String? ?? 'unknown';
                  final reason = value['reason'] as String? ?? '';
                  final evidenceAssessment = value['evidenceAssessment'] as String? ?? '';
                  final requiredCorrection = value['requiredCorrection'] as String? ?? '';
                  final verdictColor = _requirementsVerdictColor(theme, verdict);
                  return Padding(
                    padding: const EdgeInsets.only(bottom: 10),
                    child: DecoratedBox(
                      decoration: BoxDecoration(
                        border: Border(
                          left: BorderSide(color: verdictColor, width: 3),
                        ),
                      ),
                      child: Padding(
                        padding: const EdgeInsets.only(left: 10),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Row(
                              children: [
                                Icon(_requirementsVerdictIcon(verdict), color: verdictColor, size: 16),
                                const SizedBox(width: 7),
                                Expanded(
                                  child: Text(
                                    entry.key,
                                    style: theme.textTheme.labelLarge?.copyWith(
                                      fontWeight: FontWeight.w800,
                                    ),
                                  ),
                                ),
                                Text(
                                  _titleCaseVerdict(verdict),
                                  style: theme.textTheme.labelSmall?.copyWith(
                                    color: verdictColor,
                                    fontWeight: FontWeight.w700,
                                  ),
                                ),
                              ],
                            ),
                            if (reason.trim().isNotEmpty) ...[
                              const SizedBox(height: 5),
                              Text(
                                reason.trim(),
                                style: theme.textTheme.bodySmall?.copyWith(
                                  height: 1.34,
                                  color: theme.colorScheme.onSurface.withValues(alpha: 0.78),
                                ),
                              ),
                            ],
                            if (evidenceAssessment.trim().isNotEmpty) ...[
                              const SizedBox(height: 5),
                              Text(
                                'Evidence: ${evidenceAssessment.trim()}',
                                style: theme.textTheme.bodySmall?.copyWith(
                                  height: 1.32,
                                  color: theme.colorScheme.onSurface.withValues(alpha: 0.68),
                                ),
                              ),
                            ],
                            if (requiredCorrection.trim().isNotEmpty) ...[
                              const SizedBox(height: 5),
                              Text(
                                'Correction: ${requiredCorrection.trim()}',
                                style: theme.textTheme.bodySmall?.copyWith(
                                  height: 1.32,
                                  color: verdictColor.withValues(alpha: 0.9),
                                  fontWeight: FontWeight.w600,
                                ),
                              ),
                            ],
                          ],
                        ),
                      ),
                    ),
                  );
                }),
              ],
            ),
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
  const _AnimatedStatusDot({
    required this.color,
  });

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

Map<String, dynamic>? _requirementsClaimPayloadFromBody(ChatEntry entry) {
  if (entry.isStreaming || entry.author != 'Assistant') {
    return null;
  }
  final text = entry.body.trim();
  if (!text.startsWith('{') || !text.endsWith('}')) {
    return null;
  }
  try {
    final decoded = jsonDecode(text);
    if (decoded is! Map<String, dynamic>) {
      return null;
    }
    final disposition = decoded['finalDisposition'];
    final requirements = decoded['requirements'];
    if (decoded['summary'] is String &&
        (requirements == null || requirements is Map<String, dynamic>)) {
      return decoded;
    }
    if (disposition is String && decoded['summary'] is String) {
      return decoded;
    }
  } catch (_) {
    return null;
  }
  return null;
}

Map<String, dynamic>? _requirementsVerdictPayloadFromBody(ChatEntry entry) {
  if (entry.isStreaming || entry.author != 'Assistant') {
    return null;
  }
  final text = entry.body.trim();
  if (!text.startsWith('{') || !text.endsWith('}')) {
    return null;
  }
  try {
    final decoded = jsonDecode(text);
    if (decoded is! Map<String, dynamic>) {
      return null;
    }
    final nestedRequirements = decoded['requirements'];
    final verdictPayload = nestedRequirements is Map<String, dynamic>
        ? nestedRequirements
        : decoded;
    final overall = verdictPayload['overallVerdict'];
    final route = verdictPayload['route'];
	    if (overall is String && route is Map<String, dynamic>) {
	      return verdictPayload;
	    }
  } catch (_) {
    return null;
  }
  return null;
}

IconData _requirementsDispositionIcon(String disposition) {
  return switch (disposition) {
    'readyForRequirementsReview' => Icons.fact_check_rounded,
    'blockedNeedsOwnerAction' => Icons.warning_amber_rounded,
    'continueWorkNeeded' => Icons.build_circle_outlined,
    _ => Icons.rule_outlined,
  };
}

IconData _requirementsVerdictIcon(String verdict) {
  return switch (verdict) {
    'pass' => Icons.verified_rounded,
    'fail' => Icons.cancel_rounded,
    'acceptedBlocked' => Icons.warning_amber_rounded,
    'rejectedBlocked' => Icons.report_problem_outlined,
    'needsHumanWaiver' => Icons.gavel_rounded,
    _ => Icons.rate_review_outlined,
  };
}

IconData _requirementsClaimIcon(String claim) {
  return switch (claim) {
    'satisfied' => Icons.check_circle_rounded,
    'notSatisfied' => Icons.cancel_rounded,
    'blocked' => Icons.warning_amber_rounded,
    'notApplicable' => Icons.remove_circle_outline,
    _ => Icons.radio_button_checked_rounded,
  };
}

Color _requirementsVerdictColor(ThemeData theme, String verdict) {
  return switch (verdict) {
    'pass' => Colors.green.shade700,
    'fail' || 'rejectedBlocked' => theme.colorScheme.error,
    'acceptedBlocked' || 'needsHumanWaiver' => Colors.amber.shade800,
    _ => theme.colorScheme.secondary,
  };
}

Color _requirementsClaimColor(ThemeData theme, String claim) {
  return switch (claim) {
    'satisfied' => Colors.green.shade700,
    'notSatisfied' => theme.colorScheme.error,
    'blocked' => Colors.amber.shade800,
    'notApplicable' => theme.colorScheme.outline,
    _ => theme.colorScheme.secondary,
  };
}

String _titleCaseVerdict(String verdict) {
  return switch (verdict) {
    'acceptedBlocked' => 'Accepted blocker',
    'rejectedBlocked' => 'Rejected blocker',
    'needsHumanWaiver' => 'Needs waiver',
    _ => verdict.isEmpty ? 'Unknown' : verdict[0].toUpperCase() + verdict.substring(1),
  };
}

String _titleCaseClaim(String claim) {
  return switch (claim) {
    'notSatisfied' => 'Not satisfied',
    'notApplicable' => 'Not applicable',
    _ => claim.isEmpty ? 'Unknown' : claim[0].toUpperCase() + claim.substring(1),
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
      backgroundColor: theme.colorScheme.surfaceContainerHighest.withValues(alpha: 0.9),
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
      color: theme.colorScheme.onSurface.withValues(alpha: 0.82),
    ),
    listBullet: theme.textTheme.bodySmall?.copyWith(
      height: 1.35,
    ),
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
  return nodes.map((node) => _highlightNodeToSpan(node, baseStyle, theme)).toList();
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
  return TextSpan(
    text: node.value ?? '',
    style: style,
  );
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
    this.bridgeBaseUri,
  });

  final ChatEntry entry;
  final bool expanded;
  final ValueChanged<bool> onExpandedChanged;
  final ValueChanged<String>? onTerminateCommandExecution;
  final Uri? bridgeBaseUri;

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
          bridgeBaseUri: bridgeBaseUri,
        );
      case 'imageView':
        return _ImageGenerationEventRow(
          entry: entry,
          bridgeBaseUri: bridgeBaseUri,
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
    required this.bridgeBaseUri,
  });

  final ChatEntry entry;
  final Uri? bridgeBaseUri;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final timestampLabel = formatLocalTimeLabel(entry.timestamp);
    final path = entry.output?.trim();
    final canLoad = bridgeBaseUri != null && path != null && path.isNotEmpty;
    final thumbnailUri = canLoad
        ? bridgeBaseUri!.resolve('/images/thumbnail').replace(
              queryParameters: {'saved_path': path},
            )
        : null;
    final imageUri = canLoad
        ? bridgeBaseUri!.resolve('/images/image').replace(
              queryParameters: {'saved_path': path},
            )
        : null;

    return ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: 760),
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 4),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            if (thumbnailUri != null) ...[
              InkWell(
                borderRadius: BorderRadius.circular(14),
                onTap: () => _showImageDialog(context, imageUri!, path!),
                child: ClipRRect(
                  borderRadius: BorderRadius.circular(14),
                  child: Image.network(
                    thumbnailUri.toString(),
                    width: 100,
                    height: 100,
                    fit: BoxFit.cover,
                    errorBuilder: (_, _, _) => const _ImagePlaceholder(),
                  ),
                ),
              ),
              const SizedBox(width: 12),
            ],
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
                          color: theme.colorScheme.onSurface.withValues(alpha: 0.9),
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
                          color: theme.colorScheme.onSurface.withValues(alpha: 0.58),
                        ),
                      ),
                    ],
                  ),
                  if (entry.command?.trim().isNotEmpty ?? false) ...[
                    const SizedBox(height: 6),
                    SelectableText(
                      entry.command!.trim(),
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: theme.colorScheme.onSurface.withValues(alpha: 0.88),
                      ),
                    ),
                  ],
                  if (path != null && path.isNotEmpty) ...[
                    const SizedBox(height: 6),
                    SelectableText(
                      path,
                      style: theme.textTheme.bodySmall?.copyWith(
                        fontFamily: 'monospace',
                        color: theme.colorScheme.onSurface.withValues(alpha: 0.62),
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

  void _showImageDialog(BuildContext context, Uri imageUri, String path) {
    showDialog<void>(
      context: context,
      builder: (context) {
        return Dialog(
          backgroundColor: Colors.black,
          insetPadding: const EdgeInsets.all(18),
          child: Stack(
            children: [
              InteractiveViewer(
                minScale: 0.5,
                maxScale: 6,
                child: Center(
                  child: Image.network(
                    imageUri.toString(),
                    fit: BoxFit.contain,
                    errorBuilder: (_, _, _) => const Padding(
                      padding: EdgeInsets.all(32),
                      child: _ImagePlaceholder(),
                    ),
                  ),
                ),
              ),
              Positioned(
                top: 8,
                right: 8,
                child: IconButton.filledTonal(
                  tooltip: 'Close',
                  onPressed: () => Navigator.of(context).pop(),
                  icon: const Icon(Icons.close),
                ),
              ),
              Positioned(
                left: 12,
                right: 64,
                bottom: 12,
                child: SelectableText(
                  path,
                  style: Theme.of(context).textTheme.bodySmall?.copyWith(
                        color: Colors.white.withValues(alpha: 0.78),
                      ),
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}

class _ImagePlaceholder extends StatelessWidget {
  const _ImagePlaceholder();

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
    final canExpand = _hasValue(entry.output) || _hasValue(entry.body) || _hasValue(entry.processId);
    final canTerminate = (entry.isStreaming || _isInProgressStatus(entry.status)) &&
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
                        expanded ? Icons.keyboard_arrow_up_rounded : Icons.keyboard_arrow_down_rounded,
                        size: 18,
                        color: theme.colorScheme.onSurface.withValues(alpha: 0.68),
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
                    key: ValueKey('semantic.command.terminate.${entry.processId}'),
                    container: true,
                    button: true,
                    label: 'Terminate command process ${entry.processId}',
                    child: ExcludeSemantics(
                      child: IconButton(
                        onPressed: () => onTerminateCommandExecution?.call(entry.processId!),
                        icon: const Icon(Icons.stop_circle_outlined, size: 16),
                        tooltip: 'Terminate command',
                        splashRadius: 14,
                        padding: EdgeInsets.zero,
                        constraints: const BoxConstraints.tightFor(width: 22, height: 22),
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
                entry.command?.trim().isNotEmpty == true ? entry.command!.trim() : entry.body.trim(),
                style: theme.textTheme.bodySmall?.copyWith(
                  fontFamily: 'monospace',
                  height: 1.38,
                  color: theme.colorScheme.onSurface.withValues(alpha: 0.88),
                ),
              )
            else
              Text(
                _compactPreview(entry.command?.trim().isNotEmpty == true ? entry.command!.trim() : entry.body.trim()),
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
    final canExpand = _hasValue(entry.output) || _needsExpansion(entry.command) || _needsExpansion(entry.body);
    final title = entry.subtitle?.trim().isNotEmpty == true ? entry.subtitle!.trim() : 'Tool';
    final preview = entry.command?.trim().isNotEmpty == true ? entry.command!.trim() : entry.body.trim();

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
                      color: theme.colorScheme.onSurface.withValues(alpha: 0.88),
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
                        expanded ? Icons.keyboard_arrow_up_rounded : Icons.keyboard_arrow_down_rounded,
                        size: 18,
                        color: theme.colorScheme.onSurface.withValues(alpha: 0.68),
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
    final hasUnifiedDiff = _hasValue(entry.output) && _looksLikeUnifiedDiff(entry.output!);
    final canExpand = !hasUnifiedDiff && (_hasValue(entry.output) || _hasValue(entry.command));
    final summary = entry.command?.trim().isNotEmpty == true ? entry.command!.trim() : entry.body.trim();

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
                        style: theme.textTheme.labelMedium?.copyWith(fontWeight: FontWeight.w800),
                      ),
                      if (canExpand) ...[
                        const SizedBox(width: 2),
                        InkWell(
                          borderRadius: BorderRadius.circular(999),
                          onTap: () => onExpandedChanged(!expanded),
                          child: Padding(
                            padding: const EdgeInsets.all(2),
                            child: Icon(
                              expanded ? Icons.keyboard_arrow_up_rounded : Icons.keyboard_arrow_down_rounded,
                              size: 18,
                              color: theme.colorScheme.onSurface.withValues(alpha: 0.68),
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
                        color: theme.colorScheme.onSurface.withValues(alpha: 0.76),
                      ),
                    ),
                  ],
                  if (hasUnifiedDiff) ...[
                    const SizedBox(height: 8),
                    _DiffEventSection(value: entry.output!),
                  ] else if (expanded && _hasValue(entry.output)) ...[
                    const SizedBox(height: 8),
                    _EventSection(label: 'Diff', value: entry.output!, mono: true),
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
    final canExpand = _needsExpansion(entry.body) || _hasValue(entry.output) || _hasValue(entry.command);
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
          _AnimatedStatusDot(
            color: _eventStatusColor(theme, entry),
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Text(
                      label,
                      style: theme.textTheme.labelMedium?.copyWith(fontWeight: FontWeight.w700),
                    ),
                    if (canExpand) ...[
                      const SizedBox(width: 2),
                      InkWell(
                        borderRadius: BorderRadius.circular(999),
                        onTap: () => onExpandedChanged(!expanded),
                        child: Padding(
                          padding: const EdgeInsets.all(2),
                          child: Icon(
                            expanded ? Icons.keyboard_arrow_up_rounded : Icons.keyboard_arrow_down_rounded,
                            size: 18,
                            color: theme.colorScheme.onSurface.withValues(alpha: 0.68),
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
        ],
      ),
    );
  }
}

class _DiffEventSection extends StatelessWidget {
  const _DiffEventSection({
    required this.value,
  });

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
        borderRadius: BorderRadius.circular(12),
        color: theme.colorScheme.surfaceContainerHighest.withValues(alpha: 0.18),
        border: Border.all(
          color: theme.colorScheme.outline.withValues(alpha: 0.18),
        ),
      ),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(12),
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
  return lines.asMap().entries.map((entry) {
    final line = entry.value;
    final kind = _classifyDiffLine(line);
    final color = switch (kind) {
      _DiffLineKind.added => Colors.green.shade100,
      _DiffLineKind.removed => Colors.red.shade100,
      _DiffLineKind.hunk => theme.colorScheme.primary.withValues(alpha: 0.94),
      _DiffLineKind.header => theme.colorScheme.onSurface.withValues(alpha: 0.62),
      _DiffLineKind.context => theme.colorScheme.onSurface.withValues(alpha: 0.82),
    };
    final backgroundColor = switch (kind) {
      _DiffLineKind.added => Colors.green.shade900.withValues(alpha: 0.18),
      _DiffLineKind.removed => Colors.red.shade900.withValues(alpha: 0.16),
      _DiffLineKind.hunk => theme.colorScheme.primary.withValues(alpha: 0.12),
      _DiffLineKind.header => theme.colorScheme.surfaceContainerHighest.withValues(alpha: 0.42),
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
  }).toList(growable: false);
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
        color: theme.colorScheme.surfaceContainerHighest.withValues(alpha: 0.28),
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
  if (status.contains('fail') || status.contains('error') || status.contains('reject')) {
    return theme.colorScheme.error;
  }
  if (status.contains('progress') || status.contains('pending') || status.contains('running')) {
    return Colors.amber.shade700;
  }
  if (status.contains('complete') || status.contains('success') || status.contains('approved')) {
    return Colors.green.shade700;
  }
  return theme.colorScheme.secondary;
}


IconData _eventStatusIcon(ChatEntry entry) {
  final status = entry.status?.toLowerCase();
  if (status == null || status.isEmpty) {
    return entry.isStreaming ? Icons.schedule_rounded : Icons.check_circle_rounded;
  }
  if (status.contains('fail') || status.contains('error') || status.contains('reject')) {
    return Icons.cancel_rounded;
  }
  if (status.contains('progress') || status.contains('pending') || status.contains('running')) {
    return Icons.schedule_rounded;
  }
  if (status.contains('complete') || status.contains('success') || status.contains('approved')) {
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
      .where((line) => !RegExp(r'^\[(pending|in_progress|completed)\]\s+').hasMatch(line))
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

List<PlanChecklistItem> _planItemsFromBody(String body) {
  final statusPattern = RegExp(r'^\[(pending|in_progress|completed)\]\s+(.+)$');
  return body
      .replaceAll('\r\n', '\n')
      .split('\n')
      .map((line) => line.trim())
      .map(statusPattern.firstMatch)
      .whereType<RegExpMatch>()
      .map((match) {
        final status = match.group(1)!;
        return PlanChecklistItem(
          text: match.group(2)!.trim(),
          completed: status == 'completed',
          status: status,
        );
      })
      .toList(growable: false);
}
