use std::{f32::consts, rc::Rc, time};

use async_std::task::block_on;
use glam::{Quat, Vec3};
use wgpu::{util::DeviceExt, CommandEncoder, Device, TextureView};

use crate::{
    bind_group_layout_descriptors,
    camera_controller::CameraController,
    instance::{self, Instance},
    light_controller::LightController,
    model::{Material, Mesh, Model},
    pipelines::{self, MainRP},
    primitive_shapes,
    renderer::Renderer,
    resources,
    skybox::Skybox,
    texture,
};

const NUM_INSTANCES_PER_ROW: u32 = 10;

pub struct World {
    pub obj_model: Model,
    pub instances: Vec<Instance>,
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
        let tree_texture_raw = include_bytes!("../assets/happy-tree.png");

        let tree_texture = texture::Texture::from_bytes(
            &renderer.device,
            &renderer.queue,
            tree_texture_raw,
            "treeTexture",
        )
        .unwrap();
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
        let instance_buffer =
            renderer
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Instance Buffer"),
                    contents: bytemuck::cast_slice(&instance_data),
                    usage: wgpu::BufferUsages::VERTEX,
                });

        let obj_model = resources::load_model("cube.obj", &renderer.device, &renderer.queue)
            .await
            .unwrap();

        let texture_bind_group = renderer
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &renderer
                    .device
                    .create_bind_group_layout(&bind_group_layout_descriptors::DIFFUSE_TEXTURE),
                entries: &[
                    tree_texture.get_texture_bind_group_entry(0),
                    tree_texture.get_sampler_bind_group_entry(1),
                ],
                label: Some("diffuse_bind_group"),
            });

        let square_material = Some(Rc::new(Material {
            name: "Tree texture material".into(),
            diffuse_texture: tree_texture,
            bind_group: texture_bind_group,
        }));

        let square = primitive_shapes::square(&renderer.device, square_material);

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

        let main_rp = pipelines::MainRP::new(&renderer.device, renderer.config.format).await;
        let gbuffer_rp = pipelines::GBufferGeometryRP::new(
            &renderer.device,
            renderer.config.width,
            renderer.config.height,
        );
        let forward_rp = pipelines::ForwardRP::new(&renderer.device, renderer.config.format);

        World {
            obj_model,
            instances,
            instance_buffer,
            square,
            square_instance_buffer,
            skybox,
            camera_controller,
            light_controller,
            main_rp,
            gbuffer_rp,
            forward_rp,
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
                    1,
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

    pub fn recompile_shaders_if_needed(&mut self, device: &Device) {
        let result = block_on(self.main_rp.try_recompile_shader(device));
        match result {
            Ok(render_pipeline) => self.main_rp = render_pipeline,
            Err(_) => {}
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
