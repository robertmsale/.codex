import 'package:flutter/material.dart';

class AgentRuntimeCodeEditorSync {
  AgentRuntimeCodeEditorSync({
    required this.widgetController,
    required this.editorController,
    required this.editorText,
    required this.setEditorText,
  }) {
    setEditorText(widgetController.text);
    editorController.addListener(_syncOut);
    widgetController.addListener(_syncIn);
  }

  TextEditingController widgetController;
  final dynamic editorController;
  final String Function() editorText;
  final ValueChanged<String> setEditorText;
  bool _syncing = false;

  void updateWidgetController(TextEditingController nextController) {
    if (identical(widgetController, nextController)) {
      return;
    }
    widgetController.removeListener(_syncIn);
    widgetController = nextController;
    widgetController.addListener(_syncIn);
    _syncIn();
  }

  void _syncIn() {
    if (_syncing || editorText() == widgetController.text) {
      return;
    }
    _syncing = true;
    setEditorText(widgetController.text);
    _syncing = false;
  }

  void _syncOut() {
    final next = editorText();
    if (_syncing || widgetController.text == next) {
      return;
    }
    _syncing = true;
    widgetController.text = next;
    _syncing = false;
  }

  void dispose() {
    widgetController.removeListener(_syncIn);
    editorController.removeListener(_syncOut);
  }
}
