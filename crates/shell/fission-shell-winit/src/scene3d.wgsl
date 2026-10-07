struct SceneUniforms {
    view_projection: mat4x4<f32>,
    camera_position: vec4<f32>,
    ambient: vec4<f32>,
    directional_color: vec4<f32>,
    directional_direction: vec4<f32>,
    point_position_range: array<vec4<f32>, 4>,
    point_color_intensity: array<vec4<f32>, 4>,
    point_count: vec4<u32>,
};

struct TransformUniforms {
    model: mat4x4<f32>,
    normal_model: mat4x4<f32>,
};

struct MaterialUniforms {
    base_color: vec4<f32>,
    emissive: vec4<f32>,
    factors: vec4<f32>,
    flags: vec4<u32>,
};

@group(0) @binding(0) var<uniform> scene: SceneUniforms;
@group(1) @binding(0) var<uniform> transform: TransformUniforms;
@group(2) @binding(0) var<uniform> material: MaterialUniforms;
@group(2) @binding(1) var base_color_texture: texture_2d<f32>;
@group(2) @binding(2) var emissive_texture: texture_2d<f32>;
@group(2) @binding(3) var base_color_sampler: sampler;
@group(2) @binding(4) var emissive_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    let world = transform.model * vec4<f32>(input.position, 1.0);
    output.clip_position = scene.view_projection * world;
    output.world_position = world.xyz;
    output.world_normal = normalize((transform.normal_model * vec4<f32>(input.normal, 0.0)).xyz);
    output.uv = input.uv;
    return output;
}

fn light_contribution(normal: vec3<f32>, world_position: vec3<f32>) -> vec3<f32> {
    var light = scene.ambient.rgb * scene.ambient.a;
    let directional = normalize(-scene.directional_direction.xyz);
    light += scene.directional_color.rgb * scene.directional_color.a
        * max(dot(normal, directional), 0.0);
    for (var index = 0u; index < min(scene.point_count.x, 4u); index += 1u) {
        let delta = scene.point_position_range[index].xyz - world_position;
        let distance = length(delta);
        let range = max(scene.point_position_range[index].w, 0.0001);
        let attenuation = pow(max(1.0 - distance / range, 0.0), 2.0);
        let diffuse = max(dot(normal, normalize(delta)), 0.0);
        light += scene.point_color_intensity[index].rgb
            * scene.point_color_intensity[index].a * attenuation * diffuse;
    }
    return light;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    var base = material.base_color;
    if ((material.flags.x & 2u) != 0u) {
        base *= textureSample(base_color_texture, base_color_sampler, input.uv);
    }
    if ((material.flags.x & 8u) != 0u && base.a < material.factors.z) {
        discard;
    }
    var emissive = material.emissive.rgb;
    if ((material.flags.x & 4u) != 0u) {
        emissive *= textureSample(emissive_texture, emissive_sampler, input.uv).rgb;
    }
    if ((material.flags.x & 1u) != 0u) {
        return vec4<f32>(base.rgb + emissive, base.a);
    }
    let normal = normalize(input.world_normal);
    let view_direction = normalize(scene.camera_position.xyz - input.world_position);
    let lighting = light_contribution(normal, input.world_position);
    let roughness = clamp(material.factors.y, 0.04, 1.0);
    let metallic = clamp(material.factors.x, 0.0, 1.0);
    let half_vector = normalize(view_direction + normalize(-scene.directional_direction.xyz));
    let specular_power = mix(128.0, 4.0, roughness);
    let specular = pow(max(dot(normal, half_vector), 0.0), specular_power);
    let dielectric = vec3<f32>(0.04);
    let specular_color = mix(dielectric, base.rgb, metallic);
    let diffuse = base.rgb * (1.0 - metallic) * lighting;
    let directional_specular = specular_color * specular
        * scene.directional_color.rgb * scene.directional_color.a;
    return vec4<f32>(diffuse + directional_specular + emissive, base.a);
}
