import 'package:flutter_test/flutter_test.dart';

void main() {
  test('flutterq surfaces a compact failing test even with noisy stdout', () {
    for (var index = 0; index < 12; index += 1) {
      print('noise-line-$index: this line should not drown the real failure');
    }

    final payload = List<String>.generate(
      8,
      (index) => 'payload-segment-$index',
    ).join(' | ');
    print('structured-noise: $payload');

    expect(
      2 + 2,
      5,
      reason: 'Intentional failure to evaluate flutterq test output shaping.',
    );
  });
}
