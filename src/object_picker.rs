use std::collections::VecDeque;

use wgpu::{
    BindGroup, CommandEncoder, Device, Extent3d, ImageCopyTexture, RenderPassColorAttachment,
    RenderPassDepthStencilAttachment, TextureAspect, TextureDimension, TextureFormat,
    TextureUsages, TextureView,
};

use crate::{
    model::Renderable,
    pipelines::{ObjectPickerRP, ShaderCompilationSuccess},
    pollable_gpu_buffer::PollableGpuBuffer,
    renderer::Renderer,
    texture::{SampledTexture, SampledTextureDescriptor, SamplingType},
};

const OBJECT_PICKER_TEXTURE_FORMAT: TextureFormat = TextureFormat::R32Uint;
const CLEAR_COLOR: wgpu::Color = wgpu::Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};
const NUM_OF_PICK_BUFFERS: usize = 8;

/// The contents of a 2D texture in a buffer, that might have been padded
/// Because of this, some information needs to be stored, so we can get the
/// value at (x, y)
struct SingleDimensionPaddedImageBuffer {
    data: Vec<u32>,
    padded_row_size: u32,
}

impl SingleDimensionPaddedImageBuffer {
    fn get(&self, x: u32, y: u32) -> Option<u32> {
        self.data
            .get((y * self.padded_row_size + x) as usize)
            .map(|result| *result)
    }
}

pub struct ObjectPickManager {
    pub object_id_texture: SampledTexture,

    width: u32,
    height: u32,
    object_picker_rp: ObjectPickerRP,

    // The buffer length is usually only 1, no need to reallocate the buffer over and over again,
    // just keep the gpu memory and pingpong with 2 (or maybe more) buffers
    output_buffers: VecDeque<PollableGpuBuffer>,
    latest_object_id_buffer: SingleDimensionPaddedImageBuffer,
}

impl ObjectPickManager {
    pub fn new(renderer: &Renderer) -> Self {
        let texture = Self::create_texture(renderer);

        let render_pipeline = ObjectPickerRP::new(
            &renderer.device,
            OBJECT_PICKER_TEXTURE_FORMAT,
            SampledTexture::DEPTH_FORMAT,
        )
        .unwrap();

        Self {
            object_id_texture: texture,
            width: renderer.config.width,
            height: renderer.config.height,
            object_picker_rp: render_pipeline,
            output_buffers: VecDeque::with_capacity(NUM_OF_PICK_BUFFERS),
            latest_object_id_buffer: SingleDimensionPaddedImageBuffer {
                data: Vec::new(),
                padded_row_size: 0,
            },
        }
    }

    pub fn get_object_id_at(&self, x: u32, y: u32) -> Option<u32> {
        // 0 is not a valid ID, so we return it as None - the rest of the application can handle it like that
        match self.latest_object_id_buffer.get(x, y) {
            Some(id) => {
                if id == 0 {
                    None
                } else {
                    Some(id)
                }
            }
            None => None,
        }
    }

    pub fn try_recompile_shader(
        &mut self,
        device: &Device,
    ) -> anyhow::Result<ShaderCompilationSuccess> {
        self.object_picker_rp.try_recompile_shader(
            device,
            OBJECT_PICKER_TEXTURE_FORMAT,
            SampledTexture::DEPTH_FORMAT,
        )
    }

    pub fn resize(&mut self, renderer: &Renderer) {
        self.object_id_texture = Self::create_texture(renderer);
        self.width = renderer.config.width;
        self.height = renderer.config.height;
    }

    pub fn update(&mut self) {
        let mut should_pop_front = false;
        self.output_buffers.front().map(|item| {
            if let Some(padded_row_size) =
                item.poll_mapped_buffer(&mut self.latest_object_id_buffer.data)
            {
                self.latest_object_id_buffer.padded_row_size = padded_row_size;
                should_pop_front = true;
            }
        });

        if should_pop_front {
            self.output_buffers.pop_front();
        }
    }

    pub fn on_end_frame(&mut self) {
        self.output_buffers.back().unwrap().post_render();
    }

    fn create_readable_buffer(device: &wgpu::Device, width: u32, height: u32) -> PollableGpuBuffer {
        PollableGpuBuffer::new(
            device,
            &Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            &OBJECT_PICKER_TEXTURE_FORMAT,
        )
    }

    fn create_texture(renderer: &Renderer) -> SampledTexture {
        let texture_extents = Extent3d {
            width: renderer.config.width,
            height: renderer.config.height,
            depth_or_array_layers: 1,
        };
        let descriptor = SampledTextureDescriptor {
            format: OBJECT_PICKER_TEXTURE_FORMAT,
            usages: TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_SRC
                | wgpu::TextureUsages::STORAGE_BINDING,
            extents: texture_extents,
            dimension: TextureDimension::D2,
            mip_count: 1,
            sampling_type: SamplingType::Nearest,
        };

        SampledTexture::new(&renderer.device, descriptor, "Texture for object picking")
    }

    pub fn render<'a, T>(
        &'a mut self,
        encoder: &'a mut CommandEncoder,
        device: &Device,
        renderables: T,
        camera_bind_group: &'a BindGroup,
        depth_texture: &'a TextureView,
    ) where
        T: Clone,
        T: Iterator<Item = &'a Renderable>,
    {
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

            self.object_picker_rp.render(
                &mut render_pass.forget_lifetime(),
                renderables,
                camera_bind_group,
            );
        }

        let readable_buffer = Self::create_readable_buffer(device, self.width, self.height);
        encoder.copy_texture_to_buffer(
            ImageCopyTexture {
                aspect: TextureAspect::All,
                texture: &self.object_id_texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            wgpu::ImageCopyBuffer {
                buffer: &readable_buffer.mapable_buffer.buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(readable_buffer.mapable_buffer.padded_row_size as u32),
                    rows_per_image: Some(self.height),
                },
            },
            readable_buffer.mapable_buffer.texture_extent,
        );

        self.output_buffers.push_back(readable_buffer);
    }
}
