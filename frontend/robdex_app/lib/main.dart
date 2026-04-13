import 'package:flutter/widgets.dart';
import 'package:rinf/rinf.dart';

import 'src/bindings/bindings.dart';
import 'src/app/robdex_app.dart';

Future<void> main() async {
  await initializeRust(assignRustSignal);
  runApp(const RobdexApp());
}
