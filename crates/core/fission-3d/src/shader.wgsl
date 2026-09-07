struct SceneUniforms {
    view_projection: mat4x4<f32>,
    camera_position: vec4<f32>,
    light_direction: vec4<f32>,
    light_color: vec4<f32>,
    lighting: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> scene: SceneUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) world_position: vec3<f32>,
    @location(2) normal: vec3<f32>,
};

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = scene.view_projection * vec4<f32>(model.position, 1.0);
    out.color = model.color;
    out.world_position = model.position;
    out.normal = model.normal;

    return out;
}

fn normalized_or(value: vec3<f32>, fallback: vec3<f32>) -> vec3<f32> {
    let length_squared = dot(value, value);
    if length_squared > 1e-8 {
        return value * inverseSqrt(length_squared);
    }
    return fallback;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalized_or(in.normal, vec3<f32>(0.0, 1.0, 0.0));
    let light = normalized_or(scene.light_direction.xyz, vec3<f32>(0.0, 1.0, 0.0));
    let view = normalized_or(
        scene.camera_position.xyz - in.world_position,
        vec3<f32>(0.0, 0.0, 1.0),
    );
    let halfway = normalized_or(light + view, normal);
    let diffuse = max(dot(normal, light), 0.0) * scene.lighting.y;
    let specular = pow(max(dot(normal, halfway), 0.0), scene.lighting.w)
        * scene.lighting.z
        * select(0.0, 1.0, diffuse > 0.0);
    let illumination = vec3<f32>(scene.lighting.x) + scene.light_color.rgb * diffuse;
    let rgb = in.color.rgb * illumination + scene.light_color.rgb * specular;
    return vec4<f32>(rgb, in.color.a);
}
