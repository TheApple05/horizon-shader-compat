//! Original complete-chain and strict-refusal controls; run serially per explicit backend.
use super::depth_test::{Gpu, SIZE, fixture_frame};
use super::*;
use crate::hud::HudBatch;
use std::{
    io::{Cursor, Write},
    path::Path,
};
use wgpu::{TextureFormat as T, util::DeviceExt};

#[test]
#[ignore = "requires an explicitly selected Vulkan or OpenGL adapter"]
fn explicit_hdr_handoff_retains_scene_range_and_pack_alpha() {
    use crate::gfx::hdr::{
        Hdr, Output,
        tests::{fill, texel},
    };
    let gpu = Gpu::new().expect("host GPU required");
    let color = gpu.texture(T::Rgba8Unorm);
    let depth = gpu.texture(T::Depth32Float);
    let hud = HudBatch::new(SIZE);
    let frame = fixture_frame(&hud);
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../horizon-shaders/tests/packs/compat-hdr");
    let pack = horizon_shaders::Pack::open(&path).unwrap();
    for enabled in [false, true, false] {
        let mut macros = horizon_shaders::Macros::standard();
        if enabled {
            macros.define("HORIZON_COMPAT_HDR_OUTPUT", "1");
        }
        let (properties, _) = horizon_shaders::read_properties(&pack, &mut macros);
        let post = horizon_shaders::load_post(&pack, "", &properties, &macros, &Default::default());
        let mut post = PackPost::new_checked(
            &gpu.device,
            &gpu.queue,
            &gpu.adapter,
            post,
            T::Rgba16Float,
            World {
                atlas: None,
                sprite_rects: &[],
                material_maps: None,
                lightmap: color.clone(),
                terrain: None,
                block_ids: &[],
                celestial: None,
                gui_size: SIZE,
                clouds: None,
                weather: false,
                hand: false,
            },
            &Default::default(),
        )
        .unwrap();
        let mut hdr = Hdr::new(
            &gpu.device,
            &gpu.queue,
            T::Rgba16Float,
            Output::ScRgb,
            enabled,
        );
        // Read the production scene attachment directly without changing the fixture's source.
        post.copied |= 1;
        hdr.paper_white = 80.0;
        hdr.peak = 800.0;
        hdr.black = 0.0;
        hdr.compat_sdr = true;
        hdr.lighting = false;
        let screen = hdr.frame(&gpu.device, SIZE);
        let screen_view = screen.create_view(&Default::default());
        gpu.clear(&color, &depth, 0.3);
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        post.run(
            &gpu.device,
            &gpu.queue,
            &mut encoder,
            &color,
            &depth,
            &screen_view,
            &frame,
            &frame.pack,
            SIZE,
            SIZE,
            &[],
            &mut |_, _, _, _| {},
        );
        let encoded = texel(&gpu.device, &gpu.queue, encoder, &screen);
        let linear: [f32; 3] = if enabled {
            [4.0, 1.0, 0.25]
        } else {
            [0.8, 0.5, 0.2]
        };
        for (got, expected) in encoded[..3].iter().zip(linear) {
            assert!(
                (got.powf(2.2) - expected).abs() < 0.008,
                "{enabled}: {encoded:?}"
            );
        }
        assert!(
            (encoded[3] - 0.375).abs() < 0.001,
            "pack alpha: {encoded:?}"
        );
        let targets = post.targets.as_ref().unwrap();
        let scene = targets.color[0].as_ref().unwrap()[usize::from(post.main & 1 != 0)].texture();
        let encoder = gpu.device.create_command_encoder(&Default::default());
        let raw = texel(&gpu.device, &gpu.queue, encoder, scene);
        assert_eq!(raw, [4.0, 1.0, 0.25, 0.375]);
        hdr.begin(&gpu.queue, true);
        let output = gpu.texture(T::Rgba16Float);
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        if let Some(view) = hdr.coverage_view() {
            crate::gfx::hud_coverage::clear(&mut encoder, &view);
        }
        hdr.tonemap(&mut encoder, None);
        hdr.keep_world(&mut encoder, None);
        hdr.present(&gpu.queue, &mut encoder, output.texture(), true, None);
        let presented = texel(&gpu.device, &gpu.queue, encoder, output.texture());
        for (got, expected) in presented[..3].iter().zip(linear) {
            assert!(
                (got - expected).abs() < 0.012,
                "presentation {enabled}: {presented:?}"
            );
        }
        eprintln!(
            "COMPAT_HDR enabled={enabled} raw={raw:?} encoded={encoded:?} scrgb={presented:?}"
        );
        if enabled {
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            fill(&mut encoder, hdr.hud().unwrap(), [1.0; 4]);
            fill(
                &mut encoder,
                hdr.coverage_view().unwrap().texture(),
                [1.0; 4],
            );
            hdr.present(&gpu.queue, &mut encoder, output.texture(), true, None);
            let ui = texel(&gpu.device, &gpu.queue, encoder, output.texture());
            assert!(
                ui[..3].iter().all(|v| (v - 1.0).abs() < 0.005),
                "HUD white: {ui:?}"
            );
        }
    }
}

fn pack(extra: Option<(&str, &str)>, gain: &str) -> horizon_shaders::Pack {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../horizon-shaders/tests/packs/compat-baseline");
    let source = horizon_shaders::Pack::open(&root).unwrap();
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for file in source.all_files() {
        if extra.is_some_and(|(name, _)| name == file) {
            continue;
        }
        zip.start_file(
            format!("shaders/{file}"),
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(&source.read(&file).unwrap()).unwrap();
    }
    if let Some((name, code)) = extra {
        zip.start_file(
            format!("shaders/{name}"),
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(code.as_bytes()).unwrap();
    }
    horizon_shaders::Pack::from_zip_bytes(
        "original-complete-chain",
        zip.finish().unwrap().into_inner(),
    )
    .unwrap()
    .with_chosen([("GAIN".into(), gain.into())].into())
}

#[test]
#[ignore = "requires an explicitly selected host Vulkan or OpenGL adapter; run serially"]
fn complete_chain_options_and_failure_recovery() {
    let gpu = Gpu::new().expect("an explicit host GPU is required");
    if let Ok(expected) = std::env::var("WGPU_BACKEND") {
        let actual = gpu.adapter.get_info().backend;
        assert!(
            matches!(
                (expected.as_str(), actual),
                ("vulkan", wgpu::Backend::Vulkan) | ("gl", wgpu::Backend::Gl)
            ),
            "requested {expected}, got {actual:?}"
        );
    }
    let color = gpu.texture(T::Rgba8Unorm);
    let depth = gpu.texture(T::Depth32Float);
    const ATTRIBUTES: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0=>Float32x3];
    let layouts = [Some(wgpu::VertexBufferLayout {
        array_stride: 12,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATTRIBUTES,
    })];
    let inputs = horizon_shaders::GameInputs {
        terrain: Some(horizon_shaders::GbuffersInputs {
            declarations: "layout(location=0) in vec3 fixture_position;".into(),
            body: "hz_Vertex=vec4(fixture_position,1.0);hz_Color=vec4(1.0);hz_Normal=vec3(0,0,1);"
                .into(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let world = World {
        atlas: None,
        sprite_rects: &[],
        material_maps: None,
        lightmap: color.clone(),
        terrain: Some(Geometry {
            buffers: &layouts,
            table: None,
            depth_write: true,
            atlas: None,
            material_maps: false,
            ids: None,
            cull: false,
        }),
        block_ids: &[],
        celestial: None,
        gui_size: SIZE,
        clouds: None,
        weather: false,
        hand: false,
    };
    let build = |pack: horizon_shaders::Pack, dimension| {
        let mut macros = horizon_shaders::Macros::standard();
        let (properties, _) = horizon_shaders::read_properties(&pack, &mut macros);
        let folder = horizon_shaders::dimension_folder(&pack, dimension);
        let post = horizon_shaders::load_post(&pack, &folder, &properties, &macros, &inputs);
        PackPost::new_checked(
            &gpu.device,
            &gpu.queue,
            &gpu.adapter,
            post,
            T::Rgba8Unorm,
            world.clone(),
            &Default::default(),
        )
    };
    let positions: [[f32; 3]; 6] = [
        [-1., -1., 0.],
        [1., -1., 0.],
        [1., 1., 0.],
        [-1., -1., 0.],
        [1., 1., 0.],
        [-1., 1., 0.],
    ];
    let vertices = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Original compatibility triangle pair"),
            contents: bytemuck::cast_slice(&positions),
            usage: wgpu::BufferUsages::VERTEX,
        });
    let hud = HudBatch::new(SIZE);
    let mut frame = fixture_frame(&hud);
    frame.pack.day_time = 12000;
    for dimension in [0, -1, 1] {
        for (gain, red) in [("1", 32u8), ("2", 64u8), ("1", 32u8)] {
            let mut post = build(pack(None, gain), dimension).unwrap_or_else(|e| panic!("{e:?}"));
            gpu.clear(&color, &depth, 0.3);
            let pixels = gpu.render(
                &mut post,
                &color,
                &depth,
                &frame,
                &mut |pass, list, _, _| {
                    if list == TerrainList::Solid {
                        pass.set_vertex_buffer(0, vertices.slice(..));
                        pass.draw(0..6, 0..1);
                    }
                },
            );
            let expected = [red, 128, 128, 255];
            assert!(
                pixels
                    .iter()
                    .all(|p| p.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 1)),
                "dimension {dimension}, gain {gain}: {:?}",
                pixels[0]
            );
            assert!(post.compatibility.failure().is_none());
            eprintln!(
                "COMPAT_CHAIN dimension={dimension} gain={gain} expected={expected:?} first={:?} pixels={}",
                pixels[0],
                pixels.len()
            );
        }
        let refused = build(
            pack(
                Some((
                    "composite.fsh",
                    "#version 120\n#include \"missing-original.glsl\"",
                )),
                "1",
            ),
            dimension,
        )
        .err()
        .expect("a required failed composite must refuse the pack");
        assert_eq!(
            refused.compatibility.failure().unwrap().requested,
            "composite"
        );
        assert!(
            build(pack(None, "1"), dimension).is_ok(),
            "editing a failed pack permits recovery"
        );
        let mut depth_only = build(
            pack(
                Some((
                    "gbuffers_terrain.fsh",
                    "#version 120\n/* DRAWBUFFERS:N */\nvoid main(){gl_FragDepth=0.25;}",
                )),
                "1",
            ),
            dimension,
        )
        .unwrap();
        gpu.clear(&color, &depth, 0.3);
        let pixels = gpu.render(
            &mut depth_only,
            &color,
            &depth,
            &frame,
            &mut |pass, list, _, _| {
                if list == TerrainList::Solid {
                    pass.set_vertex_buffer(0, vertices.slice(..));
                    pass.draw(0..6, 0..1);
                }
            },
        );
        assert!(
            pixels.iter().all(|p| p[1].abs_diff(64) <= 1),
            "depth-only: {:?}",
            pixels[0]
        );
        eprintln!(
            "COMPAT_DEPTH_ONLY dimension={dimension} expected_depth=0.25 first={:?}",
            pixels[0]
        );
    }
}

#[test]
#[ignore = "requires an explicit backend and HORIZON_SHADER_PACK_WITHOUT capability mask"]
fn required_compute_and_fragment_storage_refuse_when_forced_unavailable() {
    let without = std::env::var("HORIZON_SHADER_PACK_WITHOUT").unwrap_or_default();
    assert!(
        !without.is_empty(),
        "set HORIZON_SHADER_PACK_WITHOUT to compute or fragment-storage"
    );
    let gpu = Gpu::new().expect("an explicit host GPU is required");
    let (name, source, properties, capability) = if without == "compute" {
        (
            "composite.csh",
            "#version 430\nlayout(local_size_x=1) in;void main(){}",
            "",
            "compute",
        )
    } else {
        assert_eq!(without, "fragment-storage");
        (
            "composite.fsh",
            "#version 430 compatibility\nlayout(std430,binding=0) buffer OriginalState { uint value; };void main(){value=1u;gl_FragColor=vec4(0.25);}",
            "bufferObject.0=16",
            "fragment-storage",
        )
    };
    let mut p = pack(Some((name, source)), "1");
    if !properties.is_empty() {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for file in p.all_files() {
            zip.start_file(
                format!("shaders/{file}"),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            let bytes = if file == "shaders.properties" {
                properties.as_bytes().to_vec()
            } else {
                p.read(&file).unwrap()
            };
            zip.write_all(&bytes).unwrap();
        }
        p = horizon_shaders::Pack::from_zip_bytes(
            "original-capability-control",
            zip.finish().unwrap().into_inner(),
        )
        .unwrap();
    }
    let mut macros = horizon_shaders::Macros::standard();
    let (properties, _) = horizon_shaders::read_properties(&p, &mut macros);
    let post = horizon_shaders::load_post(&p, "", &properties, &macros, &Default::default());
    let color = gpu.texture(T::Rgba8Unorm);
    let world = World {
        atlas: None,
        sprite_rects: &[],
        material_maps: None,
        lightmap: color,
        terrain: None,
        block_ids: &[],
        celestial: None,
        gui_size: SIZE,
        clouds: None,
        weather: false,
        hand: false,
    };
    let refused = PackPost::new_checked(
        &gpu.device,
        &gpu.queue,
        &gpu.adapter,
        post,
        T::Rgba8Unorm,
        world,
        &Default::default(),
    )
    .err()
    .expect("a missing required GPU operation refuses the pack");
    assert!(
        refused
            .compatibility
            .programs
            .iter()
            .any(|p| p.capability.as_deref() == Some(capability)),
        "{refused:?}"
    );
    eprintln!("COMPAT_CAPABILITY_REJECTED {capability}");
}
