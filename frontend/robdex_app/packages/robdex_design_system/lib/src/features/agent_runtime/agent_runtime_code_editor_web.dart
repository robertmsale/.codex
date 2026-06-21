import 'package:code_forge_web/code_forge_web.dart';
import 'package:flutter/material.dart';

import 'agent_runtime_code_editor_sync.dart';

class AgentRuntimeCodeEditor extends StatefulWidget {
  const AgentRuntimeCodeEditor({
    super.key,
    required this.controller,
    this.readOnly = false,
  });

  final TextEditingController controller;
  final bool readOnly;

  @override
  State<AgentRuntimeCodeEditor> createState() => _AgentRuntimeCodeEditorState();
}

class _AgentRuntimeCodeEditorState extends State<AgentRuntimeCodeEditor> {
  late final CodeForgeWebController _codeController;
  late final AgentRuntimeCodeEditorSync _sync;

  @override
  void initState() {
    super.initState();
    _codeController = CodeForgeWebController();
    _sync = AgentRuntimeCodeEditorSync(
      widgetController: widget.controller,
      editorController: _codeController,
      editorText: () => _codeController.text,
      setEditorText: (value) => _codeController.text = value,
    );
  }

  @override
  void didUpdateWidget(covariant AgentRuntimeCodeEditor oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != widget.controller) {
      _sync.updateWidgetController(widget.controller);
    }
  }

  @override
  void dispose() {
    _sync.dispose();
    _codeController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Semantics(
      label: 'CodeForge role instructions editor',
      value: 'CodeForge active',
      textField: true,
      child: CodeForgeWeb(
        controller: _codeController,
        readOnly: widget.readOnly,
        lineWrap: true,
        enableGutter: true,
        enableFolding: false,
        editorTheme: const {
          'root': TextStyle(color: Color(0xFFE5EDF8), backgroundColor: Color(0xFF07101A)),
        },
        textStyle: const TextStyle(fontSize: 12, color: Color(0xFFE5EDF8)),
        innerPadding: const EdgeInsets.all(8),
      ),
    );
  }
}
