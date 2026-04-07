use anyhow::{Context, Result};
use codex_app_server_protocol::{
    ClientInfo, ClientRequest, InitializeCapabilities, InitializeParams, JSONRPCMessage, JSONRPCNotification,
    JSONRPCRequest, RequestId, ServerNotification, ServerRequest, ThreadLoadedListParams, ThreadListParams,
    ThreadReadParams, ThreadUnsubscribeParams,
};

pub fn initialize_request(
    request_id: RequestId,
    client_name: impl Into<String>,
    client_version: impl Into<String>,
    experimental_api: bool,
) -> Result<JSONRPCRequest> {
    to_jsonrpc_request(&ClientRequest::Initialize {
        request_id,
        params: InitializeParams {
            client_info: ClientInfo {
                name: client_name.into(),
                title: None,
                version: client_version.into(),
            },
            capabilities: Some(InitializeCapabilities {
                experimental_api,
                opt_out_notification_methods: None,
            }),
        },
    })
}

pub fn thread_list_request(request_id: RequestId, params: ThreadListParams) -> Result<JSONRPCRequest> {
    to_jsonrpc_request(&ClientRequest::ThreadList { request_id, params })
}

pub fn thread_loaded_list_request(request_id: RequestId, params: ThreadLoadedListParams) -> Result<JSONRPCRequest> {
    to_jsonrpc_request(&ClientRequest::ThreadLoadedList { request_id, params })
}

pub fn thread_read_request(
    request_id: RequestId,
    thread_id: impl Into<String>,
    include_turns: bool,
) -> Result<JSONRPCRequest> {
    to_jsonrpc_request(&ClientRequest::ThreadRead {
        request_id,
        params: ThreadReadParams {
            thread_id: thread_id.into(),
            include_turns,
        },
    })
}

pub fn thread_unsubscribe_request(
    request_id: RequestId,
    thread_id: impl Into<String>,
) -> Result<JSONRPCRequest> {
    to_jsonrpc_request(&ClientRequest::ThreadUnsubscribe {
        request_id,
        params: ThreadUnsubscribeParams {
            thread_id: thread_id.into(),
        },
    })
}

pub fn parse_jsonrpc_message(raw: &str) -> Result<JSONRPCMessage> {
    serde_json::from_str(raw).context("failed to parse app-server JSON-RPC message")
}

pub fn parse_server_notification(message: JSONRPCNotification) -> Result<ServerNotification> {
    ServerNotification::try_from(message).context("failed to decode server notification")
}

pub fn parse_server_request(message: JSONRPCRequest) -> Result<ServerRequest> {
    ServerRequest::try_from(message).context("failed to decode server request")
}

pub fn encode_jsonrpc_message(message: &JSONRPCMessage) -> Result<String> {
    serde_json::to_string(message).context("failed to encode JSON-RPC message")
}

fn to_jsonrpc_request(request: &ClientRequest) -> Result<JSONRPCRequest> {
    serde_json::from_value(serde_json::to_value(request)?).context("failed to convert client request to JSON-RPC")
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::{RequestId, ThreadListParams};

    #[test]
    fn initialize_request_serializes_as_expected() {
        let request = initialize_request(
            RequestId::Integer(1),
            "robdex-bridge",
            "0.1.0",
            true,
        )
        .expect("request");

        assert_eq!(request.method, "initialize");
        let params = request.params.expect("params");
        assert_eq!(params["clientInfo"]["name"], "robdex-bridge");
        assert_eq!(params["capabilities"]["experimentalApi"], true);
    }

    #[test]
    fn thread_read_request_uses_real_upstream_wire_name() {
        let request = thread_read_request(RequestId::String("req-1".to_string()), "thread-1", true)
            .expect("request");
        assert_eq!(request.method, "thread/read");
        let params = request.params.expect("params");
        assert_eq!(params["threadId"], "thread-1");
        assert_eq!(params["includeTurns"], true);
    }

    #[test]
    fn server_notification_decodes_from_jsonrpc_notification() {
        let notification = JSONRPCNotification {
            method: "thread/started".to_string(),
            params: Some(serde_json::json!({
                "thread": {
                    "id": "thread-1",
                    "preview": "demo",
                    "ephemeral": false,
                    "modelProvider": "openai",
                    "status": { "type": "idle" },
                    "updatedAt": 1712188800,
                    "createdAt": 1712188800,
                    "path": null,
                    "cwd": "/tmp",
                    "cliVersion": "0.116.0",
                    "source": "appServer",
                    "agentNickname": null,
                    "agentRole": null,
                    "gitInfo": null,
                    "name": "demo",
                    "turns": []
                }
            })),
        };

        let parsed = parse_server_notification(notification).expect("parsed");
        assert_eq!(parsed.to_string(), "thread/started");
    }

    #[test]
    fn thread_list_request_round_trips_to_jsonrpc() {
        let request = thread_list_request(
            RequestId::Integer(2),
            ThreadListParams {
                cursor: None,
                limit: Some(50),
                sort_key: None,
                model_providers: None,
                source_kinds: None,
                archived: Some(false),
                cwd: None,
                search_term: None,
            },
        )
        .expect("request");

        let wire = encode_jsonrpc_message(&JSONRPCMessage::Request(request)).expect("wire");
        let parsed = parse_jsonrpc_message(&wire).expect("parsed");
        match parsed {
            JSONRPCMessage::Request(req) => assert_eq!(req.method, "thread/list"),
            other => panic!("unexpected message: {other:?}"),
        }
    }
}
