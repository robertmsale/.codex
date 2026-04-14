import 'package:flutter_markdown_plus/flutter_markdown_plus.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:highlight/highlight.dart' as hl;

import '../../core/models/workbench_models.dart';
import '../composer/composer_panel.dart';

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
    this.showComposer = true,
    this.headerControls,
    this.overlay,
    this.leading,
    this.onTerminateCommandExecution,
  });

  final String? threadId;
  final List<ChatEntry> entries;
  final String title;
  final int? contextWindowRemainingPercent;
  final ValueChanged<String> onSend;
  final VoidCallback onInterrupt;
  final bool composerEnabled;
  final bool isRunning;
  final bool showComposer;
  final Widget? headerControls;
  final Widget? overlay;
  final Widget? leading;
  final ValueChanged<String>? onTerminateCommandExecution;

  @override
  State<ChatTimeline> createState() => _ChatTimelineState();
}

class _ChatTimelineState extends State<ChatTimeline> {
  late final ScrollController _scrollController;
  final Set<String> _expandedEntryKeys = <String>{};

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

    final hadClients = _scrollController.hasClients;
    final previousPixels = hadClients ? _scrollController.position.pixels : 0.0;

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

    final shouldStickToBottom = threadChanged || _isNearBottom();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_scrollController.hasClients || !shouldStickToBottom) {
        if (!mounted || !_scrollController.hasClients) {
          return;
        }
        final position = _scrollController.position;
        final target = previousPixels.clamp(
          position.minScrollExtent,
          position.maxScrollExtent,
        );
        if ((position.pixels - target).abs() > 1) {
          _scrollController.jumpTo(target);
        }
        return;
      }
      final position = _scrollController.position;
      final target = position.maxScrollExtent.clamp(
        position.minScrollExtent,
        position.maxScrollExtent,
      );
      if ((position.pixels - target).abs() > 1) {
        _scrollController.jumpTo(target);
      }
    });
  }

  bool _isNearBottom() {
    if (!_scrollController.hasClients) {
      return true;
    }
    final position = _scrollController.position;
    return position.maxScrollExtent - position.pixels < 96;
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            if (widget.leading != null) ...[
              widget.leading!,
              const SizedBox(width: 8),
            ],
            Expanded(
              child: Text(
                widget.title,
                style: theme.textTheme.bodyMedium?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
            ),
            Text(
              widget.contextWindowRemainingPercent == null
                  ? '--% Remaining'
                  : '${widget.contextWindowRemainingPercent!}% Remaining',
              style: theme.textTheme.labelSmall,
            ),
          ],
        ),
        if (widget.headerControls != null) ...[
          const SizedBox(height: 6),
          widget.headerControls!,
        ],
        const SizedBox(height: 8),
        Expanded(
          child: Stack(
            children: [
              SelectionArea(
                child: ListView.separated(
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
                      onTerminateCommandExecution: widget.onTerminateCommandExecution,
                    );
                  },
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
          const SizedBox(height: 8),
          ComposerPanel(
            enabled: widget.composerEnabled,
            isRunning: widget.isRunning,
            onSend: widget.onSend,
            onInterrupt: widget.onInterrupt,
          ),
        ],
      ],
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
      : '${entry.kind}|${entry.timestampLabel}|${entry.processId ?? ''}|${entry.command ?? entry.body}';
  return stableId;
}

class _ChatBubble extends StatelessWidget {
  const _ChatBubble({
    super.key,
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
    final isConversation = !entry.isTool &&
        (entry.author == 'User' || entry.author == 'Assistant' || entry.author == 'Operator');

    if (entry.hasPlanItems) {
      return _PlanUpdateCard(entry: entry);
    }

    if (!isConversation) {
      return _InlineEventRow(
        entry: entry,
        expanded: expanded,
        onExpandedChanged: onExpandedChanged,
        onTerminateCommandExecution: onTerminateCommandExecution,
      );
    }

    final isUser = entry.author == 'User' || entry.author == 'Operator';
    final isPending = entry.deliveryState == 'pending';
    final bubbleColor = isUser
        ? theme.colorScheme.primary.withValues(alpha: isPending ? 0.1 : 0.18)
        : theme.colorScheme.surface.withValues(alpha: 0.9);

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
                  ? theme.colorScheme.primary.withValues(alpha: isPending ? 0.28 : 0.45)
                  : theme.colorScheme.outline,
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
                              fontWeight: FontWeight.w700,
                              color: isPending
                                  ? theme.colorScheme.onSurface.withValues(alpha: 0.68)
                                  : null,
                            ),
                          ),
                          Text(entry.timestampLabel, style: theme.textTheme.labelSmall),
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
                    IconButton(
                      onPressed: () => _copyBubbleText(context, entry.body),
                      icon: const Icon(Icons.content_copy_rounded, size: 14),
                      tooltip: 'Copy',
                      splashRadius: 14,
                      padding: EdgeInsets.zero,
                      constraints: const BoxConstraints.tightFor(width: 22, height: 22),
                      visualDensity: VisualDensity.compact,
                    ),
                  ],
                ),
                const SizedBox(height: 4),
                SelectionArea(
                  child: MarkdownBody(
                    data: entry.body,
                    selectable: false,
                    styleSheet: _conversationMarkdownStyle(theme, isPending),
                    syntaxHighlighter: _ChatCodeSyntaxHighlighter(theme),
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

class _PlanUpdateCard extends StatelessWidget {
  const _PlanUpdateCard({
    required this.entry,
  });

  final ChatEntry entry;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final note = _planSummary(entry.body);
    final completedCount = entry.planItems.where((item) => item.completed).length;
    final activeCount = entry.planItems.where((item) => item.isInProgress).length;
    final pendingCount = entry.planItems.length - completedCount - activeCount;
    final accent = entry.isStreaming
        ? Colors.amber.shade700
        : theme.colorScheme.primary;
    final surfaceTone = Color.alphaBlend(
      accent.withValues(alpha: 0.08),
      theme.colorScheme.surfaceContainerLow,
    );

    return Align(
      alignment: Alignment.centerLeft,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 720),
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: surfaceTone,
            borderRadius: BorderRadius.circular(18),
            border: Border.all(
              color: accent.withValues(alpha: 0.35),
            ),
            boxShadow: [
              BoxShadow(
                color: theme.colorScheme.shadow.withValues(alpha: 0.08),
                blurRadius: 12,
                offset: const Offset(0, 4),
              ),
            ],
          ),
          child: Padding(
            padding: const EdgeInsets.fromLTRB(14, 12, 14, 14),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Container(
                      width: 30,
                      height: 30,
                      alignment: Alignment.center,
                      decoration: BoxDecoration(
                        color: accent.withValues(alpha: 0.14),
                        borderRadius: BorderRadius.circular(10),
                      ),
                      child: Text(
                        '◻',
                        style: theme.textTheme.titleSmall?.copyWith(
                          color: accent,
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                    ),
                    const SizedBox(width: 10),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            entry.displayLabel,
                            style: theme.textTheme.titleSmall?.copyWith(
                              fontWeight: FontWeight.w800,
                            ),
                          ),
                          const SizedBox(height: 2),
                          Wrap(
                            spacing: 6,
                            runSpacing: 6,
                            children: [
                              _PlanMetaPill(
                                label: '$completedCount done',
                                tone: Colors.green.shade700,
                              ),
                              if (activeCount > 0)
                                _PlanMetaPill(
                                  label: '$activeCount active',
                                  tone: Colors.amber.shade800,
                                ),
                              if (pendingCount > 0)
                                _PlanMetaPill(
                                  label: '$pendingCount queued',
                                  tone: theme.colorScheme.onSurface.withValues(alpha: 0.72),
                                ),
                            ],
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(width: 8),
                    Column(
                      crossAxisAlignment: CrossAxisAlignment.end,
                      children: [
                        Text(
                          entry.timestampLabel,
                          style: theme.textTheme.labelSmall,
                        ),
                        if (entry.isStreaming) ...[
                          const SizedBox(height: 8),
                          const SizedBox(
                            width: 12,
                            height: 12,
                            child: CircularProgressIndicator(strokeWidth: 1.8),
                          ),
                        ],
                      ],
                    ),
                  ],
                ),
                const SizedBox(height: 12),
                Container(
                  width: double.infinity,
                  padding: const EdgeInsets.fromLTRB(12, 10, 12, 10),
                  decoration: BoxDecoration(
                    color: theme.colorScheme.surface.withValues(alpha: 0.72),
                    borderRadius: BorderRadius.circular(14),
                    border: Border.all(
                      color: accent.withValues(alpha: 0.18),
                    ),
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        'Checklist',
                        style: theme.textTheme.labelMedium?.copyWith(
                          color: accent,
                          fontWeight: FontWeight.w800,
                          letterSpacing: 0.2,
                        ),
                      ),
                      if (note != null) ...[
                        const SizedBox(height: 8),
                        Text(
                          note,
                          style: theme.textTheme.bodySmall?.copyWith(
                            color: theme.colorScheme.onSurface.withValues(alpha: 0.8),
                            height: 1.4,
                          ),
                        ),
                      ],
                      const SizedBox(height: 12),
                      ...entry.planItems.map(
                        (item) => Padding(
                          padding: const EdgeInsets.only(bottom: 10),
                          child: _PlanChecklistRow(item: item),
                        ),
                      ),
                    ],
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

class _PlanMetaPill extends StatelessWidget {
  const _PlanMetaPill({
    required this.label,
    required this.tone,
  });

  final String label;
  final Color tone;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return DecoratedBox(
      decoration: BoxDecoration(
        color: tone.withValues(alpha: 0.12),
        borderRadius: BorderRadius.circular(999),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
        child: Text(
          label,
          style: theme.textTheme.labelSmall?.copyWith(
            color: tone,
            fontWeight: FontWeight.w700,
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
            : theme.colorScheme.onSurface.withValues(alpha: 0.72);
    final glyph = item.completed ? '☑' : '◻';

    return DecoratedBox(
      decoration: BoxDecoration(
        color: theme.colorScheme.surface.withValues(alpha: 0.92),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(
          color: accent.withValues(alpha: item.isInProgress ? 0.45 : 0.22),
        ),
      ),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(10, 10, 10, 10),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Padding(
              padding: const EdgeInsets.only(top: 1),
              child: Text(
                glyph,
                style: theme.textTheme.bodyMedium?.copyWith(
                  color: accent,
                  fontWeight: FontWeight.w700,
                ),
              ),
            ),
            const SizedBox(width: 10),
            Expanded(
              child: Text(
                item.text,
                style: theme.textTheme.bodyMedium?.copyWith(
                  height: 1.35,
                  color: item.completed
                      ? theme.colorScheme.onSurface.withValues(alpha: 0.68)
                      : theme.colorScheme.onSurface,
                  decoration: item.completed ? TextDecoration.lineThrough : null,
                  decorationColor: accent.withValues(alpha: 0.7),
                ),
              ),
            ),
            if (item.isInProgress) ...[
              const SizedBox(width: 8),
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
                decoration: BoxDecoration(
                  color: accent.withValues(alpha: 0.12),
                  borderRadius: BorderRadius.circular(999),
                ),
                child: Text(
                  'Active',
                  style: theme.textTheme.labelSmall?.copyWith(
                    color: accent,
                    fontWeight: FontWeight.w700,
                  ),
                ),
              ),
            ],
          ],
        ),
      ),
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
  });

  final ChatEntry entry;
  final bool expanded;
  final ValueChanged<bool> onExpandedChanged;
  final ValueChanged<String>? onTerminateCommandExecution;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final entry = this.entry;
    final detailRows = <Widget>[
      if (_shouldShowSummary(entry))
        _DetailLine(
          label: 'Summary',
          value: entry.body,
          expanded: expanded,
        ),
      if (_hasValue(entry.processId))
        _DetailLine(
          label: 'PID',
          value: entry.processId!,
          expanded: true,
        ),
      if (_hasValue(entry.command))
        _DetailLine(
          label: entry.kind == 'mcpToolCall' ? 'Input' : entry.kind == 'fileChange' ? 'Files' : 'Command',
          value: entry.command!,
          expanded: expanded,
        ),
      if (_hasValue(entry.output))
        _DetailLine(
          label: entry.kind == 'fileChange' ? 'Diff' : 'Output',
          value: entry.output!,
          expanded: expanded,
        ),
    ];
    final canExpand = detailRows.isNotEmpty &&
        (detailRows.length > 1 ||
            _needsExpansion(entry.body) ||
            _needsExpansion(entry.command) ||
            _needsExpansion(entry.output));
    final canTerminate = entry.kind == 'commandExecution' &&
        (entry.isStreaming || _isInProgressStatus(entry.status)) &&
        (entry.processId?.trim().isNotEmpty ?? false) &&
        entry.id.trim().isNotEmpty &&
        onTerminateCommandExecution != null;

    return DecoratedBox(
      decoration: BoxDecoration(
        border: Border(
          left: BorderSide(
            color: _eventAccentColor(theme, entry),
            width: 2,
          ),
        ),
      ),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(8, 2, 0, 2),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Text(
                  entry.displayLabel,
                  style: theme.textTheme.labelSmall?.copyWith(
                    fontWeight: FontWeight.w700,
                  ),
                ),
                if (_hasValue(entry.subtitle)) ...[
                  const SizedBox(width: 6),
                  Expanded(
                    child: Text(
                      entry.subtitle!,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: theme.textTheme.labelSmall?.copyWith(
                        color: _eventAccentColor(theme, entry),
                      ),
                    ),
                  ),
                ] else
                  const Spacer(),
                const SizedBox(width: 6),
                Text(entry.timestampLabel, style: theme.textTheme.labelSmall),
                if (entry.isStreaming && entry.displayLabel != 'Diff') ...[
                  const SizedBox(width: 6),
                  const SizedBox(
                    width: 8,
                    height: 8,
                    child: CircularProgressIndicator(strokeWidth: 1.5),
                  ),
                ],
                if (canTerminate) ...[
                  const SizedBox(width: 4),
                  IconButton(
                    onPressed: () => onTerminateCommandExecution?.call(entry.processId!),
                    icon: const Icon(Icons.stop_circle_outlined, size: 16),
                    tooltip: 'Terminate command',
                    splashRadius: 14,
                    padding: EdgeInsets.zero,
                    constraints: const BoxConstraints.tightFor(width: 22, height: 22),
                    visualDensity: VisualDensity.compact,
                    color: theme.colorScheme.error,
                  ),
                ],
                if (canExpand) ...[
                  const SizedBox(width: 4),
                  TextButton(
                    onPressed: () => onExpandedChanged(!expanded),
                    style: TextButton.styleFrom(
                      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 0),
                      minimumSize: const Size(0, 20),
                      tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                      visualDensity: VisualDensity.compact,
                    ),
                    child: Text(expanded ? 'Less' : 'More'),
                  ),
                ],
              ],
            ),
            const SizedBox(height: 3),
            ...detailRows,
          ],
        ),
      ),
    );
  }
}

class _DetailLine extends StatelessWidget {
  const _DetailLine({
    required this.label,
    required this.value,
    required this.expanded,
  });

  final String label;
  final String value;
  final bool expanded;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final text = expanded ? value : _compactPreview(value);
    return Padding(
      padding: const EdgeInsets.only(bottom: 2),
      child: Text.rich(
        TextSpan(
          style: theme.textTheme.bodySmall?.copyWith(
            fontFamily: 'monospace',
            height: 1.3,
            color: theme.colorScheme.onSurface,
          ),
          children: [
            TextSpan(
              text: '$label ',
              style: theme.textTheme.labelSmall?.copyWith(
                fontFamily: 'monospace',
                fontWeight: FontWeight.w700,
                color: _eventAccentColor(theme, null),
              ),
            ),
            TextSpan(text: text),
          ],
        ),
        textScaler: MediaQuery.textScalerOf(context),
      ),
    );
  }
}

bool _hasValue(String? value) => value != null && value.trim().isNotEmpty;

Color _eventAccentColor(ThemeData theme, ChatEntry? entry) {
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

bool _shouldShowSummary(ChatEntry entry) {
  if (entry.hasPlanItems) {
    return false;
  }
  final body = entry.body.trim();
  if (body.isEmpty) {
    return false;
  }
  if (entry.kind == 'commandExecution' && entry.command?.trim() == body) {
    return false;
  }
  return true;
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
