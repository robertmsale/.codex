import 'package:flutter_test/flutter_test.dart';
import 'package:robdex_design_system/robdex_design_system.dart';
import 'package:robdex_app/src/web/dom_mirror/dom_mirror.dart';
import 'package:robdex_app/src/web/dom_mirror/dom_mirror_snapshot.dart';

void main() {
  test('dom mirror controller is no-op in non-web tests', () {
    final controller = DomMirrorController();
    expect(controller.isEnabled, isFalse);
    expect(() => controller.update(mockWorkbenchData), returnsNormally);
    expect(() => controller.clear(), returnsNormally);
    expect(() => controller.dispose(), returnsNormally);
  });

  test('snapshot captures selected thread settings', () {
    final snapshot = DomMirrorSnapshot.fromWorkbench(mockWorkbenchData);

    expect(snapshot.selection.projectName, 'Codex Control Plane');
    expect(snapshot.selection.threadId, 'config-operator');
    expect(snapshot.selection.threadRole, 'operator');
    expect(snapshot.selection.model, 'gpt-5');
    expect(snapshot.selection.approvalPolicy, 'never');
    expect(snapshot.selection.sandboxMode, 'danger-full-access');
    expect(snapshot.selection.networkAccess, true);
    expect(snapshot.selection.reasoningEffort, 'high');
  });

  test('snapshot includes visible projects and threads', () {
    final snapshot = DomMirrorSnapshot.fromWorkbench(mockWorkbenchData);

    expect(snapshot.projects.length, 2);
    final codexProject = snapshot.projects.firstWhere(
      (project) => project.name == 'Codex Control Plane',
    );
    expect(codexProject.threads.length, 2);
    expect(codexProject.threads.any((thread) => thread.title == 'Config Operator'), true);
    expect(codexProject.threads.any((thread) => thread.title == 'Approval Smoke Worker'), true);
  });

  test('snapshot includes command and file-change chat rows', () {
    final snapshot = DomMirrorSnapshot.fromWorkbench(
      mockWorkbenchData.copyWith(
        chatEntries: [
          ...mockWorkbenchData.chatEntries,
          const ChatEntry(
            id: 'cmd-1',
            author: 'Tool',
            displayLabel: 'Run command',
            timestamp: 1715555555,
            body: 'git status',
            kind: 'commandExecution',
            status: 'completed',
            command: 'git status',
            output: 'working tree clean',
            isTool: true,
          ),
          const ChatEntry(
            id: 'file-1',
            author: 'Tool',
            displayLabel: 'File update',
            timestamp: 1715555556,
            body: 'frontend/robdex_app/lib/src/app/robdex_app.dart',
            kind: 'fileChange',
            status: 'completed',
            isTool: true,
          ),
        ],
      ),
    ).chatEntries;

    final ids = snapshot.map((entry) => entry.id).toSet();
    expect(ids.contains('cmd-1'), true);
    expect(ids.contains('file-1'), true);
    expect(snapshot.any((entry) => entry.kind == 'commandExecution'), true);
    expect(snapshot.any((entry) => entry.kind == 'fileChange'), true);
  });

  test('snapshot preserves body text and truncates oversized command output', () {
    final longOutput = List<String>.filled(4000, 'x').join();
    final longBody = List<String>.filled(5000, 'b').join();
    final snapshot = DomMirrorSnapshot.fromWorkbench(
      mockWorkbenchData.copyWith(
        chatEntries: [
          ChatEntry(
            id: '1',
            author: 'Tool',
            displayLabel: 'Large output',
            timestamp: 0,
            body: longBody,
            kind: 'commandExecution',
            status: 'completed',
            command: List<String>.filled(5000, 'c').join(),
            output: longOutput,
            isTool: true,
          ),
        ],
      ),
    );

    expect(snapshot.chatEntries.single.outputPreview, isNotNull);
    expect(snapshot.chatEntries.single.outputPreview!.length <= 2401, true);
    expect(snapshot.chatEntries.single.command!.length <= 1201, true);
    expect(snapshot.chatEntries.single.body, longBody);
  });

  test('dom mirror requirements summary handles failed and blocked keys', () {
    final snapshot = DomMirrorSnapshot.fromWorkbench(
      mockWorkbenchData.copyWith(
        requirementReview: const RequirementReviewSummary(
          activeRequirementCount: 3,
          storedRequirementCount: 3,
          requirementSetActive: true,
          status: 'failed',
          reviewerThreadId: 'reviewer',
          parentThreadId: 'source',
          requirementSetId: 'set-1',
          latestClaimPacket: null,
          latestVerdictPacket: null,
          passedCount: 1,
          failedCount: 1,
          blockedCount: 1,
          waiverRequiredCount: 0,
          unknownCount: 1,
          updatedAt: null,
          requirements: [],
          verdicts: [
            RequirementVerdictSummary(
              key: 'k1',
              verdict: 'fail',
              reason: null,
              evidenceAssessment: null,
              requiredCorrection: null,
            ),
            RequirementVerdictSummary(
              key: 'k2',
              verdict: 'acceptedBlocked',
              reason: null,
              evidenceAssessment: null,
              requiredCorrection: null,
            ),
          ],
        ),
      ),
    ).requirementsReview;

    expect(snapshot, isNotNull);
    expect(snapshot!.failedKeys, contains('k1'));
    expect(snapshot.failedKeys, isNot(contains('k2')));
    expect(snapshot.blockedKeys, contains('k2'));
  });

  test('thread list mirrors requirement active status even when review status is null', () {
    final snapshot = DomMirrorSnapshot.fromWorkbench(
      mockWorkbenchData.copyWith(
        threads: [
          ThreadItem(
            id: 'active-thread',
            title: 'Active Requirements',
              role: 'worker',
              projectId: mockWorkbenchData.projects.first.id,
              projectRootPath: mockWorkbenchData.projects.first.rootPath,
              projectName: 'stale-display-name',
              preview: 'Has active requirements',
            isRunning: false,
            unreadCount: 0,
            requirementReview: const RequirementReviewSummary(
              activeRequirementCount: 2,
              storedRequirementCount: 2,
              requirementSetActive: true,
              status: null,
              reviewerThreadId: null,
              parentThreadId: null,
              requirementSetId: null,
              latestClaimPacket: null,
              latestVerdictPacket: null,
              passedCount: 0,
              failedCount: 0,
              blockedCount: 0,
              waiverRequiredCount: 0,
              unknownCount: 2,
              updatedAt: null,
              requirements: [],
              verdicts: [],
            ),
          ),
        ],
      ),
    );

    final project = snapshot.projects.singleWhere(
      (project) => project.name == 'Codex Control Plane',
    );
    expect(project.threads.single.requirementReviewStatus, 'Requirements active');
  });

  test('thread sorting follows operator/orchestrator/worker/designer/qa/hidden order', () {
    final snapshot = DomMirrorSnapshot.fromWorkbench(
      mockWorkbenchData.copyWith(
        threads: [
          ThreadItem(
            id: 't4',
            title: 'Designer thread',
            role: 'designer',
            projectName: 'Codex Control Plane',
            preview: 'designer',
            isRunning: false,
            unreadCount: 0,
            requirementReview: null,
          ),
          ThreadItem(
            id: 't5',
            title: 'Worker thread',
            role: 'worker',
            projectName: 'Codex Control Plane',
            preview: 'worker',
            isRunning: false,
            unreadCount: 0,
            requirementReview: null,
          ),
          ThreadItem(
            id: 't6',
            title: 'Hidden thread',
            role: 'hidden',
            projectName: 'Codex Control Plane',
            preview: 'hidden',
            isRunning: false,
            unreadCount: 0,
            requirementReview: null,
          ),
          ThreadItem(
            id: 't3',
            title: 'Orchestrator thread',
            role: 'orchestrator',
            projectName: 'Codex Control Plane',
            preview: 'orchestrator',
            isRunning: false,
            unreadCount: 0,
            requirementReview: null,
          ),
          ThreadItem(
            id: 't1',
            title: 'QA thread',
            role: 'qa',
            projectName: 'Codex Control Plane',
            preview: 'qa',
            isRunning: false,
            unreadCount: 0,
            requirementReview: null,
          ),
          ThreadItem(
            id: 't2',
            title: 'Operator thread',
            role: 'operator',
            projectName: 'Codex Control Plane',
            preview: 'operator',
            isRunning: false,
            unreadCount: 0,
            requirementReview: null,
          ),
          ThreadItem(
            id: 't7',
            title: 'Other role thread',
            role: 'consultant',
            projectName: 'Codex Control Plane',
            preview: 'consultant',
            isRunning: false,
            unreadCount: 0,
            requirementReview: null,
          ),
        ],
      ),
    );

    final codexThreads = snapshot.projects
        .firstWhere((project) => project.name == 'Codex Control Plane')
        .threads;
    final titles = codexThreads.map((thread) => thread.title).toList(growable: false);
    expect(titles, [
      'Operator thread',
      'Orchestrator thread',
      'Worker thread',
      'Designer thread',
      'QA thread',
      'Hidden thread',
      'Other role thread',
    ]);
  });

  test('requirements verdict details mirror reason/evidence/requiredCorrection', () {
    final snapshot = DomMirrorSnapshot.fromWorkbench(
      mockWorkbenchData.copyWith(
        requirementReview: const RequirementReviewSummary(
          activeRequirementCount: 1,
          storedRequirementCount: 1,
          requirementSetActive: true,
          status: 'failed',
          reviewerThreadId: 'review-thread',
          parentThreadId: 'source-thread',
          requirementSetId: 'set-1',
          latestClaimPacket: null,
          latestVerdictPacket: null,
          passedCount: 0,
          failedCount: 1,
          blockedCount: 0,
          waiverRequiredCount: 0,
          unknownCount: 0,
          updatedAt: null,
          requirements: [],
          verdicts: [
            RequirementVerdictSummary(
              key: 'security.test',
              verdict: 'fail',
              reason: 'security check did not pass',
              evidenceAssessment: 'missing required flag in output',
              requiredCorrection: 'set security flag true',
            ),
          ],
        ),
      ),
    ).requirementsReview!;

    expect(snapshot.failedVerdicts.length, 1);
    final verdict = snapshot.failedVerdicts.first;
    expect(verdict.key, 'security.test');
    expect(verdict.reason, 'security check did not pass');
    expect(verdict.evidenceAssessment, 'missing required flag in output');
    expect(verdict.requiredCorrection, 'set security flag true');
  });
}
