import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:http/http.dart' as http;

import '../../core/models/workbench_models.dart';
import '../requirements/requirement_set_form.dart';

class InspectorPanel extends StatelessWidget {
  const InspectorPanel({
    super.key,
    required this.selection,
    required this.availableModels,
    required this.threadGroups,
    required this.workerMetadata,
    required this.requirementReview,
    required this.bridgeBaseUri,
    required this.onOpenThread,
    required this.onSettingsChanged,
    required this.onRunningStateChanged,
    required this.onRenameThread,
    required this.onArchiveThread,
    required this.onWarmHandoff,
    required this.onCreateThreadGroup,
    required this.onRenameThreadGroup,
    required this.onDeleteThreadGroup,
    required this.onArchiveThreadGroup,
    required this.onMoveSelectedThreadToGroup,
    required this.onUpdateWorkerMetadata,
  });

  final WorkspaceSelection selection;
  final List<ModelItem> availableModels;
  final List<ThreadGroupItem> threadGroups;
  final WorkerMetadata? workerMetadata;
  final RequirementReviewSummary? requirementReview;
  final Uri? bridgeBaseUri;
  final ValueChanged<String> onOpenThread;
  final ValueChanged<ThreadSettingsDraft> onSettingsChanged;
  final ValueChanged<bool> onRunningStateChanged;
  final ValueChanged<String> onRenameThread;
  final VoidCallback onArchiveThread;
  final ValueChanged<String> onWarmHandoff;
  final ValueChanged<String> onCreateThreadGroup;
  final Future<void> Function(ThreadGroupItem group) onRenameThreadGroup;
  final ValueChanged<String> onDeleteThreadGroup;
  final ValueChanged<String> onArchiveThreadGroup;
  final ValueChanged<String?> onMoveSelectedThreadToGroup;
  final ValueChanged<WorkerMetadataDraft> onUpdateWorkerMetadata;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Expanded(
          child: ListView(
            padding: EdgeInsets.zero,
            children: [
              _ThreadSettingsCard(
                selection: selection,
                availableModels: availableModels,
                onSettingsChanged: onSettingsChanged,
                onRunningStateChanged: onRunningStateChanged,
                onRenameThread: onRenameThread,
                onArchiveThread: onArchiveThread,
                onWarmHandoff: onWarmHandoff,
              ),
              const SizedBox(height: 10),
              _ProjectGroupsCard(
                selection: selection,
                threadGroups: threadGroups,
                onCreateThreadGroup: onCreateThreadGroup,
                onRenameThreadGroup: onRenameThreadGroup,
                onDeleteThreadGroup: onDeleteThreadGroup,
                onArchiveThreadGroup: onArchiveThreadGroup,
                onMoveSelectedThreadToGroup: onMoveSelectedThreadToGroup,
              ),
              if (workerMetadata != null) ...[
                const SizedBox(height: 10),
                _WorkerMetadataCard(
                  workerMetadata: workerMetadata!,
                  onUpdateWorkerMetadata: onUpdateWorkerMetadata,
                ),
              ],
              if (selection.threadId != null) ...[
                const SizedBox(height: 10),
                _RequirementsReviewCard(
                  summary: requirementReview,
                  sourceThreadId: selection.threadId,
                  bridgeBaseUri: bridgeBaseUri,
                  onOpenThread: onOpenThread,
                ),
              ],
            ],
          ),
        ),
      ],
    );
  }
}

class _InspectorSection extends StatelessWidget {
  const _InspectorSection({
    required this.title,
    required this.child,
  });

  final String title;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      margin: const EdgeInsets.only(bottom: 4),
      decoration: BoxDecoration(
        color: theme.colorScheme.surface.withValues(alpha: 0.48),
        border: Border.all(color: theme.colorScheme.outline.withValues(alpha: 0.88)),
        borderRadius: BorderRadius.circular(6),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.12),
            blurRadius: 18,
            offset: const Offset(0, 10),
          ),
        ],
      ),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(12, 10, 12, 12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              title,
              style: theme.textTheme.bodyMedium?.copyWith(
                fontSize: 13,
                fontWeight: FontWeight.w700,
                letterSpacing: 0,
              ),
            ),
            const SizedBox(height: 8),
            child,
          ],
        ),
      ),
    );
  }
}

class _InspectorIconBox extends StatelessWidget {
  const _InspectorIconBox({
    required this.icon,
    this.foreground,
  });

  final IconData icon;
  final Color? foreground;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      width: 28,
      height: 28,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: theme.colorScheme.onSurface.withValues(alpha: 0.045),
        borderRadius: BorderRadius.circular(6),
      ),
      child: Icon(
        icon,
        size: 17,
        color: foreground ?? theme.colorScheme.onSurface.withValues(alpha: 0.68),
      ),
    );
  }
}

class _OverviewFact extends StatelessWidget {
  const _OverviewFact({
    required this.icon,
    required this.label,
    required this.value,
    this.statusColor,
  });

  final IconData icon;
  final String label;
  final String value;
  final Color? statusColor;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Row(
      children: [
        Icon(icon, size: 16, color: theme.colorScheme.onSurface.withValues(alpha: 0.72)),
        const SizedBox(width: 8),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                label,
                style: theme.textTheme.labelSmall?.copyWith(
                  color: theme.colorScheme.onSurface.withValues(alpha: 0.56),
                  fontSize: 10,
                  height: 1.1,
                ),
              ),
              const SizedBox(height: 2),
              Row(
                children: [
                  if (statusColor != null) ...[
                    Container(
                      width: 7,
                      height: 7,
                      decoration: BoxDecoration(
                        color: statusColor,
                        shape: BoxShape.circle,
                      ),
                    ),
                    const SizedBox(width: 7),
                  ],
                  Expanded(
                    child: Text(
                      value,
                      overflow: TextOverflow.ellipsis,
                      style: theme.textTheme.bodySmall?.copyWith(
                        fontWeight: FontWeight.w600,
                        height: 1.15,
                      ),
                    ),
                  ),
                ],
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _SettingsRow extends StatelessWidget {
  const _SettingsRow({
    required this.icon,
    required this.title,
    required this.subtitle,
    required this.control,
    this.iconColor,
  });

  final IconData icon;
  final String title;
  final String subtitle;
  final Widget control;
  final Color? iconColor;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      decoration: BoxDecoration(
        border: Border(
          bottom: BorderSide(color: theme.colorScheme.outline.withValues(alpha: 0.68)),
        ),
      ),
      padding: const EdgeInsets.symmetric(vertical: 5),
      child: Row(
        children: [
          Icon(
            icon,
            size: 17,
            color: iconColor ?? theme.colorScheme.onSurface.withValues(alpha: 0.68),
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  title,
                  style: theme.textTheme.bodySmall?.copyWith(fontWeight: FontWeight.w700),
                ),
                const SizedBox(height: 2),
                Text(
                  subtitle,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: theme.textTheme.labelSmall?.copyWith(
                    color: theme.colorScheme.onSurface.withValues(alpha: 0.56),
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(width: 12),
          SizedBox(width: 150, child: control),
        ],
      ),
    );
  }
}

class _InspectorActionButton extends StatelessWidget {
  const _InspectorActionButton({
    required this.label,
    required this.icon,
    required this.onPressed,
    this.primary = false,
    this.danger = false,
  });

  final String label;
  final IconData icon;
  final VoidCallback? onPressed;
  final bool primary;
  final bool danger;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final foreground = danger ? theme.colorScheme.error : theme.colorScheme.onSurface.withValues(alpha: 0.86);
    final child = Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(icon, size: 14),
        const SizedBox(width: 7),
        Flexible(child: Text(label, overflow: TextOverflow.ellipsis)),
      ],
    );
    if (primary) {
      return FilledButton(
        onPressed: onPressed,
        style: FilledButton.styleFrom(
          backgroundColor: const Color(0xFF2D7DFF),
          foregroundColor: Colors.white,
        ),
        child: child,
      );
    }
    return OutlinedButton(
      onPressed: onPressed,
      style: OutlinedButton.styleFrom(
        foregroundColor: foreground,
        side: BorderSide(
          color: danger
              ? foreground.withValues(alpha: 0.48)
              : theme.colorScheme.outline.withValues(alpha: 0.9),
        ),
      ),
      child: child,
    );
  }
}

class _RequirementsReviewCard extends StatelessWidget {
  const _RequirementsReviewCard({
    required this.summary,
    required this.sourceThreadId,
    required this.bridgeBaseUri,
    required this.onOpenThread,
  });

  final RequirementReviewSummary? summary;
  final String? sourceThreadId;
  final Uri? bridgeBaseUri;
  final ValueChanged<String> onOpenThread;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final summary = this.summary;
    final reviewerThreadId = summary?.reviewerThreadId;
    final hasActiveRequirements = (summary?.activeRequirementCount ?? 0) > 0;
    return _InspectorSection(
      title: 'Requirements',
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            if (summary == null)
              Row(
                children: [
                  _InspectorIconBox(
                    icon: Icons.verified_user_outlined,
                    foreground: theme.colorScheme.onSurface.withValues(alpha: 0.72),
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          'No active requirements',
                          style: theme.textTheme.bodySmall?.copyWith(fontWeight: FontWeight.w700),
                        ),
                        const SizedBox(height: 2),
                        Text(
                          'Attach a requirement set to gate future turns.',
                          style: theme.textTheme.labelSmall?.copyWith(
                            color: theme.colorScheme.onSurface.withValues(alpha: 0.58),
                          ),
                        ),
                      ],
                    ),
                  ),
                ],
              )
            else ...[
              _ReviewFact(label: 'Status', value: summary.displayStatus),
              if (summary.requirementSetId != null)
                _ReviewFact(label: 'Requirement set', value: summary.requirementSetId!),
              if (summary.updatedAt != null)
                _ReviewFact(label: 'Last updated', value: _formatTimestamp(summary.updatedAt!)),
              _ReviewFact(
                label: 'Verdicts',
                value:
                    '${summary.passedCount} passed · ${summary.failedCount} failed · ${summary.blockedCount} blocked',
              ),
            ],
            const SizedBox(height: 8),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                _InspectorActionButton(
                  label: hasActiveRequirements ? 'Replace Requirements' : 'Attach Requirements',
                  icon: Icons.rule_folder_outlined,
                  onPressed: sourceThreadId == null || bridgeBaseUri == null
                      ? null
                      : () => _setRequirements(context),
                ),
                if (reviewerThreadId != null && reviewerThreadId.isNotEmpty)
                  _InspectorActionButton(
                    label: 'Open Review Thread',
                    icon: Icons.rate_review_outlined,
                    onPressed: () => onOpenThread(reviewerThreadId),
                  ),
                if (hasActiveRequirements)
                  _InspectorActionButton(
                    label: 'Request Review',
                    icon: Icons.outgoing_mail,
                    onPressed: sourceThreadId == null || bridgeBaseUri == null
                        ? null
                        : () => _requestReview(context),
                  ),
              ],
            ),
            if (summary != null) ...[
              const SizedBox(height: 8),
              if (summary.verdicts.isEmpty)
                Text(
                  'No reviewer verdict packet yet.',
                  style: theme.textTheme.labelSmall,
                )
              else
                ...summary.verdicts.map((verdict) {
                  final color = switch (verdict.verdict) {
                    'pass' => Colors.green.shade700,
                    'fail' || 'rejectedBlocked' => theme.colorScheme.error,
                    'acceptedBlocked' => Colors.amber.shade800,
                    'waiverRequired' => Colors.deepOrange.shade700,
                    _ => theme.colorScheme.onSurface.withValues(alpha: 0.62),
                  };
                  final icon = switch (verdict.verdict) {
                    'pass' => Icons.check_rounded,
                    'fail' || 'rejectedBlocked' => Icons.close_rounded,
                    'acceptedBlocked' => Icons.warning_amber_rounded,
                    'waiverRequired' => Icons.policy_outlined,
                    _ => Icons.more_horiz,
                  };
                  final hasDetails = verdict.reason != null ||
                      verdict.evidenceAssessment != null ||
                      verdict.requiredCorrection != null;
                  return ExpansionTile(
                    tilePadding: EdgeInsets.zero,
                    childrenPadding: const EdgeInsets.only(left: 24, right: 4, bottom: 8),
                    leading: Icon(icon, size: 16, color: color),
                    title: Text(
                      verdict.key,
                      style: theme.textTheme.bodySmall?.copyWith(fontWeight: FontWeight.w700),
                    ),
                    subtitle: Text(
                      verdict.displayVerdict,
                      style: theme.textTheme.labelSmall?.copyWith(color: color),
                    ),
                    enabled: hasDetails,
                    children: [
                      if (verdict.reason != null)
                        _ReviewDetail(label: 'Reason', value: verdict.reason!),
                      if (verdict.evidenceAssessment != null)
                        _ReviewDetail(
                          label: 'Evidence',
                          value: verdict.evidenceAssessment!,
                        ),
                      if (verdict.requiredCorrection != null)
                        _ReviewDetail(
                          label: 'Required correction',
                          value: verdict.requiredCorrection!,
                        ),
                    ],
                  );
                }),
            ],
          ],
        ),
      ),
    );
  }

  Future<void> _setRequirements(BuildContext context) async {
    final sourceId = sourceThreadId;
    final baseUri = bridgeBaseUri;
    if (sourceId == null || baseUri == null) {
      return;
    }
    final initialJson = summary == null
        ? null
        : requirementSetJsonFromReviewSummary(summary!);
    final submitted = await showRequirementSetFormDialog(
      context,
      initialJson: initialJson,
      title: 'Set Requirements',
      actionLabel: 'Set',
      helperText: 'Define active requirements for this thread. Robdex generates and submits the JSON contract.',
      showDeactivate: (summary?.activeRequirementCount ?? 0) > 0,
      bridgeBaseUri: bridgeBaseUri,
    );
    if (submitted == null) {
      return;
    }
    Object? decoded;
    if (submitted.trim().isEmpty) {
      decoded = null;
    } else {
      try {
        decoded = jsonDecode(submitted);
      } catch (error) {
        if (!context.mounted) {
          return;
        }
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Invalid requirements JSON: $error')),
        );
        return;
      }
    }
    try {
      final response = await http.post(
        baseUri.resolve('/orchestrator/requirements/set'),
        headers: const {
          'Accept': 'application/json',
          'Content-Type': 'application/json',
        },
        body: jsonEncode({
          'senderThreadId': sourceId,
          'recipientThreadId': sourceId,
          'requirementSet': decoded,
        }),
      );
      if (!context.mounted) {
        return;
      }
      if (response.statusCode >= 200 && response.statusCode < 300) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(decoded == null ? 'Requirements cleared.' : 'Requirements set.')),
        );
      } else {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Set requirements failed: ${response.body}')),
        );
      }
    } catch (error) {
      if (!context.mounted) {
        return;
      }
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Set requirements failed: $error')),
      );
    }
  }

  Future<void> _requestReview(BuildContext context) async {
    final sourceId = sourceThreadId;
    final baseUri = bridgeBaseUri;
    if (sourceId == null || baseUri == null) {
      return;
    }
    try {
      final response = await http.post(
        baseUri.resolve('/orchestrator/requirements/request-review'),
        headers: const {
          'Accept': 'application/json',
          'Content-Type': 'application/json',
        },
        body: jsonEncode({
          'senderThreadId': sourceId,
          'recipientThreadId': sourceId,
        }),
      );
      if (!context.mounted) {
        return;
      }
      if (response.statusCode >= 200 && response.statusCode < 300) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Requirements review requested.')),
        );
      } else {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Review request failed: ${response.body}')),
        );
      }
    } catch (error) {
      if (!context.mounted) {
        return;
      }
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Review request failed: $error')),
      );
    }
  }
}

class _ReviewFact extends StatelessWidget {
  const _ReviewFact({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: 3),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 105,
            child: Text(label, style: theme.textTheme.labelSmall),
          ),
          Expanded(
            child: Text(
              value,
              style: theme.textTheme.labelSmall?.copyWith(fontWeight: FontWeight.w600),
            ),
          ),
        ],
      ),
    );
  }
}

class _ReviewDetail extends StatelessWidget {
  const _ReviewDetail({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(label, style: theme.textTheme.labelSmall?.copyWith(fontWeight: FontWeight.w700)),
          const SizedBox(height: 2),
          SelectableText(value, style: theme.textTheme.bodySmall),
        ],
      ),
    );
  }
}

String _formatTimestamp(int seconds) {
  final time = DateTime.fromMillisecondsSinceEpoch(seconds * 1000).toLocal();
  final hour = time.hour == 0 ? 12 : time.hour > 12 ? time.hour - 12 : time.hour;
  final minute = time.minute.toString().padLeft(2, '0');
  final suffix = time.hour >= 12 ? 'PM' : 'AM';
  return '$hour:$minute $suffix';
}

class ThreadSettingsDraft {
  const ThreadSettingsDraft({
    required this.role,
    required this.approvalPolicy,
    required this.sandboxMode,
    required this.networkAccessMode,
    required this.modelId,
    required this.reasoningEffort,
    required this.serviceTier,
  });

  final String role;
  final String approvalPolicy;
  final String sandboxMode;
  final String networkAccessMode;
  final String modelId;
  final String reasoningEffort;
  final String serviceTier;
}

class WorkerMetadataDraft {
  const WorkerMetadataDraft({
    required this.issueNumber,
    required this.pullRequestNumber,
    required this.blockedReason,
    required this.unblockWhen,
    required this.clearBlocked,
  });

  final String issueNumber;
  final String pullRequestNumber;
  final String blockedReason;
  final String unblockWhen;
  final bool clearBlocked;
}

class _ThreadSettingsCard extends StatefulWidget {
  const _ThreadSettingsCard({
    required this.selection,
    required this.availableModels,
    required this.onSettingsChanged,
    required this.onRunningStateChanged,
    required this.onRenameThread,
    required this.onArchiveThread,
    required this.onWarmHandoff,
  });

  final WorkspaceSelection selection;
  final List<ModelItem> availableModels;
  final ValueChanged<ThreadSettingsDraft> onSettingsChanged;
  final ValueChanged<bool> onRunningStateChanged;
  final ValueChanged<String> onRenameThread;
  final VoidCallback onArchiveThread;
  final ValueChanged<String> onWarmHandoff;

  @override
  State<_ThreadSettingsCard> createState() => _ThreadSettingsCardState();
}

class _ProjectGroupsCard extends StatelessWidget {
  const _ProjectGroupsCard({
    required this.selection,
    required this.threadGroups,
    required this.onCreateThreadGroup,
    required this.onRenameThreadGroup,
    required this.onDeleteThreadGroup,
    required this.onArchiveThreadGroup,
    required this.onMoveSelectedThreadToGroup,
  });

  final WorkspaceSelection selection;
  final List<ThreadGroupItem> threadGroups;
  final ValueChanged<String> onCreateThreadGroup;
  final Future<void> Function(ThreadGroupItem group) onRenameThreadGroup;
  final ValueChanged<String> onDeleteThreadGroup;
  final ValueChanged<String> onArchiveThreadGroup;
  final ValueChanged<String?> onMoveSelectedThreadToGroup;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final enabled = selection.projectId != null;
    final canAssign = enabled && selection.threadId != null;
    return _InspectorSection(
      title: 'Project',
      child: Container(
        width: double.infinity,
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
        decoration: BoxDecoration(
          color: theme.colorScheme.onSurface.withValues(alpha: 0.025),
          border: Border.all(color: theme.colorScheme.outline.withValues(alpha: 0.82)),
          borderRadius: BorderRadius.circular(6),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Icon(
                  Icons.groups_outlined,
                  size: 16,
                  color: theme.colorScheme.onSurface.withValues(alpha: 0.72),
                ),
                const SizedBox(width: 8),
                Flexible(
                  child: Text(
                    selection.projectOrchestratorName == null
                        ? 'No orchestrator assigned'
                        : 'Orchestrator: ${selection.projectOrchestratorName}',
                    textAlign: TextAlign.center,
                    overflow: TextOverflow.ellipsis,
                    style: theme.textTheme.bodySmall?.copyWith(fontWeight: FontWeight.w700),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 2),
            Text(
              'Assign an orchestrator or create a group for this project.',
              textAlign: TextAlign.center,
              style: theme.textTheme.labelSmall?.copyWith(
                color: theme.colorScheme.onSurface.withValues(alpha: 0.58),
              ),
            ),
            const SizedBox(height: 8),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              alignment: WrapAlignment.center,
              children: [
                _InspectorActionButton(
                  label: 'Create Group',
                  icon: Icons.create_new_folder_outlined,
                  onPressed: enabled
                      ? () async {
                          final title = await _promptInline(context, 'New Thread Group', '');
                          if (title != null && title.trim().isNotEmpty) {
                            onCreateThreadGroup(title);
                          }
                        }
                      : null,
                ),
                _InspectorActionButton(
                  label: 'Ungroup Selected',
                  icon: Icons.link_off_outlined,
                  onPressed: canAssign ? () => onMoveSelectedThreadToGroup(null) : null,
                ),
              ],
            ),
            if (threadGroups.isNotEmpty) ...[
              const SizedBox(height: 12),
              ...threadGroups.map(
                (group) => Padding(
                  padding: const EdgeInsets.only(bottom: 6),
                  child: _ThreadGroupTile(
                    group: group,
                    canAssignSelected: canAssign,
                    onAssignSelected: () => onMoveSelectedThreadToGroup(group.id),
                    onRename: () => onRenameThreadGroup(group),
                    onDelete: () => onDeleteThreadGroup(group.id),
                    onArchive: () => onArchiveThreadGroup(group.id),
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

class _ThreadGroupTile extends StatelessWidget {
  const _ThreadGroupTile({
    required this.group,
    required this.canAssignSelected,
    required this.onAssignSelected,
    required this.onRename,
    required this.onDelete,
    required this.onArchive,
  });

  final ThreadGroupItem group;
  final bool canAssignSelected;
  final VoidCallback onAssignSelected;
  final VoidCallback onRename;
  final VoidCallback onDelete;
  final VoidCallback onArchive;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return DecoratedBox(
      decoration: BoxDecoration(
        border: Border(
          left: BorderSide(color: theme.colorScheme.outline, width: 2),
        ),
      ),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(8, 4, 0, 4),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Expanded(
                  child: Text(
                    group.title,
                    style: theme.textTheme.bodySmall?.copyWith(fontWeight: FontWeight.w700),
                  ),
                ),
                Text(
                  '${group.threadIds.length} threads',
                  style: theme.textTheme.labelSmall,
                ),
              ],
            ),
            const SizedBox(height: 4),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                OutlinedButton(
                  onPressed: canAssignSelected ? onAssignSelected : null,
                  child: const Text('Assign Selected'),
                ),
                TextButton(
                  onPressed: onRename,
                  child: const Text('Rename'),
                ),
                TextButton(
                  onPressed: onDelete,
                  child: const Text('Delete'),
                ),
                TextButton(
                  onPressed: onArchive,
                  child: const Text('Archive'),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

class _WorkerMetadataCard extends StatefulWidget {
  const _WorkerMetadataCard({
    required this.workerMetadata,
    required this.onUpdateWorkerMetadata,
  });

  final WorkerMetadata workerMetadata;
  final ValueChanged<WorkerMetadataDraft> onUpdateWorkerMetadata;

  @override
  State<_WorkerMetadataCard> createState() => _WorkerMetadataCardState();
}

class _WorkerMetadataCardState extends State<_WorkerMetadataCard> {
  late TextEditingController _issueController;
  late TextEditingController _pullRequestController;
  late TextEditingController _blockedReasonController;
  late TextEditingController _unblockWhenController;

  @override
  void initState() {
    super.initState();
    _syncControllers();
  }

  @override
  void didUpdateWidget(covariant _WorkerMetadataCard oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.workerMetadata.threadId != widget.workerMetadata.threadId ||
        oldWidget.workerMetadata.issueNumber != widget.workerMetadata.issueNumber ||
        oldWidget.workerMetadata.pullRequestNumber != widget.workerMetadata.pullRequestNumber ||
        oldWidget.workerMetadata.blockedReason != widget.workerMetadata.blockedReason ||
        oldWidget.workerMetadata.unblockWhen != widget.workerMetadata.unblockWhen) {
      _issueController.dispose();
      _pullRequestController.dispose();
      _blockedReasonController.dispose();
      _unblockWhenController.dispose();
      _syncControllers();
    }
  }

  void _syncControllers() {
    _issueController = TextEditingController(
      text: widget.workerMetadata.issueNumber?.toString() ?? '',
    );
    _pullRequestController = TextEditingController(
      text: widget.workerMetadata.pullRequestNumber?.toString() ?? '',
    );
    _blockedReasonController = TextEditingController(
      text: widget.workerMetadata.blockedReason ?? '',
    );
    _unblockWhenController = TextEditingController(
      text: widget.workerMetadata.unblockWhen ?? '',
    );
  }

  @override
  void dispose() {
    _issueController.dispose();
    _pullRequestController.dispose();
    _blockedReasonController.dispose();
    _unblockWhenController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return _InspectorSection(
      title: 'Worker metadata',
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Expanded(
                child: TextField(
                  controller: _issueController,
                  decoration: const InputDecoration(
                    labelText: 'Issue',
                    hintText: 'e.g. #1234',
                  ),
                  keyboardType: TextInputType.number,
                ),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: TextField(
                  controller: _pullRequestController,
                  decoration: const InputDecoration(
                    labelText: 'PR',
                    hintText: 'e.g. #5678',
                  ),
                  keyboardType: TextInputType.number,
                ),
              ),
            ],
          ),
          const SizedBox(height: 8),
          TextField(
            controller: _blockedReasonController,
            minLines: 1,
            maxLines: 3,
            decoration: const InputDecoration(
              labelText: 'Blocked reason',
              hintText: 'Add a reason...',
              alignLabelWithHint: true,
            ),
          ),
          const SizedBox(height: 8),
          TextField(
            controller: _unblockWhenController,
            minLines: 1,
            maxLines: 3,
            decoration: const InputDecoration(
              labelText: 'Unblock when',
              hintText: 'Add condition...',
              alignLabelWithHint: true,
            ),
          ),
          const SizedBox(height: 8),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              _InspectorActionButton(
                label: 'Save Metadata',
                icon: Icons.save_outlined,
                primary: true,
                onPressed: () {
                  widget.onUpdateWorkerMetadata(
                    WorkerMetadataDraft(
                      issueNumber: _issueController.text,
                      pullRequestNumber: _pullRequestController.text,
                      blockedReason: _blockedReasonController.text,
                      unblockWhen: _unblockWhenController.text,
                      clearBlocked: false,
                    ),
                  );
                },
              ),
              _InspectorActionButton(
                label: 'Clear Blocked',
                icon: Icons.block_outlined,
                onPressed: () {
                  _blockedReasonController.clear();
                  _unblockWhenController.clear();
                  widget.onUpdateWorkerMetadata(
                    WorkerMetadataDraft(
                      issueNumber: _issueController.text,
                      pullRequestNumber: _pullRequestController.text,
                      blockedReason: '',
                      unblockWhen: '',
                      clearBlocked: true,
                    ),
                  );
                },
              ),
            ],
          ),
        ],
      ),
    );
  }
}

Future<String?> _promptInline(BuildContext context, String title, String initialValue) async {
  final controller = TextEditingController(text: initialValue);
  final result = await showDialog<String>(
    context: context,
    builder: (context) => AlertDialog(
      title: Text(title),
      content: TextField(
        controller: controller,
        decoration: const InputDecoration(labelText: 'Title'),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Cancel'),
        ),
        FilledButton(
          onPressed: () => Navigator.of(context).pop(controller.text),
          child: const Text('Save'),
        ),
      ],
    ),
  );
  controller.dispose();
  return result;
}

Future<String?> _promptWarmHandoff(BuildContext context) async {
  final controller = TextEditingController();
  final result = await showDialog<String>(
    context: context,
    builder: (context) => AlertDialog(
      title: const Text('Warm Handoff'),
      content: SizedBox(
        width: 440,
        child: TextField(
          controller: controller,
          minLines: 4,
          maxLines: 10,
          decoration: const InputDecoration(
            labelText: 'New Initial Prompt',
            alignLabelWithHint: true,
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Cancel'),
        ),
        FilledButton(
          onPressed: () => Navigator.of(context).pop(controller.text),
          child: const Text('Replace Agent'),
        ),
      ],
    ),
  );
  controller.dispose();
  return result;
}

class _ThreadSettingsCardState extends State<_ThreadSettingsCard> {
  late TextEditingController _nameController;
  late String _role;
  late String _approvalPolicy;
  late String _sandboxMode;
  late String _networkAccessMode;
  late String _modelId;
  late String _reasoningEffort;
  late String _serviceTier;

  @override
  void initState() {
    super.initState();
    _syncFromSelection();
  }

  @override
  void didUpdateWidget(covariant _ThreadSettingsCard oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.selection.threadId != widget.selection.threadId ||
        oldWidget.selection.threadRole != widget.selection.threadRole ||
        oldWidget.selection.approvalPolicy != widget.selection.approvalPolicy ||
        oldWidget.selection.sandboxMode != widget.selection.sandboxMode ||
        oldWidget.selection.networkAccess != widget.selection.networkAccess ||
        oldWidget.selection.model != widget.selection.model ||
        oldWidget.selection.reasoningEffort != widget.selection.reasoningEffort ||
        oldWidget.selection.serviceTier != widget.selection.serviceTier ||
        oldWidget.selection.threadName != widget.selection.threadName) {
      _syncFromSelection();
    }
  }

  void _syncFromSelection() {
    if (_isNameControllerInitialized) {
      _nameController.dispose();
    }
    _nameController = TextEditingController(text: widget.selection.threadName);
    _role = widget.selection.threadRole ?? 'worker';
    _approvalPolicy = widget.selection.approvalPolicy ?? '';
    _sandboxMode = widget.selection.sandboxMode ?? '';
    _networkAccessMode = switch (widget.selection.networkAccess) {
      true => 'enabled',
      false => 'disabled',
      null => 'default',
    };
    _modelId = widget.selection.model ?? '';
    _reasoningEffort = widget.selection.reasoningEffort ?? '';
    _serviceTier = widget.selection.serviceTier ?? '';
  }

  bool get _isNameControllerInitialized {
    try {
      _nameController;
      return true;
    } catch (_) {
      return false;
    }
  }

  @override
  void dispose() {
    _nameController.dispose();
    super.dispose();
  }

  void _emitSettings() {
    widget.onSettingsChanged(
      ThreadSettingsDraft(
        role: _role,
        approvalPolicy: _approvalPolicy,
        sandboxMode: _sandboxMode,
        networkAccessMode: _networkAccessMode,
        modelId: _modelId,
        reasoningEffort: _reasoningEffort,
        serviceTier: _serviceTier,
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final enabled = widget.selection.threadId != null;
    String titleCaseWords(String value) => value
        .split(RegExp(r'[\s_-]+'))
        .where((part) => part.isNotEmpty)
        .map((part) => part[0].toUpperCase() + part.substring(1))
        .join(' ');
    String inheritedLabel(String value) => '(${titleCaseWords(value)})';
    String inheritedOrSystem(String? value, {String system = 'System'}) =>
        inheritedLabel((value?.trim().isNotEmpty ?? false) ? value! : system);
    String serviceTierLabel(String? value) => inheritedOrSystem(value);
    String modelLabel(String? modelId) {
      if (modelId == null || modelId.trim().isEmpty) {
        return inheritedOrSystem(null);
      }
      ModelItem? match;
      for (final model in widget.availableModels) {
        if (model.id == modelId) {
          match = model;
          break;
        }
      }
      final display = match?.name?.trim().isNotEmpty == true ? match!.name! : modelId;
      return inheritedLabel(display);
    }
    String networkLabel(bool? enabled) => inheritedLabel(
          switch (enabled) {
            true => 'Enabled',
            false => 'Disabled',
            null => 'System',
          },
        );
    final modelItems = [
      DropdownMenuItem<String>(
        value: '',
        child: Text(modelLabel(widget.selection.effectiveModel)),
      ),
      ...widget.availableModels
          .where((model) => !model.hidden || model.id == _modelId)
          .map(
            (model) => DropdownMenuItem<String>(
              value: model.id,
              child: Text(model.name?.trim().isNotEmpty == true ? model.name! : model.id),
            ),
          ),
    ];
    final controlTextStyle = theme.textTheme.bodySmall?.copyWith(
      fontSize: 11,
      fontWeight: FontWeight.w500,
    );
    return _InspectorSection(
      title: 'Thread overview',
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Expanded(
                flex: 4,
                child: _OverviewFact(
                  icon: Icons.group_outlined,
                  label: 'Agent name',
                  value: widget.selection.threadName,
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                flex: 3,
                child: _OverviewFact(
                  icon: Icons.badge_outlined,
                  label: 'Role',
                  value: titleCaseWords(widget.selection.threadRole ?? 'worker'),
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                flex: 3,
                child: _OverviewFact(
                  icon: Icons.folder_outlined,
                  label: 'Project',
                  value: widget.selection.projectName,
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                flex: 2,
                child: _OverviewFact(
                  icon: Icons.circle,
                  label: 'Status',
                  value: widget.selection.isRunning ? 'Running' : 'Idle',
                  statusColor: widget.selection.isRunning ? theme.colorScheme.primary : Colors.greenAccent.shade400,
                ),
              ),
            ],
          ),
          const SizedBox(height: 10),
          _SettingsRow(
            icon: Icons.memory_outlined,
            title: 'Model',
            subtitle: 'Model used by this agent',
            control: DropdownButtonFormField<String>(
              key: ValueKey('model-${widget.selection.threadId}-$_modelId'),
              initialValue: modelItems.any((item) => item.value == _modelId) ? _modelId : '',
              isExpanded: true,
              style: controlTextStyle,
              decoration: const InputDecoration(),
              items: modelItems,
              onChanged: enabled
                  ? (value) {
                      if (value == null) return;
                      setState(() => _modelId = value);
                    }
                  : null,
            ),
          ),
          _SettingsRow(
            icon: Icons.psychology_alt_outlined,
            title: 'Reasoning',
            subtitle: 'Default reasoning effort',
            control: DropdownButtonFormField<String>(
              key: ValueKey('reasoning-${widget.selection.threadId}-$_reasoningEffort'),
              initialValue: _reasoningEffort,
              isExpanded: true,
              style: controlTextStyle,
              decoration: const InputDecoration(),
              items: [
                DropdownMenuItem(
                  value: '',
                  child: Text(inheritedOrSystem(widget.selection.effectiveReasoningEffort)),
                ),
                const DropdownMenuItem(value: 'low', child: Text('Low')),
                const DropdownMenuItem(value: 'medium', child: Text('Medium')),
                const DropdownMenuItem(value: 'high', child: Text('High')),
              ],
              onChanged: enabled
                  ? (value) {
                      if (value == null) return;
                      setState(() => _reasoningEffort = value);
                    }
                  : null,
            ),
          ),
          _SettingsRow(
            icon: Icons.key_outlined,
            title: 'Approval',
            subtitle: 'Tool / action approval policy',
            control: DropdownButtonFormField<String>(
              key: ValueKey('approval-${widget.selection.threadId}-$_approvalPolicy'),
              initialValue: _approvalPolicy,
              isExpanded: true,
              style: controlTextStyle,
              decoration: const InputDecoration(),
              items: [
                DropdownMenuItem(
                  value: '',
                  child: Text(inheritedOrSystem(widget.selection.effectiveApprovalPolicy)),
                ),
                const DropdownMenuItem(value: 'untrusted', child: Text('Untrusted', maxLines: 1)),
                const DropdownMenuItem(value: 'on-failure', child: Text('On failure', maxLines: 1)),
                const DropdownMenuItem(value: 'on-request', child: Text('On request', maxLines: 1)),
                const DropdownMenuItem(value: 'never', child: Text('Never', maxLines: 1)),
              ],
              onChanged: enabled
                  ? (value) {
                      if (value == null) return;
                      setState(() => _approvalPolicy = value);
                    }
                  : null,
            ),
          ),
          _SettingsRow(
            icon: Icons.shield_outlined,
            title: 'Sandbox',
            subtitle: 'Filesystem & network access',
            iconColor: theme.colorScheme.error,
            control: DropdownButtonFormField<String>(
              key: ValueKey('sandbox-${widget.selection.threadId}-$_sandboxMode'),
              initialValue: _sandboxMode,
              isExpanded: true,
              style: controlTextStyle,
              decoration: const InputDecoration(),
              items: [
                DropdownMenuItem(
                  value: '',
                  child: Text(inheritedOrSystem(widget.selection.effectiveSandboxMode)),
                ),
                const DropdownMenuItem(value: 'workspace-write', child: Text('Workspace', maxLines: 1)),
                const DropdownMenuItem(
                  value: 'danger-full-access',
                  child: Text(
                    'Danger full access',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ],
              onChanged: enabled
                  ? (value) {
                      if (value == null) return;
                      setState(() => _sandboxMode = value);
                    }
                  : null,
            ),
          ),
          _SettingsRow(
            icon: Icons.public_outlined,
            title: 'Network',
            subtitle: 'Allow outbound network',
            control: DropdownButtonFormField<String>(
              key: ValueKey('network-${widget.selection.threadId}-$_networkAccessMode'),
              initialValue: _networkAccessMode,
              isExpanded: true,
              style: controlTextStyle,
              decoration: const InputDecoration(),
              items: [
                DropdownMenuItem(
                  value: 'default',
                  child: Text(networkLabel(widget.selection.effectiveNetworkAccess)),
                ),
                const DropdownMenuItem(value: 'enabled', child: Text('Enabled')),
                const DropdownMenuItem(value: 'disabled', child: Text('Disabled')),
              ],
              onChanged: enabled
                  ? (value) {
                      if (value == null) return;
                      setState(() => _networkAccessMode = value);
                    }
                  : null,
            ),
          ),
          _SettingsRow(
            icon: Icons.layers_outlined,
            title: 'Service tier',
            subtitle: 'Service tier for this thread',
            control: DropdownButtonFormField<String>(
              key: ValueKey('service-tier-${widget.selection.threadId}-$_serviceTier'),
              initialValue: _serviceTier,
              isExpanded: true,
              style: controlTextStyle,
              decoration: const InputDecoration(),
              items: [
                DropdownMenuItem(
                  value: '',
                  child: Text(serviceTierLabel(widget.selection.effectiveServiceTier)),
                ),
                const DropdownMenuItem(value: 'fast', child: Text('Fast')),
                const DropdownMenuItem(value: 'flex', child: Text('Flex')),
              ],
              onChanged: enabled
                  ? (value) {
                      if (value == null) return;
                      setState(() => _serviceTier = value);
                    }
                  : null,
            ),
          ),
          const SizedBox(height: 8),
          Text(
            'Thread actions',
            style: theme.textTheme.bodySmall?.copyWith(fontWeight: FontWeight.w700),
          ),
          const SizedBox(height: 8),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              _InspectorActionButton(
                label: 'Save Changes',
                icon: Icons.check_rounded,
                primary: true,
                onPressed: enabled
                    ? () {
                        final trimmed = _nameController.text.trim();
                        if (trimmed.isNotEmpty && trimmed != widget.selection.threadName) {
                          widget.onRenameThread(trimmed);
                        }
                        _emitSettings();
                      }
                    : null,
              ),
              _InspectorActionButton(
                label: widget.selection.isRunning ? 'Mark Idle' : 'Mark Running',
                icon: widget.selection.isRunning ? Icons.pause_rounded : Icons.play_arrow_rounded,
                onPressed: enabled
                    ? () => widget.onRunningStateChanged(!widget.selection.isRunning)
                    : null,
              ),
              _InspectorActionButton(
                label: 'Warm Handoff',
                icon: Icons.whatshot_outlined,
                onPressed: enabled
                    ? () async {
                        final prompt = await _promptWarmHandoff(context);
                        if (prompt != null && prompt.trim().isNotEmpty) {
                          widget.onWarmHandoff(prompt.trim());
                        }
                      }
                    : null,
              ),
              _InspectorActionButton(
                label: 'Archive',
                icon: Icons.delete_outline,
                danger: true,
                onPressed: enabled ? widget.onArchiveThread : null,
              ),
            ],
          ),
        ],
      ),
    );
  }
}
