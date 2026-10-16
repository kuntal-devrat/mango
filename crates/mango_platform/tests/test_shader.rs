#[test]
fn test_compile_wgsl_to_spirv() {
    let wgsl = r#"
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
    var uvs = array<vec2<f32>, 6>(
        vec2<f32>(pc.uv_rect.x, pc.uv_rect.y),
        vec2<f32>(pc.uv_rect.x, pc.uv_rect.w),
        vec2<f32>(pc.uv_rect.z, pc.uv_rect.w),
        vec2<f32>(pc.uv_rect.x, pc.uv_rect.y),
        vec2<f32>(pc.uv_rect.z, pc.uv_rect.w),
        vec2<f32>(pc.uv_rect.z, pc.uv_rect.y),
    );
    let uv = uvs[vid % 6u];
    out.uv = uv;
    let x = mix(pc.rect.x, pc.rect.z, select(0.0, 1.0, uv.x == pc.uv_rect.z));
    let y = mix(pc.rect.y, pc.rect.w, select(0.0, 1.0, uv.y == pc.uv_rect.w));
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
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
"#;

    let module = match naga::front::wgsl::parse_str(wgsl) {
        Ok(m) => m,
        Err(e) => {
            panic!("Parse error: {:?}", e);
        }
    };

    let caps = naga::valid::Capabilities::all();
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        caps,
    )
    .validate(&module)
    .expect("validation error");

    let mut options = naga::back::spv::Options::default();
    options.lang_version = (1, 3);
    let spv = naga::back::spv::write_vec(&module, &info, &options, None)
        .expect("spv generation error");

    println!("SPIR-V word count: {}", spv.len());
    assert!(!spv.is_empty());

    let instance = match lgpu::Instance::new() {
        Ok(inst) => inst,
        Err(e) => {
            println!("Skipping GPU device creation (no Vulkan driver found in CI): {:?}", e);
            return;
        }
    };
    let adapters = match instance.enumerate_adapters() {
        Ok(ad) => ad,
        Err(e) => {
            println!("Skipping GPU device creation (enumerate_adapters failed): {:?}", e);
            return;
        }
    };
    if !adapters.is_empty() {
        let (device, _queue) = adapters[0].request_device(&instance).expect("device");
        let shader = device.create_shader_spirv(&spv).expect("shader");
        let desc = lgpu::GraphicsPipelineDesc {
            label: Some("Mango Compositor Pipeline"),
            vs_entry: "vs_main",
            fs_entry: "fs_main",
            color_format: lgpu::TextureFormat::Bgra8Unorm,
            depth_format: None,
            depth_test: false,
            depth_write: false,
            sample_count: 1,
            primitive: lgpu::PrimitiveState::default(),
            blend: Some(lgpu::BlendState::ALPHA_BLENDING),
        };
        let _pipeline = device.create_graphics_pipeline_with_desc(&shader, &desc).expect("pipeline");
        println!("Successfully created lgpu graphics pipeline with ALPHA_BLENDING on real GPU!");
    }
}
