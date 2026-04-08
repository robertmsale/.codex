import 'package:flutter/material.dart';

ThemeData buildRobdexTheme() {
  const shell = Color(0xFF0B1017);
  const panel = Color(0xFF101821);
  const accent = Color(0xFFEBA434);
  const text = Color(0xFFE8EEF5);
  const stroke = Color(0xFF243445);

  final base = ThemeData.dark(useMaterial3: true);

  return base.copyWith(
    scaffoldBackgroundColor: shell,
    colorScheme: const ColorScheme.dark(
      primary: accent,
      secondary: Color(0xFF53D4A5),
      surface: panel,
      onSurface: text,
      outline: stroke,
    ),
    textTheme: base.textTheme.apply(
      bodyColor: text,
      displayColor: text,
      fontFamily: '.AppleSystemUIFont',
    ),
    textButtonTheme: TextButtonThemeData(
      style: TextButton.styleFrom(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
        minimumSize: const Size(0, 30),
        textStyle: const TextStyle(
          fontSize: 11,
          fontWeight: FontWeight.w500,
        ),
      ),
    ),
    filledButtonTheme: FilledButtonThemeData(
      style: FilledButton.styleFrom(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
        minimumSize: const Size(0, 32),
        textStyle: const TextStyle(
          fontSize: 11,
          fontWeight: FontWeight.w600,
        ),
      ),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
        minimumSize: const Size(0, 32),
        side: const BorderSide(color: stroke),
        textStyle: const TextStyle(
          fontSize: 11,
          fontWeight: FontWeight.w500,
        ),
      ),
    ),
    inputDecorationTheme: InputDecorationTheme(
      isDense: true,
      filled: true,
      fillColor: const Color(0xFF0F1720),
      contentPadding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
      border: OutlineInputBorder(
        borderRadius: BorderRadius.circular(8),
        borderSide: const BorderSide(color: stroke),
      ),
      enabledBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(8),
        borderSide: const BorderSide(color: stroke),
      ),
      focusedBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(8),
        borderSide: const BorderSide(color: accent),
      ),
      labelStyle: const TextStyle(
        color: const Color(0xFF9FB0C2),
        fontSize: 11,
      ),
      hintStyle: const TextStyle(
        color: const Color(0xFF7E8EA0),
        fontSize: 11,
      ),
    ),
    cardTheme: const CardThemeData(
      color: Colors.transparent,
      margin: EdgeInsets.zero,
      elevation: 0,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.zero,
        side: BorderSide(color: Colors.transparent),
      ),
    ),
    chipTheme: base.chipTheme.copyWith(
      backgroundColor: const Color(0xFF182330),
      side: const BorderSide(color: stroke),
      labelStyle: const TextStyle(
        fontFamily: 'monospace',
        color: const Color(0xFF90A2B5),
        fontSize: 10,
        fontWeight: FontWeight.w500,
      ),
    ),
    dividerTheme: const DividerThemeData(
      color: stroke,
      thickness: 1,
      space: 1,
    ),
  );
}
