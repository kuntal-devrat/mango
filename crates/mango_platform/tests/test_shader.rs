#[test]
fn test_compile_wgsl_to_spirv() {
    let wgsl = include_str!("../src/shaders/compositor.wgsl");

    let module = match naga::front::wgsl::parse_str(wgsl) {
        Ok(m) => m,
        Err(e) => {
            panic!("Parse error: {:?}", e);
        }
    };

    let caps = naga::valid::Capabilities::all();
    let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), caps)
        .validate(&module)
        .expect("validation error");

    let options = naga::back::spv::Options {
        lang_version: (1, 3),
        ..Default::default()
    };
    let spv =
        naga::back::spv::write_vec(&module, &info, &options, None).expect("spv generation error");

    println!("SPIR-V word count: {}", spv.len());
    assert!(!spv.is_empty());

    let instance = match lgpu::Instance::new() {
        Ok(inst) => inst,
        Err(e) => {
            println!(
                "Skipping GPU device creation (no Vulkan driver found in CI): {:?}",
                e
            );
            return;
        }
    };
    let adapters = match instance.enumerate_adapters() {
        Ok(ad) => ad,
        Err(e) => {
            println!(
                "Skipping GPU device creation (enumerate_adapters failed): {:?}",
                e
            );
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
        let _pipeline = device
            .create_graphics_pipeline_with_desc(&shader, &desc)
            .expect("pipeline");
        println!("Successfully created lgpu graphics pipeline with ALPHA_BLENDING on real GPU!");
    }
}
