#include <flutter/runtime_effect.glsl>

uniform vec2 uSize;
uniform float uTime;

out vec4 fragColor;

float hash(vec2 p) {
  return fract(sin(dot(p, vec2(41.17, 289.93))) * 43758.5453);
}

float noise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(
      mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x),
      mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x),
      u.y);
}

void main() {
  vec2 fragCoord = FlutterFragCoord().xy;
  vec2 uv = fragCoord / uSize;

  float horizontalGrain = noise(vec2(uv.y * 720.0, uTime * 0.025));
  float fineGrain = noise(vec2(uv.y * 2300.0, uv.x * 4.0));
  float broadBrush = noise(vec2(uv.y * 130.0, uv.x * 0.72 + 8.0));
  float longScratch = smoothstep(0.70, 1.0, noise(vec2(uv.y * 105.0, uv.x * 1.7 + 3.0)));
  float hairline = smoothstep(0.76, 1.0, noise(vec2(uv.y * 3000.0, 12.0)));
  float anisotropy = sin((uv.y * uSize.y) * 2.15) * 0.5 + 0.5;
  float anisotropyFine = sin((uv.y * uSize.y) * 6.8) * 0.5 + 0.5;

  float verticalSheen = smoothstep(0.0, 0.34, uv.x) * (1.0 - smoothstep(0.72, 1.0, uv.x));
  float edgeLeft = 1.0 - smoothstep(0.0, 0.11, uv.x);
  float edgeRight = smoothstep(0.72, 1.0, uv.x);
  float topGlow = 1.0 - smoothstep(0.0, 0.22, uv.y);
  float bottomShade = smoothstep(0.78, 1.0, uv.y);

  vec3 base = vec3(0.044, 0.057, 0.071);
  vec3 coolLift = vec3(0.10, 0.14, 0.17) * verticalSheen * 0.11;
  float brushedLines = (horizontalGrain - 0.5) * 0.024 + (fineGrain - 0.5) * 0.011 + (broadBrush - 0.5) * 0.018;
  brushedLines += (anisotropy - 0.5) * 0.011 + (anisotropyFine - 0.5) * 0.006;
  vec3 grain = vec3(brushedLines);
  vec3 scratches = vec3(longScratch * 0.008 + hairline * 0.005);
  vec3 edge = vec3(0.14, 0.18, 0.21) * edgeLeft * 0.05;
  edge += vec3(0.0, 0.0, 0.0) * edgeRight;
  vec3 warmRim = vec3(0.9, 0.58, 0.22) * edgeLeft * 0.008;
  vec3 top = vec3(0.16, 0.20, 0.24) * topGlow * 0.028;

  vec3 color = base + coolLift + grain + scratches + edge + warmRim + top;
  color *= 1.0 - bottomShade * 0.05;
  color *= 1.0 - edgeRight * 0.07;

  float alpha = 0.66;
  fragColor = vec4(color, alpha);
}
