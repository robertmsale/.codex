#include <flutter/runtime_effect.glsl>

uniform vec2 uSize;
uniform sampler2D uTexture;

uniform vec2 uFocus;
uniform float uStart;
uniform float uEnd;
uniform float uBlurStrength;
uniform float uAberrationStrength;
uniform float uWarpStrength;

out vec4 fragColor;

vec4 sampleInput(vec2 uv) {
#ifdef IMPELLER_TARGET_OPENGLES
  uv.y = 1.0 - uv.y;
#endif
  return texture(uTexture, clamp(uv, 0.0, 1.0));
}

void main() {
  vec2 uv = FlutterFragCoord().xy / uSize;
  vec2 center = uFocus;
  vec2 delta = uv - center;
  vec2 viewportScale = uSize / min(uSize.x, uSize.y);
  vec2 ellipseDelta = delta * viewportScale;

  float dist = length(ellipseDelta / vec2(0.34, 0.34));
  float peripheral = smoothstep(uStart, uEnd, dist);
  float falloff = peripheral * peripheral * (3.0 - 2.0 * peripheral);
  float edgeBoost = smoothstep(0.78, 1.0, falloff);
  float warpRamp = min(1.0, (falloff * 0.74) + (edgeBoost * 0.36));

  vec2 direction = length(delta) > 0.0001 ? normalize(delta) : vec2(0.0, 0.0);
  vec2 warp = direction * (uWarpStrength * warpRamp);

  float blur = uBlurStrength * mix(falloff, warpRamp, 0.4);
  float chroma = uAberrationStrength * warpRamp;

  vec2 texel = 1.0 / uSize;
  vec2 tangent = vec2(-direction.y, direction.x);

  vec2 sampleUv = uv + warp;
  vec4 base = sampleInput(sampleUv);

  vec4 blurA = sampleInput(sampleUv + direction * texel * blur * 2.2);
  vec4 blurB = sampleInput(sampleUv - direction * texel * blur * 1.8);
  vec4 blurC = sampleInput(sampleUv + tangent * texel * blur * 1.05);
  vec4 blurD = sampleInput(sampleUv - tangent * texel * blur * 1.05);
  vec4 blurred = (base * 0.24) + (blurA * 0.24) + (blurB * 0.22) + (blurC * 0.15) + (blurD * 0.15);

  float red = sampleInput(sampleUv + direction * texel * chroma * 1.2).r;
  float green = blurred.g;
  float blue = sampleInput(sampleUv - direction * texel * chroma * 1.2).b;

  vec3 chromaColor = vec3(red, green, blue);
  vec3 color = mix(base.rgb, chromaColor, warpRamp * 0.03);
  color = mix(color, blurred.rgb, mix(falloff, warpRamp, 0.55) * 0.72);

  vec3 edgeTint = mix(
    vec3(1.0, 0.18, 0.42),
    vec3(0.22, 0.58, 1.0),
    clamp((direction.x * 0.5) + 0.5, 0.0, 1.0)
  );
  color = mix(color, color * edgeTint, warpRamp * 0.12);
  color *= 1.0 - (warpRamp * 0.12);

  fragColor = vec4(color, base.a);
}
