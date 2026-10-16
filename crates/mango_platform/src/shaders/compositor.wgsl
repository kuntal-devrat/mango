enable wgpu_binding_array;

struct PushConstants {
    rect: vec4<f32>,
    uv_rect: vec4<f32>,
    tex_index: u32,
    sampler_index: u32,
    flags: u32,
    opacity: f32,
}
var<immediate> pc: PushConstants;

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VertexOutput {
    var out: VertexOutput;
    let idx = vid % 6u;
    var pos_table = array<vec2<f32>, 6>(
        vec2<f32>(pc.rect.x, pc.rect.y),
        vec2<f32>(pc.rect.x, pc.rect.w),
        vec2<f32>(pc.rect.z, pc.rect.w),
        vec2<f32>(pc.rect.x, pc.rect.y),
        vec2<f32>(pc.rect.z, pc.rect.w),
        vec2<f32>(pc.rect.z, pc.rect.y),
    );
    var uv_table = array<vec2<f32>, 6>(
        vec2<f32>(pc.uv_rect.x, pc.uv_rect.y),
        vec2<f32>(pc.uv_rect.x, pc.uv_rect.w),
        vec2<f32>(pc.uv_rect.z, pc.uv_rect.w),
        vec2<f32>(pc.uv_rect.x, pc.uv_rect.y),
        vec2<f32>(pc.uv_rect.z, pc.uv_rect.w),
        vec2<f32>(pc.uv_rect.z, pc.uv_rect.y),
    );
    out.pos = vec4<f32>(pos_table[idx], 0.0, 1.0);
    out.uv = uv_table[idx];
    return out;
}

@group(0) @binding(1)
var textures: binding_array<texture_2d<f32>>;
@group(0) @binding(2)
var samplers: binding_array<sampler>;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(textures[pc.tex_index], samplers[pc.sampler_index], in.uv);
    return color * pc.opacity;
}
