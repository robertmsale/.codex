// ignore_for_file: type=lint, type=warning
// ignore_for_file: unused_import
library signals_types;

import 'dart:typed_data';
import 'package:meta/meta.dart';
import 'package:tuple/tuple.dart';
import '../serde/serde.dart';
import '../bincode/bincode.dart';

import 'dart:async';
import 'package:rinf/rinf.dart';

export '../serde/serde.dart';

part 'trait_helpers.dart';
part 'archive_thread_group_signal.dart';
part 'archive_thread_signal.dart';
part 'bridge_task_result_signal.dart';
part 'clear_project_hook_logs_signal.dart';
part 'create_project_signal.dart';
part 'create_thread_group_signal.dart';
part 'create_thread_signal.dart';
part 'decide_approval_signal.dart';
part 'delete_project_signal.dart';
part 'delete_thread_group_signal.dart';
part 'fetch_thread_history_signal.dart';
part 'hook_toast_signal.dart';
part 'initialize_workbench_signal.dart';
part 'interrupt_thread_signal.dart';
part 'load_period_stats_signal.dart';
part 'load_project_hook_logs_signal.dart';
part 'load_requirement_composables_signal.dart';
part 'load_thread_stats_signal.dart';
part 'move_selected_thread_to_group_signal.dart';
part 'reload_workbench_signal.dart';
part 'rename_thread_group_signal.dart';
part 'rename_thread_signal.dart';
part 'select_project_signal.dart';
part 'select_thread_signal.dart';
part 'send_thread_message_signal.dart';
part 'set_project_orchestrator_signal.dart';
part 'set_thread_requirements_signal.dart';
part 'set_thread_running_state_signal.dart';
part 'spawn_agent_signal.dart';
part 'terminal_close_all_signal.dart';
part 'terminal_close_signal.dart';
part 'terminal_event_signal.dart';
part 'terminal_input_signal.dart';
part 'terminal_open_signal.dart';
part 'terminal_resize_signal.dart';
part 'terminate_command_execution_signal.dart';
part 'thread_compact_signal.dart';
part 'thread_history_state_signal.dart';
part 'update_global_settings_signal.dart';
part 'update_project_signal.dart';
part 'update_thread_settings_signal.dart';
part 'update_worker_metadata_signal.dart';
part 'upload_image_bytes_signal.dart';
part 'warm_handoff_signal.dart';
part 'workbench_state_signal.dart';
part 'signal_handlers.dart';
