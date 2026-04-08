import '../models/mock_workbench_data.dart';
import '../models/workbench_view_data.dart';
import 'workbench_gateway.dart';

class MockWorkbenchGateway implements WorkbenchGateway {
  const MockWorkbenchGateway();

  @override
  Future<WorkbenchViewData> loadInitialView() async {
    return mockWorkbenchData;
  }

  @override
  Future<WorkbenchViewData> selectThread(String threadId, WorkbenchViewData current) async {
    return current;
  }

  @override
  Future<WorkbenchViewData> createProject({
    required String name,
    required String rootPath,
    required String defaultCwd,
  }) async {
    return mockWorkbenchData;
  }

  @override
  Future<WorkbenchViewData> createThread({
    required String title,
    String role = 'worker',
  }) async {
    return mockWorkbenchData;
  }

  @override
  Future<void> sendMessage({
    required String threadId,
    required String text,
  }) async {}

  @override
  Future<void> decideApproval({
    required String senderThreadId,
    required String approvalId,
    required String decision,
    String? message,
  }) async {}

  @override
  Future<WorkbenchViewData> selectProject(String? projectId) async {
    return mockWorkbenchData;
  }

  @override
  Future<WorkbenchViewData> deleteProject(String projectId) async {
    return mockWorkbenchData;
  }

  @override
  Stream<WorkbenchViewData> watch({
    required WorkbenchViewData current,
    required String? selectedThreadId,
  }) {
    return const Stream.empty();
  }

  @override
  void dispose() {}
}
