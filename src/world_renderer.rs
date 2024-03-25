use async_std::task::block_on;
use pipelines::PipelineRecreationResult;
use wgpu::{CommandEncoder, Device, Extent3d, SurfaceTexture};

use crate::{
    camera_controller::CameraController,
    equirectangular_to_cubemap_renderer::EquirectangularToCubemapRenderer,
    light_controller::LightController,
    model::{InstancedRenderableMesh, InstancedTexturedRenderableMesh},
    pipelines::{self, MainRP},
    post_process_manager::PostProcessManager,
    renderer::Renderer,
    resource_loader::{PrimitiveShape, ResourceLoader},
    skybox::Skybox,
};

pub enum MeshType {
    DebugMesh(InstancedRenderableMesh),
    TexturedMesh(InstancedTexturedRenderableMesh),
}

impl MeshType {
    pub fn _get_mesh(&self) -> &InstancedRenderableMesh {
        match self {
            MeshType::DebugMesh(mesh) => mesh,
            MeshType::TexturedMesh(mesh) => &mesh.mesh,
        }
    }
}

pub struct WorldRenderer {
    pub skybox: Skybox,
    main_rp: MainRP,
    post_process_manager: PostProcessManager,
    forward_rp: pipelines::ForwardRP,
    gbuffer_rp: pipelines::GBufferGeometryRP,
    equirec_to_cubemap_renderer: EquirectangularToCubemapRenderer,
}

impl WorldRenderer {
    pub async fn new(renderer: &Renderer, resource_loader: &mut ResourceLoader) -> Self {
        let main_rp = pipelines::MainRP::new(&renderer.device).await.unwrap();
        let gbuffer_rp = pipelines::GBufferGeometryRP::new(
            &renderer.device,
            renderer.config.width,
            renderer.config.height,
        )
        .await
        .unwrap();
        let forward_rp =
            pipelines::ForwardRP::new(&renderer.device, wgpu::TextureFormat::Rgba16Float);

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

        // TODO: change the format, or use some constant here
        let equirec_to_cubemap_renderer = EquirectangularToCubemapRenderer::new(
            &renderer.device,
            &renderer.queue,
            wgpu::TextureFormat::Rgba16Float,
            resource_loader.get_primitive_shape(PrimitiveShape::Cube),
        )
        .await
        .unwrap();

        WorldRenderer {
            skybox,
            main_rp,
            gbuffer_rp,
            forward_rp,

            post_process_manager,
            equirec_to_cubemap_renderer,
        }
    }

    pub fn render(
        &self,
        renderer: &Renderer,
        encoder: &mut CommandEncoder,
        final_fbo_image_texture: &SurfaceTexture,
        renderables: &Vec<MeshType>,
        light_controller: &LightController,
        camera_controller: &CameraController,
    ) -> Result<(), wgpu::SurfaceError> {
        {
            self.equirec_to_cubemap_renderer.render(encoder);

            light_controller.render_shadows(encoder, &renderables);

            {
                let mut render_pass = self.gbuffer_rp.begin_render(encoder);
                for renderable in renderables {
                    if let MeshType::TexturedMesh(mesh) = renderable {
                        self.gbuffer_rp.render_mesh(
                            &mut render_pass,
                            mesh,
                            &camera_controller.bind_group,
                        );
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
                    &camera_controller,
                    &light_controller,
                    &self.gbuffer_rp.bind_group,
                    &light_controller.shadow_bind_group,
                    &self.post_process_manager.compute_bind_group_1_to_0,
                    renderer.config.width,
                    renderer.config.height,
                );
            }
            {
                let mut render_pass = renderer.begin_render_pass(
                    encoder,
                    &self
                        .post_process_manager
                        .full_screen_render_target_ping_pong_textures[0]
                        .view,
                    &self.gbuffer_rp.textures.depth_texture.view,
                );

                self.skybox.render(
                    &mut render_pass,
                    &camera_controller,
                    &self.equirec_to_cubemap_renderer.cube_map_to_sample,
                );

                {
                    for renderable in renderables {
                        if let MeshType::DebugMesh(mesh) = renderable {
                            self.forward_rp.render_model(
                                &mut render_pass,
                                mesh,
                                &camera_controller.bind_group,
                                &light_controller.light_bind_group,
                            );
                        }
                    }
                }
            }
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

        Ok(())
    }

    pub fn handle_size_changed(&mut self, renderer: &Renderer, width: u32, height: u32) {
        self.gbuffer_rp.resize(&renderer.device, width, height);
        self.post_process_manager
            .resize(&renderer.device, width, height);
    }
}
