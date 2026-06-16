import 'package:flutter/material.dart';

import '../../core/models/agent_runtime_control_tower_models.dart';

class AgentRuntimeControlTower extends StatelessWidget {
  const AgentRuntimeControlTower({
    super.key,
    required this.data,
    required this.baseUrlController,
    required this.onConnect,
    required this.onPollStream,
    required this.onDisconnect,
  });

  final AgentRuntimeControlTowerData data;
  final TextEditingController baseUrlController;
  final VoidCallback onConnect;
  final VoidCallback onPollStream;
  final VoidCallback onDisconnect;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return DecoratedBox(
      decoration: const BoxDecoration(color: Color(0xFF05090F)),
      child: SafeArea(
        child: Column(
          children: [
            _StatusStrip(
              data: data,
              baseUrlController: baseUrlController,
              onConnect: onConnect,
              onPollStream: onPollStream,
              onDisconnect: onDisconnect,
            ),
            if (data.errorMessage case final error?)
              Container(
                width: double.infinity,
                margin: const EdgeInsets.fromLTRB(16, 0, 16, 10),
                padding: const EdgeInsets.all(10),
                decoration: BoxDecoration(
                  color: const Color(0xFF3A1217),
                  border: Border.all(color: const Color(0xFF92323D)),
                  borderRadius: BorderRadius.circular(10),
                ),
                child: Text(
                  error,
                  style: theme.textTheme.bodySmall?.copyWith(color: const Color(0xFFFFC9CF)),
                ),
              ),
            Expanded(
              child: Padding(
                padding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    SizedBox(
                      width: 260,
                      child: _Panel(
                        title: 'Sessions',
                        child: data.sessions.isEmpty
                            ? const _EmptyText('No sessions in projection')
                            : ListView.separated(
                                itemCount: data.sessions.length,
                                separatorBuilder: (_, _) => const SizedBox(height: 8),
                                itemBuilder: (context, index) => _SessionTile(data.sessions[index]),
                              ),
                      ),
                    ),
                    const SizedBox(width: 12),
                    Expanded(
                      child: _Panel(
                        title: 'Selected session stream',
                        child: data.timeline.isEmpty
                            ? const _EmptyText('No selected-session timeline in packet')
                            : ListView.separated(
                                itemCount: data.timeline.length,
                                separatorBuilder: (_, _) => const Divider(height: 16),
                                itemBuilder: (context, index) => _TimelineTile(data.timeline[index]),
                              ),
                      ),
                    ),
                    const SizedBox(width: 12),
                    SizedBox(
                      width: 300,
                      child: Column(
                        children: [
                          Expanded(
                            child: _Panel(
                              title: 'Action queue',
                              child: data.actions.isEmpty
                                  ? const _EmptyText('No Rust-projected action items')
                                  : ListView.separated(
                                      itemCount: data.actions.length,
                                      separatorBuilder: (_, _) => const SizedBox(height: 8),
                                      itemBuilder: (context, index) => _ActionTile(data.actions[index]),
                                    ),
                            ),
                          ),
                          const SizedBox(height: 12),
                          Expanded(
                            child: _Panel(
                              title: 'Controller packets',
                              child: ListView(
                                children: [
                                  ...data.controllerFacts.map(_FactRow.new),
                                  const SizedBox(height: 10),
                                  Text(
                                    'Recent outputs',
                                    style: theme.textTheme.labelMedium?.copyWith(color: Colors.white70),
                                  ),
                                  const SizedBox(height: 6),
                                  ...data.outputLog.map(
                                    (line) => Text(
                                      line,
                                      style: theme.textTheme.bodySmall?.copyWith(color: const Color(0xFF98A6B8)),
                                    ),
                                  ),
                                ],
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _StatusStrip extends StatelessWidget {
  const _StatusStrip({
    required this.data,
    required this.baseUrlController,
    required this.onConnect,
    required this.onPollStream,
    required this.onDisconnect,
  });

  final AgentRuntimeControlTowerData data;
  final TextEditingController baseUrlController;
  final VoidCallback onConnect;
  final VoidCallback onPollStream;
  final VoidCallback onDisconnect;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      padding: const EdgeInsets.all(16),
      child: Row(
        children: [
          Text(
            'Agent Runtime Control Tower',
            style: theme.textTheme.titleMedium?.copyWith(color: Colors.white, fontWeight: FontWeight.w700),
          ),
          const SizedBox(width: 16),
          _Chip(label: data.connectionState),
          const SizedBox(width: 8),
          _Chip(label: 'watermark ${data.watermarkLabel}'),
          const SizedBox(width: 16),
          Expanded(
            child: TextField(
              controller: baseUrlController,
              style: const TextStyle(color: Colors.white),
              decoration: const InputDecoration(
                labelText: 'Runtime base URL',
                isDense: true,
              ),
            ),
          ),
          const SizedBox(width: 10),
          FilledButton(onPressed: onConnect, child: const Text('Connect')),
          const SizedBox(width: 8),
          OutlinedButton(onPressed: onPollStream, child: Text('Poll${data.pendingRequestCount > 0 ? ' (${data.pendingRequestCount})' : ''}')),
          const SizedBox(width: 8),
          TextButton(onPressed: onDisconnect, child: const Text('Disconnect')),
        ],
      ),
    );
  }
}

class _Panel extends StatelessWidget {
  const _Panel({required this.title, required this.child});

  final String title;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: const Color(0xCC0B111A),
        border: Border.all(color: const Color(0xFF202B3A)),
        borderRadius: BorderRadius.circular(14),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(title, style: theme.textTheme.labelLarge?.copyWith(color: Colors.white)),
          const SizedBox(height: 10),
          Expanded(child: child),
        ],
      ),
    );
  }
}

class _SessionTile extends StatelessWidget {
  const _SessionTile(this.item);

  final AgentRuntimeSessionItem item;

  @override
  Widget build(BuildContext context) {
    return _DenseTile(title: item.title, subtitle: item.subtitle, trailing: item.status);
  }
}

class _TimelineTile extends StatelessWidget {
  const _TimelineTile(this.item);

  final AgentRuntimeTimelineItem item;

  @override
  Widget build(BuildContext context) {
    return _DenseTile(title: item.title, subtitle: item.subtitle, trailing: item.status);
  }
}

class _ActionTile extends StatelessWidget {
  const _ActionTile(this.item);

  final AgentRuntimeActionItem item;

  @override
  Widget build(BuildContext context) {
    return _DenseTile(title: item.title, subtitle: item.subtitle, trailing: item.kind);
  }
}

class _DenseTile extends StatelessWidget {
  const _DenseTile({required this.title, required this.subtitle, required this.trailing});

  final String title;
  final String subtitle;
  final String trailing;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return DecoratedBox(
      decoration: BoxDecoration(
        color: const Color(0xFF0F1722),
        borderRadius: BorderRadius.circular(10),
      ),
      child: Padding(
        padding: const EdgeInsets.all(10),
        child: Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(title, maxLines: 1, overflow: TextOverflow.ellipsis, style: theme.textTheme.bodyMedium?.copyWith(color: Colors.white)),
                  const SizedBox(height: 3),
                  Text(subtitle, maxLines: 2, overflow: TextOverflow.ellipsis, style: theme.textTheme.bodySmall?.copyWith(color: const Color(0xFF97A6BA))),
                ],
              ),
            ),
            const SizedBox(width: 8),
            _Chip(label: trailing),
          ],
        ),
      ),
    );
  }
}

class _FactRow extends StatelessWidget {
  const _FactRow(this.fact);

  final AgentRuntimeFact fact;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: Row(
        children: [
          Expanded(child: Text(fact.label, style: theme.textTheme.bodySmall?.copyWith(color: const Color(0xFF7D8DA3)))),
          Text(fact.value, style: theme.textTheme.bodySmall?.copyWith(color: Colors.white)),
        ],
      ),
    );
  }
}

class _Chip extends StatelessWidget {
  const _Chip({required this.label});

  final String label;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
      decoration: BoxDecoration(
        color: const Color(0xFF162235),
        borderRadius: BorderRadius.circular(999),
        border: Border.all(color: const Color(0xFF31435C)),
      ),
      child: Text(label, style: Theme.of(context).textTheme.labelSmall?.copyWith(color: const Color(0xFFC8D5E6))),
    );
  }
}

class _EmptyText extends StatelessWidget {
  const _EmptyText(this.text);

  final String text;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Text(
        text,
        textAlign: TextAlign.center,
        style: Theme.of(context).textTheme.bodySmall?.copyWith(color: const Color(0xFF7E8DA2)),
      ),
    );
  }
}
