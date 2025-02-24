use std::collections::{HashMap, HashSet};

use wgpu::{
    BindGroup, ColorTargetState, CommandEncoder, Device, Extent3d, RenderPass,
    RenderPassColorAttachment, RenderPassDepthStencilAttachment, TextureDimension, TextureFormat,
    TextureUsages,
};

use crate::{
    bind_group_layout_descriptors,
    components::RenderableComponent,
    material::PbrMaterialDescriptor,
    model::{PbrRenderingType, Renderable},
    pipelines::ShaderCompilationSuccess,
    render_pipeline::{
        PipelineFragmentState, PipelineVertexState, RenderPipeline, RenderPipelineDescriptor,
        VertexBufferContent,
    },
    texture::{SampledTexture, SampledTextureDescriptor, SamplingType},
};

const SHADER_SOURCE_TEXTURED: &'static str = "src/shaders/gbuffer_geometry.wgsl";
const SHADER_SOURCE_FLAT_PARAMETER: &'static str =
    "src/shaders/gbuffer_geometry_flat_parameter.wgsl";

const GBUFFER_TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba16Float;
const GBUFFER_CLEAR_COLOR: wgpu::Color = wgpu::Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};

pub struct GBufferTextures {
    pub position: SampledTexture,
    pub normal: SampledTexture,
    pub albedo_and_specular: SampledTexture,
    pub depth_texture: SampledTexture,
    pub metal_rough_ao: SampledTexture,
}

struct PipelineWithObjects {
    render_pipeline: RenderPipeline,
    objects: HashSet<u32>,
}

impl PipelineWithObjects {
    fn new(render_pipeline: RenderPipeline) -> Self {
        Self {
            render_pipeline,
            objects: HashSet::new(),
        }
    }
}

fn default_color_write_state(format: wgpu::TextureFormat) -> ColorTargetState {
    wgpu::ColorTargetState {
        format,
        blend: Some(wgpu::BlendState {
            alpha: wgpu::BlendComponent::REPLACE,
            color: wgpu::BlendComponent::REPLACE,
        }),
        write_mask: wgpu::ColorWrites::ALL,
    }
}

pub struct GBufferGeometryRenderer {
    pub textures: GBufferTextures,
    pub gbuffer_textures_bind_group: wgpu::BindGroup,
    pub depth_texture_bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    render_pipelines: HashMap<GBufferRenderingParams, PipelineWithObjects>,
}

#[derive(Debug, Hash, PartialEq, Eq)]
struct GBufferRenderingParams {
    use_depth_test: bool,
    pbr_rendering_type: PbrRenderingType,
}

impl From<&RenderableComponent> for GBufferRenderingParams {
    fn from(renderable: &RenderableComponent) -> Self {
        let pbr_rendering_type = match renderable.model_descriptor.material_descriptor {
            PbrMaterialDescriptor::Texture(_) => PbrRenderingType::Textures,
            PbrMaterialDescriptor::Flat(_) => PbrRenderingType::FlatParameters,
        };

        Self {
            use_depth_test: renderable.rendering_options.use_depth_test,
            pbr_rendering_type,
        }
    }
}

impl GBufferGeometryRenderer {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let textures = Self::create_textures(device, width, height);
        let bind_group = Self::create_gbuffer_bind_group(device, &textures);

        let depth_texture_bind_group =
            Self::create_depth_bind_group(device, &textures.depth_texture);

        Self {
            textures,
            gbuffer_textures_bind_group: bind_group,
            width,
            height,
            render_pipelines: HashMap::new(),
            depth_texture_bind_group,
        }
    }

    pub fn add_renderable(
        &mut self,
        device: &Device,
        id: u32,
        renderable_component: &RenderableComponent,
    ) -> anyhow::Result<()> {
        let gbuffer_render_params = GBufferRenderingParams::from(renderable_component);
        if let Some(pipeline_with_objects) = self.render_pipelines.get_mut(&gbuffer_render_params) {
            pipeline_with_objects.objects.insert(id);
        } else {
            let pipeline =
                Self::create_render_pipeline(device, &gbuffer_render_params, &self.textures)?;
            self.render_pipelines
                .insert(gbuffer_render_params, PipelineWithObjects::new(pipeline));
        }

        Ok(())
    }

    pub fn remove_renderable(&mut self, id: &u32) {
        for pipeline_with_items in self.render_pipelines.values_mut() {
            if pipeline_with_items.objects.remove(id) {
                break;
            }
        }
    }

    pub fn try_recompile_shader(
        &mut self,
        device: &Device,
    ) -> anyhow::Result<ShaderCompilationSuccess> {
        let mut any_shader_changed = false;

        // If any of the shaders were actually recompiled, then return recompiled in the end
        for render_pipeline in self.render_pipelines.values_mut() {
            any_shader_changed = render_pipeline
                .render_pipeline
                .try_recompile_shader(device)?
                == ShaderCompilationSuccess::Recompiled
                || any_shader_changed;
        }

        if any_shader_changed {
            Ok(ShaderCompilationSuccess::Recompiled)
        } else {
            Ok(ShaderCompilationSuccess::AlreadyUpToDate)
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.textures = Self::create_textures(device, width, height);
        self.gbuffer_textures_bind_group = Self::create_gbuffer_bind_group(device, &self.textures);
        self.depth_texture_bind_group =
            Self::create_depth_bind_group(device, &self.textures.depth_texture);
        self.width = width;
        self.height = height;
    }

    fn create_textures(device: &wgpu::Device, width: u32, height: u32) -> GBufferTextures {
        let texture_extents = Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let descriptor = SampledTextureDescriptor {
            format: GBUFFER_TEXTURE_FORMAT,
            usages: TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING,
            extents: texture_extents,
            dimension: TextureDimension::D2,
            mip_count: 1,
            sampling_type: SamplingType::Nearest,
        };

        let position_texture =
            SampledTexture::new(device, descriptor.clone(), "GBuffer position texture");
        let normal_texture =
            SampledTexture::new(device, descriptor.clone(), "GBuffer normal texture");
        let albedo_and_specular_texture = SampledTexture::new(
            device,
            descriptor.clone(),
            "GBuffer albedo and specular texture",
        );
        let metal_rough_ao =
            SampledTexture::new(device, descriptor.clone(), "GBuffer metal+rough+ao texture");

        let depth_texture = SampledTexture::create_depth_texture(
            device,
            texture_extents,
            None,
            SamplingType::Nearest,
            "GBuffer depth texture",
        );

        GBufferTextures {
            position: position_texture,
            normal: normal_texture,
            albedo_and_specular: albedo_and_specular_texture,
            depth_texture,
            metal_rough_ao,
        }
    }

    fn create_render_pipeline(
        device: &Device,
        rendering_params: &GBufferRenderingParams,
        textures: &GBufferTextures,
    ) -> anyhow::Result<RenderPipeline> {
        let vertex_state = PipelineVertexState {
            entry_point: "vs_main",
            vertex_layouts: vec![
                VertexBufferContent::VertexWithTangent,
                VertexBufferContent::TransformComponent,
            ],
        };

        let primitive_state = wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            unclipped_depth: false,
            ..Default::default()
        };

        let depth_stencil_state = Some(wgpu::DepthStencilState {
            format: SampledTexture::DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Greater,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        });

        let fragment_state = PipelineFragmentState {
            entry_point: "fs_main",
            color_targets: vec![
                default_color_write_state(textures.position.texture.format()),
                default_color_write_state(textures.normal.texture.format()),
                default_color_write_state(textures.albedo_and_specular.texture.format()),
                default_color_write_state(textures.metal_rough_ao.texture.format()),
            ],
        };

        let bgroup_layouts = match rendering_params.pbr_rendering_type {
            PbrRenderingType::Textures => {
                vec![
                    device.create_bind_group_layout(&bind_group_layout_descriptors::PBR_TEXTURE),
                    device.create_bind_group_layout(
                        &bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE,
                    ),
                    device.create_bind_group_layout(
                        &bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE,
                    ),
                ]
            }
            PbrRenderingType::FlatParameters => {
                vec![
                    device.create_bind_group_layout(
                        &bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE,
                    ),
                    device.create_bind_group_layout(
                        &bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE,
                    ),
                    device.create_bind_group_layout(
                        &bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE,
                    ),
                ]
            }
        };

        let shader_source_path = match rendering_params.pbr_rendering_type {
            PbrRenderingType::Textures => SHADER_SOURCE_TEXTURED.to_owned(),
            PbrRenderingType::FlatParameters => SHADER_SOURCE_FLAT_PARAMETER.to_owned(),
        };

        let render_pipeline_descriptor = RenderPipelineDescriptor {
            name: Some("Render pipeline that creates the gbuffer textures".to_owned()),
            shader_source_path,
            vertex: vertex_state,
            primitive: primitive_state,
            depth_stencil: depth_stencil_state,
            fragment: fragment_state,
            bind_group_layouts: bgroup_layouts,
            material_bind_group_index: Some(0),
        };

        RenderPipeline::new(device, render_pipeline_descriptor)
    }

    fn create_gbuffer_bind_group(
        device: &wgpu::Device,
        textures: &GBufferTextures,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &device.create_bind_group_layout(&bind_group_layout_descriptors::GBUFFER),
            entries: &[
                textures.position.get_texture_bind_group_entry(0),
                textures.position.get_sampler_bind_group_entry(1),
                textures.normal.get_texture_bind_group_entry(2),
                textures.normal.get_sampler_bind_group_entry(3),
                textures.albedo_and_specular.get_texture_bind_group_entry(4),
                textures.albedo_and_specular.get_sampler_bind_group_entry(5),
                textures.metal_rough_ao.get_texture_bind_group_entry(6),
                textures.metal_rough_ao.get_sampler_bind_group_entry(7),
            ],
            label: Some("GBuffer bind group"),
        })
    }

    fn create_depth_bind_group(device: &Device, depth_texture: &SampledTexture) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &device.create_bind_group_layout(&bind_group_layout_descriptors::DEPTH_TEXTURE),
            entries: &[
                depth_texture.get_texture_bind_group_entry(0),
                depth_texture.get_sampler_bind_group_entry(1),
            ],
            label: Some("Main frame depth bind group"),
        })
    }

    pub fn begin_render<'a>(&'a self, encoder: &'a mut CommandEncoder) -> RenderPass<'a> {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("GBuffer pass"),
            color_attachments: &[
                Some(RenderPassColorAttachment {
                    view: &self.textures.position.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(GBUFFER_CLEAR_COLOR),
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(RenderPassColorAttachment {
                    view: &self.textures.normal.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(GBUFFER_CLEAR_COLOR),
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(RenderPassColorAttachment {
                    view: &self.textures.albedo_and_specular.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(GBUFFER_CLEAR_COLOR),
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(RenderPassColorAttachment {
                    view: &self.textures.metal_rough_ao.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(GBUFFER_CLEAR_COLOR),
                        store: wgpu::StoreOp::Store,
                    },
                }),
            ],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &self.textures.depth_texture.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        })
    }

    pub fn render<'a, T: Iterator<Item = &'a Renderable> + Clone>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        renderables: T,
        camera_bind_group: &'a BindGroup,
        global_gpu_params_bind_group: &'a BindGroup,
    ) {
        for pipeline_with_items in self.render_pipelines.values() {
            let items_for_current_pipeline = renderables
                .clone()
                .filter(|renderable| pipeline_with_items.objects.contains(&renderable.id));

            pipeline_with_items.render_pipeline.render(
                render_pass,
                &[camera_bind_group, global_gpu_params_bind_group],
                items_for_current_pipeline,
                1,
            );
        }
    }
}
