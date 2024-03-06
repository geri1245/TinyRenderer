use std::{
    collections::HashMap,
    f32::consts::{FRAC_PI_2, PI},
    rc::Rc,
    time,
};

use async_std::task::block_on;
use glam::{Quat, Vec3};
use pipelines::PipelineRecreationResult;
use wgpu::{CommandEncoder, Device, Extent3d, SurfaceTexture};

use crate::{
    camera_controller::CameraController,
    instance::Instance,
    light_controller::LightController,
    model::{InstancedRenderableMesh, Material, TexturedRenderableMesh},
    pipelines::{self, MainRP},
    post_process_manager::PostProcessManager,
    primitive_shapes,
    renderer::Renderer,
    resource_loader::ResourceLoader,
    skybox::Skybox,
    texture::{self, SampledTexture, TextureUsage},
};

pub struct WorldRenderer {
    models: Vec<InstancedRenderableMesh>,
    pub skybox: Skybox,
    pub camera_controller: CameraController,
    pub light_controller: LightController,
    main_rp: MainRP,
    post_process_manager: PostProcessManager,
    forward_rp: pipelines::ForwardRP,
    gbuffer_rp: pipelines::GBufferGeometryRP,
    pending_textures: HashMap<TextureUsage, SampledTexture>,
    resource_loader: ResourceLoader,
}

impl WorldRenderer {
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

        let mut resource_loader = ResourceLoader::new(&renderer.device, &renderer.queue);

        let (obj_model, loading_id) = resource_loader
            .load_asset_file("cube", &renderer.device)
            .await
            .unwrap();

        let default_normal_bytes = include_bytes!("../assets/defaults/normal.png");
        let default_normal_texture = texture::SampledTexture::from_bytes(
            &renderer.device,
            &renderer.queue,
            default_normal_bytes,
            texture::TextureUsage::Normal,
            "default normal texture",
        )
        .unwrap();
        let default_albedo_bytes = include_bytes!("../assets/defaults/albedo.png");
        let default_albedo_texture = texture::SampledTexture::from_bytes(
            &renderer.device,
            &renderer.queue,
            default_albedo_bytes,
            texture::TextureUsage::Albedo,
            "default albedo texture",
        )
        .unwrap();

        let plane_texture_raw = include_bytes!("../assets/happy-tree.png");
        let plane_albedo = texture::SampledTexture::from_bytes(
            &renderer.device,
            &renderer.queue,
            plane_texture_raw,
            texture::TextureUsage::Albedo,
            "plane texture",
        )
        .unwrap();

        let mut default_material_textures = HashMap::new();
        default_material_textures.insert(TextureUsage::Albedo, default_albedo_texture);
        default_material_textures.insert(TextureUsage::Normal, default_normal_texture);

        let default_material = Rc::new(Material::new(&renderer.device, &default_material_textures));
        let square = primitive_shapes::square(&renderer.device);
        let square_with_material = TexturedRenderableMesh {
            material: default_material.clone(),
            mesh: square,
        };

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

        let camera_controller = CameraController::new(&renderer);
        let light_controller = LightController::new(&renderer.device).await;

        let main_rp = pipelines::MainRP::new(&renderer.device).await.unwrap();
        let gbuffer_rp = pipelines::GBufferGeometryRP::new(
            &renderer.device,
            renderer.config.width,
            renderer.config.height,
        )
        .await
        .unwrap();
        let forward_rp = pipelines::ForwardRP::new(&renderer.device, renderer.config.format);

        let meshes = vec![
            InstancedRenderableMesh::new(&renderer.device, square_with_material, square_instances),
            InstancedRenderableMesh::new(&renderer.device, obj_model, cube_instances),
        ];

        let post_process_manager = PostProcessManager::new(
            &renderer.device,
            renderer.config.width,
            renderer.config.height,
        )
        .await;

        let skybox = Skybox::new(
            &renderer.device,
            &renderer.queue,
            post_process_manager.full_screen_render_target_ping_pong_textures[0]
                .texture
                .format(),
        );

        WorldRenderer {
            models: meshes,
            skybox,
            camera_controller,
            light_controller,
            main_rp,
            gbuffer_rp,
            forward_rp,
            pending_textures: Default::default(),
            resource_loader,
            post_process_manager,
        }
    }

    pub fn render(
        &self,
        renderer: &Renderer,
        encoder: &mut CommandEncoder,
        final_fbo_image_texture: &SurfaceTexture,
    ) -> Result<(), wgpu::SurfaceError> {
        {
            self.light_controller
                .render_shadows(encoder, &self.models[1]);

            {
                let mut render_pass = self.gbuffer_rp.begin_render(encoder);
                self.gbuffer_rp.render_mesh(
                    &mut render_pass,
                    &self.models[1],
                    &self.camera_controller.bind_group,
                );

                self.gbuffer_rp.render_mesh(
                    &mut render_pass,
                    &self.models[0],
                    &self.camera_controller.bind_group,
                );
            }

            {
                let mut render_pass = renderer.begin_main_render_pass(
                    encoder,
                    &self
                        .post_process_manager
                        .full_screen_render_target_ping_pong_textures[0]
                        .view,
                    &self.gbuffer_rp.textures.depth_texture.view,
                );

                self.skybox
                    .render(&mut render_pass, &self.camera_controller);

                {
                    // self.forward_rp.render_model(
                    //     &mut render_pass,
                    //     &self.obj_model,
                    //     &self.camera_controller.bind_group,
                    //     &self.light_controller.light_bind_group,
                    //     1,
                    //     &self.light_controller.light_instance_buffer,
                    // );
                }
            }
        }

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Compute pass"),
                timestamp_writes: None,
            });

            self.main_rp.render(
                &mut compute_pass,
                &self.camera_controller,
                &self.light_controller,
                &self.gbuffer_rp.bind_group,
                &self.light_controller.shadow_rp.bind_group,
                &self.post_process_manager.compute_bind_group_1_to_0,
                renderer.config.width,
                renderer.config.height,
            );

            self.post_process_manager.render(
                &mut compute_pass,
                renderer.config.width,
                renderer.config.height,
            );
        }

        encoder.copy_texture_to_texture(
            self.post_process_manager
                .full_screen_render_target_ping_pong_textures[1]
                .texture
                .as_image_copy(),
            final_fbo_image_texture.texture.as_image_copy(),
            Extent3d {
                depth_or_array_layers: 1,
                width: renderer.config.width,
                height: renderer.config.height,
            },
        );

        Ok(())
    }

    pub fn recompile_shaders_if_needed(&mut self, device: &Device) -> anyhow::Result<()> {
        // Note, that we stop at the first error and don't process the other shaders if something goes wrong.
        // This is a conscious decision, as for now one usually touches one shader at a time, so
        // it's not a real limitation at this point
        // If later more heavyweight modifications are necessary, then this can be "fixed"
        {
            let main_result = block_on(self.main_rp.try_recompile_shader(device));

            match main_result {
                PipelineRecreationResult::AlreadyUpToDate => Ok(()),
                PipelineRecreationResult::Success(new_pipeline) => {
                    self.main_rp = new_pipeline;
                    Ok(())
                }
                PipelineRecreationResult::Failed(error) => Err(error),
            }?;
        }
        {
            let gbuffer_geometry_result = block_on(self.gbuffer_rp.try_recompile_shader(device));
            match gbuffer_geometry_result {
                PipelineRecreationResult::AlreadyUpToDate => Ok(()),
                PipelineRecreationResult::Success(new_pipeline) => {
                    self.gbuffer_rp = new_pipeline;
                    Ok(())
                }
                PipelineRecreationResult::Failed(error) => Err(error),
            }?;
        }

        self.light_controller.try_recompile_shaders(device)?;

        Ok(())
    }

    pub fn resize_main_camera(&mut self, renderer: &Renderer, width: u32, height: u32) {
        self.gbuffer_rp.resize(&renderer.device, width, height);
        self.post_process_manager
            .resize(&renderer.device, width, height);

        self.camera_controller.resize(width as f32 / height as f32);
    }

    pub fn update(
        &mut self,
        delta_time: time::Duration,
        device: &wgpu::Device,
        render_queue: &wgpu::Queue,
    ) {
        self.camera_controller.update(delta_time, &render_queue);

        self.light_controller.update(delta_time, &render_queue);

        let materials_loaded = self
            .resource_loader
            .poll_loaded_textures(device, render_queue);

        for (id, material) in materials_loaded {
            self.models[1].mesh.material = material;
        }
    }
}