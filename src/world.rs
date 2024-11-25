use std::{collections::HashMap, time::Duration};

use wgpu::{CommandEncoder, Device, SurfaceTexture};

use crate::{
    actions::RenderingAction, camera::Camera, camera_controller::CameraController,
    light_controller::LightController, lights::Light, model::WorldObject, renderer::Renderer,
    resource_loader::ResourceLoader, world_renderer::WorldRenderer,
};

pub struct World {
    pub world_renderer: WorldRenderer,
    pub camera_controller: CameraController,

    meshes: HashMap<u32, WorldObject>,
    lights: Vec<Light>,

    next_object_id: u32,
}

impl World {
    pub fn new(mut world_renderer: WorldRenderer, camera_controller: CameraController) -> Self {
        // Initial environment cubemap generation from the equirectangular map
        world_renderer.add_action(RenderingAction::GenerateCubeMapFromEquirectangular);

        World {
            meshes: HashMap::new(),
            lights: vec![],
            next_object_id: 1, // 0 stands for the placeholder "no object"
            world_renderer,
            camera_controller,
        }
    }

    pub fn add_object(&mut self, object: WorldObject) -> u32 {
        self.meshes.insert(self.next_object_id, object.clone());
        self.world_renderer.add_object(object, self.next_object_id);

        let ret_val = self.next_object_id;
        self.next_object_id += 1;

        ret_val
    }

    pub fn remove_object(&mut self, object_id_to_remove: u32) {
        self.meshes.remove(&object_id_to_remove);
        self.world_renderer.remove_object(object_id_to_remove);
    }

    pub fn get_object(&self, id: u32) -> Option<&WorldObject> {
        self.meshes.get(&id)
    }

    pub fn get_object_mut(&mut self, id: u32) -> Option<&mut WorldObject> {
        self.meshes.get_mut(&id)
    }

    pub fn get_object_id_at(&self, x: u32, y: u32) -> Option<u32> {
        self.world_renderer
            .object_picker
            .get_object_id_at_position(x, y)
    }

    pub fn add_light(&mut self, light: Light) -> usize {
        self.lights.push(light);

        self.lights.len()
    }

    pub fn set_camera(&mut self, camera: &Camera) {
        self.camera_controller.camera = camera.clone();
    }

    pub fn get_light(&mut self, handle: &u32) -> Option<&mut Light> {
        let id = *handle as usize;
        if id < self.lights.len() {
            Some(&mut self.lights[id])
        } else {
            None
        }
    }

    pub fn update(
        &mut self,
        delta: Duration,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resource_loader: &ResourceLoader,
    ) {
        for (id, mesh) in &mut self.meshes {
            if mesh.is_transform_dirty {
                self.world_renderer
                    .update_object_transform(*id, mesh.reset_transform_dirty());
            }
        }

        self.camera_controller.update(delta, queue);
        self.world_renderer.update(device, queue, resource_loader);
    }

    pub fn render(
        &mut self,
        renderer: &Renderer,
        encoder: &mut CommandEncoder,
        final_fbo_image_texture: &SurfaceTexture,
        light_controller: &LightController,
    ) -> Result<(), wgpu::SurfaceError> {
        self.world_renderer.render(
            renderer,
            encoder,
            final_fbo_image_texture,
            light_controller,
            &self.camera_controller,
        )
    }

    pub fn post_render(&mut self) {
        self.world_renderer.post_render();
    }

    pub fn recompile_shaders_if_needed(&mut self, device: &Device) -> anyhow::Result<()> {
        self.world_renderer.recompile_shaders_if_needed(device)
    }

    pub fn add_action(&mut self, action: RenderingAction) {
        self.world_renderer.add_action(action);
    }

    pub fn handle_size_changed(&mut self, renderer: &Renderer, width: u32, height: u32) {
        self.world_renderer
            .handle_size_changed(renderer, width, height);
        self.camera_controller.resize(width, height);
    }

    pub fn get_lights(&self) -> &Vec<Light> {
        &self.lights
    }

    pub fn get_meshes(&self) -> Vec<&WorldObject> {
        self.meshes.values().collect::<Vec<_>>()
    }
}
