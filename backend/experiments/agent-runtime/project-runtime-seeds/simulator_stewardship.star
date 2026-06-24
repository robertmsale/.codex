# Simulator stewardship project-runtime seed.
#
# The workflow is declared as deterministic config. Rust validates steward
# roles, command visibility, resource leases, and turn-completion obligations.

role_definition(
    id = "simulator-steward",
    display_name = "Simulator Steward",
    tool_bundle = "simulator_steward_tools",
)

role_tool_bundle(
    id = "simulator_steward_tools",
    tools = ["simulator.list", "simulator.boot", "simulator.assign", "simulator.release", "simulator.repair"],
)

channel(
    id = "simulator_resource_packets",
    packet_types = ["resource.request", "resource.lease_handle"],
)

resource_type(
    id = "iosSimulator",
    lease_policy = {"exclusive": True, "release_on_session_close": True},
)

steward_binding(
    resource_type = "iosSimulator",
    steward_role = "simulator-steward",
)

hook_binding(
    name = "on_packet_recorded",
    source = """
def hook(ctx):
    packet = ctx["recent_packet_summaries"][0]
    if packet["packet_type"] == "resource.request" and packet["payload"]["resource_type"] == "iosSimulator":
        return [route_packet(packet_id = packet["packet_id"], target = "role:simulator-steward")]
    return []
""",
)

hook_binding(
    name = "on_turn_complete",
    source = """
def hook(ctx):
    intents = []
    for lease in ctx["resource_lease_summaries"]:
        if lease["resource_type"] == "iosSimulator" and lease["status"] == "assigned":
            intents.append(notify_session(message = "Simulator lease is idle after turn completion."))
    return intents
""",
)
