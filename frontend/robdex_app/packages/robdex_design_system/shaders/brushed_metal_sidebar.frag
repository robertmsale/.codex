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

float brushedSegment(vec2 fragCoord, float rowHeight, float segmentWidth, float seed) {
  float rowPosition = fragCoord.y / rowHeight;
  float row = floor(rowPosition);
  float localY = fract(rowPosition);
  float lineProfile = smoothstep(0.08, 0.42, localY) * (1.0 - smoothstep(0.55, 0.96, localY));
  float rowJitter = (hash(vec2(row, seed)) - 0.5) * segmentWidth * 0.72;
  float segmentPosition = (fragCoord.x + rowJitter) / segmentWidth;
  float segment = floor(segmentPosition);
  float localX = fract(segmentPosition);
  float segmentOn = step(0.34, hash(vec2(row * 1.7 + seed, segment * 3.1 - seed)));
  float taper = smoothstep(0.04, 0.28, localX) * (1.0 - smoothstep(0.72, 0.98, localX));
  float lengthVariation = noise(vec2(fragCoord.x * 0.038 + seed, row * 11.0));
  return segmentOn * lineProfile * taper * mix(0.62, 1.08, lengthVariation);
}

float hairline(vec2 fragCoord, float spacing, float seed) {
  float rowPosition = (fragCoord.y + seed) / spacing;
  float localY = abs(fract(rowPosition) - 0.5);
  float line = 1.0 - smoothstep(0.018, 0.13, localY);
  float continuity = noise(vec2(fragCoord.x * 0.026 + seed, floor(rowPosition) * 2.7));
  return line * mix(0.45, 1.0, continuity);
}

void main() {
  vec2 fragCoord = FlutterFragCoord().xy;
  vec2 uv = fragCoord / max(uSize, vec2(1.0));

  float row = floor(fragCoord.y / 2.0);
  float rowOffset = (hash(vec2(row, 17.0)) - 0.5) * 30.0;
  float longBrush = noise(vec2((fragCoord.x + rowOffset) * 0.026, fragCoord.y * 7.6));
  float fineBrush = noise(vec2((fragCoord.x - rowOffset) * 0.052, fragCoord.y * 19.0));
  float valleyField = noise(vec2((fragCoord.x - rowOffset) * 0.070, fragCoord.y * 13.5));
  float crest = max(
    brushedSegment(fragCoord, 1.75, 46.0, 7.0) * 0.54,
    max(
      brushedSegment(fragCoord + vec2(17.0, 0.8), 2.65, 72.0, 29.0) * 0.38,
      brushedSegment(fragCoord + vec2(-11.0, 0.35), 1.20, 34.0, 53.0) * 0.28));
  float lightHair = hairline(fragCoord, 2.8, 5.0) * 0.32 + hairline(fragCoord, 5.6, 41.0) * 0.22;
  float darkHair = hairline(fragCoord + vec2(rowOffset * 0.15, 0.7), 3.4, 19.0);
  float scratchCut = smoothstep(0.72, 0.94, valleyField) * darkHair;

  float verticalSheen = smoothstep(0.0, 0.32, uv.x) * (1.0 - smoothstep(0.76, 1.0, uv.x));
  float edgeLeft = 1.0 - smoothstep(0.0, 0.10, uv.x);
  float edgeRight = smoothstep(0.74, 1.0, uv.x);
  float topGlow = 1.0 - smoothstep(0.0, 0.22, uv.y);
  float bottomShade = smoothstep(0.78, 1.0, uv.y);

  vec3 base = vec3(0.043, 0.056, 0.070);
  vec3 coolLift = vec3(0.10, 0.14, 0.17) * verticalSheen * 0.090;
  float brushedRelief = 0.0;
  brushedRelief += (longBrush - 0.5) * 0.015;
  brushedRelief += (fineBrush - 0.5) * 0.010;
  brushedRelief += crest * 0.018;
  brushedRelief += lightHair * 0.004;
  brushedRelief -= scratchCut * 0.022;

  vec3 grain = vec3(brushedRelief);
  vec3 coolCrestridge = vec3(0.018, 0.026, 0.031) * crest;
  vec3 darkValley = vec3(0.024, 0.031, 0.037) * scratchCut;
  vec3 edge = vec3(0.14, 0.18, 0.21) * edgeLeft * 0.05;
  vec3 warmRim = vec3(0.9, 0.58, 0.22) * edgeLeft * 0.007;
  vec3 top = vec3(0.16, 0.20, 0.24) * topGlow * 0.024;

  vec3 color = base + coolLift + grain + coolCrestridge - darkValley + edge + warmRim + top;
  color *= 1.0 - bottomShade * 0.05;
  color *= 1.0 - edgeRight * 0.07;

  float alpha = 0.70;
  fragColor = vec4(color, alpha);
}
