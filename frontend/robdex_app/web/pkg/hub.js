/* @ts-self-types="./hub.d.ts" */

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_archive_thread_group_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_archive_thread_group_signal(ptr0, len0, ptr1, len1);
}

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_archive_thread_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_archive_thread_signal(ptr0, len0, ptr1, len1);
}

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_create_project_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_create_project_signal(ptr0, len0, ptr1, len1);
}

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_create_thread_group_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_create_thread_group_signal(ptr0, len0, ptr1, len1);
}

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_create_thread_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_create_thread_signal(ptr0, len0, ptr1, len1);
}

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_decide_approval_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_decide_approval_signal(ptr0, len0, ptr1, len1);
}

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_delete_project_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_delete_project_signal(ptr0, len0, ptr1, len1);
}

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_delete_thread_group_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_delete_thread_group_signal(ptr0, len0, ptr1, len1);
}

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_fetch_thread_history_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_fetch_thread_history_signal(ptr0, len0, ptr1, len1);
}

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_initialize_workbench_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_initialize_workbench_signal(ptr0, len0, ptr1, len1);
}

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_interrupt_thread_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_interrupt_thread_signal(ptr0, len0, ptr1, len1);
}

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_move_selected_thread_to_group_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_move_selected_thread_to_group_signal(ptr0, len0, ptr1, len1);
}

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_reload_workbench_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_reload_workbench_signal(ptr0, len0, ptr1, len1);
}

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_rename_thread_group_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_rename_thread_group_signal(ptr0, len0, ptr1, len1);
}

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_rename_thread_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_rename_thread_signal(ptr0, len0, ptr1, len1);
}

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_select_project_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_select_project_signal(ptr0, len0, ptr1, len1);
}

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_select_thread_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_select_thread_signal(ptr0, len0, ptr1, len1);
}

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_send_thread_message_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_send_thread_message_signal(ptr0, len0, ptr1, len1);
}

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_set_project_orchestrator_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_set_project_orchestrator_signal(ptr0, len0, ptr1, len1);
}

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_set_thread_running_state_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_set_thread_running_state_signal(ptr0, len0, ptr1, len1);
}

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_spawn_agent_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_spawn_agent_signal(ptr0, len0, ptr1, len1);
}

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_terminate_command_execution_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_terminate_command_execution_signal(ptr0, len0, ptr1, len1);
}

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_thread_compact_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_thread_compact_signal(ptr0, len0, ptr1, len1);
}

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_update_project_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_update_project_signal(ptr0, len0, ptr1, len1);
}

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_update_thread_settings_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_update_thread_settings_signal(ptr0, len0, ptr1, len1);
}

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_update_worker_metadata_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_update_worker_metadata_signal(ptr0, len0, ptr1, len1);
}

/**
 * @param {Uint8Array} message_bytes
 * @param {Uint8Array} binary
 */
export function rinf_send_dart_signal_warm_handoff_signal(message_bytes, binary) {
    const ptr0 = passArray8ToWasm0(message_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(binary, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    wasm.rinf_send_dart_signal_warm_handoff_signal(ptr0, len0, ptr1, len1);
}

export function rinf_start_rust_logic_extern() {
    wasm.rinf_start_rust_logic_extern();
}

/**
 * Entry point invoked by JavaScript in a worker.
 * @param {number} ptr
 */
export function task_worker_entry_point(ptr) {
    const ret = wasm.task_worker_entry_point(ptr);
    if (ret[1]) {
        throw takeFromExternrefTable0(ret[0]);
    }
}

function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg___wbindgen_debug_string_dd5d2d07ce9e6c57: function(arg0, arg1) {
            const ret = debugString(arg1);
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_is_function_49868bde5eb1e745: function(arg0) {
            const ret = typeof(arg0) === 'function';
            return ret;
        },
        __wbg___wbindgen_is_string_b29b5c5a8065ba1a: function(arg0) {
            const ret = typeof(arg0) === 'string';
            return ret;
        },
        __wbg___wbindgen_is_undefined_c0cca72b82b86f4d: function(arg0) {
            const ret = arg0 === undefined;
            return ret;
        },
        __wbg___wbindgen_string_get_914df97fcfa788f2: function(arg0, arg1) {
            const obj = arg1;
            const ret = typeof(obj) === 'string' ? obj : undefined;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_throw_81fc77679af83bc6: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg__wbg_cb_unref_3c3b4f651835fbcb: function(arg0) {
            arg0._wbg_cb_unref();
        },
        __wbg_addEventListener_4696109b6f15c412: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0.addEventListener(getStringFromWasm0(arg1, arg2), arg3, arg4);
        }, arguments); },
        __wbg_addEventListener_83ef16da0995f634: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            arg0.addEventListener(getStringFromWasm0(arg1, arg2), arg3);
        }, arguments); },
        __wbg_close_f181fdc02ee236e6: function() { return handleError(function (arg0) {
            arg0.close();
        }, arguments); },
        __wbg_code_c96efa5c1a80b2d9: function(arg0) {
            const ret = arg0.code;
            return ret;
        },
        __wbg_data_60b50110c5bd9349: function(arg0) {
            const ret = arg0.data;
            return ret;
        },
        __wbg_dispatchEvent_7be0dc433e312ebc: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.dispatchEvent(arg1);
            return ret;
        }, arguments); },
        __wbg_error_ba2b2915aeba36d8: function(arg0, arg1) {
            console.error(getStringFromWasm0(arg0, arg1));
        },
        __wbg_fetch_8d9b732df7467c44: function(arg0) {
            const ret = fetch(arg0);
            return ret;
        },
        __wbg_has_3ec5c22db2e5237a: function() { return handleError(function (arg0, arg1) {
            const ret = Reflect.has(arg0, arg1);
            return ret;
        }, arguments); },
        __wbg_instanceof_ArrayBuffer_ff7c1337a5e3b33a: function(arg0) {
            let result;
            try {
                result = arg0 instanceof ArrayBuffer;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Error_e3390d6805733dad: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Error;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Response_06795eab66cc4036: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Response;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_length_0c32cb8543c8e4c8: function(arg0) {
            const ret = arg0.length;
            return ret;
        },
        __wbg_message_7367f8c7d0fa1589: function(arg0) {
            const ret = arg0.message;
            return ret;
        },
        __wbg_name_cb583806cac84fe0: function(arg0) {
            const ret = arg0.name;
            return ret;
        },
        __wbg_new_0fec9fb02d03a383: function() { return handleError(function (arg0, arg1) {
            const ret = new URL(getStringFromWasm0(arg0, arg1));
            return ret;
        }, arguments); },
        __wbg_new_3a112826a89cb962: function() { return handleError(function () {
            const ret = new Headers();
            return ret;
        }, arguments); },
        __wbg_new_40792555590ec35c: function(arg0, arg1) {
            try {
                var state0 = {a: arg0, b: arg1};
                var cb0 = (arg0, arg1) => {
                    const a = state0.a;
                    state0.a = 0;
                    try {
                        return wasm_bindgen__convert__closures_____invoke__h20bf947e7f788635(a, state0.b, arg0, arg1);
                    } finally {
                        state0.a = a;
                    }
                };
                const ret = new Promise(cb0);
                return ret;
            } finally {
                state0.a = 0;
            }
        },
        __wbg_new_4f9fafbb3909af72: function() {
            const ret = new Object();
            return ret;
        },
        __wbg_new_7681c4155808e30a: function() { return handleError(function () {
            const ret = new URLSearchParams();
            return ret;
        }, arguments); },
        __wbg_new_a2d8434834334bbf: function() { return handleError(function (arg0, arg1) {
            const ret = new WebSocket(getStringFromWasm0(arg0, arg1));
            return ret;
        }, arguments); },
        __wbg_new_a560378ea1240b14: function(arg0) {
            const ret = new Uint8Array(arg0);
            return ret;
        },
        __wbg_new_from_slice_2580ff33d0d10520: function(arg0, arg1) {
            const ret = new Uint8Array(getArrayU8FromWasm0(arg0, arg1));
            return ret;
        },
        __wbg_new_with_event_init_dict_90cf1fb15e06f148: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = new CloseEvent(getStringFromWasm0(arg0, arg1), arg2);
            return ret;
        }, arguments); },
        __wbg_new_with_str_9dca18ad543fe832: function() { return handleError(function (arg0, arg1) {
            const ret = new Request(getStringFromWasm0(arg0, arg1));
            return ret;
        }, arguments); },
        __wbg_new_with_str_and_init_f663b6d334baa878: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = new Request(getStringFromWasm0(arg0, arg1), arg2);
            return ret;
        }, arguments); },
        __wbg_now_88621c9c9a4f3ffc: function() {
            const ret = Date.now();
            return ret;
        },
        __wbg_ok_36f7b13b74596c24: function(arg0) {
            const ret = arg0.ok;
            return ret;
        },
        __wbg_postMessage_2b529c5fbb0ae01c: function() { return handleError(function (arg0, arg1) {
            arg0.postMessage(arg1);
        }, arguments); },
        __wbg_prototypesetcall_3e05eb9545565046: function(arg0, arg1, arg2) {
            Uint8Array.prototype.set.call(getArrayU8FromWasm0(arg0, arg1), arg2);
        },
        __wbg_queueMicrotask_abaf92f0bd4e80a4: function(arg0) {
            const ret = arg0.queueMicrotask;
            return ret;
        },
        __wbg_queueMicrotask_df5a6dac26d818f3: function(arg0) {
            queueMicrotask(arg0);
        },
        __wbg_readyState_631d9f7c37e595d7: function(arg0) {
            const ret = arg0.readyState;
            return ret;
        },
        __wbg_reason_85e58391371e868d: function(arg0, arg1) {
            const ret = arg1.reason;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_removeEventListener_e5033ab3bcad443c: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            arg0.removeEventListener(getStringFromWasm0(arg1, arg2), arg3);
        }, arguments); },
        __wbg_resolve_0a79de24e9d2267b: function(arg0) {
            const ret = Promise.resolve(arg0);
            return ret;
        },
        __wbg_rinf_send_rust_signal_extern_045ebe93047411e4: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            rinfBindings.rinf_send_rust_signal_extern(getStringFromWasm0(arg0, arg1), arg2, arg3);
        }, arguments); },
        __wbg_search_bd3fc2fcfcfc32a2: function(arg0, arg1) {
            const ret = arg1.search;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_send_4f53c94146f0274d: function() { return handleError(function (arg0, arg1, arg2) {
            arg0.send(getStringFromWasm0(arg1, arg2));
        }, arguments); },
        __wbg_send_64dd480ad0d86a31: function() { return handleError(function (arg0, arg1, arg2) {
            arg0.send(getArrayU8FromWasm0(arg1, arg2));
        }, arguments); },
        __wbg_setTimeout_3b5e32486c12c54e: function(arg0, arg1) {
            globalThis.setTimeout(arg0, arg1);
        },
        __wbg_set_aa391f3af1ff0e9c: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0.set(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
        }, arguments); },
        __wbg_set_binaryType_95c0a0f7586a3903: function(arg0, arg1) {
            arg0.binaryType = __wbindgen_enum_BinaryType[arg1];
        },
        __wbg_set_body_a304d09cb50cefbe: function(arg0, arg1) {
            arg0.body = arg1;
        },
        __wbg_set_code_602fbf0ab3cb39f3: function(arg0, arg1) {
            arg0.code = arg1;
        },
        __wbg_set_headers_6ab1105e542834e2: function(arg0, arg1) {
            arg0.headers = arg1;
        },
        __wbg_set_method_1971272fe557e972: function(arg0, arg1, arg2) {
            arg0.method = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_once_21b4f52a7651545b: function(arg0, arg1) {
            arg0.once = arg1 !== 0;
        },
        __wbg_set_reason_bdf8bb55943f05b9: function(arg0, arg1, arg2) {
            arg0.reason = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_search_527da9642b10495d: function(arg0, arg1, arg2) {
            arg0.search = getStringFromWasm0(arg1, arg2);
        },
        __wbg_static_accessor_GLOBAL_THIS_a1248013d790bf5f: function() {
            const ret = typeof globalThis === 'undefined' ? null : globalThis;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_GLOBAL_f2e0f995a21329ff: function() {
            const ret = typeof global === 'undefined' ? null : global;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_SELF_24f78b6d23f286ea: function() {
            const ret = typeof self === 'undefined' ? null : self;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_WINDOW_59fd959c540fe405: function() {
            const ret = typeof window === 'undefined' ? null : window;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_status_44ecb0ac1da253f4: function(arg0) {
            const ret = arg0.status;
            return ret;
        },
        __wbg_text_43bdfba45e602cf9: function() { return handleError(function (arg0) {
            const ret = arg0.text();
            return ret;
        }, arguments); },
        __wbg_then_00eed3ac0b8e82cb: function(arg0, arg1, arg2) {
            const ret = arg0.then(arg1, arg2);
            return ret;
        },
        __wbg_then_a0c8db0381c8994c: function(arg0, arg1) {
            const ret = arg0.then(arg1);
            return ret;
        },
        __wbg_toString_6bb93e4c281b55a5: function(arg0) {
            const ret = arg0.toString();
            return ret;
        },
        __wbg_toString_891d991e862e1d44: function(arg0) {
            const ret = arg0.toString();
            return ret;
        },
        __wbg_url_fa6a0c3c3dd41ac6: function(arg0, arg1) {
            const ret = arg1.url;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_wasClean_919e018e809fd9da: function(arg0) {
            const ret = arg0.wasClean;
            return ret;
        },
        __wbindgen_cast_0000000000000001: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [Externref], shim_idx: 417, ret: Result(Unit), inner_ret: Some(Result(Unit)) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen__convert__closures_____invoke__h0a67fa76e899c8a4);
            return ret;
        },
        __wbindgen_cast_0000000000000002: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [NamedExternref("CloseEvent")], shim_idx: 359, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen__convert__closures_____invoke__h6058dcc82724e2e4);
            return ret;
        },
        __wbindgen_cast_0000000000000003: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [NamedExternref("Event")], shim_idx: 359, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen__convert__closures_____invoke__h6058dcc82724e2e4_2);
            return ret;
        },
        __wbindgen_cast_0000000000000004: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [NamedExternref("MessageEvent")], shim_idx: 359, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen__convert__closures_____invoke__h6058dcc82724e2e4_3);
            return ret;
        },
        __wbindgen_cast_0000000000000005: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [], shim_idx: 361, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen__convert__closures_____invoke__hb73350ad9f3925dd);
            return ret;
        },
        __wbindgen_cast_0000000000000006: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            const offset = table.grow(4);
            table.set(0, undefined);
            table.set(offset + 0, undefined);
            table.set(offset + 1, null);
            table.set(offset + 2, true);
            table.set(offset + 3, false);
        },
    };
    return {
        __proto__: null,
        "./hub_bg.js": import0,
    };
}

function wasm_bindgen__convert__closures_____invoke__hb73350ad9f3925dd(arg0, arg1) {
    wasm.wasm_bindgen__convert__closures_____invoke__hb73350ad9f3925dd(arg0, arg1);
}

function wasm_bindgen__convert__closures_____invoke__h6058dcc82724e2e4(arg0, arg1, arg2) {
    wasm.wasm_bindgen__convert__closures_____invoke__h6058dcc82724e2e4(arg0, arg1, arg2);
}

function wasm_bindgen__convert__closures_____invoke__h6058dcc82724e2e4_2(arg0, arg1, arg2) {
    wasm.wasm_bindgen__convert__closures_____invoke__h6058dcc82724e2e4_2(arg0, arg1, arg2);
}

function wasm_bindgen__convert__closures_____invoke__h6058dcc82724e2e4_3(arg0, arg1, arg2) {
    wasm.wasm_bindgen__convert__closures_____invoke__h6058dcc82724e2e4_3(arg0, arg1, arg2);
}

function wasm_bindgen__convert__closures_____invoke__h0a67fa76e899c8a4(arg0, arg1, arg2) {
    const ret = wasm.wasm_bindgen__convert__closures_____invoke__h0a67fa76e899c8a4(arg0, arg1, arg2);
    if (ret[1]) {
        throw takeFromExternrefTable0(ret[0]);
    }
}

function wasm_bindgen__convert__closures_____invoke__h20bf947e7f788635(arg0, arg1, arg2, arg3) {
    wasm.wasm_bindgen__convert__closures_____invoke__h20bf947e7f788635(arg0, arg1, arg2, arg3);
}


const __wbindgen_enum_BinaryType = ["blob", "arraybuffer"];

function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc();
    wasm.__wbindgen_externrefs.set(idx, obj);
    return idx;
}

const CLOSURE_DTORS = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(state => wasm.__wbindgen_destroy_closure(state.a, state.b));

function debugString(val) {
    // primitive types
    const type = typeof val;
    if (type == 'number' || type == 'boolean' || val == null) {
        return  `${val}`;
    }
    if (type == 'string') {
        return `"${val}"`;
    }
    if (type == 'symbol') {
        const description = val.description;
        if (description == null) {
            return 'Symbol';
        } else {
            return `Symbol(${description})`;
        }
    }
    if (type == 'function') {
        const name = val.name;
        if (typeof name == 'string' && name.length > 0) {
            return `Function(${name})`;
        } else {
            return 'Function';
        }
    }
    // objects
    if (Array.isArray(val)) {
        const length = val.length;
        let debug = '[';
        if (length > 0) {
            debug += debugString(val[0]);
        }
        for(let i = 1; i < length; i++) {
            debug += ', ' + debugString(val[i]);
        }
        debug += ']';
        return debug;
    }
    // Test for built-in
    const builtInMatches = /\[object ([^\]]+)\]/.exec(toString.call(val));
    let className;
    if (builtInMatches && builtInMatches.length > 1) {
        className = builtInMatches[1];
    } else {
        // Failed to match the standard '[object ClassName]'
        return toString.call(val);
    }
    if (className == 'Object') {
        // we're a user defined class or Object
        // JSON.stringify avoids problems with cycles, and is generally much
        // easier than looping through ownProperties of `val`.
        try {
            return 'Object(' + JSON.stringify(val) + ')';
        } catch (_) {
            return 'Object';
        }
    }
    // errors
    if (val instanceof Error) {
        return `${val.name}: ${val.message}\n${val.stack}`;
    }
    // TODO we could test for more things here, like `Set`s and `Map`s.
    return className;
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(ptr, len);
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store(idx);
    }
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

function makeMutClosure(arg0, arg1, f) {
    const state = { a: arg0, b: arg1, cnt: 1 };
    const real = (...args) => {

        // First up with a closure we increment the internal reference
        // count. This ensures that the Rust closure environment won't
        // be deallocated while we're invoking it.
        state.cnt++;
        const a = state.a;
        state.a = 0;
        try {
            return f(a, state.b, ...args);
        } finally {
            state.a = a;
            real._wbg_cb_unref();
        }
    };
    real._wbg_cb_unref = () => {
        if (--state.cnt === 0) {
            wasm.__wbindgen_destroy_closure(state.a, state.b);
            state.a = 0;
            CLOSURE_DTORS.unregister(state);
        }
    };
    CLOSURE_DTORS.register(real, state, state);
    return real;
}

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasm;
function __wbg_finalize_init(instance, module) {
    wasm = instance.exports;
    wasmModule = module;
    cachedDataViewMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = module.ok && expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('hub_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
