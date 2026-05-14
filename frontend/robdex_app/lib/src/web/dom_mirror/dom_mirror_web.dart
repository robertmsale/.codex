import 'dart:html';
// ignore_for_file: deprecated_member_use, avoid_web_libraries_in_flutter

import 'package:robdex_design_system/robdex_design_system.dart';
import 'dom_mirror_snapshot.dart';

const String _mirrorElementId = 'robdex-dom-mirror';
const String _mirrorQueryFlag = 'robdexDomMirror';
const bool _defineFlag = bool.fromEnvironment('ROBDEX_DOM_MIRROR', defaultValue: false);
const bool _disableDefineFlag = bool.fromEnvironment(
  'ROBDEX_DOM_MIRROR_DISABLED',
  defaultValue: false,
);

class DomMirrorController {
  void update(WorkbenchViewData? view) {
    if (!_isEnabled) {
      clear();
      return;
    }
    if (view == null) {
      clear();
      return;
    }
    final snapshot = DomMirrorSnapshot.fromWorkbench(view);
    _render(snapshot);
  }

  void clear() {
    final root = document.getElementById(_mirrorElementId);
    if (root == null) {
      return;
    }
    root.remove();
  }

  bool get isEnabled => _isEnabled;

  void dispose() {
    clear();
  }

  bool get _isEnabled {
    if (_disableDefineFlag) {
      return false;
    }
    if (_defineFlag) {
      return true;
    }
    final queryValue = Uri.base.queryParameters[_mirrorQueryFlag];
    if (queryValue == '0' || queryValue?.toLowerCase() == 'false') {
      return false;
    }
    if (queryValue == '1' || queryValue?.toLowerCase() == 'true') {
      return true;
    }
    try {
      final localValue = window.localStorage[_mirrorQueryFlag];
      if (localValue == '0' || localValue?.toLowerCase() == 'false') {
        return false;
      }
      if (localValue == '1' || localValue?.toLowerCase() == 'true') {
        return true;
      }
    } catch (_) {
      return true;
    }
    return true;
  }

  void _render(DomMirrorSnapshot snapshot) {
    final root = _ensureRoot();
    final content = _buildSection(snapshot);
    root.children.clear();
    root.append(content);
  }

  Element _ensureRoot() {
    if (document.getElementById(_mirrorElementId) case final Element existing) {
      _applyMirrorStyles(existing);
      return existing;
    }
    final section = Element.tag('section')
      ..id = _mirrorElementId
      ..setAttribute('aria-label', 'Robdex Workbench DOM Mirror');
    _applyMirrorStyles(section);
    document.body?.append(section);
    return section;
  }

  void _applyMirrorStyles(Element root) {
    root.style.setProperty('position', 'fixed');
    root.style.setProperty('inset', '0');
    root.style.setProperty('opacity', '0.001');
    root.style.setProperty('pointer-events', 'none');
    root.style.setProperty('overflow', 'hidden');
    root.style.setProperty('z-index', '-1');
    root.style.setProperty('top', '0');
    root.style.setProperty('left', '0');
    root.style.setProperty('right', '0');
    root.style.setProperty('bottom', '0');
  }

  Element _buildSection(DomMirrorSnapshot snapshot) {
    final root = Element.tag('section');
    root.append(_title());
    root.append(_mirrorStateSection(snapshot));
    root.append(_identitySection(snapshot));
    root.append(_statusSection(snapshot));
    root.append(_projectsSection(snapshot.projects));
    root.append(_chatSection(snapshot.chatEntries));
    root.append(_approvalsSection(snapshot.pendingApprovals));
    if (snapshot.requirementsReview != null) {
      root.append(_requirementsSection(snapshot.requirementsReview!));
    }
    root.append(_liveProcessSection(snapshot.liveProcesses));
    root.append(_composerSection(snapshot.composerVisible));
    return root;
  }

  Element _title() {
    return _heading('h1', 'Robdex Workbench');
  }

  Element _mirrorStateSection(DomMirrorSnapshot snapshot) {
    final section = Element.tag('section')
      ..setAttribute('aria-label', 'DOM mirror state')
      ..setAttribute('data-generated-at', snapshot.generatedAt.toString());
    section.append(_sectionHeading('DOM mirror state'));
    section.append(_paragraph('Generated at: ${snapshot.generatedAt}'));
    section.append(_paragraph('Selected chat entries: ${snapshot.chatEntries.length}'));
    if (snapshot.chatEntries.isNotEmpty) {
      final latest = snapshot.chatEntries.last;
      section.append(_paragraph('Latest entry author: ${latest.author}'));
      section.append(_paragraph('Latest entry label: ${latest.displayLabel}'));
      if (latest.kind != null) {
        section.append(_paragraph('Latest entry kind: ${latest.kind}'));
      }
      if (latest.status != null) {
        section.append(_paragraph('Latest entry status: ${latest.status}'));
      }
      if (latest.command != null) {
        section.append(_paragraph('Latest entry command: ${latest.command}'));
      }
      if (latest.body != null && latest.body!.trim().isNotEmpty) {
        section.append(_paragraph('Latest entry body: ${latest.body}'));
      }
      if (latest.outputPreview != null && latest.outputPreview!.trim().isNotEmpty) {
        section.append(_paragraph('Latest entry output preview: ${latest.outputPreview}'));
      }
    }
    return section;
  }

  Element _identitySection(DomMirrorSnapshot snapshot) {
    final section = Element.tag('section');
    section.setAttribute('aria-label', 'Selected thread');
    final threadName = ParagraphElement()
      ..text = 'Thread: ${snapshot.selection.threadName}';
    final threadRole = ParagraphElement()
      ..text = 'Role: ${snapshot.selection.threadRole ?? 'unknown'}';
    final project = ParagraphElement()
      ..text = 'Project: ${snapshot.selection.projectName}';
    final projectPath = snapshot.selection.projectRootPath == null
        ? null
        : (ParagraphElement()..text = 'Project path: ${snapshot.selection.projectRootPath}');
    final model = ParagraphElement()
      ..text = 'Model: ${snapshot.selection.model ?? 'default'}';
    final approval = ParagraphElement()
      ..text = 'Approval policy: ${snapshot.selection.approvalPolicy ?? 'default'}';
    final sandbox = ParagraphElement()
      ..text = 'Sandbox: ${snapshot.selection.sandboxMode ?? 'default'}';
    final network = ParagraphElement()
      ..text = 'Network access: ${snapshot.selection.networkAccess == null ? 'default' : snapshot.selection.networkAccess! ? 'enabled' : 'disabled'}';
    final serviceTier = ParagraphElement()
      ..text = 'Service tier: ${snapshot.selection.serviceTier ?? 'default'}';
    final reasoningEffort = ParagraphElement()
      ..text = 'Reasoning effort: ${snapshot.selection.reasoningEffort ?? 'default'}';
    section
      ..append(_sectionHeading('Selected thread'))
      ..append(threadName)
      ..append(project)
      ..append(threadRole)
      ..append(model)
      ..append(approval)
      ..append(sandbox)
      ..append(network)
      ..append(serviceTier)
      ..append(reasoningEffort);
    if (snapshot.selection.threadId != null) {
      section.append(
        (ParagraphElement()..text = 'Selected thread id: ${snapshot.selection.threadId}'),
      );
    }
    if (projectPath != null) {
      section.append(projectPath);
    }
    return section;
  }

  Element _statusSection(DomMirrorSnapshot snapshot) {
    final section = Element.tag('section');
    section.setAttribute('aria-label', 'Connection status');
    section
      ..append(_sectionHeading('Connection'))
      ..append(_paragraph('Status headline: ${snapshot.statusHeadline}'))
      ..append(_paragraph('Status detail: ${snapshot.statusDetail}'))
      ..append(_paragraph('Connection label: ${snapshot.connectionLabel}'));
    return section;
  }

  Element _projectsSection(List<DomMirrorProject> projects) {
    final section = Element.tag('section')..setAttribute('aria-label', 'Threads');
    section.append(_sectionHeading('Projects and threads'));
    for (final project in projects) {
      final projectNode = Element.tag('article');
      projectNode.setAttribute('data-project-id', project.id);
      projectNode.setAttribute('data-project-name', project.name);
      projectNode.append(_subtitle(project.name));
      projectNode.append(_paragraph('Path: ${project.rootPath}'));
      if (project.threads.isEmpty) {
        projectNode.append(_paragraph('No visible threads'));
      } else {
        for (final thread in project.threads) {
          final reviewStatus = thread.requirementReviewStatus;
          final requirementSuffix =
              reviewStatus == null || reviewStatus.trim().isEmpty
                  ? ''
                  : ' review:$reviewStatus';
          final threadLine = ParagraphElement()
            ..setAttribute('data-thread-role', thread.role)
            ..setAttribute('data-thread-running', thread.isRunning.toString())
            ..setAttribute('data-thread-unread', thread.unreadCount.toString())
            ..text =
                '${thread.title} (${thread.role}) [${thread.isRunning ? 'running' : 'idle'}] unread ${thread.unreadCount}$requirementSuffix';
          projectNode.append(threadLine);
        }
      }
      section.append(projectNode);
    }
    return section;
  }

  Element _chatSection(List<DomMirrorChatEntry> entries) {
    final section = Element.tag('section')
      ..setAttribute('aria-label', 'Chat timeline');
    section.append(_sectionHeading('Chat timeline'));
    section.append(_paragraph('Newest entries are listed first for browser-agent inspection.'));
    for (final entry in entries.reversed) {
      final article = Element.tag('article')
        ..setAttribute('data-entry-id', entry.id)
        ..setAttribute('data-entry-kind', entry.kind ?? 'message')
        ..setAttribute('data-entry-status', entry.status ?? 'unknown')
        ..setAttribute('data-entry-tool', entry.isTool.toString())
        ..setAttribute('data-entry-streaming', entry.isStreaming.toString());
      article.append(_paragraph('Author: ${entry.author}'));
      article.append(_paragraph('Label: ${entry.displayLabel}'));
      if (entry.kind != null) {
        article.append(_paragraph('Kind: ${entry.kind}'));
      }
      if (entry.status != null) {
        article.append(_paragraph('Status: ${entry.status}'));
      }
      if (entry.timestamp != null) {
        article.append(_paragraph('Timestamp: ${entry.timestamp}'));
      }
      if (entry.command != null) {
        final line = entry.kind == 'fileChange' ? 'Changed files/path: ${entry.command}' : 'Command: ${entry.command}';
        article.append(_paragraph(line));
      } else if (entry.body != null && entry.body!.trim().isNotEmpty) {
        article.append(_paragraph('Body: ${entry.body}'));
      }
      if (entry.outputPreview != null) {
        article.append(_paragraph('Output preview: ${entry.outputPreview}'));
      }
      section.append(article);
    }
    if (entries.isEmpty) {
      section.append(_paragraph('No visible timeline entries'));
    }
    return section;
  }

  Element _approvalsSection(List<DomMirrorPendingApproval> approvals) {
    final section = Element.tag('section')
      ..setAttribute('aria-label', 'Pending approvals');
    section.append(_sectionHeading('Pending approvals'));
    for (final approval in approvals) {
      final article = Element.tag('article')
        ..setAttribute('data-approval-id', approval.id)
        ..setAttribute('data-approval-kind', approval.kind)
        ..setAttribute('data-approval-thread-id', approval.threadId);
      article.append(_paragraph('Title: ${approval.title}'));
      article.append(_paragraph('Thread: ${approval.threadId}'));
      if (approval.command != null) {
        article.append(_paragraph('Command: ${approval.command}'));
      }
      if (approval.commandCwd != null) {
        article.append(_paragraph('Command CWD: ${approval.commandCwd}'));
      }
      if (approval.filePaths.isNotEmpty) {
        article.append(_paragraph('File paths: ${approval.filePaths.join(', ')}'));
      }
      if (approval.detail != null) {
        article.append(_paragraph('Detail: ${approval.detail}'));
      }
      section.append(article);
    }
    if (approvals.isEmpty) {
      section.append(_paragraph('No pending approvals'));
    }
    return section;
  }

  Element _requirementsSection(DomMirrorRequirementsReview review) {
    final section = Element.tag('section')
      ..setAttribute('aria-label', 'Requirements review');
    section.append(_sectionHeading('Requirements review'));
    section.append(_paragraph('Status: ${review.status}'));
    section.append(_paragraph('Active requirements: ${review.activeRequirementCount}'));
    section.append(
      _paragraph('Passed: ${review.passedCount} · Failed: ${review.failedCount} · Blocked: ${review.blockedCount}'),
    );
    if (review.reviewerThreadId != null && review.reviewerThreadId!.isNotEmpty) {
      section.append(
        _paragraph(
          'Reviewer thread (for selected source thread): ${review.reviewerThreadId}',
        ),
      );
    }
    if (review.failedKeys.isNotEmpty) {
      section.append(
        _paragraph('Failed keys: ${review.failedKeys.join(', ')}'),
      );
    }
    if (review.blockedKeys.isNotEmpty) {
      section.append(
        _paragraph('Blocked keys: ${review.blockedKeys.join(', ')}'),
      );
    }
    for (final failed in review.failedVerdicts) {
      final details = _requirementVerdictLine(failed);
      section.append(_paragraph('Failed verdict: $details'));
    }
    for (final blocked in review.blockedVerdicts) {
      final details = _requirementVerdictLine(blocked);
      section.append(_paragraph('Blocked verdict: $details'));
    }
    return section;
  }

  Element _liveProcessSection(List<DomMirrorLiveProcess> processes) {
    final section = Element.tag('section')..setAttribute('aria-label', 'Live processes');
    section.append(_sectionHeading('Live processes'));
    if (processes.isEmpty) {
      section.append(_paragraph('No live processes'));
      return section;
    }
    for (final process in processes) {
      final line = Element.tag('article');
      line.append(_paragraph('id: ${process.processId}'));
      line.append(_paragraph('command: ${process.command}'));
      if (process.pid != null) {
        line.append(_paragraph('pid: ${process.pid}'));
      }
      if (process.processGroupId != null) {
        line.append(_paragraph('groupId: ${process.processGroupId}'));
      }
      if (process.cwd != null) {
        line.append(_paragraph('cwd: ${process.cwd}'));
      }
      if (process.startedAt != null) {
        line.append(_paragraph('startedAt: ${process.startedAt}'));
      }
      section.append(line);
    }
    return section;
  }

  Element _composerSection(bool visible) {
    final section = Element.tag('section')..setAttribute('aria-label', 'Composer');
    section.append(_sectionHeading('Composer'));
    section.append(
      ParagraphElement()
        ..text = visible
            ? 'Composer input is available for the selected thread.'
            : 'Composer input is unavailable for the selected thread.',
    );
    return section;
  }

  Element _sectionHeading(String text) {
    return _heading('h2', text);
  }

  Element _subtitle(String text) {
    return _heading('h3', text);
  }

  Element _paragraph(String text) {
    return Element.tag('p')..text = text;
  }

  Element _heading(String tag, String text) {
    return Element.tag(tag)..text = text;
  }

  String _requirementVerdictLine(DomMirrorRequirementVerdict verdict) {
    final details = <String>[];
    if (verdict.verdict != null) {
      details.add('verdict=${verdict.verdict}');
    }
    if (verdict.reason != null && verdict.reason!.isNotEmpty) {
      details.add('reason=${verdict.reason}');
    }
    if (verdict.evidenceAssessment != null &&
        verdict.evidenceAssessment!.isNotEmpty) {
      details.add('evidence=${verdict.evidenceAssessment}');
    }
    if (verdict.requiredCorrection != null &&
        verdict.requiredCorrection!.isNotEmpty) {
      details.add('requiredCorrection=${verdict.requiredCorrection}');
    }
    return '${verdict.key}: ${details.join(', ')}';
  }
}
