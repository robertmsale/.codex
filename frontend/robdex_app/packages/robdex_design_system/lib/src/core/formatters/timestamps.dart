String formatLocalTimeLabel(int? epochSeconds) {
  if (epochSeconds == null || epochSeconds <= 0) {
    return 'now';
  }
  final local = DateTime.fromMillisecondsSinceEpoch(
    epochSeconds * 1000,
    isUtc: true,
  ).toLocal();
  return _formatTime(local);
}

String formatLocalDateTimeLabel(int? epochSeconds) {
  if (epochSeconds == null || epochSeconds <= 0) {
    return 'now';
  }
  final local = DateTime.fromMillisecondsSinceEpoch(
    epochSeconds * 1000,
    isUtc: true,
  ).toLocal();
  return '${local.month}/${local.day} ${_formatTime(local)}';
}

String _formatTime(DateTime value) {
  final hour = value.hour % 12 == 0 ? 12 : value.hour % 12;
  final minute = value.minute.toString().padLeft(2, '0');
  final suffix = value.hour >= 12 ? 'PM' : 'AM';
  return '$hour:$minute $suffix';
}
