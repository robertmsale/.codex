# Agent Runtime Workbench Rinf Binding Record

The connected Agent Runtime GUI is a Robdex Workbench-compatible chat product. The Rinf boundary carries typed generated requests and typed generated outputs. Dart sends generated intent variants and renders Rust-shaped DTOs; Rust owns runtime validation, projection, settings, session/project/model semantics, operation enablement, and errors.

The binding exposes the Workbench shell snapshot with selected session identity, selected project identity, latest selected chat entries, modal surface summaries, pending actions, session rows, role/model/project options, and watermark/delta metadata. Selected chat is backed by turns, model events, tool calls, script/process rows, and output artifacts. Runtime audit events belong to History and Diagnostics modal surfaces.

Operational surfaces are modal or sheet affordances from the Workbench toolbar: Session, History, Diagnostics, Compaction, Statistics, Process Manager, Settings, Role Admin, Workflow Memory, Approvals, and Command Registry. The connected layout does not define or mount a permanent operations pane.

Generated Agent Runtime request variants include typed session creation, session settings, runtime settings, selected-session send, stop/close/archive/fork, process controls, approval decisions, command-registry actions, role administration, and workflow-memory feedback. The transport uses typed generated request and output variants for the connected UI.

Legacy note: older experiment names used dashboard terminology. That terminology is obsolete for product behavior. Current code and docs must describe the connected product as a Workbench shell with canonical ChatTimeline, Composer, left session rail, modal operations, typed Rinf operations, table-derived stats, project/model-aware sessions, selected-chat deltas, and lifecycle reconciliation.
