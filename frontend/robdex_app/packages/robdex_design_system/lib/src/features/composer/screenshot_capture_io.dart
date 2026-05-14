import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:screen_capturer/screen_capturer.dart';

Future<String?> captureRobdexScreenshot() async {
  if (defaultTargetPlatform != TargetPlatform.macOS) {
    return null;
  }

  final hasAccess = await screenCapturer.isAccessAllowed();
  if (!hasAccess) {
    await screenCapturer.requestAccess();
  }

  final directory = Directory('${Directory.systemTemp.path}/robdex/screenshots');
  if (!directory.existsSync()) {
    directory.createSync(recursive: true);
  }

  final imagePath =
      '${directory.path}/screenshot-${DateTime.now().millisecondsSinceEpoch}.png';
  final captured = await screenCapturer.capture(
    mode: CaptureMode.region,
    imagePath: imagePath,
    copyToClipboard: false,
  );
  final capturedPath = captured?.imagePath ?? imagePath;
  if (captured == null || !File(capturedPath).existsSync()) {
    return null;
  }
  return capturedPath;
}
