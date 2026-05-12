{{flutter_js}}
{{flutter_build_config}}

// Robdex is served by the local bridge and should always load the current
// bundle. Flutter's generated service worker can keep stale app code alive
// across rebuilds, which makes local operator debugging misleading.
if ('serviceWorker' in navigator) {
  navigator.serviceWorker.getRegistrations().then((registrations) => {
    for (const registration of registrations) {
      registration.unregister();
    }
  });
}

_flutter.loader.load();
