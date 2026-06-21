import 'package:code_forge/code_forge.dart';
import 'package:flutter/foundation.dart';
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
  CodeForgeController? _codeController;
  AgentRuntimeCodeEditorSync? _sync;

  @override
  void initState() {
    super.initState();
    try {
      final codeController = CodeForgeController();
      _codeController = codeController;
      _sync = AgentRuntimeCodeEditorSync(
        widgetController: widget.controller,
        editorController: codeController,
        editorText: () => codeController.text,
        setEditorText: (value) => codeController.text = value,
      );
    } catch (_) {
      if (!kDebugMode) {
        rethrow;
      }
      _codeController = null;
      _sync = null;
    }
  }

  @override
  void didUpdateWidget(covariant AgentRuntimeCodeEditor oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != widget.controller) {
      _sync?.updateWidgetController(widget.controller);
    }
  }

  @override
  void dispose() {
    _sync?.dispose();
    _codeController?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final codeController = _codeController;
    if (codeController == null) {
      return Semantics(
        label: 'CodeForge role instructions editor unavailable',
        readOnly: true,
        child: ColoredBox(
          color: const Color(0xFF07101A),
          child: Padding(
            padding: const EdgeInsets.all(8),
            child: Text(
              'CodeForge editor is unavailable in this debug/test runtime. Rebuild the app with native CodeForge initialized before editing role instructions.',
              style: Theme.of(context).textTheme.bodySmall?.copyWith(color: const Color(0xFFFFC75F)),
            ),
          ),
        ),
      );
    }
    return Semantics(
      label: 'CodeForge role instructions editor',
      value: 'CodeForge active',
      textField: true,
      child: CodeForge(
        controller: codeController,
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
