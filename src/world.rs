use std::collections::HashMap;

use wgpu::{CommandEncoder, Device, SurfaceTexture};

use crate::{
    camera_controller::CameraController,
    input_actions::RenderingAction,
    light_controller::LightController,
    lights::Light,
    model::{ObjectWithMaterial, WorldObject},
    renderer::Renderer,
    resource_loader::ResourceLoader,
    world_renderer::WorldRenderer,
};

pub struct ObjectHandle {
    id: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DirtyState {
    /// No changes, nothing needs to be updated
    NothingChanged,
    /// In this case we might have to regenerate the buffers, as the number of items might have changed
    ItemsChanged,
    /// In this case it's enough to copy the new data to the existing buffers,
    /// as the number/structure of items remains the same
    ItemPropertiesChanged,
}

pub struct World {
    world_renderer: WorldRenderer,

    meshes: HashMap<u32, WorldObject>,
    lights: Vec<Light>,
    lights_dirty_state: DirtyState,

    next_object_id: u32,
}

impl World {
    pub fn new(mut world_renderer: WorldRenderer) -> Self {
        // Initial environment cubemap generation from the equirectangular map
        world_renderer.add_action(RenderingAction::GenerateCubeMapFromEquirectangular);

        World {
            meshes: HashMap::new(),
            lights: vec![],
            lights_dirty_state: DirtyState::ItemsChanged,
            next_object_id: 0,
            world_renderer,
        }
    }

    pub fn add_object(&mut self, object: WorldObject) {
        self.meshes.insert(self.next_object_id, object.clone());
        self.world_renderer.add_object(object, self.next_object_id);

        self.next_object_id += 1;
    }

    pub fn add_light(&mut self, light: Light) -> usize {
        self.lights.push(light);

        self.lights.len()
    }

    pub fn get_light(&mut self, handle: &ObjectHandle) -> Option<&mut Light> {
        if handle.id < self.lights.len() {
            self.lights_dirty_state = DirtyState::ItemPropertiesChanged;
            Some(&mut self.lights[handle.id])
        } else {
            None
        }
    }

    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resource_loader: &ResourceLoader,
    ) {
        self.world_renderer.update(device, queue, resource_loader);
    }

    pub fn render(
        &mut self,
        renderer: &Renderer,
        encoder: &mut CommandEncoder,
        final_fbo_image_texture: &SurfaceTexture,
        light_controller: &LightController,
        camera_controller: &CameraController,
    ) -> Result<(), wgpu::SurfaceError> {
        self.world_renderer.render(
            renderer,
            encoder,
            final_fbo_image_texture,
            light_controller,
            camera_controller,
        )
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
    }

    // This will be raypicking in the future
    pub fn pick(&self) -> ObjectHandle {
        return ObjectHandle { id: 0 };
    }

    pub fn get_lights_dirty_state(&self) -> DirtyState {
        self.lights_dirty_state
    }

    pub fn set_lights_udpated(&mut self) {
        self.lights_dirty_state = DirtyState::NothingChanged;
    }

    pub fn get_lights(&self) -> &Vec<Light> {
        &self.lights
    }

    pub fn get_meshes(&self) -> Vec<&WorldObject> {
        self.meshes.values().collect::<Vec<_>>()
    }
}
