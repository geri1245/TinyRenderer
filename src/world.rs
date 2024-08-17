use crate::{lights::Light, model::Renderable, resource_loader::ResourceLoader};

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
    meshes: Vec<Renderable>,
    lights: Vec<Light>,
    lights_dirty_state: DirtyState,

    pending_lights: Vec<Light>,
}

impl World {
    pub fn new() -> Self {
        World {
            meshes: Vec::new(),
            lights: vec![],
            lights_dirty_state: DirtyState::ItemsChanged,
            pending_lights: vec![],
        }
    }

    pub fn add_object(&mut self, object: Renderable) {
        self.meshes.push(object);
    }

    pub fn add_light(&mut self, light: Light) -> usize {
        self.lights.push(light);

        if let Light::Point(_) = light {
            self.pending_lights.push(light);
        }

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

    pub fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {}

    // This will be raypicing in the future
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

    pub fn get_meshes(&self) -> &Vec<Renderable> {
        &self.meshes
    }
}
