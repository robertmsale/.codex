import 'package:flutter/material.dart';

import '../../core/models/agent_runtime_workbench_models.dart';

class AgentRuntimeSessionControlPlane extends StatefulWidget {
  const AgentRuntimeSessionControlPlane({
    super.key,
    required this.data,
    required this.onClose,
    required this.onSave,
    required this.onCloseSession,
    required this.onArchiveSession,
    required this.onForkSession,
    required this.onCompact,
    required this.onGrantGodMode,
    required this.onRevokeGodMode,
    required this.onTerminateProcess,
    required this.onFlushProcess,
    required this.onInputProcess,
    required this.onApprove,
    required this.onDeny,
    required this.onResumeApproval,
    required this.onPreviewCommandRequest,
    required this.onApproveCommandRequest,
    required this.onDenyCommandRequest,
    required this.onApplyCommandRequest,
    required this.onSetRequirements,
  });

  final AgentRuntimeWorkbenchData data;
  final VoidCallback onClose;
  final void Function({required String sessionId, required String project, required String role, required String model, required String workdir, required String worktreeRoot, required String title, required String name, required bool tracked}) onSave;
  final ValueChanged<String> onCloseSession;
  final ValueChanged<String> onArchiveSession;
  final ValueChanged<String> onForkSession;
  final ValueChanged<String> onCompact;
  final ValueChanged<String> onGrantGodMode;
  final ValueChanged<String> onRevokeGodMode;
  final ValueChanged<String> onTerminateProcess;
  final ValueChanged<String> onFlushProcess;
  final void Function(String handle, String text) onInputProcess;
  final void Function(String approvalId, String reason) onApprove;
  final void Function(String approvalId, String reason) onDeny;
  final ValueChanged<String> onResumeApproval;
  final ValueChanged<String> onPreviewCommandRequest;
  final ValueChanged<String> onApproveCommandRequest;
  final ValueChanged<String> onDenyCommandRequest;
  final ValueChanged<String> onApplyCommandRequest;
  final ValueChanged<String> onSetRequirements;

  @override
  State<AgentRuntimeSessionControlPlane> createState() => _AgentRuntimeSessionControlPlaneState();
}

class _AgentRuntimeSessionControlPlaneState extends State<AgentRuntimeSessionControlPlane> {
  late final TextEditingController _title;
  late final TextEditingController _name;
  late final TextEditingController _workdir;
  late final TextEditingController _worktreeRoot;
  late final TextEditingController _reason;
  String _role = '';
  String _project = '';
  String _model = '';
  bool _tracked = true;

  @override
  void initState() {
    super.initState();
    final control = widget.data.selectedSessionControlPlane;
    _title = TextEditingController(text: control?.title ?? '');
    _name = TextEditingController(text: control?.name ?? '');
    _workdir = TextEditingController(text: control?.workdir ?? '');
    _worktreeRoot = TextEditingController(text: control?.worktreeRoot ?? '');
    _reason = TextEditingController(text: 'Reviewed by owner');
    _role = control?.roleId ?? '';
    _project = control?.projectKey ?? '';
    _model = control?.activeModel ?? '';
    _tracked = control?.tracked ?? true;
  }

  @override
  void dispose() {
    _title.dispose();
    _name.dispose();
    _workdir.dispose();
    _worktreeRoot.dispose();
    _reason.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final control = widget.data.selectedSessionControlPlane;
    if (control == null) {
      return const Scaffold(
        backgroundColor: Color(0xFF05090F),
        body: Center(child: Text('Select a session to open settings.')),
      );
    }
    final wide = MediaQuery.sizeOf(context).width >= 980;
    final content = wide
        ? Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Expanded(flex: 47, child: _leftColumn(control)),
            const SizedBox(width: 18),
            Expanded(flex: 53, child: _rightColumn(control)),
          ])
        : Column(children: [_leftColumn(control), const SizedBox(height: 18), _rightColumn(control)]);
    return Scaffold(
      backgroundColor: const Color(0xFF05090F),
      body: SafeArea(
        child: Container(
          decoration: const BoxDecoration(
            gradient: LinearGradient(begin: Alignment.topLeft, end: Alignment.bottomRight, colors: [Color(0xFF07111C), Color(0xFF04101A), Color(0xFF081B2B)]),
          ),
          child: Material(
            color: Colors.transparent,
            child: Column(
            children: [
              _header(control),
              Expanded(
                child: SingleChildScrollView(
                  padding: const EdgeInsets.fromLTRB(26, 18, 26, 28),
                  child: content,
                ),
              ),
              _quickActions(control),
            ],
            ),
          ),
        ),
      ),
    );
  }

  Widget _header(AgentRuntimeSelectedSessionControlPlane control) {
    return Container(
      padding: const EdgeInsets.fromLTRB(28, 20, 18, 18),
      decoration: const BoxDecoration(border: Border(bottom: BorderSide(color: Color(0x223B82F6)))),
      child: Row(children: [
        const Icon(Icons.terminal_rounded, color: Colors.white, size: 28),
        const SizedBox(width: 18),
        Expanded(child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Row(children: [
            const Text('Session Settings', style: TextStyle(color: Colors.white, fontSize: 24, fontWeight: FontWeight.w700)),
            const SizedBox(width: 10),
            _pill(control.status, Colors.green),
          ]),
          const SizedBox(height: 5),
          const Text('Manage and control this runtime session', style: TextStyle(color: Color(0xFFB8C4D3), fontSize: 14)),
        ])),
        IconButton.outlined(onPressed: widget.onClose, icon: const Icon(Icons.close_rounded, color: Colors.white)),
      ]),
    );
  }

  Widget _leftColumn(AgentRuntimeSelectedSessionControlPlane control) {
    return Column(children: [
      _section('Session Identity', [
        _factRow('Session ID', control.sessionId),
        _field('Title', _title),
        _select('Role', _role, _roleOptions(control), (value) => setState(() => _role = value)),
        _select('Model', _model, control.modelOptions.map((model) => model.id).where((id) => id.isNotEmpty).toList(growable: false), (value) => setState(() => _model = value)),
        _select('Project', _project, _projectOptions(), (value) => setState(() => _project = value)),
        _field('Workdir', _workdir),
        _field('Worktree Root', _worktreeRoot),
        _field('Name', _name),
        Material(
          color: Colors.transparent,
          child: SwitchListTile(
            value: _tracked,
            onChanged: (value) => setState(() => _tracked = value),
            contentPadding: EdgeInsets.zero,
            title: const Text('Tracked', style: TextStyle(color: Color(0xFFD7DFEA))),
          ),
        ),
        Align(
          alignment: Alignment.centerRight,
          child: FilledButton(
            onPressed: () => widget.onSave(sessionId: control.sessionId, project: _project, role: _role, model: _model, workdir: _workdir.text, worktreeRoot: _worktreeRoot.text, title: _title.text, name: _name.text, tracked: _tracked),
            child: const Text('Save changes'),
          ),
        ),
      ]),
      const SizedBox(height: 14),
      _section('Session Actions', [
        Wrap(spacing: 12, runSpacing: 12, children: [
          _actionTile(Icons.close_rounded, 'Close Session', 'Gracefully close and terminate', Colors.redAccent, () => widget.onCloseSession(control.sessionId)),
          _actionTile(Icons.archive_outlined, 'Archive Session', 'Hide from active sessions', Colors.orange, () => widget.onArchiveSession(control.sessionId)),
          _actionTile(Icons.call_split_rounded, 'Fork Session', 'Create a new session from a turn', Colors.purpleAccent, () => widget.onForkSession(control.sessionId)),
          _actionTile(Icons.copy_all_rounded, 'Duplicate Settings', 'Use current values for a new session', Colors.blueAccent, null),
        ]),
      ]),
      const SizedBox(height: 14),
      _section('Requirements Review', [
        _factRow('State', control.requirementsReview.active ? 'Active' : 'Inactive'),
        _factRow('Progress', control.requirementsReview.progressSummary),
        _factRow('Reviewer', control.requirementsReview.reviewerStatus),
        _factRow('Latest packet', control.requirementsReview.latestPacketStatus.isEmpty ? 'Unavailable' : control.requirementsReview.latestPacketStatus),
      ]),
    ]);
  }

  Widget _rightColumn(AgentRuntimeSelectedSessionControlPlane control) {
    return Column(children: [
      _processSection(control),
      const SizedBox(height: 14),
      _approvalSection(control),
      const SizedBox(height: 14),
      _commandSection(control),
    ]);
  }

  Widget _processSection(AgentRuntimeSelectedSessionControlPlane control) {
    final first = control.managedProcesses.isEmpty ? null : control.managedProcesses.first;
    return _section('Processes (${control.managedProcesses.length})', [
      for (final process in control.managedProcesses) _processRow(process),
      if (first != null) _detailPanel([
        _factRow('PID', first.pid.isEmpty ? 'Unavailable' : first.pid),
        _factRow('CWD', first.cwd),
        _factRow('Policy', 'End of turn: ${first.endOfTurnBehavior} · End of session: ${first.endOfSessionBehavior}'),
        _factRow('Output', first.latestOutputSummary),
      ]),
    ]);
  }

  Widget _approvalSection(AgentRuntimeSelectedSessionControlPlane control) => _section('Approvals (${control.approvals.length} pending)', [
        for (final approval in control.approvals)
          _decisionCard(approval.title, approval.contextSummary, approval.status, [
            _smallButton('Approve', Colors.green, approval.canDecide ? () => widget.onApprove(approval.id, _reason.text) : null),
            _smallButton('Deny', Colors.redAccent, approval.canDecide ? () => widget.onDeny(approval.id, _reason.text) : null),
            _smallButton('Resume', Colors.blueAccent, approval.canResume ? () => widget.onResumeApproval(approval.id) : null),
          ]),
      ]);

  Widget _commandSection(AgentRuntimeSelectedSessionControlPlane control) => _section('Command Requests (${control.commandRequests.length} pending)', [
        for (final request in control.commandRequests)
          _decisionCard(request.title, '${request.scopeSummary} · ${request.policySummary}', request.status, [
            _smallButton('Preview', Colors.blueAccent, request.canPreview ? () => widget.onPreviewCommandRequest(request.id) : null),
            _smallButton('Approve', Colors.green, request.canDecide ? () => widget.onApproveCommandRequest(request.id) : null),
            _smallButton('Deny', Colors.redAccent, request.canDecide ? () => widget.onDenyCommandRequest(request.id) : null),
            _smallButton('Apply', Colors.orange, request.canApply ? () => widget.onApplyCommandRequest(request.id) : null),
          ]),
      ]);

  Widget _quickActions(AgentRuntimeSelectedSessionControlPlane control) {
    return Container(
      padding: const EdgeInsets.fromLTRB(26, 18, 26, 22),
      decoration: const BoxDecoration(border: Border(top: BorderSide(color: Color(0x223B82F6)))),
      child: Wrap(crossAxisAlignment: WrapCrossAlignment.center, spacing: 12, runSpacing: 10, children: [
        const Text('Quick Actions', style: TextStyle(color: Color(0xFFB8C4D3), fontWeight: FontWeight.w600)),
        _quick('Compact…', Colors.orange, () => widget.onCompact(control.sessionId)),
        _quick(control.godMode.active ? 'Revoke God Mode…' : 'Grant God Mode…', Colors.orange, () => control.godMode.active ? widget.onRevokeGodMode(control.sessionId) : widget.onGrantGodMode(control.sessionId)),
        _quick('Set Requirements…', Colors.blueAccent, () => widget.onSetRequirements(control.sessionId)),
        _quick('Export Bundle…', Colors.blueGrey, null),
        _quick('Danger Zone', Colors.redAccent, () => _showDanger(control)),
      ]),
    );
  }

  Widget _section(String title, List<Widget> children) => Container(
        width: double.infinity,
        padding: const EdgeInsets.all(18),
        decoration: BoxDecoration(color: const Color(0x66101A27), border: Border.all(color: const Color(0x263B82F6)), borderRadius: BorderRadius.circular(8)),
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(title, style: const TextStyle(color: Colors.white, fontSize: 18, fontWeight: FontWeight.w700)),
          const SizedBox(height: 16),
          ...children,
        ]),
      );

  Widget _field(String label, TextEditingController controller) {
    final keyName = label.toLowerCase().replaceAll(' ', '');
    return Padding(
        padding: const EdgeInsets.only(bottom: 12),
        child: Row(children: [
          SizedBox(width: 120, child: Text(label, style: const TextStyle(color: Color(0xFFB8C4D3)))),
          Expanded(child: TextField(key: ValueKey('agentRuntime.sessionControl.$keyName'), controller: controller, style: const TextStyle(color: Colors.white), decoration: _input())),
        ]),
      );
  }

  Widget _select(String label, String value, List<String> options, ValueChanged<String> onChanged) {
    final values = options.contains(value) ? options : [if (value.isNotEmpty) value, ...options];
    return Padding(
      padding: const EdgeInsets.only(bottom: 12),
      child: Row(children: [
        SizedBox(width: 120, child: Text(label, style: const TextStyle(color: Color(0xFFB8C4D3)))),
        Expanded(child: DropdownButtonFormField<String>(initialValue: values.isEmpty ? null : value, items: [for (final item in values) DropdownMenuItem(value: item, child: Text(item))], onChanged: (value) { if (value != null) onChanged(value); }, decoration: _input())),
      ]),
    );
  }

  InputDecoration _input() => const InputDecoration(isDense: true, filled: true, fillColor: Color(0x6607111C), border: OutlineInputBorder());

  Widget _factRow(String label, String value) => Padding(padding: const EdgeInsets.only(bottom: 10), child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [SizedBox(width: 120, child: Text(label, style: const TextStyle(color: Color(0xFFB8C4D3)))), Expanded(child: Text(value.isEmpty ? 'Unavailable' : value, style: const TextStyle(color: Color(0xFFE8EEF6))))]));
  Widget _pill(String text, Color color) => Container(padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 4), decoration: BoxDecoration(color: color.withValues(alpha: .18), borderRadius: BorderRadius.circular(7)), child: Text(text, style: TextStyle(color: color, fontSize: 12)));
  Widget _detailPanel(List<Widget> children) => Container(margin: const EdgeInsets.only(top: 8), padding: const EdgeInsets.all(12), decoration: BoxDecoration(border: Border.all(color: const Color(0x1FFFFFFF)), borderRadius: BorderRadius.circular(8)), child: Column(children: children));
  Widget _processRow(AgentRuntimeManagedProcessRow p) => Padding(padding: const EdgeInsets.symmetric(vertical: 8), child: Row(children: [Expanded(flex: 2, child: Text(p.handle, style: const TextStyle(color: Color(0xFFE8EEF6)))), Expanded(flex: 3, child: Text(p.command, style: const TextStyle(color: Color(0xFFE8EEF6)))), Expanded(child: _pill(p.status, p.status == 'running' ? Colors.green : Colors.orange)), IconButton(onPressed: p.canTerminate ? () => widget.onTerminateProcess(p.handle) : null, icon: const Icon(Icons.close_rounded, size: 18))]));
  Widget _decisionCard(String title, String subtitle, String status, List<Widget> actions) => Container(margin: const EdgeInsets.only(bottom: 12), padding: const EdgeInsets.all(14), decoration: BoxDecoration(border: Border.all(color: const Color(0x668B5CF6)), borderRadius: BorderRadius.circular(8)), child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [Row(children: [Expanded(child: Text(title, style: const TextStyle(color: Colors.white, fontWeight: FontWeight.w600))), _pill(status, Colors.purpleAccent)]), const SizedBox(height: 6), Text(subtitle, style: const TextStyle(color: Color(0xFFB8C4D3))), const SizedBox(height: 10), Wrap(spacing: 10, children: actions)]));
  Widget _smallButton(String label, Color color, VoidCallback? onPressed) => OutlinedButton(onPressed: onPressed, style: OutlinedButton.styleFrom(foregroundColor: color), child: Text(label));
  Widget _quick(String label, Color color, VoidCallback? onPressed) => Padding(padding: const EdgeInsets.only(right: 12), child: OutlinedButton(onPressed: onPressed, style: OutlinedButton.styleFrom(foregroundColor: color), child: Text(label)));
  Widget _actionTile(IconData icon, String title, String subtitle, Color color, VoidCallback? onTap) {
    final keyName = title.toLowerCase().replaceAll(' ', '');
    return SizedBox(width: 190, child: InkWell(key: ValueKey('agentRuntime.sessionControl.$keyName'), onTap: onTap, child: Container(padding: const EdgeInsets.all(14), decoration: BoxDecoration(color: const Color(0x3307111C), border: Border.all(color: const Color(0x1FFFFFFF)), borderRadius: BorderRadius.circular(7)), child: Row(children: [Icon(icon, color: color), const SizedBox(width: 10), Expanded(child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [Text(title, style: const TextStyle(color: Colors.white)), Text(subtitle, style: const TextStyle(color: Color(0xFFB8C4D3), fontSize: 12))]))]))));
  }

  List<String> _roleOptions(AgentRuntimeSelectedSessionControlPlane control) => {control.roleId, ...widget.data.roleAdmin.rows.map((role) => role.id)}.where((value) => value.isNotEmpty).toList(growable: false);
  List<String> _projectOptions() => widget.data.operationSurfaces.expand((surface) => surface.rows).where((fact) => fact.label == 'Project').map((fact) => fact.value).where((value) => value.isNotEmpty).toSet().toList(growable: false);

  void _showDanger(AgentRuntimeSelectedSessionControlPlane control) {
    showModalBottomSheet<void>(
      context: context,
      builder: (context) => SafeArea(
        child: Padding(
          padding: const EdgeInsets.all(18),
          child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
            const Text('Danger Zone', style: TextStyle(fontSize: 20, fontWeight: FontWeight.w700)),
            const SizedBox(height: 12),
            OutlinedButton(onPressed: () { Navigator.of(context).pop(); widget.onCloseSession(control.sessionId); }, child: const Text('Close session')),
            OutlinedButton(onPressed: () { Navigator.of(context).pop(); widget.onArchiveSession(control.sessionId); }, child: const Text('Archive session')),
          ]),
        ),
      ),
    );
  }
}
