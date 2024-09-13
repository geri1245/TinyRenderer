use std::collections::VecDeque;

use async_std::task::block_on;
use crossbeam_channel::Sender;
use log::warn;
use rayon::{ThreadPool, ThreadPoolBuilder};
use wgpu::{
    BindGroup, CommandEncoder, Device, Extent3d, ImageCopyTexture, RenderPassColorAttachment,
    RenderPassDepthStencilAttachment, TextureAspect, TextureFormat, TextureUsages, TextureView,
};

use crate::{
    actions::UserInputAction,
    buffer_capture::OutputBuffer,
    buffer_reader::ReadableBuffer,
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
const NUM_OF_PICK_BUFFERS: usize = 8;

pub struct ObjectPickManager {
    pub object_id_texture: SampledTexture,
    width: u32,
    height: u32,
    object_picker_rp: ObjectPickerRP,
    thread_pool: ThreadPool,

    output_buffers: VecDeque<ReadableBuffer>,
    selected_object_id_sender: Sender<UserInputAction>,
    should_allocate_new_buffer: bool,
}

impl ObjectPickManager {
    pub async fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        selected_object_id_sender: Sender<UserInputAction>,
    ) -> Self {
        let texture = Self::create_texture(device, width, height);

        let render_pipeline = ObjectPickerRP::new(
            device,
            OBJECT_PICKER_TEXTURE_FORMAT,
            SampledTexture::DEPTH_FORMAT,
        )
        .await
        .unwrap();

        Self {
            object_id_texture: texture,
            width,
            height,
            object_picker_rp: render_pipeline,
            output_buffers: VecDeque::with_capacity(NUM_OF_PICK_BUFFERS),
            should_allocate_new_buffer: true,
            thread_pool: ThreadPoolBuilder::new()
                .num_threads(NUM_OF_PICK_BUFFERS)
                .build()
                .unwrap(),
            selected_object_id_sender,
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
        self.width = width;
        self.height = height;
    }

    pub fn update(&mut self, device: &Device) {
        let mut should_pop_front = false;
        self.output_buffers.front().map(|item| {
            if let Some(data) = item.get_value_at_position::<u32>(device) {
                warn!("Got ID {data}");
                self.selected_object_id_sender
                    .send(UserInputAction::SelectObject(data))
                    .unwrap();
                should_pop_front = true;
            }
        });

        if should_pop_front {
            self.output_buffers.pop_front();
        }
    }

    fn create_readable_buffer(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        x: u32,
        y: u32,
    ) -> ReadableBuffer {
        ReadableBuffer::new(
            device,
            &Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            &OBJECT_PICKER_TEXTURE_FORMAT,
            x,
            y,
        )
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
                | TextureUsages::COPY_SRC
                | wgpu::TextureUsages::STORAGE_BINDING,
            extents: texture_extents,
        };

        SampledTexture::new(device, descriptor, "Texture for object picking")
    }

    pub fn render<'a, T>(
        &'a mut self,
        encoder: &mut CommandEncoder,
        device: &Device,
        renderables: T,
        camera_bind_group: &'a BindGroup,
        depth_texture: &TextureView,
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

            for renderable in renderables {
                self.object_picker_rp.render(&mut render_pass, renderable)
            }
        }

        if self.output_buffers.len() < 1 {
            let readable_buffer =
                Self::create_readable_buffer(device, self.width, self.height, 3, 145);
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
}
