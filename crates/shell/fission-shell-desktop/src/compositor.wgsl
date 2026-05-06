struct LayerUniform {
  rect: vec4<f32>,
  clip: vec4<f32>,
  clip_local: vec4<f32>,
  clip_shape: vec4<f32>,
  viewport_and_opacity: vec4<f32>,
  transform: mat4x4<f32>,
};

@group(0) @binding(0) var layer_tex: texture_2d<f32>;
@group(0) @binding(1) var layer_sampler: sampler;
@group(0) @binding(2) var<uniform> uniforms: LayerUniform;

struct VsOut {
  @builtin(position) position: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

fn rounded_rect_sdf(point: vec2<f32>, rect_min: vec2<f32>, rect_max: vec2<f32>, radius: f32) -> f32 {
  let rect_size = max(rect_max - rect_min, vec2<f32>(0.0, 0.0));
  let clamped_radius = min(radius, min(rect_size.x, rect_size.y) * 0.5);
  let center = (rect_min + rect_max) * 0.5;
  let half_size = max(rect_size * 0.5 - vec2<f32>(clamped_radius, clamped_radius), vec2<f32>(0.0, 0.0));
  let q = abs(point - center) - half_size;
  return length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - clamped_radius;
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VsOut {
  var local = array<vec2<f32>, 4>(
    vec2<f32>(0.0, 0.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 1.0),
  );

  let uv = local[vertex_index];
  let px = uniforms.rect.xy + uv * uniforms.rect.zw;
  let transformed = uniforms.transform * vec4<f32>(px, 0.0, 1.0);
  let ndc = vec2<f32>(
    (transformed.x / uniforms.viewport_and_opacity.x) * 2.0 - 1.0,
    1.0 - (transformed.y / uniforms.viewport_and_opacity.y) * 2.0,
  );

  var out: VsOut;
  out.position = vec4<f32>(ndc, 0.0, 1.0);
  out.uv = uv;
  return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
  let local_px = in.uv * uniforms.rect.zw;
  let clip_kind = uniforms.clip_shape.x;
  var clip_alpha = 1.0;
  if clip_kind > 0.5 {
    let clip_min = uniforms.clip_local.xy;
    let clip_max = uniforms.clip_local.xy + uniforms.clip_local.zw;
    if local_px.x < clip_min.x || local_px.y < clip_min.y || local_px.x > clip_max.x || local_px.y > clip_max.y {
      discard;
    }
    if clip_kind > 1.5 {
      let radius = uniforms.clip_shape.y;
      let distance = rounded_rect_sdf(local_px, clip_min, clip_max, radius);
      if distance > 1.0 {
        discard;
      }
      clip_alpha = 1.0 - smoothstep(-0.75, 0.75, distance);
    }
  }
  let color = textureSample(layer_tex, layer_sampler, in.uv);
  return vec4<f32>(color.rgb, color.a * uniforms.viewport_and_opacity.z * clip_alpha);
}
