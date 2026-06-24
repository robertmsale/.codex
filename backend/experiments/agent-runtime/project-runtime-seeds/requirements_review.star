# Shipped Requirements Review project-runtime seed.
#
# Rust validates this deterministic Starlark config, stores the source and
# compiled manifest in PostgreSQL, and owns all side effects produced by hook
# intents. The Starlark program declares workflow intent only.

role_definition(
    id = "requirements-reviewer",
    display_name = "Requirements Reviewer",
    tool_bundle = "reviewer_read_only",
)

role_tool_bundle(
    id = "reviewer_read_only",
    tools = ["read_packet", "route_packet"],
)

channel(
    id = "requirements_review_packets",
    packet_types = ["requirements.claim", "requirements.verdict", "requirements.claim_or_verdict"],
)

contract_workflow(
    id = "requirements_review",
    contract_type = "requirements",
    packet_types = ["requirements.claim", "requirements.verdict"],
)

hook_binding(
    name = "on_model_request",
    source = """
def hook(ctx):
    if ctx.get("active_contracts"):
        return [require_output_schema(
            key = "requirements-source-or-reviewer",
            packet_type = "requirements.claim_or_verdict",
            schema_name = "requirements_review_structured_output",
            schema = ctx["active_contracts"][0]["schema"],
        )]
    return []
""",
)

hook_binding(
    name = "on_packet_recorded",
    source = """
def hook(ctx):
    packet = ctx["recent_packet_summaries"][0]
    if packet["packet_type"] == "requirements.claim":
        return [
            ensure_subagent(
                key = "requirements-reviewer",
                workflow_identity = packet["contract_id"],
                kind = "requirementsReviewer",
                role_id = "requirements-reviewer",
            ),
            route_packet(packet_id = packet["packet_id"], target = "subagent:requirements-reviewer"),
        ]
    if packet["packet_type"] == "requirements.verdict":
        return [update_contract_progress(contract_id = packet["contract_id"], progress_key = "overall", status = packet["status"])]
    return []
""",
)
