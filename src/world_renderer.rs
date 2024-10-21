use std::collections::VecDeque;

use async_std::task::block_on;
use wgpu::{CommandEncoder, Device, Extent3d, RenderPassDepthStencilAttachment, SurfaceTexture};

use crate::{
    actions::RenderingAction,
    camera_controller::CameraController,
    diffuse_irradiance_renderer::DiffuseIrradianceRenderer,
    equirectangular_to_cubemap_renderer::EquirectangularToCubemapRenderer,
    forward_renderer::ForwardRenderer,
    gbuffer_geometry_renderer::GBufferGeometryRenderer,
    instance::TransformComponent,
    light_controller::LightController,
    model::{Renderable, WorldObject},
    object_picker::ObjectPickManager,
    pipelines::{self, MainRP, ShaderCompilationSuccess},
    post_process_manager::PostProcessManager,
    renderer::Renderer,
    resource_loader::{PrimitiveShape, ResourceLoader},
    skybox::Skybox,
    super_hash_map::SuperHashMap,
};

pub struct WorldRenderer {
    pub diffuse_irradiance_renderer: DiffuseIrradianceRenderer,

    skybox: Skybox,
    main_rp: MainRP,
    post_process_manager: PostProcessManager,
    forward_renderer: ForwardRenderer,
    gbuffer_geometry_renderer: GBufferGeometryRenderer,
    equirec_to_cubemap_renderer: EquirectangularToCubemapRenderer,
    pub object_picker: ObjectPickManager,

    first_render: bool,
    actions_to_process: VecDeque<RenderingAction>,

    renderables: SuperHashMap<Renderable>,
    dirty_objects: Vec<u32>,

    /// These are waiting to be loaded
    pending_renderables: Vec<(u32, WorldObject)>,
}

impl WorldRenderer {
    pub async fn new(renderer: &Renderer, resource_loader: &mut ResourceLoader) -> Self {
        let main_rp = pipelines::MainRP::new(&renderer.device).await.unwrap();
        let gbuffer_geometry_renderer = GBufferGeometryRenderer::new(
            &renderer.device,
            renderer.config.width,
            renderer.config.height,
        )
        .await;

        let forward_rp =
            ForwardRenderer::new(&renderer.device, wgpu::TextureFormat::Rgba16Float).await;

        let post_process_manager = PostProcessManager::new(
            &renderer.device,
            renderer.config.width,
            renderer.config.height,
        )
        .await;

        let skybox = Skybox::new(
            &renderer.device,
            post_process_manager.full_screen_render_target_ping_pong_textures[0]
                .texture
                .format(),
        )
        .await;

        // TODO: change the format, or use some constant here
        let equirec_to_cubemap_renderer = EquirectangularToCubemapRenderer::new(
            &renderer.device,
            &renderer.queue,
            wgpu::TextureFormat::Rgba16Float,
            resource_loader.get_primitive_shape(PrimitiveShape::Cube),
        )
        .await
        .unwrap();

        // TODO: change the format, or use some constant here
        let diffuse_irradiance_renderer = DiffuseIrradianceRenderer::new(
            &renderer.device,
            &renderer.queue,
            wgpu::TextureFormat::Rgba16Float,
            resource_loader.get_primitive_shape(PrimitiveShape::Cube),
        )
        .await
        .unwrap();

        let object_picker = ObjectPickManager::new(
            &renderer.device,
            renderer.config.width,
            renderer.config.height,
        )
        .await;

        WorldRenderer {
            skybox,
            main_rp,
            gbuffer_geometry_renderer,
            forward_renderer: forward_rp,
            object_picker,

            post_process_manager,
            equirec_to_cubemap_renderer,
            diffuse_irradiance_renderer,
            first_render: true,
            actions_to_process: VecDeque::new(),
            renderables: SuperHashMap::new(),
            pending_renderables: Vec::new(),
            dirty_objects: Vec::new(),
        }
    }

    pub fn add_action(&mut self, action: RenderingAction) {
        self.actions_to_process.push_back(action);
    }

    pub fn add_object(&mut self, new_renderable_descriptor: WorldObject, new_renderable_id: u32) {
        self.pending_renderables
            .push((new_renderable_id, new_renderable_descriptor));
    }

    pub fn remove_object(&mut self, renderable_id_to_remove: u32) {
        self.renderables.remove(renderable_id_to_remove);
    }

    pub fn update_object_transform(&mut self, id: u32, new_transform: TransformComponent) {
        if let Some(renderable) = self.renderables.get_mut(id) {
            renderable.description.transform = new_transform;
            self.mark_object_dirty(id);
        }
    }

    fn mark_object_dirty(&mut self, id: u32) {
        self.dirty_objects.push(id);
    }

    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resource_loader: &ResourceLoader,
    ) {
        for (object_id, object) in self.pending_renderables.drain(..) {
            let loaded_model = resource_loader
                .load_model(&object.object, device, queue)
                .unwrap();
            let new_renderable = Renderable::new(
                object.object.clone(),
                object.get_transform(),
                loaded_model.primitive,
                loaded_model.material,
                device,
                object_id,
            );
            self.renderables.insert(object_id, new_renderable);
        }

        for object_id in self.dirty_objects.drain(..) {
            if let Some(renderable) = self.renderables.get_mut(object_id) {
                renderable.update_transform_render_state(queue, object_id);
            }
        }

        self.object_picker.update();
    }

    pub fn render(
        &mut self,
        renderer: &Renderer,
        encoder: &mut CommandEncoder,
        final_fbo_image_texture: &SurfaceTexture,
        light_controller: &LightController,
        camera_controller: &CameraController,
    ) -> Result<(), wgpu::SurfaceError> {
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

        let renderables = self.renderables.into_iter();

        light_controller.render_shadows(encoder, renderables.clone());

        {
            let mut render_pass = self.gbuffer_geometry_renderer.begin_render(encoder);
            for renderable in renderables.clone() {
                self.gbuffer_geometry_renderer.render(
                    &mut render_pass,
                    renderable,
                    &camera_controller.bind_group,
                );
            }
        }

        self.object_picker.render(
            encoder,
            &renderer.device,
            renderables,
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
                &self.gbuffer_geometry_renderer.bind_group,
                light_controller.get_shadow_bind_group(),
                &self.diffuse_irradiance_renderer.diffuse_irradiance_cubemap,
                &self.post_process_manager.compute_bind_group_1_to_0,
                renderer.config.width,
                renderer.config.height,
            );
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("After GBuffer pass"),
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

            self.skybox.render(
                &mut render_pass,
                &camera_controller,
                &self.equirec_to_cubemap_renderer.cube_map_to_sample,
            );

            // for renderable in renderables {
            //     self.forward_renderer.render(
            //         &mut render_pass,
            //         mesh,
            //         &camera_controller.bind_group,
            //         &light_controller.light_bind_group,
            //     );
            // }
        }

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Postprocessing"),
                timestamp_writes: None,
            });

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

    pub fn post_render(&mut self) {
        self.object_picker.post_render();
    }

    pub fn recompile_shaders_if_needed(&mut self, device: &Device) -> anyhow::Result<()> {
        // Note, that we stop at the first error and don't process the other shaders if something goes wrong.
        // This is a conscious decision, as for now one usually touches one shader at a time, so
        // it's not a real limitation at this point
        // If later more heavyweight modifications are necessary, then this can be "fixed"
        {
            block_on(self.main_rp.try_recompile_shader(device))?;
            block_on(self.gbuffer_geometry_renderer.try_recompile_shader(device))?;
            if block_on(
                self.equirec_to_cubemap_renderer
                    .try_recompile_shader(device),
            )? == ShaderCompilationSuccess::Recompiled
            {
                self.add_action(RenderingAction::GenerateCubeMapFromEquirectangular);
            }

            block_on(self.post_process_manager.try_recompile_shader(device))?;
            block_on(self.skybox.try_recompile_shader(device))?;
            block_on(self.forward_renderer.try_recompile_shader(device))?;
            block_on(self.object_picker.try_recompile_shader(device))?;
            if block_on(
                self.diffuse_irradiance_renderer
                    .try_recompile_shader(device),
            )? == ShaderCompilationSuccess::Recompiled
            {
                self.add_action(RenderingAction::BakeDiffuseIrradianceMap);
            }
        }

        // Force the single-shot renderers to render again
        self.first_render = true;

        Ok(())
    }

    pub fn handle_size_changed(&mut self, renderer: &Renderer, width: u32, height: u32) {
        self.gbuffer_geometry_renderer
            .resize(&renderer.device, width, height);
        self.post_process_manager
            .resize(&renderer.device, width, height);
        self.object_picker.resize(&renderer.device, width, height);
    }
}
