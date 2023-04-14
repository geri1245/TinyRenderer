use async_std::task::block_on;
use glam::{Quat, Vec3};
use std::{collections::HashMap, f32::consts, fs::File, hash::Hash, io::Write, num::NonZeroU32};
use wgpu::{util::DeviceExt, InstanceDescriptor, RenderPassDepthStencilAttachment};

use crate::{
    bind_group_layout_descriptors,
    buffer_content::BufferContent,
    camera_controller::CameraController,
    drawable::Drawable,
    instance::{self, Instance},
    light_controller::LightController,
    model::Model,
    primitive_shapes::{self, TexturedPrimitive},
    render_pipeline::RenderPipeline,
    resources,
    texture::{self, Texture},
    vertex,
};

const NUM_INSTANCES_PER_ROW: u32 = 10;
const MAX_LIGHTS: usize = 10;
const SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SHADOW_SIZE: wgpu::Extent3d = wgpu::Extent3d {
    width: 1024,
    height: 1024,
    depth_or_array_layers: MAX_LIGHTS as u32,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum BindGroupLayoutType {
    Camera,
    Light,
    DiffuseTexture,
    DepthTexture,
}

struct BufferDimensions {
    width: usize,
    height: usize,
    unpadded_bytes_per_row: usize,
    padded_bytes_per_row: usize,
}

impl BufferDimensions {
    fn new(width: usize, height: usize) -> Self {
        let bytes_per_pixel = std::mem::size_of::<u32>();
        let unpadded_bytes_per_row = width * bytes_per_pixel;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
        let padded_bytes_per_row_padding = (align - unpadded_bytes_per_row % align) % align;
        let padded_bytes_per_row = unpadded_bytes_per_row + padded_bytes_per_row_padding;
        Self {
            width,
            height,
            unpadded_bytes_per_row,
            padded_bytes_per_row,
        }
    }
}

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub bind_group_layouts: HashMap<BindGroupLayoutType, wgpu::BindGroupLayout>,

    surface: wgpu::Surface,

    render_pipeline: RenderPipeline,
    light_render_pipeline: RenderPipeline,
    obj_model: Model,
    depth_texture: texture::Texture,
    instances: Vec<Instance>,
    instance_buffer: wgpu::Buffer,
    output_buffer: wgpu::Buffer,
    buffer_dimensions: BufferDimensions,
    texture_extent: wgpu::Extent3d,
    should_capture_frame_content: bool,

    square: TexturedPrimitive,
    square_instance_buffer: wgpu::Buffer,
    // shadows
    shadow_target_views: Vec<wgpu::TextureView>,
    shadow_pipeline: wgpu::RenderPipeline,
    shadow_bind_group: wgpu::BindGroup,
}

impl Renderer {
    pub async fn new(window: &winit::window::Window) -> Renderer {
        let size = window.inner_size();

        // Backends::all => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(InstanceDescriptor {
            backends: wgpu::Backends::DX12,
            ..Default::default()
        });
        let surface = unsafe { instance.create_surface(window) }.unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    // WebGL doesn't support all of wgpu's features, so if
                    // we're building for the web we'll have to disable some.
                    limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                    label: None,
                },
                None,
            )
            .await
            .unwrap();

        let surface_capabilities = surface.get_capabilities(&adapter);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            format: surface_capabilities.formats[0],
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let bind_group_layouts = Self::create_bind_group_layouts(&device);

        // It is a WebGPU requirement that ImageCopyBuffer.layout.bytes_per_row % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT == 0
        // So we calculate padded_bytes_per_row by rounding unpadded_bytes_per_row
        // up to the next multiple of wgpu::COPY_BYTES_PER_ROW_ALIGNMENT.
        // https://en.wikipedia.org/wiki/Data_structure_alignment#Computing_padding
        let buffer_dimensions =
            BufferDimensions::new(config.width as usize, config.height as usize);
        // The output buffer lets us retrieve the data as an array
        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Buffer to copy frame content into"),
            size: (buffer_dimensions.padded_bytes_per_row * buffer_dimensions.height) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let texture_extent = wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        };

        let tree_texture_raw = include_bytes!("../assets/happy-tree.png");

        let tree_texture =
            texture::Texture::from_bytes(&device, &queue, tree_texture_raw, "treeTexture").unwrap();

        let depth_texture =
            texture::Texture::create_depth_texture(&device, &config, "depth_texture");

        let shader_desc = wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shader.wgsl").into()),
        };

        let main_shader = device.create_shader_module(shader_desc);

        // Shadows

        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        let shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
            size: SHADOW_SIZE,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SHADOW_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            label: None,
            view_formats: &[],
        });
        let shadow_view = shadow_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let shadow_target_views = (0..2)
            .map(|i| {
                shadow_texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("shadow"),
                    format: None,
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: 0,
                    mip_level_count: None,
                    base_array_layer: i as u32,
                    array_layer_count: NonZeroU32::new(1),
                })
            })
            .collect::<Vec<_>>();

        let shadow_pipeline = {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("shadow"),
                bind_group_layouts: &[&bind_group_layouts
                    .get(&BindGroupLayoutType::Light)
                    .unwrap()],
                push_constant_ranges: &[],
            });

            let shadow_shader_desc = wgpu::ShaderModuleDescriptor {
                label: Some("Shadow bake shader"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("shaders/shadow_bake_vert.wgsl").into(),
                ),
            };

            let shadow_shader = device.create_shader_module(shadow_shader_desc);

            // Create the render pipeline
            let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("shadow"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shadow_shader,
                    entry_point: "vs_bake",
                    buffers: &[
                        vertex::VertexRaw::buffer_layout(),
                        instance::InstanceRaw::buffer_layout(),
                    ],
                },
                fragment: None,
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    unclipped_depth: device
                        .features()
                        .contains(wgpu::Features::DEPTH_CLIP_CONTROL),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: SHADOW_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState {
                        constant: 2, // corresponds to bilinear filtering
                        slope_scale: 2.0,
                        clamp: 0.0,
                    },
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

            shadow_pipeline
        };

        let shadow_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layouts
                .get(&BindGroupLayoutType::DepthTexture)
                .unwrap(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                },
            ],
            label: None,
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    &bind_group_layouts.get(&BindGroupLayoutType::Light).unwrap(),
                    &bind_group_layouts
                        .get(&BindGroupLayoutType::Camera)
                        .unwrap(),
                    &bind_group_layouts
                        .get(&BindGroupLayoutType::DiffuseTexture)
                        .unwrap(),
                    &bind_group_layouts
                        .get(&BindGroupLayoutType::DepthTexture)
                        .unwrap(),
                ],
                push_constant_ranges: &[],
            });

        let render_pipeline = RenderPipeline::new(
            Some("Main render pipeline"),
            &device,
            &render_pipeline_layout,
            config.format,
            Some(texture::Texture::DEPTH_FORMAT),
            &[
                vertex::VertexRaw::buffer_layout(),
                instance::InstanceRaw::buffer_layout(),
            ],
            &main_shader,
        );

        let light_render_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Light Pipeline Layout"),
                bind_group_layouts: &[
                    &bind_group_layouts.get(&BindGroupLayoutType::Light).unwrap(),
                    &bind_group_layouts
                        .get(&BindGroupLayoutType::Camera)
                        .unwrap(),
                ],
                push_constant_ranges: &[],
            });
            let light_shader_desc = wgpu::ShaderModuleDescriptor {
                label: Some("Light Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/light.wgsl").into()),
            };
            let light_shader = device.create_shader_module(light_shader_desc);
            RenderPipeline::new(
                Some("Light render pipeline"),
                &device,
                &layout,
                config.format,
                Some(texture::Texture::DEPTH_FORMAT),
                &[vertex::VertexRaw::buffer_layout()],
                &light_shader,
            )
        };

        const SPACE_BETWEEN: f32 = 4.0;
        const SCALE: Vec3 = Vec3::new(1.0, 1.0, 1.0);
        let instances = (0..NUM_INSTANCES_PER_ROW)
            .flat_map(|z| {
                (0..NUM_INSTANCES_PER_ROW).map(move |x| {
                    let x = SPACE_BETWEEN * (x as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);
                    let z = SPACE_BETWEEN * (z as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);

                    let position = Vec3 { x, y: 0.0, z };

                    let rotation = if position == Vec3::ZERO {
                        Quat::from_axis_angle(Vec3::Z, 0.0)
                    } else {
                        Quat::from_axis_angle(position.normalize(), consts::FRAC_PI_4)
                    };

                    Instance {
                        position,
                        rotation,
                        scale: SCALE,
                    }
                })
            })
            .collect::<Vec<_>>();

        let instance_data = instances
            .iter()
            .map(instance::Instance::to_raw)
            .collect::<Vec<_>>();
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(&instance_data),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let obj_model = resources::load_model(
            "cube.obj",
            &device,
            &queue,
            &bind_group_layouts
                .get(&BindGroupLayoutType::DiffuseTexture)
                .unwrap(),
        )
        .await
        .unwrap();

        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layouts
                .get(&BindGroupLayoutType::DiffuseTexture)
                .unwrap(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&tree_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&tree_texture.sampler),
                },
            ],
            label: Some("diffuse_bind_group"),
        });

        let square = TexturedPrimitive {
            primitive_shape: primitive_shapes::PrimitiveShape::square(&device),
            texture_bind_group,
        };

        let square_instances = vec![Instance {
            position: Vec3::new(0.0, -10.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: 100.0_f32
                * Vec3 {
                    x: 1.0_f32,
                    y: 1.0,
                    z: 1.0,
                },
        }];

        let square_instance_raw = square_instances
            .iter()
            .map(|instance| instance.to_raw())
            .collect::<Vec<_>>();
        let square_instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Square Instance Buffer"),
            contents: bytemuck::cast_slice(&square_instance_raw),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Renderer {
            surface,
            device,
            queue,
            config,
            size,
            bind_group_layouts,
            render_pipeline,
            light_render_pipeline,
            obj_model,
            depth_texture,
            instances,
            instance_buffer,
            output_buffer,
            buffer_dimensions,
            texture_extent,
            should_capture_frame_content: false,
            square,
            square_instance_buffer,
            shadow_target_views,
            shadow_pipeline,
            shadow_bind_group,
        }
    }

    fn create_bind_group_layouts(
        render_device: &wgpu::Device,
    ) -> HashMap<BindGroupLayoutType, wgpu::BindGroupLayout> {
        let mut bind_group_layouts = HashMap::new();
        bind_group_layouts.insert(
            BindGroupLayoutType::Camera,
            render_device.create_bind_group_layout(
                &bind_group_layout_descriptors::CAMERA_BIND_GROUP_LAYOUT_DESCRIPTOR,
            ),
        );
        bind_group_layouts.insert(
            BindGroupLayoutType::DepthTexture,
            render_device.create_bind_group_layout(
                &bind_group_layout_descriptors::DEPTH_TEXTURE_BIND_GROUP_LAYOUT_DESCRIPTOR,
            ),
        );
        bind_group_layouts.insert(
            BindGroupLayoutType::DiffuseTexture,
            render_device.create_bind_group_layout(
                &bind_group_layout_descriptors::DIFFUSE_TEXTURE_BIND_GROUP_LAYOUT_DESCRIPTOR,
            ),
        );
        bind_group_layouts.insert(
            BindGroupLayoutType::Light,
            render_device.create_bind_group_layout(
                &bind_group_layout_descriptors::LIGHT_BIND_GROUP_LAYOUT_DESCRIPTOR,
            ),
        );

        bind_group_layouts
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.size = new_size;
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
        self.depth_texture =
            Texture::create_depth_texture(&self.device, &self.config, "Depth texture");
    }

    pub fn render(
        &mut self,
        camera_controller: &CameraController,
        light_controller: &LightController,
    ) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        //Shadow pass
        {
            let mut shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Shadow pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &self.shadow_target_views[0],
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });

            shadow_pass.set_pipeline(&self.shadow_pipeline);

            shadow_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            shadow_pass.set_bind_group(0, &light_controller.bind_group, &[]);

            self.obj_model
                .draw_instanced(&mut shadow_pass, 0..self.instances.len() as u32);

            shadow_pass.set_vertex_buffer(1, self.square_instance_buffer.slice(..));
            self.square.draw_instanced(&mut shadow_pass, 0..1);
        }
        // Forward pass
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Forward Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: true,
                    },
                })],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });

            use crate::model::DrawLight;
            render_pass.set_pipeline(&self.light_render_pipeline.pipeline);
            render_pass.draw_light_model(
                &self.obj_model,
                &camera_controller.bind_group,
                &light_controller.bind_group,
            );

            render_pass.set_pipeline(&self.render_pipeline.pipeline);

            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            render_pass.set_bind_group(1, &camera_controller.bind_group, &[]);
            render_pass.set_bind_group(0, &light_controller.bind_group, &[]);
            render_pass.set_bind_group(3, &self.shadow_bind_group, &[]);

            self.obj_model
                .draw_instanced(&mut render_pass, 0..self.instances.len() as u32);

            render_pass.set_vertex_buffer(1, self.square_instance_buffer.slice(..));
            self.square.draw_instanced(&mut render_pass, 0..1);
        }

        encoder.copy_texture_to_buffer(
            output.texture.as_image_copy(),
            wgpu::ImageCopyBuffer {
                buffer: &self.output_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(
                        std::num::NonZeroU32::new(
                            self.buffer_dimensions.padded_bytes_per_row as u32,
                        )
                        .unwrap(),
                    ),
                    rows_per_image: None,
                },
            },
            self.texture_extent,
        );

        let submission_index = self.queue.submit(Some(encoder.finish()));
        output.present();

        if self.should_capture_frame_content {
            self.should_capture_frame_content = false;
            let future = self.create_png("./frame.png", submission_index);
            block_on(future);
        }

        Ok(())
    }

    async fn create_png(&self, png_output_path: &str, submission_index: wgpu::SubmissionIndex) {
        let buffer_slice = self.output_buffer.slice(..);
        // Sets the buffer up for mapping, sending over the result of the mapping back to us when it is finished.
        let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

        // TODO: Either Poll without blocking or move the blocking polling to another thread
        self.device
            .poll(wgpu::Maintain::WaitForSubmissionIndex(submission_index));

        let has_file_system_available = cfg!(not(target_arch = "wasm32"));
        if !has_file_system_available {
            return;
        }

        if let Some(Ok(())) = receiver.receive().await {
            let padded_buffer = buffer_slice.get_mapped_range();

            let mut png_encoder = png::Encoder::new(
                File::create(png_output_path).unwrap(),
                self.buffer_dimensions.width as u32,
                self.buffer_dimensions.height as u32,
            );
            png_encoder.set_depth(png::BitDepth::Eight);
            png_encoder.set_color(png::ColorType::Rgba);
            let mut png_writer = png_encoder
                .write_header()
                .unwrap()
                .into_stream_writer_with_size(self.buffer_dimensions.unpadded_bytes_per_row)
                .unwrap();

            // from the padded_buffer we write just the unpadded bytes into the image
            for chunk in padded_buffer.chunks(self.buffer_dimensions.padded_bytes_per_row) {
                png_writer
                    .write_all(&chunk[..self.buffer_dimensions.unpadded_bytes_per_row])
                    .unwrap();
            }
            png_writer.finish().unwrap();

            // With the current interface, we have to make sure all mapped views are
            // dropped before we unmap the buffer.
            drop(padded_buffer);

            self.output_buffer.unmap();
        }
    }
}
