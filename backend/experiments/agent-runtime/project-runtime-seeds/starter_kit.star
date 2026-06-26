# Agent Runtime starter-kit seed. The Rust runtime owns execution, storage,
# packet routing, server ports, image artifacts, and approval boundaries.
starter_kit = {
    "toolBundles": {
        "worker": ["file.head", "file.tail", "file.read_lines", "file.line_count", "file.search", "tree.list", "tree.find", "file.replace_exact", "git.status", "git.diff", "git.restore", "git.add", "git.commit", "tooling.request"],
        "designer": ["file.head", "file.tail", "file.read_lines", "file.line_count", "file.search", "tree.list", "tree.find", "image.capture_from_file", "image.describe", "tooling.request"],
        "qa": ["file.head", "file.tail", "file.read_lines", "file.line_count", "file.search", "tree.list", "tree.find", "server.status", "server.logs", "image.capture_from_file", "image.describe", "tooling.request"],
        "orchestrator": ["tooling.request", "project_runtime.request_change", "git.status", "git.diff"],
        "project-progenitor": ["tooling.request", "project_runtime.request_change", "file.head", "tree.list", "tree.find", "git.status", "git.diff"],
        "simulator-steward": ["tooling.request", "image.capture_from_file", "image.describe"],
        "operator-admin": ["tooling.request", "project_runtime.request_change", "server.start", "server.stop", "server.status", "server.logs"]
    }
}
