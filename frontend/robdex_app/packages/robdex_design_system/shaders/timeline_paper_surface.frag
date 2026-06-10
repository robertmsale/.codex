#include <flutter/runtime_effect.glsl>

uniform vec2 uSize;
uniform float uTime;

out vec4 fragColor;

float hash(vec2 p) {
  return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123);
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
  vec2 uv = fragCoord / max(uSize, vec2(1.0));
  vec2 centered = uv - vec2(0.5);
  centered.x *= uSize.x / max(uSize.y, 1.0);

  float paperGrain = noise(fragCoord * 1.35);
  float fineGrain = hash(floor(fragCoord * 1.95));
  float compressedFiberH = noise(vec2(fragCoord.x * 0.055 + noise(fragCoord * 0.018) * 7.0, fragCoord.y * 8.6));
  float compressedFiberV = noise(vec2(fragCoord.x * 7.4, fragCoord.y * 0.048 + noise(fragCoord * 0.021) * 5.0));
  float shortFibers = noise(vec2(fragCoord.x * 0.42, fragCoord.y * 2.2 + noise(fragCoord * 0.075) * 2.0));
  float pinPores = smoothstep(0.965, 1.0, hash(floor(fragCoord * 0.72)))
      - smoothstep(0.0, 0.035, hash(floor(fragCoord * 1.18 + vec2(13.0, 29.0))));
  float broadTone = noise(uv * vec2(2.1, 2.4) + vec2(5.0, -2.0));

  float texture = 0.0;
  texture += (paperGrain - 0.5) * 0.0065;
  texture += (fineGrain - 0.5) * 0.0045;
  texture += (compressedFiberH - 0.5) * 0.0040;
  texture += (compressedFiberV - 0.5) * 0.0030;
  texture += (shortFibers - 0.5) * 0.0028;
  texture += pinPores * 0.0032;
  texture += (broadTone - 0.5) * 0.0022;

  float softReadLift = 1.0 - smoothstep(0.10, 0.92, length(centered * vec2(0.72, 1.12)));
  float edgeShade = smoothstep(0.44, 1.04, length(centered));
  float topAir = 1.0 - smoothstep(0.0, 0.36, uv.y);

  vec3 graphitePaper = vec3(0.061, 0.073, 0.083);
  vec3 coolFiber = vec3(0.006, 0.010, 0.013) * (compressedFiberH - 0.42);
  vec3 warmFiber = vec3(0.006, 0.005, 0.003) * (compressedFiberV - 0.42);
  vec3 color = graphitePaper + coolFiber + warmFiber + vec3(texture);
  color += vec3(0.012, 0.016, 0.017) * softReadLift * 0.12;
  color += vec3(0.010, 0.014, 0.018) * topAir * 0.14;
  color *= 1.0 - edgeShade * 0.08;
  color *= 1.0 - smoothstep(0.88, 1.0, uv.y) * 0.035;

  fragColor = vec4(color, 0.62);
}
