use core::f32;
use std::{collections::HashMap, path::PathBuf, str::FromStr};

use glam::{Quat, Vec3};

use crate::{
    instance::TransformComponent,
    material::PbrMaterialDescriptor,
    math::Line,
    model::{MeshSource, ObjectWithMaterial, PbrParameters, WorldObject},
    world::World,
};

pub enum GizmoUpdateResult {
    Nothing,
    GizmoAddedWithPosition,
    GizmoSelectedWithAxis(Line),
}

pub struct Gizmo {
    pub active_object_id: Option<u32>,
    pub gizmo_position: Option<Vec3>,
    gizmo_parts: HashMap<u32, Vec3>,
}

impl Gizmo {
    pub fn new() -> Self {
        Self {
            active_object_id: None,
            gizmo_parts: HashMap::new(),
            gizmo_position: None,
        }
    }

    pub fn get_axis_with_id(&self, id: u32) -> Option<&Vec3> {
        self.gizmo_parts.get(&id)
    }

    pub fn update_with_new_object_id(
        &mut self,
        object_id: Option<u32>,
        world: &mut World,
    ) -> GizmoUpdateResult {
        // Clean up old gizmo, if necessary. If a gizmo is selected, we don't want to remove it from the world
        if object_id.is_none() || !self.gizmo_parts.contains_key(&object_id.unwrap()) {
            match self.active_object_id {
                Some(id) => {
                    if object_id.is_none() || object_id.unwrap() != id {
                        for (gizmo_id, _) in self.gizmo_parts.drain() {
                            world.remove_object(gizmo_id);
                        }
                        self.gizmo_position = None;
                    }
                }
                None => {}
            }
        }

        // Add new gizmo
        match object_id {
            Some(object_id) => {
                if let Some(axis) = self.get_axis_with_id(object_id) {
                    if let Some(gizmo_position) = self.gizmo_position {
                        // Gizmo was selected, don't show new gizmo
                        return GizmoUpdateResult::GizmoSelectedWithAxis(Line {
                            position: gizmo_position,
                            direction: *axis,
                        });
                    } else {
                        log::warn!("This should not happen! When selecting a gizmo, we should have a valid position");
                        self.active_object_id = None;
                        GizmoUpdateResult::Nothing
                    }
                } else {
                    if let Some(object) = world.get_object(object_id) {
                        self.active_object_id = Some(object_id);
                        let selected_object_transform = object.get_transform();
                        let arrow_object = ObjectWithMaterial {
                            mesh_source: MeshSource::FromFile(
                                PathBuf::from_str("./assets/models/arrow/arrow.obj").unwrap(),
                            ),
                            material_descriptor: PbrMaterialDescriptor::Flat(
                                PbrParameters::default(),
                            ),
                        };

                        self.gizmo_position = Some(selected_object_transform.position);

                        let gizmo_parts = vec![
                            (
                                Vec3::X,
                                Quat::from_axis_angle(Vec3::Z, -f32::consts::FRAC_PI_2),
                            ),
                            (Vec3::Y, Quat::IDENTITY),
                            (
                                Vec3::Z,
                                Quat::from_axis_angle(Vec3::X, f32::consts::FRAC_PI_2),
                            ),
                        ];

                        for (gizmo_axis, rotation) in gizmo_parts {
                            let gizmo_transform = TransformComponent {
                                position: selected_object_transform.position,
                                scale: Vec3::splat(1.0),
                                rotation,
                            };
                            let gizmo_id = world.add_object(WorldObject::new(
                                arrow_object.clone(),
                                Some(gizmo_transform),
                            ));
                            self.gizmo_parts.insert(gizmo_id, gizmo_axis);
                        }

                        GizmoUpdateResult::GizmoAddedWithPosition
                    } else {
                        self.active_object_id = None;
                        GizmoUpdateResult::Nothing
                    }
                }
            }
            None => {
                self.active_object_id = object_id;
                GizmoUpdateResult::Nothing
            }
        }
    }

    pub fn update_position(&mut self, new_position: Vec3, world: &mut World) {
        self.gizmo_position = Some(new_position);
        for (id, _axis) in &self.gizmo_parts {
            let object = world.get_object_mut(*id).unwrap();
            object.set_location(new_position);
        }
    }
}
