use std::{collections::HashMap, path::PathBuf, time::Duration};

use crate::{
    camera::Camera,
    camera_controller::CameraController,
    components::{OmnipresentComponentType, SceneComponentType},
    renderer::Renderer,
    world_object::{OmnipresentObject, WorldObject},
};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GlobalWorldSettings {
    sykbox_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub enum ModificationType {
    Added,
    Removed,
    Modified,
}

#[derive(Debug, Clone)]
pub struct DirtyObject {
    pub id: u32,
    pub modification_type: ModificationType,
}

pub struct World {
    pub camera_controller: CameraController,
    pub dirty_objects: Vec<DirtyObject>,

    world_objects: HashMap<u32, WorldObject>,
    omnipresent_objects: HashMap<u32, OmnipresentObject>,

    global_settings: GlobalWorldSettings,

    next_object_id: u32,
}

impl World {
    pub fn new(camera_controller: CameraController) -> Self {
        World {
            world_objects: HashMap::new(),
            omnipresent_objects: HashMap::new(),
            dirty_objects: vec![],
            next_object_id: 1, // 0 stands for the placeholder "no object"
            camera_controller,
            global_settings: GlobalWorldSettings { sykbox_path: None },
        }
    }

    pub fn add_world_object(&mut self, mut world_object: WorldObject) -> u32 {
        let new_object_id = self.next_object_id;

        if let Some(_light_component) = world_object.get_light_component() {
            world_object.add_light_debug_object();
        }

        self.dirty_objects.push(DirtyObject {
            id: new_object_id,
            modification_type: ModificationType::Added,
        });

        self.world_objects.insert(new_object_id, world_object);

        self.next_object_id += 1;

        new_object_id
    }

    pub fn add_omnipresent_object(&mut self, omnipresent_object: OmnipresentObject) -> u32 {
        let new_object_id = self.next_object_id;

        for component in &omnipresent_object.components {
            match component {
                OmnipresentComponentType::DirectionalLight(_directional_light) => {
                    self.dirty_objects.push(DirtyObject {
                        id: new_object_id,
                        modification_type: ModificationType::Added,
                    });
                }
            }
        }

        self.omnipresent_objects
            .insert(new_object_id, omnipresent_object);

        self.next_object_id += 1;

        new_object_id
    }

    pub fn remove_world_object(&mut self, object_id_to_remove: u32) {
        self.world_objects.remove(&object_id_to_remove);
        self.dirty_objects.push(DirtyObject {
            id: object_id_to_remove,
            modification_type: ModificationType::Removed,
        });
    }

    pub fn get_world_object(&self, id: &u32) -> Option<&WorldObject> {
        self.world_objects.get(id)
    }

    pub fn get_world_object_mut(&mut self, id: &u32) -> Option<&mut WorldObject> {
        self.dirty_objects.push(DirtyObject {
            id: *id,
            modification_type: ModificationType::Modified,
        });

        self.world_objects.get_mut(id)
    }

    pub fn get_omnipresent_object(&self, id: &u32) -> Option<&OmnipresentObject> {
        self.omnipresent_objects.get(id)
    }

    pub fn get_omnipresent_object_mut(&mut self, id: &u32) -> Option<&mut WorldObject> {
        self.dirty_objects.push(DirtyObject {
            id: *id,
            modification_type: ModificationType::Modified,
        });

        self.world_objects.get_mut(id)
    }

    pub fn set_camera(&mut self, camera: &Camera) {
        self.camera_controller.camera = camera.clone();
    }

    pub fn update(&mut self, delta: Duration, renderer: &Renderer) {
        self.camera_controller.update(delta, &renderer.queue);
    }

    pub fn on_end_frame(&mut self) {
        let dirty_objects = self
            .dirty_objects
            .drain(..)
            .map(|item| item.id)
            .collect::<Vec<_>>();
        for dirty_object_id in dirty_objects {
            if let Some(world_object) = self.get_world_object_mut(&dirty_object_id) {
                world_object.on_end_frame();
            } else if let Some(omnipresent_object) =
                self.get_omnipresent_object_mut(&dirty_object_id)
            {
                omnipresent_object.on_end_frame();
            }
        }

        self.dirty_objects.clear();
    }

    pub fn handle_size_changed(&mut self, width: u32, height: u32) {
        self.camera_controller.resize(width, height);
    }

    pub fn get_omnipresent_objects(&self) -> Vec<&OmnipresentObject> {
        self.omnipresent_objects.values().collect::<Vec<_>>()
    }

    pub fn get_world_objects(&self) -> Vec<&WorldObject> {
        self.world_objects.values().collect::<Vec<_>>()
    }
}
