import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:file_selector/file_selector.dart';

import '../../core/models/workbench_models.dart';

typedef RequirementComposableLoader = Future<List<Map<String, dynamic>>> Function({
  String? senderThreadId,
  String? recipientThreadId,
  String? projectPath,
});

typedef ImageBytesUploader = Future<String> Function({
  required String filename,
  required String contentType,
  required Uint8List bytes,
});

const _severityOptions = ['blocker', 'high', 'medium', 'low'];
const _verificationOptions = [
  'diffReview',
  'screenshotReview',
  'testOutput',
  'commandOutput',
  'manualEvidence',
  'designComparison',
];

String requirementSetJsonFromReviewSummary(
  RequirementReviewSummary summary, {
  bool? active,
}) {
  final requirements = summary.requirements.map<Map<String, dynamic>>((requirement) {
    return {
      'key': requirement.key,
      'statement': requirement.statement,
      'severity': requirement.severity,
      'verificationMethod': requirement.verificationMethod,
    };
  }).toList(growable: false);
  return const JsonEncoder.withIndent('  ').convert({
    'id': summary.requirementSetId ?? '',
    'title': summary.requirementSetId ?? '',
    'active': active ?? summary.requirementSetActive,
    'enforceOnTurns': active ?? summary.requirementSetActive,
    'requirements': requirements,
  });
}

Future<String?> showRequirementSetFormDialog(
  BuildContext context, {
  String? initialJson,
  String title = 'Requirements',
  String actionLabel = 'Save',
  String helperText = 'Define the requirements. Robdex will generate the JSON contract.',
  bool showActivationToggle = false,
  bool requirementsActive = false,
  String? senderThreadId,
  String? recipientThreadId,
  String? projectPath,
  List<Map<String, dynamic>>? initialComposableItems,
  RequirementComposableLoader? loadComposableItems,
  ImageBytesUploader? uploadImageBytes,
}) {
  return showDialog<String?>(
    context: context,
    builder: (context) => _RequirementSetFormDialog(
      initialJson: initialJson,
      title: title,
      actionLabel: actionLabel,
      helperText: helperText,
      showActivationToggle: showActivationToggle,
      requirementsActive: requirementsActive,
      senderThreadId: senderThreadId,
      recipientThreadId: recipientThreadId,
      projectPath: projectPath,
      initialComposableItems: initialComposableItems,
      loadComposableItems: loadComposableItems,
      uploadImageBytes: uploadImageBytes,
    ),
  );
}

class _RequirementSetFormDialog extends StatefulWidget {
  const _RequirementSetFormDialog({
    required this.initialJson,
    required this.title,
    required this.actionLabel,
    required this.helperText,
    required this.showActivationToggle,
    required this.requirementsActive,
    required this.senderThreadId,
    required this.recipientThreadId,
    required this.projectPath,
    required this.initialComposableItems,
    required this.loadComposableItems,
    required this.uploadImageBytes,
  });

  final String? initialJson;
  final String title;
  final String actionLabel;
  final String helperText;
  final bool showActivationToggle;
  final bool requirementsActive;
  final String? senderThreadId;
  final String? recipientThreadId;
  final String? projectPath;
  final List<Map<String, dynamic>>? initialComposableItems;
  final RequirementComposableLoader? loadComposableItems;
  final ImageBytesUploader? uploadImageBytes;

  @override
  State<_RequirementSetFormDialog> createState() => _RequirementSetFormDialogState();
}

class _RequirementSetFormDialogState extends State<_RequirementSetFormDialog> {
  late final TextEditingController _titleController;
  final List<_RequirementDraft> _requirements = <_RequirementDraft>[];
  final Set<String> _selectedComposableIds = <String>{};
  List<_RequirementComposable> _composables = const <_RequirementComposable>[];
  bool _loadingComposables = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    final parsed = _parseInitial(widget.initialJson);
    _titleController = TextEditingController(text: parsed.$1);
    _requirements.addAll(parsed.$2);
    if (_requirements.isEmpty) {
      _requirements.add(_RequirementDraft());
    }
    final initialComposableItems = widget.initialComposableItems;
    if (initialComposableItems != null) {
      _composables = initialComposableItems
          .map(_RequirementComposable.fromJson)
          .toList(growable: false);
      _selectPermanentComposables(_composables);
      return;
    }
    _loadComposables();
  }

  @override
  void dispose() {
    _titleController.dispose();
    for (final requirement in _requirements) {
      requirement.dispose();
    }
    super.dispose();
  }

  (String, List<_RequirementDraft>) _parseInitial(String? jsonText) {
    if (jsonText == null || jsonText.trim().isEmpty) {
      return ('', <_RequirementDraft>[]);
    }
    try {
      final decoded = jsonDecode(jsonText);
      if (decoded is! Map<String, dynamic>) {
        return ('', <_RequirementDraft>[]);
      }
      final requirements = (decoded['requirements'] as List<dynamic>? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(_RequirementDraft.fromJson)
          .toList(growable: false);
      return (
        decoded['title'] as String? ?? '',
        requirements,
      );
    } catch (_) {
      return ('', <_RequirementDraft>[]);
    }
  }

  String _slugFromTitle(String title) {
    final words = RegExp(r'[A-Za-z0-9]+')
        .allMatches(title)
        .map((match) => match.group(0)!.toLowerCase())
        .where((word) => word.isNotEmpty)
        .toList(growable: false);
    if (words.isEmpty) {
      return 'requirements-${DateTime.now().millisecondsSinceEpoch}';
    }
    return words.join('-');
  }

  String _semanticKeyFromStatement(String statement, int index) {
    final words = RegExp(r'[A-Za-z0-9]+')
        .allMatches(statement)
        .map((match) => match.group(0)!)
        .where((word) => word.isNotEmpty)
        .take(7)
        .toList(growable: false);
    if (words.isEmpty) {
      return 'requirement${index + 1}';
    }
    final first = words.first.toLowerCase();
    final rest = words.skip(1).map((word) {
      final lower = word.toLowerCase();
      return lower[0].toUpperCase() + lower.substring(1);
    }).join();
    return '$first$rest';
  }

  String _statementWithReferences(_RequirementDraft draft) {
    final statement = draft.statement.text.trim();
    if (draft.referenceImagePaths.isEmpty) {
      return statement;
    }
    final references = draft.referenceImagePaths
        .map((path) => '- $path')
        .join('\n');
    return '$statement\n\nReference image paths for this requirement. The worker and reviewer must inspect these with `view_image` when relevant:\n$references';
  }

  String _generateJson({bool active = true}) {
    final requirements = <Map<String, dynamic>>[];
    final usedKeys = <String>{};
    for (final composable in _composables) {
      if (!_selectedComposableIds.contains(composable.id)) {
        continue;
      }
      for (final requirement in composable.requirements) {
        final key = requirement['key'] as String? ?? '';
        if (key.isEmpty) {
          throw StateError('Composable ${composable.id} has a requirement without a key.');
        }
        final existing = requirements.where((item) => item['key'] == key).toList(growable: false);
        if (existing.isNotEmpty) {
          if (mapEquals(existing.first, requirement)) {
            continue;
          }
          throw StateError('Composable requirement key conflict: $key.');
        }
        usedKeys.add(key);
        requirements.add(Map<String, dynamic>.from(requirement));
      }
    }
    for (var index = 0; index < _requirements.length; index += 1) {
      final draft = _requirements[index];
      final statement = _statementWithReferences(draft);
      var key = _semanticKeyFromStatement(draft.statement.text.trim(), index);
      var suffix = 2;
      while (usedKeys.contains(key)) {
        key = '${_semanticKeyFromStatement(draft.statement.text.trim(), index)}$suffix';
        suffix += 1;
      }
      usedKeys.add(key);
      if (!RegExp(r'^[a-z][A-Za-z0-9]*$').hasMatch(key)) {
        throw StateError('Requirement keys must be camelCase and start with a lowercase letter.');
      }
      if (statement.isEmpty) {
        throw StateError('Every requirement needs a statement.');
      }
      requirements.add({
        'key': key,
        'statement': statement,
        'severity': draft.severity,
        'verificationMethod': draft.verificationMethod,
      });
    }
    final title = _titleController.text.trim();
    return const JsonEncoder.withIndent('  ').convert({
      'id': _slugFromTitle(title),
      'title': title,
      'active': active,
      'enforceOnTurns': active,
      'includeComposables': _selectedComposableIds.toList(growable: false),
      'requirements': requirements,
    });
  }

  void _selectPermanentComposables(Iterable<_RequirementComposable> composables) {
    for (final composable in composables) {
      if (composable.permanent) {
        _selectedComposableIds.add(composable.id);
      }
    }
  }

  Future<void> _loadComposables() async {
    final loadComposableItems = widget.loadComposableItems;
    final senderThreadId = widget.senderThreadId;
    final recipientThreadId = widget.recipientThreadId;
    final projectPath = widget.projectPath;
    if (loadComposableItems == null ||
        ((recipientThreadId == null || recipientThreadId.trim().isEmpty) &&
            (projectPath == null || projectPath.trim().isEmpty))) {
      return;
    }
    setState(() => _loadingComposables = true);
    try {
      final items = await loadComposableItems(
        senderThreadId: senderThreadId,
        recipientThreadId: recipientThreadId,
        projectPath: projectPath,
      );
      final composables = items
          .map(_RequirementComposable.fromJson)
          .toList(growable: false);
      if (!mounted) {
        return;
      }
      setState(() {
        _composables = composables;
        _selectPermanentComposables(composables);
        _loadingComposables = false;
      });
    } catch (error) {
      if (!mounted) {
        return;
      }
      setState(() {
        _loadingComposables = false;
        _error = error.toString().replaceFirst('Bad state: ', '');
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return AlertDialog(
      title: Text(widget.title),
      content: SizedBox(
        width: 720,
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(widget.helperText),
              const SizedBox(height: 14),
              TextField(
                controller: _titleController,
                decoration: const InputDecoration(
                  labelText: 'Title',
                  hintText: 'Robdex frontend redesign',
                ),
              ),
              const SizedBox(height: 16),
              if (_loadingComposables)
                const LinearProgressIndicator()
              else if (_composables.isNotEmpty) ...[
                Text('Composable requirements', style: theme.textTheme.titleSmall),
                const SizedBox(height: 8),
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: [
                    for (final composable in _composables)
                      Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          FilterChip(
                            key: ValueKey('requirements.composable.${composable.id}'),
                            selected: _selectedComposableIds.contains(composable.id),
                            label: Text(
                              '${composable.id} (${composable.requirements.length})'
                              '${composable.permanent ? ' | permanent' : ''}',
                            ),
                            onSelected: composable.permanent ? null : (selected) {
                              setState(() {
                                if (selected) {
                                  _selectedComposableIds.add(composable.id);
                                } else {
                                  _selectedComposableIds.remove(composable.id);
                                }
                              });
                            },
                          ),
                          IconButton(
                            key: ValueKey('requirements.composable.inspect.${composable.id}'),
                            tooltip: 'Inspect composable',
                            icon: const Icon(Icons.info_outline, size: 18),
                            onPressed: () => _inspectComposable(composable),
                          ),
                        ],
                      ),
                  ],
                ),
                const SizedBox(height: 16),
              ],
              for (var i = 0; i < _requirements.length; i += 1) ...[
                _RequirementDraftCard(
                  index: i,
                  draft: _requirements[i],
                  uploadImageBytes: widget.uploadImageBytes,
                  onChanged: () => setState(() {}),
                  canRemove: _requirements.length > 1,
                  onRemove: () {
                    setState(() {
                      final removed = _requirements.removeAt(i);
                      removed.dispose();
                    });
                  },
                ),
                const SizedBox(height: 12),
              ],
              Align(
                alignment: Alignment.centerLeft,
                child: OutlinedButton.icon(
                  onPressed: () {
                    setState(() {
                      _requirements.add(_RequirementDraft());
                    });
                  },
                  icon: const Icon(Icons.add),
                  label: const Text('Add requirement'),
                ),
              ),
              if (_error != null) ...[
                const SizedBox(height: 12),
                Text(
                  _error!,
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: theme.colorScheme.error,
                  ),
                ),
              ],
            ],
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(null),
          child: const Text('Cancel'),
        ),
        TextButton(
          onPressed: () => Navigator.of(context).pop(''),
          child: const Text('Clear'),
        ),
        if (widget.showActivationToggle)
          TextButton(
            onPressed: () {
              try {
                Navigator.of(context).pop(_generateJson(active: !widget.requirementsActive));
              } catch (error) {
                setState(() {
                  _error = error.toString().replaceFirst('Bad state: ', '');
                });
              }
            },
            child: Text(widget.requirementsActive ? 'Deactivate' : 'Activate'),
          ),
        FilledButton(
          onPressed: () {
            try {
              Navigator.of(context).pop(_generateJson());
            } catch (error) {
              setState(() {
                _error = error.toString().replaceFirst('Bad state: ', '');
              });
            }
          },
          child: Text(widget.actionLabel),
        ),
      ],
    );
  }

  Future<void> _inspectComposable(_RequirementComposable composable) {
    return showDialog<void>(
      context: context,
      builder: (context) {
        return AlertDialog(
          title: Text(composable.title.isEmpty ? composable.id : composable.title),
          content: SizedBox(
            width: 560,
            child: SingleChildScrollView(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(composable.description.isEmpty ? composable.id : composable.description),
                  const SizedBox(height: 12),
                  for (final requirement in composable.requirements) ...[
                    Text(
                      requirement['key'] as String? ?? '',
                      style: Theme.of(context).textTheme.labelLarge,
                    ),
                    Text(requirement['statement'] as String? ?? ''),
                    const SizedBox(height: 10),
                  ],
                ],
              ),
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(context).pop(),
              child: const Text('Close'),
            ),
          ],
        );
      },
    );
  }
}

class _RequirementComposable {
  const _RequirementComposable({
    required this.id,
    required this.title,
    required this.description,
    required this.requirements,
    required this.permanent,
    required this.permanentSource,
  });

  final String id;
  final String title;
  final String description;
  final List<Map<String, dynamic>> requirements;
  final bool permanent;
  final String permanentSource;

  factory _RequirementComposable.fromJson(Map<String, dynamic> json) {
    final requirements = (json['requirements'] as List<dynamic>? ?? const [])
        .whereType<Map<String, dynamic>>()
        .map((item) {
          return {
            'key': item['key'],
            'statement': item['statement'],
            'severity': item['severity'],
            'verificationMethod': item['verificationMethod'],
          };
        })
        .toList(growable: false);
    return _RequirementComposable(
      id: json['id'] as String? ?? 'unknown',
      title: json['title'] as String? ?? '',
      description: json['description'] as String? ?? '',
      requirements: requirements,
      permanent: json['permanent'] == true,
      permanentSource: json['permanentSource'] as String? ?? '',
    );
  }
}

class _RequirementDraftCard extends StatelessWidget {
  const _RequirementDraftCard({
    required this.index,
    required this.draft,
    required this.uploadImageBytes,
    required this.onChanged,
    required this.canRemove,
    required this.onRemove,
  });

  final int index;
  final _RequirementDraft draft;
  final ImageBytesUploader? uploadImageBytes;
  final VoidCallback onChanged;
  final bool canRemove;
  final VoidCallback onRemove;

  Future<void> _attachReferenceImages(BuildContext context) async {
    try {
      final files = await openFiles(
        acceptedTypeGroups: const [
          XTypeGroup(
            label: 'Images',
            extensions: ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'heic', 'heif'],
          ),
        ],
      );
      if (files.isEmpty || !context.mounted) {
        return;
      }
      final paths = <String>[];
      final previews = <String, Uint8List>{};
      if (kIsWeb) {
        final upload = uploadImageBytes;
        if (upload == null) {
          throw StateError('Image upload is unavailable.');
        }
        for (final file in files) {
          final filename = file.name.trim().isEmpty ? 'requirement-reference.png' : file.name.trim();
          final bytes = await file.readAsBytes();
          final savedPath = await upload(
            filename: filename,
            contentType: _contentTypeFor(filename),
            bytes: bytes,
          );
          paths.add(savedPath);
          previews[savedPath] = bytes;
        }
      } else {
        for (final file in files) {
          final path = file.path;
          if (path == null || path.isEmpty) {
            continue;
          }
          paths.add(path);
          try {
            previews[path] = await file.readAsBytes();
          } catch (_) {
            // Keep the attachment usable even when preview bytes are unavailable.
          }
        }
      }
      for (final path in paths) {
        if (!draft.referenceImagePaths.contains(path)) {
          draft.referenceImagePaths.add(path);
        }
        final bytes = previews[path];
        if (bytes != null) {
          draft.referenceImagePreviewBytes[path] = bytes;
        }
      }
      onChanged();
    } catch (error) {
      if (!context.mounted) {
        return;
      }
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(error.toString().replaceFirst('Bad state: ', ''))),
      );
    }
  }

  String _contentTypeFor(String path) {
    final lower = path.toLowerCase();
    if (lower.endsWith('.jpg') || lower.endsWith('.jpeg')) return 'image/jpeg';
    if (lower.endsWith('.gif')) return 'image/gif';
    if (lower.endsWith('.webp')) return 'image/webp';
    if (lower.endsWith('.bmp')) return 'image/bmp';
    return 'image/png';
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return DecoratedBox(
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(14),
        border: Border.all(color: theme.colorScheme.outline.withValues(alpha: 0.5)),
        color: theme.colorScheme.surfaceContainerHighest.withValues(alpha: 0.18),
      ),
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                Text(
                  'Requirement ${index + 1}',
                  style: theme.textTheme.labelLarge?.copyWith(fontWeight: FontWeight.w700),
                ),
                const Spacer(),
                IconButton(
                  onPressed: canRemove ? onRemove : null,
                  tooltip: 'Remove requirement',
                  icon: const Icon(Icons.close_rounded),
                ),
              ],
            ),
            const SizedBox(height: 10),
            TextField(
              controller: draft.statement,
              minLines: 2,
              maxLines: 4,
              decoration: const InputDecoration(
                labelText: 'Statement',
                hintText: 'The UI must match the reference image on large screens.',
              ),
            ),
            const SizedBox(height: 10),
            Row(
              children: [
                Expanded(
                  child: DropdownButtonFormField<String>(
                    initialValue: draft.severity,
                    decoration: const InputDecoration(labelText: 'Severity'),
                    items: _severityOptions
                        .map((value) => DropdownMenuItem(value: value, child: Text(value)))
                        .toList(growable: false),
                    onChanged: (value) {
                      if (value != null) draft.severity = value;
                    },
                  ),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: DropdownButtonFormField<String>(
                    initialValue: draft.verificationMethod,
                    decoration: const InputDecoration(labelText: 'Verification'),
                    items: _verificationOptions
                        .map((value) => DropdownMenuItem(value: value, child: Text(value)))
                        .toList(growable: false),
                    onChanged: (value) {
                      if (value != null) draft.verificationMethod = value;
                    },
                  ),
                ),
              ],
            ),
            const SizedBox(height: 10),
            Align(
              alignment: Alignment.centerLeft,
              child: OutlinedButton.icon(
                onPressed: () => _attachReferenceImages(context),
                icon: const Icon(Icons.add_photo_alternate_outlined),
                label: const Text('Attach reference image'),
              ),
            ),
            if (draft.referenceImagePaths.isNotEmpty) ...[
              const SizedBox(height: 8),
              Wrap(
                spacing: 8,
                runSpacing: 8,
                children: [
                  for (final path in draft.referenceImagePaths)
                    InputChip(
                      avatar: _ReferenceImageAvatar(
                        bytes: draft.referenceImagePreviewBytes[path],
                      ),
                      label: Text(
                        path.split('/').last,
                        overflow: TextOverflow.ellipsis,
                      ),
                      tooltip: path,
                      onDeleted: () {
                        draft.referenceImagePaths.remove(path);
                        draft.referenceImagePreviewBytes.remove(path);
                        onChanged();
                      },
                    ),
                ],
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _ReferenceImageAvatar extends StatelessWidget {
  const _ReferenceImageAvatar({required this.bytes});

  final Uint8List? bytes;

  @override
  Widget build(BuildContext context) {
    final imageBytes = bytes;
    if (imageBytes == null || imageBytes.isEmpty) {
      return const Icon(Icons.image_outlined, size: 16);
    }
    return ClipOval(
      child: Image.memory(
        imageBytes,
        width: 24,
        height: 24,
        fit: BoxFit.cover,
        gaplessPlayback: true,
        errorBuilder: (_, _, _) => const Icon(Icons.image_outlined, size: 16),
      ),
    );
  }
}

class _RequirementDraft {
  _RequirementDraft({
    String statement = '',
    this.severity = 'blocker',
    this.verificationMethod = 'manualEvidence',
    List<String> initialReferenceImagePaths = const [],
  })  : statement = TextEditingController(text: _stripReferenceImageBlock(statement)),
        referenceImagePaths = initialReferenceImagePaths.isNotEmpty
            ? List<String>.of(initialReferenceImagePaths)
            : _referenceImagePathsFromStatement(statement);

  factory _RequirementDraft.fromJson(Map<String, dynamic> json) {
    final severity = json['severity'] as String? ?? 'blocker';
    final verificationMethod = json['verificationMethod'] as String? ?? 'manualEvidence';
    return _RequirementDraft(
      statement: json['statement'] as String? ?? '',
      severity: _severityOptions.contains(severity) ? severity : 'blocker',
      verificationMethod: _verificationOptions.contains(verificationMethod)
          ? verificationMethod
          : 'manualEvidence',
    );
  }

  final TextEditingController statement;
  final List<String> referenceImagePaths;
  final Map<String, Uint8List> referenceImagePreviewBytes =
      <String, Uint8List>{};
  String severity;
  String verificationMethod;

  void dispose() {
    statement.dispose();
  }
}

String _stripReferenceImageBlock(String statement) {
  final marker = '\n\nReference image paths for this requirement.';
  final index = statement.indexOf(marker);
  if (index < 0) {
    return statement;
  }
  return statement.substring(0, index).trimRight();
}

List<String> _referenceImagePathsFromStatement(String statement) {
  final lines = statement.split('\n');
  final result = <String>[];
  var inBlock = false;
  for (final line in lines) {
    if (line.startsWith('Reference image paths for this requirement.')) {
      inBlock = true;
      continue;
    }
    if (!inBlock) {
      continue;
    }
    final path = line.trim().replaceFirst(RegExp(r'^-\s*'), '').trim();
    if (path.startsWith('/')) {
      result.add(path);
    }
  }
  return result;
}
