//! Headless pipeline validation using the same shader inputs and device features as the game.
use super::{
    cull, flat, oit, pack_atlas, pack_entities, pack_hand, pack_lines, pack_post, pack_sky,
    pack_weather, timing,
};
use horizon_shaders::{Compatibility, GameInputs, GbuffersInputs, Post};
use serde::Serialize;

// Shared helpers follow below; the renderer also calls them.

pub struct ValidationContext {
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    atlas: pack_atlas::PackAtlas,
    flat: bool,
    table: Option<wgpu::BindGroupLayout>,
    clouds: wgpu::BindGroupLayout,
}

#[derive(Serialize)]
pub struct BackendValidation {
    pub compatibility: Compatibility,
    pub reason: Option<String>,
    pub notes: Vec<String>,
}

impl ValidationContext {
    /// `backends` is exact: validation never silently switches to a different backend.
    pub fn open(backends: wgpu::Backends) -> Result<Self, String> {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = backends;
        let instance = wgpu::Instance::new(descriptor);
        let adapter = pollster::block_on(instance.request_adapter(&Default::default()))
            .map_err(|e| e.to_string())?;
        if !backends.contains(wgpu::Backends::from(adapter.get_info().backend)) {
            return Err("the requested backend was not selected".into());
        }
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Horizon compatibility validator"),
            required_features: device_features(&adapter),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Generated validator sprite"),
            size: wgpu::Extent3d {
                width: 16,
                height: 16,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        let atlas = pack_atlas::PackAtlas::new(&device, &mut encoder, &[texture], 1, 1, 16)
            .ok_or("the generated atlas is unavailable")?;
        queue.submit([encoder.finish()]);
        let flat = flat::wanted(&adapter, &device);
        let table = flat.then(|| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Validator production flat terrain inputs"),
                entries: &[0, 1, flat::ARENA_BINDING].map(|binding| wgpu::BindGroupLayoutEntry {
                    binding,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }),
            })
        });
        let clouds = device.create_bind_group_layout(&cloud_layout());
        Ok(Self {
            adapter,
            device,
            queue,
            atlas,
            flat,
            table,
            clouds,
        })
    }

    pub fn identity(&self) -> String {
        format!(
            "{:?}/{:?}/{:?}",
            self.adapter.get_info(),
            self.device.features(),
            self.device.limits()
        )
    }

    pub fn inputs(&mut self, separate_ao: bool) -> GameInputs {
        self.atlas.separate_ao = separate_ao;
        game_inputs(Some(self.atlas.terrain_inputs(self.flat)), self.flat, &[])
    }

    /// Synthetic resources validate layouts and compilation, not a real pack's rendered appearance.
    pub fn validate(&self, post: Post) -> BackendValidation {
        let vertex = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<super::TerrainVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &super::TERRAIN_ATTRIBUTES,
        };
        let instance_attributes = wgpu::vertex_attr_array![5=>Float32x4];
        let instances = wgpu::VertexBufferLayout {
            array_stride: 16,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &instance_attributes,
        };
        let layouts = [Some(vertex), Some(instances)];
        let rects = self.atlas.rects_table();
        let world = pack_post::World {
            atlas: Some((self.atlas.view.clone(), self.atlas.size())),
            sprite_rects: &rects,
            material_maps: None,
            lightmap: self.atlas.view.clone(),
            terrain: Some(pack_post::Geometry {
                buffers: if self.flat { &layouts[..1] } else { &layouts },
                table: self.table.as_ref(),
                depth_write: true,
                atlas: None,
                material_maps: true,
                ids: None,
                cull: true,
            }),
            block_ids: &[],
            celestial: Some(self.atlas.view.clone()),
            gui_size: [16, 16],
            clouds: Some(&self.clouds),
            weather: true,
            hand: true,
        };
        let hdr_output = post.hdr_output;
        match pack_post::PackPost::new_checked(
            &self.device,
            &self.queue,
            &self.adapter,
            post,
            if hdr_output {
                wgpu::TextureFormat::Rgba16Float
            } else {
                wgpu::TextureFormat::Rgba8Unorm
            },
            world,
            &Default::default(),
        ) {
            Ok(built) => BackendValidation {
                compatibility: built.compatibility,
                reason: None,
                notes: built.notes,
            },
            Err(refused) => BackendValidation {
                compatibility: refused.compatibility,
                reason: Some(refused.reason),
                notes: refused.notes,
            },
        }
    }
}

pub(super) fn device_features(adapter: &wgpu::Adapter) -> wgpu::Features {
    timing::GpuTimer::features(adapter)
        | cull::Cull::features(adapter)
        | oit::features(adapter)
        | (adapter.features()
            & (wgpu::Features::INDIRECT_FIRST_INSTANCE
                | wgpu::Features::MULTI_DRAW_INDIRECT_COUNT
                | wgpu::Features::MAPPABLE_PRIMARY_BUFFERS
                | wgpu::Features::TEXTURE_FORMAT_16BIT_NORM
                | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
                | wgpu::Features::RG11B10UFLOAT_RENDERABLE
                | wgpu::Features::SUBGROUP
                | wgpu::Features::VERTEX_WRITABLE_STORAGE
                | wgpu::Features::CLEAR_TEXTURE
                | wgpu::Features::TEXTURE_ATOMIC
                | wgpu::Features::FLOAT32_FILTERABLE))
}

pub(super) fn cloud_layout() -> wgpu::BindGroupLayoutDescriptor<'static> {
    wgpu::BindGroupLayoutDescriptor {
        label: Some("Cloud bindings"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    }
}

pub(super) fn game_inputs(terrain: Option<GbuffersInputs>, flat: bool, lava: &[u16]) -> GameInputs {
    // A geometry stage reads flat draws' vertices itself, through their index buffer.
    let terrain_geometry = terrain.clone().filter(|_| flat).map(|mut inputs| {
        inputs.declarations.push_str(
            "layout(set = 5, binding = 0) readonly buffer HzIndices { uint hz_indices[]; };\n",
        );
        horizon_shaders::GeometryInputs {
            inputs,
            pull: pack_atlas::PULL.to_owned(),
            index: "hz_indices[@]".to_owned(),
        }
    });
    horizon_shaders::GameInputs {
        terrain_parts: terrain.as_ref().map(|t| pack_atlas::terrain_parts(t, lava)),
        terrain,
        terrain_geometry,
        entity_geometry: Some(pack_entities::geometry_inputs()),
        sky: Some(pack_sky::inputs()),
        celestial: Some(pack_sky::celestial_inputs()),
        entities: Some(pack_entities::inputs()),
        clouds: Some(pack_entities::cloud_inputs()),
        weather: Some(pack_weather::inputs()),
        lines: Some(pack_lines::inputs()),
        hand: Some(pack_hand::inputs()),
    }
}

/// The same compiler capabilities for game selection and standalone validation.
pub fn define_macros(
    macros: &mut horizon_shaders::Macros,
    mipmaps: u32,
    features: wgpu::Features,
    hdr: bool,
) {
    macros.define_renderer(mipmaps);
    if hdr {
        macros.define("HORIZON_COMPAT_HDR_OUTPUT", "1");
    }
    if features.contains(wgpu::Features::SUBGROUP) {
        for ext in [
            "basic",
            "vote",
            "arithmetic",
            "ballot",
            "shuffle",
            "shuffle_relative",
        ] {
            macros.define(format!("MC_GL_KHR_shader_subgroup_{ext}"), "");
        }
    }
}
