import '../models/workbench_view_data.dart';

abstract class WorkbenchGateway {
  Future<WorkbenchViewData> loadInitialView();
  Future<WorkbenchViewData> selectThread(String threadId, WorkbenchViewData current);
  Future<WorkbenchViewData> createProject({
    required String name,
    required String rootPath,
    required String defaultCwd,
  });
  Future<WorkbenchViewData> createThread({
    required String title,
    String role = 'worker',
  });
  Future<void> sendMessage({
    required String threadId,
    required String text,
  });
  Future<void> decideApproval({
    required String senderThreadId,
    required String approvalId,
    required String decision,
    String? message,
  });
  Future<WorkbenchViewData> selectProject(String? projectId);
  Future<WorkbenchViewData> deleteProject(String projectId);
  Stream<WorkbenchViewData> watch({
    required WorkbenchViewData current,
    required String? selectedThreadId,
  });
  void dispose();
}
