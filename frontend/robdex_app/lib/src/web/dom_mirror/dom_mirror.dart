// DOM mirror bridge controller.
//
// Enabled in web only when one of the following is true:
// - `--dart-define=ROBDEX_DOM_MIRROR=true`
// The web mirror is enabled by default so browser-based agents can inspect the
// workbench DOM even though Flutter renders the visible UI to canvas.
// Disable with query parameter `?robdexDomMirror=0` or
// `localStorage.robdexDomMirror === "0"` when manually debugging DOM noise.
export 'dom_mirror_stub.dart'
    if (dart.library.html) 'dom_mirror_web.dart';
