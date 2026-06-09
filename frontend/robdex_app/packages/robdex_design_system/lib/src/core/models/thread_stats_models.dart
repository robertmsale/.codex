class ThreadStatsData {
  const ThreadStatsData({
    required this.threadId,
    required this.sessionPath,
    required this.generatedAtMs,
    required this.totals,
    required this.estimates,
    required this.compactionCount,
    required this.timeline,
    required this.categories,
    required this.topItems,
    required this.warnings,
  });

  final String threadId;
  final String sessionPath;
  final int generatedAtMs;
  final TokenTotals totals;
  final TokenEstimates estimates;
  final int compactionCount;
  final List<TokenTimelinePoint> timeline;
  final List<TokenCategoryBreakdown> categories;
  final List<TokenTopItem> topItems;
  final List<String> warnings;

  factory ThreadStatsData.fromJson(Map<String, dynamic> json) {
    return ThreadStatsData(
      threadId: json['threadId'] as String? ?? '',
      sessionPath: json['sessionPath'] as String? ?? '',
      generatedAtMs: json['generatedAtMs'] as int? ?? 0,
      totals: TokenTotals.fromJson(json['totals'] as Map<String, dynamic>? ?? const {}),
      estimates: TokenEstimates.fromJson(json['estimates'] as Map<String, dynamic>? ?? const {}),
      compactionCount: json['compactionCount'] as int? ?? 0,
      timeline: (json['timeline'] as List<dynamic>? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(TokenTimelinePoint.fromJson)
          .toList(growable: false),
      categories: (json['categories'] as List<dynamic>? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(TokenCategoryBreakdown.fromJson)
          .toList(growable: false),
      topItems: (json['topItems'] as List<dynamic>? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(TokenTopItem.fromJson)
          .toList(growable: false),
      warnings: (json['warnings'] as List<dynamic>? ?? const [])
          .whereType<String>()
          .toList(growable: false),
    );
  }
}

class PeriodStatsData {
  const PeriodStatsData({
    required this.label,
    required this.startMs,
    required this.endMs,
    required this.generatedAtMs,
    required this.sessionCount,
    required this.totals,
    required this.estimates,
    required this.compactionCount,
    required this.categories,
    required this.topItems,
    required this.warnings,
    this.quota,
  });

  final String label;
  final int startMs;
  final int endMs;
  final int generatedAtMs;
  final int sessionCount;
  final TokenTotals totals;
  final TokenEstimates estimates;
  final int compactionCount;
  final List<TokenCategoryBreakdown> categories;
  final List<TokenTopItem> topItems;
  final List<String> warnings;
  final WeeklyQuotaData? quota;

  factory PeriodStatsData.fromJson(Map<String, dynamic> json) {
    return PeriodStatsData(
      label: json['label'] as String? ?? 'Period stats',
      startMs: json['startMs'] as int? ?? 0,
      endMs: json['endMs'] as int? ?? 0,
      generatedAtMs: json['generatedAtMs'] as int? ?? 0,
      sessionCount: json['sessionCount'] as int? ?? 0,
      totals: TokenTotals.fromJson(json['totals'] as Map<String, dynamic>? ?? const {}),
      estimates: TokenEstimates.fromJson(json['estimates'] as Map<String, dynamic>? ?? const {}),
      compactionCount: json['compactionCount'] as int? ?? 0,
      categories: (json['categories'] as List<dynamic>? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(TokenCategoryBreakdown.fromJson)
          .toList(growable: false),
      topItems: (json['topItems'] as List<dynamic>? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(TokenTopItem.fromJson)
          .toList(growable: false),
      warnings: (json['warnings'] as List<dynamic>? ?? const [])
          .whereType<String>()
          .toList(growable: false),
      quota: json['quota'] is Map<String, dynamic>
          ? WeeklyQuotaData.fromJson(json['quota'] as Map<String, dynamic>)
          : null,
    );
  }
}

class WeeklyQuotaData {
  const WeeklyQuotaData({
    required this.resetAtMs,
    required this.remainingPercent,
    required this.usedPercent,
    required this.inferredStartMs,
  });

  final int resetAtMs;
  final double remainingPercent;
  final double usedPercent;
  final int inferredStartMs;

  factory WeeklyQuotaData.fromJson(Map<String, dynamic> json) {
    return WeeklyQuotaData(
      resetAtMs: json['resetAtMs'] as int? ?? 0,
      remainingPercent: (json['remainingPercent'] as num? ?? 0).toDouble(),
      usedPercent: (json['usedPercent'] as num? ?? 0).toDouble(),
      inferredStartMs: json['inferredStartMs'] as int? ?? 0,
    );
  }
}

class TokenTotals {
  const TokenTotals({
    required this.inputTokens,
    required this.uncachedInputTokens,
    required this.outputTokens,
    required this.cachedInputTokens,
    required this.reasoningOutputTokens,
    required this.totalTokens,
  });

  final int inputTokens;
  final int uncachedInputTokens;
  final int outputTokens;
  final int cachedInputTokens;
  final int reasoningOutputTokens;
  final int totalTokens;

  factory TokenTotals.fromJson(Map<String, dynamic> json) {
    return TokenTotals(
      inputTokens: json['inputTokens'] as int? ?? 0,
      uncachedInputTokens: json['uncachedInputTokens'] as int? ??
          ((json['inputTokens'] as int? ?? 0) - (json['cachedInputTokens'] as int? ?? 0)).clamp(0, 1 << 62).toInt(),
      outputTokens: json['outputTokens'] as int? ?? 0,
      cachedInputTokens: json['cachedInputTokens'] as int? ?? 0,
      reasoningOutputTokens: json['reasoningOutputTokens'] as int? ?? 0,
      totalTokens: json['totalTokens'] as int? ?? 0,
    );
  }
}

class TokenEstimates {
  const TokenEstimates({
    required this.userMessageInputTokens,
    required this.toolOutputInputTokens,
    required this.toolCallOutputTokens,
    required this.skillInstructionInputTokens,
  });

  final int userMessageInputTokens;
  final int toolOutputInputTokens;
  final int toolCallOutputTokens;
  final int skillInstructionInputTokens;

  factory TokenEstimates.fromJson(Map<String, dynamic> json) {
    return TokenEstimates(
      userMessageInputTokens: json['userMessageInputTokens'] as int? ?? 0,
      toolOutputInputTokens: json['toolOutputInputTokens'] as int? ?? 0,
      toolCallOutputTokens: json['toolCallOutputTokens'] as int? ?? 0,
      skillInstructionInputTokens: json['skillInstructionInputTokens'] as int? ?? 0,
    );
  }
}

class TokenTimelinePoint {
  const TokenTimelinePoint({
    required this.index,
    required this.line,
    required this.inputTokens,
    required this.uncachedInputTokens,
    required this.outputTokens,
    required this.cachedInputTokens,
    required this.reasoningOutputTokens,
    required this.totalTokens,
    required this.deltaTokens,
  });

  final int index;
  final int line;
  final int inputTokens;
  final int uncachedInputTokens;
  final int outputTokens;
  final int cachedInputTokens;
  final int reasoningOutputTokens;
  final int totalTokens;
  final int deltaTokens;

  factory TokenTimelinePoint.fromJson(Map<String, dynamic> json) {
    return TokenTimelinePoint(
      index: json['index'] as int? ?? 0,
      line: json['line'] as int? ?? 0,
      inputTokens: json['inputTokens'] as int? ?? 0,
      uncachedInputTokens: json['uncachedInputTokens'] as int? ??
          ((json['inputTokens'] as int? ?? 0) - (json['cachedInputTokens'] as int? ?? 0)).clamp(0, 1 << 62).toInt(),
      outputTokens: json['outputTokens'] as int? ?? 0,
      cachedInputTokens: json['cachedInputTokens'] as int? ?? 0,
      reasoningOutputTokens: json['reasoningOutputTokens'] as int? ?? 0,
      totalTokens: json['totalTokens'] as int? ?? 0,
      deltaTokens: json['deltaTokens'] as int? ?? 0,
    );
  }
}

class TokenCategoryBreakdown {
  const TokenCategoryBreakdown({
    required this.key,
    required this.label,
    required this.tokens,
    required this.estimated,
  });

  final String key;
  final String label;
  final int tokens;
  final bool estimated;

  factory TokenCategoryBreakdown.fromJson(Map<String, dynamic> json) {
    return TokenCategoryBreakdown(
      key: json['key'] as String? ?? '',
      label: json['label'] as String? ?? '',
      tokens: json['tokens'] as int? ?? 0,
      estimated: json['estimated'] as bool? ?? false,
    );
  }
}

class TokenTopItem {
  const TokenTopItem({
    required this.label,
    required this.kind,
    required this.line,
    required this.tokens,
    required this.estimated,
  });

  final String label;
  final String kind;
  final int line;
  final int tokens;
  final bool estimated;

  factory TokenTopItem.fromJson(Map<String, dynamic> json) {
    return TokenTopItem(
      label: json['label'] as String? ?? '',
      kind: json['kind'] as String? ?? '',
      line: json['line'] as int? ?? 0,
      tokens: json['tokens'] as int? ?? 0,
      estimated: json['estimated'] as bool? ?? false,
    );
  }
}
