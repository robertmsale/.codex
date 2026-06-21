import 'package:flutter/widgets.dart';
import 'package:code_forge/code_forge.dart' as code_forge;
import 'package:rinf/rinf.dart';

import 'src/bindings/bindings.dart';
import 'src/app/robdex_app.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await code_forge.RustLib.init();
  await initializeRust(assignRustSignal);
  runApp(const RobdexApp());
}
