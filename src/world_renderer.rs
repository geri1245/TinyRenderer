use std::collections::{HashMap, VecDeque};

use wgpu::{
    BindGroup, CommandEncoder, Device, Extent3d, RenderPassDepthStencilAttachment, SurfaceTexture,
};

use crate::{
    actions::RenderingAction,
    camera_controller::CameraController,
    diffuse_irradiance_renderer::DiffuseIrradianceRenderer,
    equirectangular_to_cubemap_renderer::EquirectangularToCubemapRenderer,
    forward_renderer::ForwardRenderer,
    gbuffer_geometry_renderer::GBufferGeometryRenderer,
    light_controller::LightController,
    model::{Renderable, RenderingPass, WorldObject},
    object_picker::ObjectPickManager,
    pipelines::{self, MainRP, ShaderCompilationSuccess},
    post_process_manager::PostProcessManager,
    renderer::Renderer,
    resource_loader::{PrimitiveShape, ResourceLoader},
    skybox::Skybox,
    world::{ModificationType, ObjectModificationType, World},
};

pub struct WorldRenderer {
    diffuse_irradiance_renderer: DiffuseIrradianceRenderer,
    skybox: Skybox,
    main_rp: MainRP,
    post_process_manager: PostProcessManager,
    forward_renderer: ForwardRenderer,
    gbuffer_geometry_renderer: GBufferGeometryRenderer,
    equirec_to_cubemap_renderer: EquirectangularToCubemapRenderer,

    actions_to_process: VecDeque<RenderingAction>,

    renderables: HashMap<u32, Renderable>,
}

impl WorldRenderer {
    pub fn new(renderer: &Renderer, resource_loader: &mut ResourceLoader) -> Self {
        let main_rp = pipelines::MainRP::new(&renderer.device).unwrap();
        let gbuffer_geometry_renderer = GBufferGeometryRenderer::new(
            &renderer.device,
            renderer.config.width,
            renderer.config.height,
        );

        let forward_rp = ForwardRenderer::new(&renderer.device, wgpu::TextureFormat::Rgba16Float);

        let post_process_manager = PostProcessManager::new(
            &renderer.device,
            renderer.config.width,
            renderer.config.height,
        );

        // TODO: extract the format from here and don't reference full_screen_render_target_ping_pong_textures directly
        let skybox = Skybox::new(
            &renderer.device,
            post_process_manager.full_screen_render_target_ping_pong_textures[0]
                .texture
                .format(),
        );

        // TODO: change the format, or use some constant here
        let equirec_to_cubemap_renderer = EquirectangularToCubemapRenderer::new(
            renderer,
            wgpu::TextureFormat::Rgba16Float,
            resource_loader.get_primitive_shape(PrimitiveShape::Cube),
        )
        .unwrap();

        // TODO: change the format, or use some constant here
        let diffuse_irradiance_renderer = DiffuseIrradianceRenderer::new(
            &renderer.device,
            &renderer.queue,
            wgpu::TextureFormat::Rgba16Float,
            resource_loader.get_primitive_shape(PrimitiveShape::Cube),
        )
        .unwrap();

        WorldRenderer {
            skybox,
            main_rp,
            gbuffer_geometry_renderer,
            forward_renderer: forward_rp,

            post_process_manager,
            equirec_to_cubemap_renderer,
            diffuse_irradiance_renderer,
            actions_to_process: VecDeque::new(),
            renderables: HashMap::new(),
        }
    }

    pub fn add_action(&mut self, action: RenderingAction) {
        self.actions_to_process.push_back(action);
    }

    fn add_object(
        &mut self,
        world_object: &WorldObject,
        new_renderable_id: u32,
        resource_loader: &ResourceLoader,
        renderer: &Renderer,
    ) {
        let renderable_parts = resource_loader
            .load_model(&world_object.description.model_descriptor, renderer)
            .unwrap();
        let new_renderable = Renderable::new(
            world_object.description.model_descriptor.clone(),
            world_object.get_transform(),
            renderable_parts,
            &renderer.device,
            new_renderable_id,
            &world_object.description.rendering_options,
        );
        self.renderables.insert(new_renderable_id, new_renderable);
    }

    pub fn update(&mut self, renderer: &Renderer, world: &World, resource_loader: &ResourceLoader) {
        for modification in &world.dirty_objects {
            match &modification.modification_type {
                ObjectModificationType::Mesh(modification_type) => match &modification_type {
                    ModificationType::Added => {
                        if let Some(world_object) = world.get_object(modification.id) {
                            self.add_object(
                                world_object,
                                modification.id,
                                resource_loader,
                                renderer,
                            );
                        }
                    }
                    ModificationType::Removed => {
                        let _ = self.renderables.remove(&modification.id);
                    }
                    ModificationType::TransformModified(new_transform) => {
                        if let Some(renderable) = self.renderables.get_mut(&modification.id) {
                            renderable.update_transform_render_state(
                                &renderer.queue,
                                new_transform,
                                modification.id,
                            );
                        }
                    }
                    ModificationType::MaterialModified(new_material) => {
                        if let Some(renderable) = self.renderables.get_mut(&modification.id) {
                            renderable
                                .update_material_render_state(&renderer.device, &new_material);
                        }
                    }
                },
                ObjectModificationType::Light(_modification_type) => todo!(),
            }
        }
    }

    pub fn render(
        &mut self,
        renderer: &Renderer,
        encoder: &mut CommandEncoder,
        final_fbo_image_texture: &SurfaceTexture,
        light_controller: &LightController,
        camera_controller: &CameraController,
        global_gpu_params_bind_group: &BindGroup,
        object_picker: &mut ObjectPickManager,
    ) -> Result<(), wgpu::SurfaceError> {
        self.post_process_manager.begin_frame();

        for action in self.actions_to_process.drain(..) {
            match action {
                RenderingAction::GenerateCubeMapFromEquirectangular => {
                    self.equirec_to_cubemap_renderer.render(encoder)
                }
                RenderingAction::BakeDiffuseIrradianceMap => {
                    self.diffuse_irradiance_renderer.render(
                        encoder,
                        &self.equirec_to_cubemap_renderer.cube_map_to_sample,
                    )
                }
                RenderingAction::SaveDiffuseIrradianceMapToFile => self
                    .diffuse_irradiance_renderer
                    .write_current_ibl_to_file(&renderer.device, None),
            }
        }

        let renderables = self.renderables.values();

        light_controller.render_shadows(encoder, renderables.clone());

        {
            let deferred_pass_items = renderables.clone().filter(|renderable| {
                renderable.description.rendering_options.pass == RenderingPass::DeferredMain
            });
            let mut render_pass = self.gbuffer_geometry_renderer.begin_render(encoder);
            self.gbuffer_geometry_renderer.render(
                &mut render_pass,
                deferred_pass_items,
                &camera_controller.bind_group,
                global_gpu_params_bind_group,
            );
        }

        object_picker.render(
            encoder,
            &renderer.device,
            renderables.clone(),
            &camera_controller.bind_group,
            &self.gbuffer_geometry_renderer.textures.depth_texture.view,
        );

        {
            let mut main_shading_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Main shading pass"),
                timestamp_writes: None,
            });

            self.main_rp.render(
                &mut main_shading_pass,
                &camera_controller,
                &light_controller,
                &self.gbuffer_geometry_renderer.gbuffer_textures_bind_group,
                light_controller.get_directional_lights_depth_texture_bgroup(),
                light_controller.get_point_lights_depth_texture_bgroup(),
                &self.diffuse_irradiance_renderer.diffuse_irradiance_cubemap,
                self.post_process_manager.get_next_ping_pong_bind_group(),
                renderer.config.width,
                renderer.config.height,
            );
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Skybox + forward rendering pass"),
                timestamp_writes: None,
                occlusion_query_set: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self
                        .post_process_manager
                        .full_screen_render_target_ping_pong_textures[0]
                        .view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &self.gbuffer_geometry_renderer.textures.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
            });

            self.forward_renderer.render(
                &mut render_pass,
                renderables.filter(|renderable| {
                    renderable.description.rendering_options.pass
                        == RenderingPass::ForceForwardAfterDeferred
                }),
                &camera_controller.bind_group,
                &light_controller.get_light_bind_group(),
            );
            self.skybox.render(
                &mut render_pass,
                &camera_controller,
                &self.equirec_to_cubemap_renderer.cube_map_to_sample,
            );
        }

        {
            // Unfortunately I can't do this in the same pass, because of the pass' and encoder's lifetime
            {
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Postprocessing"),
                    timestamp_writes: None,
                });

                self.post_process_manager.render_dummy(
                    &mut compute_pass,
                    renderer.config.width,
                    renderer.config.height,
                    global_gpu_params_bind_group,
                );
            }

            {
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Postprocessing"),
                    timestamp_writes: None,
                });
                self.post_process_manager.render_screen_space_reflections(
                    &mut compute_pass,
                    renderer.config.width,
                    renderer.config.height,
                    global_gpu_params_bind_group,
                    &camera_controller.bind_group,
                    &self.equirec_to_cubemap_renderer.cube_map_to_sample,
                    &self.gbuffer_geometry_renderer.gbuffer_textures_bind_group,
                    &self.gbuffer_geometry_renderer.depth_texture_bind_group,
                );
            }

            {
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Postprocessing"),
                    timestamp_writes: None,
                });
                self.post_process_manager.apply_tone_mapping(
                    &mut compute_pass,
                    renderer.config.width,
                    renderer.config.height,
                    global_gpu_params_bind_group,
                );
            }
        }

        encoder.copy_texture_to_texture(
            self.post_process_manager
                .full_screen_render_target_ping_pong_textures[2]
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
            self.main_rp.try_recompile_shader(device)?;
            self.gbuffer_geometry_renderer
                .try_recompile_shader(device)?;
            if self
                .equirec_to_cubemap_renderer
                .try_recompile_shader(device)?
                == ShaderCompilationSuccess::Recompiled
            {
                self.add_action(RenderingAction::GenerateCubeMapFromEquirectangular);
            }

            self.post_process_manager.try_recompile_shader(device)?;
            self.skybox.try_recompile_shader(device)?;
            self.forward_renderer.try_recompile_shader(device)?;
            if self
                .diffuse_irradiance_renderer
                .try_recompile_shader(device)?
                == ShaderCompilationSuccess::Recompiled
            {
                self.add_action(RenderingAction::BakeDiffuseIrradianceMap);
            }
        }

        Ok(())
    }

    pub fn handle_size_changed(&mut self, renderer: &Renderer) {
        let width = renderer.config.width;
        let height = renderer.config.height;

        self.gbuffer_geometry_renderer
            .resize(&renderer.device, width, height);
        self.post_process_manager
            .resize(&renderer.device, width, height);
    }
}
