#include <flutter/runtime_effect.glsl>

uniform vec2 uSize;
uniform float uTime;
uniform float uWarp;

out vec4 fragColor;

float hash(vec2 p) {
  return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123);
}

float noise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);

  return mix(
      mix(hash(i + vec2(0.0, 0.0)), hash(i + vec2(1.0, 0.0)), u.x),
      mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x),
      u.y);
}

float fbm(vec2 p) {
  float value = 0.0;
  float amplitude = 0.55;
  for (int i = 0; i < 5; i++) {
    value += amplitude * noise(p);
    p = mat2(1.6, 1.2, -1.2, 1.6) * p;
    amplitude *= 0.52;
  }
  return value;
}

vec3 nebulaField(vec2 uv, float time) {
  vec2 p = uv;
  vec2 driftA = vec2(time * 0.018, -time * 0.01);
  vec2 driftB = vec2(-time * 0.012, time * 0.016);

  float n1 = fbm((p * 2.2) + driftA);
  float n2 = fbm((p * 3.9) - driftB + (n1 * 1.3));
  float n3 = fbm((p * 6.4) + vec2(n2, n1) * 0.9);

  float density = smoothstep(0.38, 0.86, n1 * 0.7 + n2 * 0.45);
  float filament = smoothstep(0.46, 0.82, n2 + (n3 * 0.35));
  float contour = smoothstep(0.62, 0.9, abs(n3 - 0.56));

  vec3 blue = vec3(0.20, 0.34, 0.92);
  vec3 azure = vec3(0.24, 0.48, 0.96);
  vec3 cyan = vec3(0.34, 0.72, 0.96);
  vec3 violet = vec3(0.58, 0.22, 0.96);
  vec3 purple = vec3(0.82, 0.24, 0.94);
  vec3 magenta = vec3(1.0, 0.28, 0.78);
  vec3 rose = vec3(1.0, 0.42, 0.56);
  vec3 ember = vec3(1.0, 0.56, 0.24);
  vec3 gold = vec3(1.0, 0.72, 0.36);

  vec3 color = vec3(0.0);
  color += blue * density * 0.18;
  color += azure * smoothstep(0.34, 0.82, n1 + n2 * 0.18) * 0.18;
  color += cyan * filament * 0.16;
  color += violet * smoothstep(0.38, 0.84, n2) * 0.28;
  color += purple * smoothstep(0.45, 0.88, n2 + n3 * 0.22) * 0.38;
  color += magenta * contour * 0.34;
  color += rose * smoothstep(0.64, 0.93, n1 + n3 * 0.2) * 0.28;
  color += ember * smoothstep(0.76, 0.97, n2 + n3 * 0.14) * 0.14;
  color += gold * smoothstep(0.9, 1.02, n1 + n2 * 0.18 + n3 * 0.08) * 0.05;

  return color;
}

void main() {
  vec2 fragCoord = FlutterFragCoord().xy;
  vec2 uv = fragCoord / uSize;
  vec2 centered = uv - vec2(0.5, 0.46);
  centered.x *= uSize.x / uSize.y;

  float time = uTime;
  float zoomPhase = fract(time * 0.02);
  float zoomA = mix(0.9, 2.1, zoomPhase);
  float zoomB = mix(0.9, 2.1, fract(zoomPhase + 0.5));
  float fadeA = sin(zoomPhase * 3.14159265);
  float fadeB = sin(fract(zoomPhase + 0.5) * 3.14159265);

  vec2 fieldA = centered * zoomA;
  vec2 fieldB = centered * zoomB;
  fieldA += vec2(-0.78, -0.18);
  fieldB += vec2(0.66, 0.34);

  vec3 color = nebulaField(fieldA, time) * fadeA;
  color += nebulaField(fieldB, time + 11.0) * fadeB;

  float edge = smoothstep(0.24, 0.95, length(centered));
  float centerSuppression = smoothstep(0.1, 0.32, length(centered));
  float warpBoost = mix(1.0, 1.12, clamp(uWarp, 0.0, 1.0));

  color *= edge * centerSuppression * warpBoost;
  color *= 0.72;

  fragColor = vec4(color, 1.0);
}
