# RTK renderer reference notes

These notes document the required RTK inspection for the Agent Runtime native renderer work. RTK was used only as behavioral inspiration; Agent Runtime does not depend on RTK, does not shell out through RTK, and does not use Robdex shim paths.

Inspected files:

- `/Users/robertsale/.codex/tmp/rtk/src/cmds/system/tree.rs`
  - Observed the tree command's default noise exclusion behavior, summary-line filtering, and structure-preserving plaintext output approach.
  - Agent Runtime equivalent: `starlark_host::tree_list` now emits native plaintext via `render_tree_plaintext`, filters common generated/noise paths, and avoids a JSON envelope.
- `/Users/robertsale/.codex/tmp/rtk/src/cmds/system/constants.rs`
  - Observed the common generated/noise directory inventory used by tree-style output.
  - Agent Runtime equivalent: `HostKernel::is_tree_noise_entry` carries the native local exclusion list.
- `/Users/robertsale/.codex/tmp/rtk/src/core/runner.rs`
  - Observed separation between raw captured command output, filtered visible output, and tracking.
  - Agent Runtime equivalent: `output_renderers::RenderedOutput` keeps visible output separate from durable raw output artifacts.
- `/Users/robertsale/.codex/tmp/rtk/src/core/guard.rs`
  - Observed the never-worse output guard concept.
  - Agent Runtime equivalent: `output_renderers::never_worse` prevents compact output from exceeding raw token estimates.
- `/Users/robertsale/.codex/tmp/rtk/src/core/truncate.rs`
  - Observed named cap classes and explicit truncation limits.
  - Agent Runtime equivalent: `output_renderers.rs` has explicit visible byte/line limits and tree omission metadata.
- `/Users/robertsale/.codex/tmp/rtk/src/core/tee.rs`
  - Observed the pattern of preserving raw/full output separately while showing bounded visible output.
  - Agent Runtime equivalent: `execution_output_artifacts` remains the durable full-output store; tree.list stores full plaintext artifacts before returning compact visible text.

Implementation boundary:

- No RTK crate or binary dependency was added.
- No `rtk` shell invocation was added.
- No `/Users/robertsale/.codex/shim/*` or PATH shim dependency was added.
