struct SceneUniforms {
    aspect: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

@group(0) @binding(0)
var<uniform> scene: SceneUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

fn rotate_y(p: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(p.x * c - p.z * s, p.y, p.x * s + p.z * c);
}

fn rotate_x(p: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(p.x, p.y * c - p.z * s, p.y * s + p.z * c);
}

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    var p = rotate_y(model.position, 0.65);
    p = rotate_x(p, -0.42);

    let camera_z = max(p.z + 6.0, 0.2);
    let aspect = max(scene.aspect, 0.1);
    let focal_length = 2.2;
    let near = 0.1;
    let far = 20.0;
    let depth = clamp((camera_z - near) / (far - near), 0.0, 1.0);

    out.clip_position = vec4<f32>(
        (p.x * focal_length) / aspect,
        p.y * focal_length,
        depth * camera_z,
        camera_z,
    );
    out.color = model.color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
