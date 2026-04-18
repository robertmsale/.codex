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
  vec2 p = uv * 0.58;
  vec2 driftA = vec2(time * 0.018, -time * 0.01);
  vec2 driftB = vec2(-time * 0.012, time * 0.016);

  float n1 = fbm((p * 1.35) + driftA);
  float n2 = fbm((p * 2.25) - driftB + (n1 * 0.95));
  float n3 = fbm((p * 3.8) + vec2(n2, n1) * 0.6);
  float n4 = fbm((p * 0.92) + vec2(-time * 0.006, time * 0.004));
  float n5 = fbm((p * 4.8) + vec2(n2, n3) * 0.34);
  float colorFieldA = fbm((p * 0.72) + vec2(4.2, -2.4));
  float colorFieldB = fbm((p * 0.88) + vec2(-3.1, 5.3));

  float ridge = 1.0 - abs((n3 * 2.0) - 1.0);
  ridge = pow(clamp(ridge, 0.0, 1.0), 1.35);
  float basin = smoothstep(0.16, 0.78, n1 - (n2 * 0.18));
  float density = smoothstep(0.36, 0.88, n1 * 0.68 + n2 * 0.38 + n4 * 0.16);
  float filament = smoothstep(0.46, 0.8, n2 + (n3 * 0.16) + (n5 * 0.05));
  float contour = smoothstep(0.5, 0.86, ridge + (filament * 0.12));
  float crest = smoothstep(0.6, 0.9, n2 + ridge * 0.28 + n4 * 0.1);
  float spark = smoothstep(0.9, 1.08, n1 + n2 * 0.16 + ridge * 0.12);
  float warmMask = smoothstep(0.38, 0.72, colorFieldA + (colorFieldB * 0.18));
  float coolMask = smoothstep(0.34, 0.76, (1.0 - colorFieldA) + colorFieldB * 0.12);
  float violetMask = smoothstep(0.4, 0.74, colorFieldB + n4 * 0.12);

  vec3 blue = vec3(0.20, 0.34, 0.92);
  vec3 azure = vec3(0.24, 0.48, 0.96);
  vec3 cyan = vec3(0.34, 0.72, 0.96);
  vec3 violet = vec3(0.58, 0.22, 0.96);
  vec3 purple = vec3(0.82, 0.24, 0.94);
  vec3 magenta = vec3(1.0, 0.28, 0.78);
  vec3 rose = vec3(1.0, 0.42, 0.56);
  vec3 scarlet = vec3(1.0, 0.24, 0.22);
  vec3 ember = vec3(1.0, 0.56, 0.24);
  vec3 gold = vec3(1.0, 0.72, 0.36);
  vec3 amber = vec3(1.0, 0.82, 0.44);

  vec3 color = vec3(0.0);
  color += blue * basin * coolMask * 0.08;
  color += azure * smoothstep(0.34, 0.82, n1 + n2 * 0.1) * coolMask * 0.1;
  color += cyan * filament * coolMask * 0.09;
  color += violet * smoothstep(0.38, 0.84, n2) * violetMask * 0.14;
  color += purple * smoothstep(0.44, 0.88, n2 + n3 * 0.12) * violetMask * 0.16;
  color += magenta * contour * violetMask * 0.14;
  color += rose * crest * warmMask * 0.16;
  color += scarlet * smoothstep(0.74, 0.95, crest + spark * 0.12) * warmMask * 0.1;
  color += ember * smoothstep(0.7, 0.94, n2 + ridge * 0.1) * warmMask * 0.1;
  color += gold * spark * warmMask * 0.07;
  color += amber * smoothstep(0.9, 1.06, spark + ridge * 0.04) * warmMask * 0.04;

  color *= mix(0.9, 1.04, contour);
  color += vec3(ridge * 0.02, ridge * 0.012, ridge * 0.006);

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
