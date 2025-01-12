use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    path::PathBuf,
    time::Duration,
};

use crate::{
    camera::Camera, camera_controller::CameraController, instance::TransformComponent,
    lights::Light, material::PbrMaterialDescriptor, model::WorldObject, renderer::Renderer,
};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GlobalWorldSettings {
    sykbox_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub enum ModificationType {
    Added,
    Removed,
    TransformModified(TransformComponent),
    MaterialModified(PbrMaterialDescriptor),
}

#[derive(Debug, Clone)]
pub enum ObjectModificationType {
    Mesh(ModificationType),
    Light(ModificationType),
}

#[derive(Debug, Clone)]
pub struct DirtyObject {
    pub id: u32,
    pub modification_type: ObjectModificationType,
}

pub struct World {
    pub camera_controller: CameraController,
    pub dirty_objects: Vec<DirtyObject>,

    meshes: HashMap<u32, WorldObject>,
    lights: Vec<Light>,
    global_settings: GlobalWorldSettings,

    next_object_id: u32,
}

pub struct ObjectSettingGuard<'a> {
    world: &'a mut World,
    id: u32,
}

impl<'a> ObjectSettingGuard<'a> {
    fn new(world: &'a mut World, id: u32) -> Self {
        Self { world, id }
    }
}

impl<'a> Drop for ObjectSettingGuard<'a> {
    fn drop(&mut self) {
        self.world.mark_object_dirty(self.id);
    }
}

impl<'a> Deref for ObjectSettingGuard<'a> {
    type Target = WorldObject;

    fn deref(&self) -> &Self::Target {
        &self.world.get_object(self.id).unwrap()
    }
}

impl<'a> DerefMut for ObjectSettingGuard<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.world.get_object_mut_internal(self.id)
    }
}

impl World {
    pub fn new(camera_controller: CameraController) -> Self {
        World {
            meshes: HashMap::new(),
            lights: vec![],
            dirty_objects: vec![],
            next_object_id: 1, // 0 stands for the placeholder "no object"
            camera_controller,
            global_settings: GlobalWorldSettings { sykbox_path: None },
        }
    }

    pub fn add_object(&mut self, object: WorldObject) -> u32 {
        self.meshes.insert(self.next_object_id, object.clone());
        self.dirty_objects.push(DirtyObject {
            id: self.next_object_id,
            modification_type: ObjectModificationType::Mesh(ModificationType::Added),
        });

        let ret_val = self.next_object_id;
        self.next_object_id += 1;

        ret_val
    }

    pub fn remove_object(&mut self, object_id_to_remove: u32) {
        self.meshes.remove(&object_id_to_remove);
        self.dirty_objects.push(DirtyObject {
            id: object_id_to_remove,
            modification_type: ObjectModificationType::Mesh(ModificationType::Removed),
        });
    }

    pub fn get_object(&self, id: u32) -> Option<&WorldObject> {
        self.meshes.get(&id)
    }

    pub fn get_object_mut<'a>(&'a mut self, id: u32) -> Option<ObjectSettingGuard> {
        Some(ObjectSettingGuard::new(self, id))
    }

    fn get_object_mut_internal(&mut self, id: u32) -> &mut WorldObject {
        self.meshes.get_mut(&id).unwrap()
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

    pub fn update(&mut self, delta: Duration, renderer: &Renderer) {
        self.camera_controller.update(delta, &renderer.queue);
    }

    pub fn on_end_frame(&mut self) {
        self.dirty_objects.clear();
    }

    pub fn handle_size_changed(&mut self, width: u32, height: u32) {
        self.camera_controller.resize(width, height);
    }

    pub fn get_lights(&self) -> &Vec<Light> {
        &self.lights
    }

    pub fn get_world_objects(&self) -> Vec<&WorldObject> {
        self.meshes.values().collect::<Vec<_>>()
    }

    fn mark_object_dirty(&mut self, id: u32) {
        let (maybe_new_mat, maybe_new_transform) = if let Some(object) = self.get_object(id) {
            let new_material = if object.is_material_dirty {
                Some(
                    object
                        .description
                        .model_descriptor
                        .material_descriptor
                        .clone(),
                )
            } else {
                None
            };

            let new_transform = if object.is_transform_dirty {
                Some(object.description.transform.clone())
            } else {
                None
            };

            (new_material, new_transform)
        } else {
            (None, None)
        };

        if let Some(new_material) = maybe_new_mat {
            self.dirty_objects.push(DirtyObject {
                id,
                modification_type: ObjectModificationType::Mesh(
                    ModificationType::MaterialModified(new_material),
                ),
            });
        }

        if let Some(new_transform) = maybe_new_transform {
            self.dirty_objects.push(DirtyObject {
                id,
                modification_type: ObjectModificationType::Mesh(
                    ModificationType::TransformModified(new_transform),
                ),
            });
        }
    }
}
