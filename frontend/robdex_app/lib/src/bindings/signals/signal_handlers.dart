part of 'signals.dart';

final assignRustSignal = <String, void Function(Uint8List, Uint8List)>{
  'HookToastSignal': (Uint8List messageBytes, Uint8List binary) {
    final message = HookToastSignal.bincodeDeserialize(messageBytes);
    final rustSignal = RustSignalPack(
      message,
      binary,
    );
    _hookToastSignalStreamController.add(rustSignal);
    HookToastSignal.latestRustSignal = rustSignal;
  },
  'ThreadHistoryStateSignal': (Uint8List messageBytes, Uint8List binary) {
    final message = ThreadHistoryStateSignal.bincodeDeserialize(messageBytes);
    final rustSignal = RustSignalPack(
      message,
      binary,
    );
    _threadHistoryStateSignalStreamController.add(rustSignal);
    ThreadHistoryStateSignal.latestRustSignal = rustSignal;
  },
  'WorkbenchStateSignal': (Uint8List messageBytes, Uint8List binary) {
    final message = WorkbenchStateSignal.bincodeDeserialize(messageBytes);
    final rustSignal = RustSignalPack(
      message,
      binary,
    );
    _workbenchStateSignalStreamController.add(rustSignal);
    WorkbenchStateSignal.latestRustSignal = rustSignal;
  },
};
