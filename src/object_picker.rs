use wgpu::{
    BindGroup, CommandEncoder, Device, Extent3d, RenderPassColorAttachment,
    RenderPassDepthStencilAttachment, TextureFormat, TextureUsages, TextureView,
};

use crate::{
    bind_group_layout_descriptors,
    model::Renderable,
    pipelines::{ObjectPickerRP, ShaderCompilationSuccess},
    texture::{SampledTexture, SampledTextureDescriptor},
};

const OBJECT_PICKER_TEXTURE_FORMAT: TextureFormat = TextureFormat::R32Uint;
const CLEAR_COLOR: wgpu::Color = wgpu::Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};

pub struct ObjectPickManager {
    pub object_id_texture: SampledTexture,
    // pub bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    object_picker_rp: ObjectPickerRP,
}

impl ObjectPickManager {
    pub async fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let texture = Self::create_texture(device, width, height);
        // let bind_group = Self::create_bind_group(device, &texture);

        let render_pipeline = ObjectPickerRP::new(
            device,
            OBJECT_PICKER_TEXTURE_FORMAT,
            SampledTexture::DEPTH_FORMAT,
        )
        .await
        .unwrap();

        Self {
            object_id_texture: texture,
            // bind_group,
            width,
            height,
            object_picker_rp: render_pipeline,
        }
    }

    pub async fn try_recompile_shader(
        &mut self,
        device: &Device,
    ) -> anyhow::Result<ShaderCompilationSuccess> {
        self.object_picker_rp
            .try_recompile_shader(
                device,
                OBJECT_PICKER_TEXTURE_FORMAT,
                SampledTexture::DEPTH_FORMAT,
            )
            .await
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.object_id_texture = Self::create_texture(device, width, height);
        // self.bind_group = Self::create_bind_group(device, &self.object_id_texture);
        self.width = width;
        self.height = height;
    }

    fn create_texture(device: &wgpu::Device, width: u32, height: u32) -> SampledTexture {
        let texture_extents = Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let descriptor = SampledTextureDescriptor {
            format: OBJECT_PICKER_TEXTURE_FORMAT,
            usages: TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING,
            extents: texture_extents,
        };

        SampledTexture::new(device, descriptor, "Texture for object picking")
    }

    fn create_bind_group(device: &wgpu::Device, texture: &SampledTexture) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &device.create_bind_group_layout(
                &bind_group_layout_descriptors::TEXTURE_2D_FRAGMENT_WITH_SAMPLER,
            ),
            entries: &[
                texture.get_texture_bind_group_entry(0),
                texture.get_sampler_bind_group_entry(1),
            ],
            label: Some("Pick target texture bind group"),
        })
    }

    pub fn render<'a, T>(
        &'a self,
        encoder: &mut CommandEncoder,
        renderables: T,
        camera_bind_group: &'a BindGroup,
        depth_texture: &TextureView,
        gbuffer_bind_group: &'a BindGroup,
    ) where
        T: Clone,
        T: Iterator<Item = &'a Renderable>,
    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Pick rendering pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &self.object_id_texture.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(CLEAR_COLOR),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: depth_texture,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_pipeline(&self.object_picker_rp.render_pipeline);

        for renderable in renderables {
            self.object_picker_rp.render(&mut render_pass, renderable)
        }
    }
}
