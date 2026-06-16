import 'package:flutter/material.dart';

class RobdexModeDestination {
  const RobdexModeDestination({
    required this.label,
    required this.icon,
    required this.selectedIcon,
  });

  final String label;
  final IconData icon;
  final IconData selectedIcon;
}

class RobdexModeShell extends StatelessWidget {
  const RobdexModeShell({
    super.key,
    required this.selectedIndex,
    required this.destinations,
    required this.children,
    required this.onDestinationSelected,
  });

  final int selectedIndex;
  final List<RobdexModeDestination> destinations;
  final List<Widget> children;
  final ValueChanged<int> onDestinationSelected;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: IndexedStack(
        index: selectedIndex,
        children: children,
      ),
      bottomNavigationBar: NavigationBar(
        selectedIndex: selectedIndex,
        onDestinationSelected: onDestinationSelected,
        destinations: [
          for (final destination in destinations)
            NavigationDestination(
              icon: Icon(destination.icon),
              selectedIcon: Icon(destination.selectedIcon),
              label: destination.label,
            ),
        ],
      ),
    );
  }
}
