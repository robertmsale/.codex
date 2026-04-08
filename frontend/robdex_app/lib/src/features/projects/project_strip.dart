import 'package:flutter/material.dart';

import '../../core/models/workbench_models.dart';

class ProjectStrip extends StatelessWidget {
  const ProjectStrip({
    super.key,
    required this.projects,
    required this.onProjectSelected,
    required this.onCreateProject,
  });

  final List<ProjectItem> projects;
  final ValueChanged<String> onProjectSelected;
  final VoidCallback onCreateProject;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return SizedBox(
      height: 34,
      child: Row(
        children: [
          Expanded(
            child: ListView.separated(
              scrollDirection: Axis.horizontal,
              itemCount: projects.length,
              separatorBuilder: (_, _) => const SizedBox(width: 6),
              itemBuilder: (context, index) {
                final project = projects[index];
                return InkWell(
                  onTap: () => onProjectSelected(project.id),
                  child: Container(
                    padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
                    decoration: BoxDecoration(
                      border: Border(
                        bottom: BorderSide(
                          color: project.isSelected
                              ? theme.colorScheme.primary
                              : Colors.transparent,
                          width: 2,
                        ),
                      ),
                    ),
                    child: Text(
                      project.name,
                      style: theme.textTheme.bodySmall?.copyWith(
                        fontWeight: project.isSelected ? FontWeight.w700 : FontWeight.w500,
                        color: project.isSelected
                            ? theme.colorScheme.primary
                            : theme.colorScheme.onSurface,
                      ),
                    ),
                  ),
                );
              },
            ),
          ),
          const SizedBox(width: 8),
          OutlinedButton.icon(
            onPressed: onCreateProject,
            icon: const Icon(Icons.add, size: 14),
            label: const Text('Project'),
          ),
        ],
      ),
    );
  }
}
