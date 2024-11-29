use wgpu::{
    BindGroup, CommandEncoder, Device, Extent3d, RenderPass, RenderPassColorAttachment,
    RenderPassDepthStencilAttachment, TextureDimension, TextureFormat, TextureUsages,
};

use crate::{
    bind_group_layout_descriptors,
    material::PbrMaterialDescriptor,
    model::Renderable,
    pipelines::{
        GBufferGeometryRP, GBufferTextures, PbrParameterVariation, ShaderCompilationSuccess,
    },
    texture::{SampledTexture, SampledTextureDescriptor},
};

const GBUFFER_TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba16Float;
const GBUFFER_CLEAR_COLOR: wgpu::Color = wgpu::Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};
pub struct GBufferGeometryRenderer {
    pub textures: GBufferTextures,
    pub bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    textured_gbuffer_rp: GBufferGeometryRP,
    flat_parameter_gbuffer_rp: GBufferGeometryRP,
}

impl GBufferGeometryRenderer {
    pub async fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let textures = Self::create_textures(device, width, height);
        let bind_group = Self::create_bind_group(device, &textures);

        let textured_gbuffer_rp =
            GBufferGeometryRP::new(device, &textures, PbrParameterVariation::Texture)
                .await
                .unwrap();

        let flat_parameter_gbuffer_rp =
            GBufferGeometryRP::new(device, &textures, PbrParameterVariation::Flat)
                .await
                .unwrap();

        Self {
            textures,
            bind_group,
            width,
            height,
            textured_gbuffer_rp,
            flat_parameter_gbuffer_rp,
        }
    }

    pub async fn try_recompile_shader(
        &mut self,
        device: &Device,
    ) -> anyhow::Result<ShaderCompilationSuccess> {
        self.textured_gbuffer_rp
            .try_recompile_shader(device, &self.textures, PbrParameterVariation::Texture)
            .await?;

        self.flat_parameter_gbuffer_rp
            .try_recompile_shader(device, &self.textures, PbrParameterVariation::Flat)
            .await
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.textures = Self::create_textures(device, width, height);
        self.bind_group = Self::create_bind_group(device, &self.textures);
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

        let depth_texture =
            SampledTexture::create_depth_texture(device, texture_extents, "GBuffer depth texture");

        GBufferTextures {
            position: position_texture,
            normal: normal_texture,
            albedo_and_specular: albedo_and_specular_texture,
            depth_texture,
            metal_rough_ao,
        }
    }

    fn create_bind_group(device: &wgpu::Device, textures: &GBufferTextures) -> wgpu::BindGroup {
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
                    load: wgpu::LoadOp::Clear(1.0),
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
        let textured_renderables = renderables.clone().filter(|renderable| {
            matches!(
                renderable.description.mesh_descriptor.material_descriptor,
                PbrMaterialDescriptor::Texture(_)
            )
        });
        self.textured_gbuffer_rp.render(
            render_pass,
            textured_renderables,
            camera_bind_group,
            global_gpu_params_bind_group,
        );

        let flat_param_renderables = renderables.clone().filter(|renderable| {
            matches!(
                renderable.description.mesh_descriptor.material_descriptor,
                PbrMaterialDescriptor::Flat(_)
            )
        });
        self.flat_parameter_gbuffer_rp.render(
            render_pass,
            flat_param_renderables,
            camera_bind_group,
            global_gpu_params_bind_group,
        );
    }
}
