use std::{
    f32::consts::{FRAC_PI_2, PI},
    time,
};

use async_std::task::block_on;
use glam::{Quat, Vec3};
use pipelines::PipelineRecreationResult;
use wgpu::{util::DeviceExt, CommandEncoder, Device, TextureView};

use crate::{
    camera_controller::CameraController,
    instance::{self, Instance},
    light_controller::LightController,
    model::{Material, Mesh, Model, TextureData, TextureType},
    pipelines::{self, MainRP},
    primitive_shapes,
    renderer::Renderer,
    resources,
    skybox::Skybox,
    texture,
};

pub struct World {
    pub obj_model: Model,
    pub instances: Vec<Instance>,
    pub square_count: usize,
    pub instance_buffer: wgpu::Buffer,
    pub square: Mesh,
    pub square_instance_buffer: wgpu::Buffer,
    pub skybox: Skybox,
    pub camera_controller: CameraController,
    pub light_controller: LightController,
    main_rp: MainRP,
    forward_rp: pipelines::ForwardRP,
    gbuffer_rp: pipelines::GBufferGeometryRP,
}

impl World {
    pub async fn new(renderer: &Renderer) -> Self {
        let cube_instances = vec![
            Instance {
                position: Vec3::new(10.0, 10.0, 0.0),
                scale: Vec3::splat(3.0),
                rotation: Quat::from_axis_angle(Vec3::ZERO, 0.0),
            },
            Instance {
                position: Vec3::new(-20.0, 10.0, 0.0),
                scale: Vec3::splat(2.0),
                rotation: Quat::from_axis_angle(Vec3::ZERO, 0.0),
            },
            Instance {
                position: Vec3::new(0.0, 10.0, 30.0),
                scale: Vec3::splat(2.0),
                rotation: Quat::from_axis_angle(Vec3::ZERO, 0.0),
            },
            Instance {
                position: Vec3::new(30.0, 20.0, 10.0),
                scale: Vec3::splat(2.0),
                rotation: Quat::from_axis_angle(Vec3::ZERO, 0.0),
            },
            Instance {
                position: Vec3::new(25.0, 10.0, 20.0),
                scale: Vec3::splat(1.5),
                rotation: Quat::from_axis_angle(Vec3::ZERO, 0.0),
            },
        ];

        let instance_data = cube_instances
            .iter()
            .map(instance::Instance::to_raw)
            .collect::<Vec<_>>();
        let instance_buffer =
            renderer
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Instance Buffer"),
                    contents: bytemuck::cast_slice(&instance_data),
                    usage: wgpu::BufferUsages::VERTEX,
                });

        let obj_model = resources::load_model("cube", &renderer.device, &renderer.queue)
            .await
            .unwrap();

        let plane_texture_raw = include_bytes!("../assets/happy-tree.png");
        let plane_albedo = texture::SampledTexture::from_bytes(
            &renderer.device,
            &renderer.queue,
            plane_texture_raw,
            texture::TextureUsage::Albedo,
            "treeTexture",
        )
        .unwrap();
        let square_texture = TextureData {
            texture_type: TextureType::Albedo,
            name: "Tree texture material".into(),
            texture: plane_albedo,
        };

        let plane_normal_raw = include_bytes!("../assets/happy-tree.png");
        let plane_normal = texture::SampledTexture::from_bytes(
            &renderer.device,
            &renderer.queue,
            plane_normal_raw,
            texture::TextureUsage::Normal,
            "treeTexture",
        )
        .unwrap();
        let plane_normal_texture = TextureData {
            texture_type: TextureType::Normal,
            name: "Tree texture material".into(),
            texture: plane_normal,
        };

        let textures = vec![
            (TextureType::Albedo, square_texture),
            (TextureType::Normal, plane_normal_texture),
        ];

        let square_material = Material::new(&renderer.device, textures);
        let square = primitive_shapes::square(&renderer.device, square_material);

        let square_instances = vec![
            // Bottom
            Instance {
                position: Vec3::new(0.0, -10.0, 0.0),
                rotation: Quat::IDENTITY,
                scale: 100.0_f32
                    * Vec3 {
                        x: 1.0_f32,
                        y: 1.0,
                        z: 1.0,
                    },
            },
            // Top
            Instance {
                position: Vec3::new(0.0, 40.0, 0.0),
                rotation: Quat::from_axis_angle(Vec3::X, PI),
                scale: 100.0_f32
                    * Vec3 {
                        x: 1.0_f32,
                        y: 1.0,
                        z: 1.0,
                    },
            },
            // +X
            Instance {
                position: Vec3::new(-40.0, 0.0, 0.0),
                rotation: Quat::from_axis_angle(Vec3::Z, -FRAC_PI_2),
                scale: 100.0_f32
                    * Vec3 {
                        x: 1.0_f32,
                        y: 1.0,
                        z: 1.0,
                    },
            },
            // -X
            Instance {
                position: Vec3::new(40.0, 0.0, 0.0),
                rotation: Quat::from_axis_angle(Vec3::Z, FRAC_PI_2),
                scale: 100.0_f32
                    * Vec3 {
                        x: 1.0_f32,
                        y: 1.0,
                        z: 1.0,
                    },
            },
            // -Z
            Instance {
                position: Vec3::new(0.0, 0.0, -40.0),
                rotation: Quat::from_axis_angle(Vec3::X, FRAC_PI_2),
                scale: 100.0_f32
                    * Vec3 {
                        x: 1.0_f32,
                        y: 1.0,
                        z: 1.0,
                    },
            },
            // Z
            Instance {
                position: Vec3::new(0.0, 0.0, 40.0),
                rotation: Quat::from_axis_angle(Vec3::X, -FRAC_PI_2),
                scale: 100.0_f32
                    * Vec3 {
                        x: 1.0_f32,
                        y: 1.0,
                        z: 1.0,
                    },
            },
        ];

        let square_instance_raw = square_instances
            .iter()
            .map(|instance| instance.to_raw())
            .collect::<Vec<_>>();
        let square_instance_buffer =
            renderer
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Square Instance Buffer"),
                    contents: bytemuck::cast_slice(&square_instance_raw),
                    usage: wgpu::BufferUsages::VERTEX,
                });

        let skybox = Skybox::new(&renderer);

        let camera_controller = CameraController::new(&renderer);
        let light_controller = LightController::new(&renderer.device);

        let main_rp = pipelines::MainRP::new(&renderer.device, renderer.config.format)
            .await
            .unwrap();
        let gbuffer_rp = pipelines::GBufferGeometryRP::new(
            &renderer.device,
            renderer.config.width,
            renderer.config.height,
        );
        let forward_rp = pipelines::ForwardRP::new(&renderer.device, renderer.config.format);

        World {
            obj_model,
            instances: cube_instances,
            instance_buffer,
            square,
            square_instance_buffer,
            skybox,
            camera_controller,
            light_controller,
            main_rp,
            gbuffer_rp,
            forward_rp,
            square_count: square_instances.len(),
        }
    }

    pub fn render(
        &self,
        renderer: &Renderer,
        encoder: &mut CommandEncoder,
        current_frame_texture_view: &TextureView,
    ) -> Result<(), wgpu::SurfaceError> {
        {
            self.light_controller.render_shadows(
                encoder,
                &self.obj_model,
                self.instances.len(),
                &self.instance_buffer,
            );

            {
                let mut render_pass = self.gbuffer_rp.begin_render(encoder);
                self.gbuffer_rp.render_model(
                    &mut render_pass,
                    &self.obj_model,
                    &self.camera_controller.bind_group,
                    self.instances.len(),
                    &self.instance_buffer,
                );

                self.gbuffer_rp.render_mesh(
                    &mut render_pass,
                    &self.square,
                    &self.camera_controller.bind_group,
                    self.square_count,
                    &self.square_instance_buffer,
                );
            }

            {
                let mut render_pass = renderer.begin_main_render_pass(
                    encoder,
                    current_frame_texture_view,
                    &self.gbuffer_rp.textures.depth_texture.view,
                );

                {
                    render_pass.push_debug_group("Cubes rendering from GBuffer");

                    self.main_rp.render(
                        &mut render_pass,
                        &self.camera_controller,
                        &self.light_controller,
                        &self.gbuffer_rp.bind_group,
                        &self.light_controller.shadow_rp.bind_group,
                    );

                    render_pass.pop_debug_group();
                }

                {
                    render_pass.push_debug_group("Forward rendering light debug objects");
                    self.forward_rp.render_model(
                        &mut render_pass,
                        &self.obj_model,
                        &self.camera_controller.bind_group,
                        &self.light_controller.light_bind_group,
                        1,
                        &self.light_controller.light_instance_buffer,
                    );

                    render_pass.pop_debug_group();
                }

                self.skybox
                    .render(&mut render_pass, &self.camera_controller);
            }
        }

        Ok(())
    }

    pub fn recompile_shaders_if_needed(&mut self, device: &Device) -> anyhow::Result<()> {
        let result = block_on(self.main_rp.try_recompile_shader(device));
        match result {
            PipelineRecreationResult::AlreadyUpToDate => Ok(()),
            PipelineRecreationResult::Success(new_pipeline) => {
                self.main_rp = new_pipeline;
                Ok(())
            }
            PipelineRecreationResult::Failed(error) => Err(error),
        }
    }

    pub fn resize_main_camera(&mut self, renderer: &Renderer, width: u32, height: u32) {
        self.gbuffer_rp.resize(&renderer.device, width, height);

        self.camera_controller.resize(width as f32 / height as f32);
    }

    pub fn update(&mut self, delta_time: time::Duration, render_queue: &wgpu::Queue) {
        self.camera_controller.update(delta_time, &render_queue);

        self.light_controller.update(delta_time, &render_queue);
    }
}
