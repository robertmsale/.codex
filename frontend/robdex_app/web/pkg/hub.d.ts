/* tslint:disable */
/* eslint-disable */

export function rinf_send_dart_signal_archive_thread_group_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_send_dart_signal_archive_thread_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_send_dart_signal_create_project_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_send_dart_signal_create_thread_group_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_send_dart_signal_create_thread_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_send_dart_signal_decide_approval_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_send_dart_signal_delete_project_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_send_dart_signal_delete_thread_group_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_send_dart_signal_fetch_thread_history_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_send_dart_signal_initialize_workbench_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_send_dart_signal_interrupt_thread_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_send_dart_signal_move_selected_thread_to_group_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_send_dart_signal_reload_workbench_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_send_dart_signal_rename_thread_group_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_send_dart_signal_rename_thread_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_send_dart_signal_select_project_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_send_dart_signal_select_thread_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_send_dart_signal_send_thread_message_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_send_dart_signal_set_project_orchestrator_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_send_dart_signal_set_thread_running_state_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_send_dart_signal_spawn_agent_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_send_dart_signal_terminate_command_execution_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_send_dart_signal_thread_compact_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_send_dart_signal_update_project_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_send_dart_signal_update_thread_settings_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_send_dart_signal_update_worker_metadata_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_send_dart_signal_warm_handoff_signal(message_bytes: Uint8Array, binary: Uint8Array): void;

export function rinf_start_rust_logic_extern(): void;

/**
 * Entry point invoked by JavaScript in a worker.
 */
export function task_worker_entry_point(ptr: number): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly rinf_send_dart_signal_archive_thread_group_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_send_dart_signal_archive_thread_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_send_dart_signal_create_project_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_send_dart_signal_create_thread_group_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_send_dart_signal_create_thread_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_send_dart_signal_decide_approval_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_send_dart_signal_delete_project_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_send_dart_signal_delete_thread_group_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_send_dart_signal_fetch_thread_history_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_send_dart_signal_initialize_workbench_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_send_dart_signal_interrupt_thread_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_send_dart_signal_move_selected_thread_to_group_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_send_dart_signal_reload_workbench_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_send_dart_signal_rename_thread_group_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_send_dart_signal_rename_thread_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_send_dart_signal_select_project_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_send_dart_signal_select_thread_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_send_dart_signal_send_thread_message_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_send_dart_signal_set_project_orchestrator_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_send_dart_signal_set_thread_running_state_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_send_dart_signal_spawn_agent_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_send_dart_signal_terminate_command_execution_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_send_dart_signal_thread_compact_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_send_dart_signal_update_project_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_send_dart_signal_update_thread_settings_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_send_dart_signal_update_worker_metadata_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_send_dart_signal_warm_handoff_signal: (a: number, b: number, c: number, d: number) => void;
    readonly rinf_start_rust_logic_extern: () => void;
    readonly task_worker_entry_point: (a: number) => [number, number];
    readonly wasm_bindgen__convert__closures_____invoke__h0a67fa76e899c8a4: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen__convert__closures_____invoke__h20bf947e7f788635: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h6058dcc82724e2e4: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h6058dcc82724e2e4_2: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h6058dcc82724e2e4_3: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__hb73350ad9f3925dd: (a: number, b: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_destroy_closure: (a: number, b: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
