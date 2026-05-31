import 'dart:convert';
import 'dart:math' as math;

import 'package:fl_chart/fl_chart.dart';
import 'package:flutter/material.dart';
import 'package:http/http.dart' as http;

import '../../core/models/thread_stats_models.dart';

Future<void> showThreadStatsModal({
  required BuildContext context,
  required String threadId,
  required Uri? bridgeBaseUri,
  Future<ThreadStatsData> Function(String threadId)? loadStats,
}) async {
  final theme = Theme.of(context);
  if (bridgeBaseUri == null && loadStats == null) {
    ScaffoldMessenger.of(context).showSnackBar(
      const SnackBar(content: Text('Thread statistics are unavailable without a bridge connection.')),
    );
    return;
  }

  showDialog<void>(
    context: context,
    barrierDismissible: false,
    builder: (context) => Dialog(
      child: SizedBox(
        width: 360,
        child: Padding(
          padding: const EdgeInsets.all(18),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              const SizedBox(
                width: 22,
                height: 22,
                child: CircularProgressIndicator(strokeWidth: 2.4),
              ),
              const SizedBox(width: 14),
              Expanded(
                child: Text(
                  'Processing thread statistics...',
                  style: theme.textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.w700),
                ),
              ),
            ],
          ),
        ),
      ),
    ),
  );

  try {
    final stats = loadStats != null
        ? await loadStats(threadId)
        : await _fetchThreadStats(threadId: threadId, bridgeBaseUri: bridgeBaseUri!);
    if (!context.mounted) {
      return;
    }
    Navigator.of(context, rootNavigator: true).pop();
    await showDialog<void>(
      context: context,
      builder: (context) => ThreadStatsModalView(stats: stats),
    );
  } catch (error) {
    if (!context.mounted) {
      return;
    }
    Navigator.of(context, rootNavigator: true).pop();
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text('Unable to load thread statistics: $error')),
    );
  }
}

Future<ThreadStatsData> _fetchThreadStats({
  required String threadId,
  required Uri bridgeBaseUri,
}) async {
  final url = bridgeBaseUri.resolve('/threads/${Uri.encodeComponent(threadId)}/stats');
  final response = await http.get(url);
  if (response.statusCode == 404) {
    throw StateError('No session statistics found for this thread.');
  }
  if (response.statusCode < 200 || response.statusCode >= 300) {
    throw StateError('HTTP ${response.statusCode}: ${response.body}');
  }
  return ThreadStatsData.fromJson(jsonDecode(response.body) as Map<String, dynamic>);
}

class ThreadStatsModalView extends StatelessWidget {
  const ThreadStatsModalView({
    super.key,
    required this.stats,
  });

  final ThreadStatsData stats;

  @override
  Widget build(BuildContext context) {
    final media = MediaQuery.of(context);
    final width = math.min(media.size.width * 0.94, 1280.0);
    final height = math.min(media.size.height * 0.9, 900.0);
    final theme = Theme.of(context);

    return Dialog(
      insetPadding: const EdgeInsets.all(28),
      child: SizedBox(
        width: width,
        height: height,
        child: Column(
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(24, 20, 14, 16),
              child: Row(
                children: [
                  Container(
                    width: 38,
                    height: 38,
                    decoration: BoxDecoration(
                      color: theme.colorScheme.primary.withValues(alpha: 0.14),
                      borderRadius: BorderRadius.circular(8),
                    ),
                    child: Icon(Icons.query_stats_rounded, color: theme.colorScheme.primary),
                  ),
                  const SizedBox(width: 14),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          'Thread Statistics',
                          style: theme.textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w800),
                        ),
                        const SizedBox(height: 3),
                        Text(
                          stats.sessionPath,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: theme.textTheme.bodySmall?.copyWith(
                            color: theme.colorScheme.onSurface.withValues(alpha: 0.58),
                          ),
                        ),
                      ],
                    ),
                  ),
                  IconButton(
                    tooltip: 'Close',
                    onPressed: () => Navigator.of(context).pop(),
                    icon: const Icon(Icons.close_rounded),
                  ),
                ],
              ),
            ),
            Divider(height: 1, color: theme.colorScheme.outline.withValues(alpha: 0.52)),
            Expanded(
              child: SingleChildScrollView(
                padding: const EdgeInsets.fromLTRB(24, 22, 24, 24),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    _HeroMetrics(stats: stats),
                    const SizedBox(height: 22),
                    LayoutBuilder(
                      builder: (context, constraints) {
                        final useTwoColumns = constraints.maxWidth >= 980;
                        if (!useTwoColumns) {
                          return Column(
                            children: [
                              _Panel(
                                title: 'Token Timeline',
                                subtitle: 'Per-call uncached input plus output and reasoning tokens. Cached prompt replay is excluded.',
                                child: SizedBox(height: 280, child: _TimelineChart(points: stats.timeline)),
                              ),
                              const SizedBox(height: 18),
                              _Panel(
                                title: 'Cumulative Usage',
                                subtitle: 'Cumulative uncached input plus output and reasoning tokens.',
                                child: SizedBox(height: 280, child: _CumulativeChart(points: stats.timeline)),
                              ),
                              const SizedBox(height: 18),
                              _Panel(
                                title: 'Attribution Breakdown',
                                subtitle: 'Estimated distribution by detectable event type. Reported totals stay in the summary tiles.',
                                child: SizedBox(height: 310, child: _CategoryBreakdownView(stats: stats)),
                              ),
                              const SizedBox(height: 18),
                              _Panel(
                                title: 'Top Expensive Items',
                                subtitle: 'Largest estimated text payloads found in the session log.',
                                child: SizedBox(height: 310, child: _TopItemsView(items: stats.topItems)),
                              ),
                            ],
                          );
                        }
                        return Column(
                          children: [
                            Row(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                Expanded(
                                  child: _Panel(
                                    title: 'Token Timeline',
                                    subtitle: 'Per-call uncached input plus output and reasoning tokens. Cached prompt replay is excluded.',
                                    child: SizedBox(height: 320, child: _TimelineChart(points: stats.timeline)),
                                  ),
                                ),
                                const SizedBox(width: 18),
                                Expanded(
                                  child: _Panel(
                                    title: 'Cumulative Usage',
                                    subtitle: 'Cumulative uncached input plus output and reasoning tokens.',
                                    child: SizedBox(height: 320, child: _CumulativeChart(points: stats.timeline)),
                                  ),
                                ),
                              ],
                            ),
                            const SizedBox(height: 18),
                            Row(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                Expanded(
                                  child: _Panel(
                                    title: 'Attribution Breakdown',
                                    subtitle: 'Estimated distribution by detectable event type. Reported totals stay in the summary tiles.',
                                    child: SizedBox(height: 340, child: _CategoryBreakdownView(stats: stats)),
                                  ),
                                ),
                                const SizedBox(width: 18),
                                Expanded(
                                  child: _Panel(
                                    title: 'Top Expensive Items',
                                    subtitle: 'Largest estimated text payloads found in the session log.',
                                    child: SizedBox(height: 340, child: _TopItemsView(items: stats.topItems)),
                                  ),
                                ),
                              ],
                            ),
                          ],
                        );
                      },
                    ),
                    if (stats.warnings.isNotEmpty) ...[
                      const SizedBox(height: 18),
                      _WarningPanel(warnings: stats.warnings),
                    ],
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _HeroMetrics extends StatelessWidget {
  const _HeroMetrics({required this.stats});

  final ThreadStatsData stats;

  @override
  Widget build(BuildContext context) {
    final totals = stats.totals;
    return LayoutBuilder(
      builder: (context, constraints) {
        final columns = constraints.maxWidth >= 1000 ? 6 : constraints.maxWidth >= 680 ? 3 : 2;
        final tileWidth = (constraints.maxWidth - (columns - 1) * 12) / columns;
        return Wrap(
          spacing: 12,
          runSpacing: 12,
          children: [
            _MetricTile(width: tileWidth, label: 'Prompt Input', value: _formatNumber(totals.inputTokens), icon: Icons.input_rounded),
            _MetricTile(width: tileWidth, label: 'Uncached Input', value: _formatNumber(totals.uncachedInputTokens), icon: Icons.new_releases_rounded),
            _MetricTile(width: tileWidth, label: 'Output Tokens', value: _formatNumber(totals.outputTokens), icon: Icons.output_rounded),
            _MetricTile(width: tileWidth, label: 'Cached Tokens', value: _formatNumber(totals.cachedInputTokens), icon: Icons.memory_rounded),
            _MetricTile(width: tileWidth, label: 'Reasoning Tokens', value: _formatNumber(totals.reasoningOutputTokens), icon: Icons.psychology_alt_rounded),
            _MetricTile(width: tileWidth, label: 'Compactions', value: stats.compactionCount.toString(), icon: Icons.compress_rounded),
            _MetricTile(width: tileWidth, label: 'Unique Tool Output', value: _formatNumber(stats.estimates.toolOutputInputTokens), icon: Icons.terminal_rounded),
          ],
        );
      },
    );
  }
}

class _MetricTile extends StatelessWidget {
  const _MetricTile({
    required this.width,
    required this.label,
    required this.value,
    required this.icon,
  });

  final double width;
  final String label;
  final String value;
  final IconData icon;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      width: width,
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: theme.colorScheme.surfaceContainerHighest.withValues(alpha: 0.52),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: theme.colorScheme.outline.withValues(alpha: 0.46)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(icon, size: 18, color: theme.colorScheme.primary),
              const Spacer(),
              Text(
                value,
                style: theme.textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w900),
              ),
            ],
          ),
          const SizedBox(height: 10),
          Text(
            label,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: theme.textTheme.labelMedium?.copyWith(
              color: theme.colorScheme.onSurface.withValues(alpha: 0.64),
              fontWeight: FontWeight.w700,
            ),
          ),
        ],
      ),
    );
  }
}

class _Panel extends StatelessWidget {
  const _Panel({
    required this.title,
    required this.subtitle,
    required this.child,
  });

  final String title;
  final String subtitle;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: theme.colorScheme.surface.withValues(alpha: 0.7),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: theme.colorScheme.outline.withValues(alpha: 0.42)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(title, style: theme.textTheme.titleMedium?.copyWith(fontWeight: FontWeight.w800)),
          const SizedBox(height: 3),
          Text(
            subtitle,
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.onSurface.withValues(alpha: 0.58),
            ),
          ),
          const SizedBox(height: 14),
          child,
        ],
      ),
    );
  }
}

class _TimelineChart extends StatelessWidget {
  const _TimelineChart({required this.points});

  final List<TokenTimelinePoint> points;

  @override
  Widget build(BuildContext context) {
    if (points.isEmpty) {
      return const _EmptyChart(text: 'No token timeline events found.');
    }
    final visible = points.length > 64
        ? points.sublist(points.length - 64)
        : points.toList(growable: false);
    final maxY = visible.map((point) => point.deltaTokens).fold<int>(1, math.max).toDouble();
    return BarChart(
      BarChartData(
        maxY: maxY * 1.18,
        gridData: _gridData(context),
        borderData: FlBorderData(show: false),
        titlesData: _bottomLeftTitles(context, leftValue: (value) => _compactTick(value.toInt())),
        barTouchData: BarTouchData(
          touchTooltipData: BarTouchTooltipData(
            getTooltipColor: (_) => Theme.of(context).colorScheme.inverseSurface,
            getTooltipItem: (group, groupIndex, rod, rodIndex) {
              final point = visible[groupIndex];
              return BarTooltipItem(
                'Event ${point.index}\n+${_formatNumber(point.deltaTokens)} uncached\n'
                'input ${_formatNumber(point.uncachedInputTokens)} · output ${_formatNumber(point.outputTokens)} · reasoning ${_formatNumber(point.reasoningOutputTokens)}\n'
                'line ${point.line}',
                TextStyle(color: Theme.of(context).colorScheme.onInverseSurface, fontWeight: FontWeight.w700),
              );
            },
          ),
        ),
        barGroups: [
          for (var i = 0; i < visible.length; i++)
            BarChartGroupData(
              x: visible[i].index,
              barRods: [
                BarChartRodData(
                  toY: visible[i].deltaTokens.toDouble(),
                  width: 12,
                  borderRadius: BorderRadius.circular(5),
                  gradient: LinearGradient(
                    begin: Alignment.bottomCenter,
                    end: Alignment.topCenter,
                    colors: [
                      Theme.of(context).colorScheme.primary,
                      Theme.of(context).colorScheme.tertiary,
                    ],
                  ),
                ),
              ],
            ),
        ],
      ),
    );
  }
}

class _CumulativeChart extends StatelessWidget {
  const _CumulativeChart({required this.points});

  final List<TokenTimelinePoint> points;

  @override
  Widget build(BuildContext context) {
    if (points.isEmpty) {
      return const _EmptyChart(text: 'No cumulative usage data found.');
    }
    final visible = _sampleTimeline(points, 96);
    final maxY = visible.map((point) => point.totalTokens).fold<int>(1, math.max).toDouble();
    return LineChart(
      LineChartData(
        minY: 0,
        maxY: maxY * 1.08,
        minX: visible.first.index.toDouble(),
        maxX: visible.last.index.toDouble(),
        gridData: _gridData(context),
        borderData: FlBorderData(show: false),
        titlesData: _bottomLeftTitles(context, leftValue: (value) => _compactTick(value.toInt())),
        lineTouchData: LineTouchData(
          touchTooltipData: LineTouchTooltipData(
            getTooltipColor: (_) => Theme.of(context).colorScheme.inverseSurface,
            getTooltipItems: (spots) => spots.map((spot) {
              final point = visible.firstWhere(
                (candidate) => candidate.index == spot.x.round(),
                orElse: () => visible.last,
              );
              return LineTooltipItem(
                'Event ${point.index}\n${_formatNumber(point.totalTokens)} cumulative uncached',
                TextStyle(color: Theme.of(context).colorScheme.onInverseSurface, fontWeight: FontWeight.w700),
              );
            }).toList(growable: false),
          ),
        ),
        lineBarsData: [
          LineChartBarData(
            spots: [
              for (var i = 0; i < visible.length; i++)
                FlSpot(visible[i].index.toDouble(), visible[i].totalTokens.toDouble()),
            ],
            isCurved: true,
            curveSmoothness: 0.18,
            color: Theme.of(context).colorScheme.primary,
            barWidth: 3,
            isStrokeCapRound: true,
            dotData: const FlDotData(show: false),
            belowBarData: BarAreaData(
              show: true,
              gradient: LinearGradient(
                begin: Alignment.topCenter,
                end: Alignment.bottomCenter,
                colors: [
                  Theme.of(context).colorScheme.primary.withValues(alpha: 0.18),
                  Theme.of(context).colorScheme.primary.withValues(alpha: 0.02),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _CategoryBreakdownView extends StatelessWidget {
  const _CategoryBreakdownView({required this.stats});

  final ThreadStatsData stats;

  @override
  Widget build(BuildContext context) {
    final categories = stats.categories.where((category) => category.tokens > 0).take(8).toList(growable: false);
    if (categories.isEmpty) {
      return const _EmptyChart(text: 'No token categories found.');
    }
    final colors = _chartColors(context);
    return Row(
      children: [
        SizedBox(
          width: 180,
          child: PieChart(
            PieChartData(
              centerSpaceRadius: 46,
              sectionsSpace: 3,
              sections: [
                for (var i = 0; i < categories.length; i++)
                  PieChartSectionData(
                    value: categories[i].tokens.toDouble(),
                    title: categories[i].estimated ? '~' : '',
                    radius: 72,
                    color: colors[i % colors.length],
                    titleStyle: Theme.of(context).textTheme.labelMedium?.copyWith(
                          color: Colors.white,
                          fontWeight: FontWeight.w900,
                        ),
                  ),
              ],
            ),
          ),
        ),
        const SizedBox(width: 18),
        Expanded(child: _CategoryLegend(categories: categories, colors: colors)),
      ],
    );
  }
}

class _CategoryLegend extends StatelessWidget {
  const _CategoryLegend({required this.categories, required this.colors});

  final List<TokenCategoryBreakdown> categories;
  final List<Color> colors;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return ListView.separated(
      itemCount: categories.length,
      separatorBuilder: (_, _) => const SizedBox(height: 8),
      itemBuilder: (context, index) {
        final category = categories[index];
        return Row(
          children: [
            Container(
              width: 10,
              height: 10,
              decoration: BoxDecoration(
                color: colors[index % colors.length],
                borderRadius: BorderRadius.circular(3),
              ),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                category.label,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: theme.textTheme.bodySmall?.copyWith(fontWeight: FontWeight.w700),
              ),
            ),
            const SizedBox(width: 8),
            Text(
              '${category.estimated ? '~' : ''}${_formatNumber(category.tokens)}',
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onSurface.withValues(alpha: 0.7),
                fontFeatures: const [FontFeature.tabularFigures()],
              ),
            ),
          ],
        );
      },
    );
  }
}

class _TopItemsView extends StatelessWidget {
  const _TopItemsView({required this.items});

  final List<TokenTopItem> items;

  @override
  Widget build(BuildContext context) {
    final visible = items.take(10).toList(growable: false);
    if (visible.isEmpty) {
      return const _EmptyChart(text: 'No expensive items found.');
    }
    final maxTokens = visible.map((item) => item.tokens).fold<int>(1, math.max);
    final theme = Theme.of(context);
    return ListView.separated(
      itemCount: visible.length,
      separatorBuilder: (_, _) => const SizedBox(height: 10),
      itemBuilder: (context, index) {
        final item = visible[index];
        final widthFactor = item.tokens / maxTokens;
        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Expanded(
                  child: Text(
                    item.label,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: theme.textTheme.bodySmall?.copyWith(fontWeight: FontWeight.w800),
                  ),
                ),
                const SizedBox(width: 10),
                Text(
                  '${item.estimated ? '~' : ''}${_formatNumber(item.tokens)}',
                  style: theme.textTheme.labelMedium?.copyWith(
                    color: theme.colorScheme.onSurface.withValues(alpha: 0.7),
                    fontFeatures: const [FontFeature.tabularFigures()],
                  ),
                ),
              ],
            ),
            const SizedBox(height: 5),
            ClipRRect(
              borderRadius: BorderRadius.circular(999),
              child: Stack(
                children: [
                  Container(height: 9, color: theme.colorScheme.onSurface.withValues(alpha: 0.08)),
                  FractionallySizedBox(
                    widthFactor: widthFactor,
                    child: Container(
                      height: 9,
                      decoration: BoxDecoration(
                        gradient: LinearGradient(
                          colors: [
                            theme.colorScheme.secondary,
                            theme.colorScheme.primary,
                          ],
                        ),
                      ),
                    ),
                  ),
                ],
              ),
            ),
            const SizedBox(height: 3),
            Text(
              '${item.kind} · line ${item.line}',
              style: theme.textTheme.labelSmall?.copyWith(
                color: theme.colorScheme.onSurface.withValues(alpha: 0.48),
              ),
            ),
          ],
        );
      },
    );
  }
}

class _WarningPanel extends StatelessWidget {
  const _WarningPanel({required this.warnings});

  final List<String> warnings;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: theme.colorScheme.errorContainer.withValues(alpha: 0.22),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: theme.colorScheme.error.withValues(alpha: 0.28)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text('Parser Warnings', style: theme.textTheme.titleSmall?.copyWith(fontWeight: FontWeight.w800)),
          const SizedBox(height: 8),
          for (final warning in warnings.take(5))
            Padding(
              padding: const EdgeInsets.only(bottom: 4),
              child: Text(warning, style: theme.textTheme.bodySmall),
            ),
        ],
      ),
    );
  }
}

class _EmptyChart extends StatelessWidget {
  const _EmptyChart({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Center(
      child: Text(
        text,
        style: theme.textTheme.bodyMedium?.copyWith(
          color: theme.colorScheme.onSurface.withValues(alpha: 0.56),
        ),
      ),
    );
  }
}

FlGridData _gridData(BuildContext context) {
  return FlGridData(
    drawVerticalLine: false,
    getDrawingHorizontalLine: (_) => FlLine(
      color: Theme.of(context).colorScheme.outline.withValues(alpha: 0.16),
      strokeWidth: 1,
    ),
  );
}

FlTitlesData _bottomLeftTitles(
  BuildContext context, {
  required String Function(double value) leftValue,
}) {
  final theme = Theme.of(context);
  final labelStyle = theme.textTheme.labelSmall?.copyWith(
    color: theme.colorScheme.onSurface.withValues(alpha: 0.42),
    fontSize: 10,
  );
  return FlTitlesData(
    topTitles: const AxisTitles(sideTitles: SideTitles(showTitles: false)),
    rightTitles: const AxisTitles(sideTitles: SideTitles(showTitles: false)),
    bottomTitles: AxisTitles(
      sideTitles: SideTitles(
        showTitles: true,
        reservedSize: 24,
        getTitlesWidget: (value, meta) {
          if (value != value.roundToDouble()) {
            return const SizedBox.shrink();
          }
          return Padding(
            padding: const EdgeInsets.only(top: 6),
            child: Text(value.toInt().toString(), style: labelStyle),
          );
        },
      ),
    ),
    leftTitles: AxisTitles(
      sideTitles: SideTitles(
        showTitles: true,
        reservedSize: 44,
        getTitlesWidget: (value, meta) => Text(leftValue(value), style: labelStyle),
      ),
    ),
  );
}

List<Color> _chartColors(BuildContext context) {
  final scheme = Theme.of(context).colorScheme;
  return [
    scheme.primary,
    scheme.tertiary,
    scheme.secondary,
    Colors.teal,
    Colors.amber.shade700,
    Colors.indigoAccent,
    Colors.pinkAccent,
    Colors.blueGrey,
  ];
}

List<TokenTimelinePoint> _sampleTimeline(List<TokenTimelinePoint> points, int maxPoints) {
  if (points.length <= maxPoints) {
    return points.toList(growable: false);
  }
  final sampled = <TokenTimelinePoint>[];
  final lastIndex = points.length - 1;
  for (var i = 0; i < maxPoints; i++) {
    final sourceIndex = (i * lastIndex / (maxPoints - 1)).round();
    final point = points[sourceIndex];
    if (sampled.isEmpty || sampled.last.index != point.index) {
      sampled.add(point);
    }
  }
  return sampled;
}

String _formatNumber(int value) {
  if (value >= 1000000) {
    return '${(value / 1000000).toStringAsFixed(1)}M';
  }
  if (value >= 1000) {
    return '${(value / 1000).toStringAsFixed(1)}K';
  }
  return value.toString();
}

String _compactTick(int value) {
  if (value <= 0) {
    return '0';
  }
  return _formatNumber(value);
}
