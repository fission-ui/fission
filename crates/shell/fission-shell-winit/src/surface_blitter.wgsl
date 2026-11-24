struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coords = vec2<f32>(
        f32((vi << 1u) & 2u),
        f32(vi & 2u),
    );
    out.position = vec4<f32>(out.tex_coords * 2.0 - 1.0, 0.0, 1.0);
    out.tex_coords.y = 1.0 - out.tex_coords.y;
    return out;
}

@group(0) @binding(0)
var source_texture: texture_2d<f32>;
@group(0) @binding(1)
var source_sampler: sampler;

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(source_texture, source_sampler, vertex.tex_coords);
}
