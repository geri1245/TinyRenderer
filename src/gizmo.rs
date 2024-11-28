use core::f32;
use std::{collections::HashMap, path::PathBuf, str::FromStr};

use glam::{Quat, Vec3};

use crate::{
    instance::TransformComponent,
    material::PbrMaterialDescriptor,
    math::Line,
    model::{
        MeshSource, ModelRenderingOptions, ObjectWithMaterial, PbrParameters, RenderingPass,
        WorldObject,
    },
    world::World,
};

const X_AXIS_COLOR: [f32; 3] = [1.0, 0.0, 0.0];
const Y_AXIS_COLOR: [f32; 3] = [0.0, 1.0, 0.0];
const Z_AXIS_COLOR: [f32; 3] = [0.0, 0.0, 1.0];
const HOVERED_GIZMO_COLOR: [f32; 3] = [0.9, 0.9, 0.0];

pub enum GizmoUpdateResult {
    Nothing,
    GizmoAddedWithPosition,
    GizmoSelectedWithAxis(Line),
}

#[derive(PartialEq, Eq, Hash)]
enum GizmoAxis {
    DragX,
    DragY,
    DragZ,
}

struct GizmoAxisDescription {
    axis_vec: Vec3,
    // What rotation do we need to get to this axis from the default arrow, which is Y up
    rotation: Quat,
    material: PbrMaterialDescriptor,
}

pub struct Gizmo {
    pub selected_object_id: Option<u32>,
    pub hovered_gizmo_part_id: Option<u32>,
    pub gizmo_position: Option<Vec3>,
    gizmo_parts_drawn: HashMap<u32, Vec3>,
    gizmo_part_descriptions: HashMap<GizmoAxis, GizmoAxisDescription>,
}

fn get_color_for_axis(axis_vec: Vec3) -> [f32; 3] {
    match axis_vec {
        Vec3::X => X_AXIS_COLOR,
        Vec3::Y => Y_AXIS_COLOR,
        Vec3::Z => Z_AXIS_COLOR,
        _ => [0.0, 0.0, 0.0],
    }
}

impl Gizmo {
    pub fn new() -> Self {
        let gizmo_part_configs = HashMap::from([
            (
                GizmoAxis::DragX,
                GizmoAxisDescription {
                    axis_vec: Vec3::X,
                    rotation: Quat::from_axis_angle(Vec3::Z, -f32::consts::FRAC_PI_2),
                    material: PbrMaterialDescriptor::from_color(get_color_for_axis(Vec3::X)),
                },
            ),
            (
                GizmoAxis::DragY,
                GizmoAxisDescription {
                    axis_vec: Vec3::Y,
                    rotation: Quat::IDENTITY,
                    material: PbrMaterialDescriptor::from_color(get_color_for_axis(Vec3::Y)),
                },
            ),
            (
                GizmoAxis::DragZ,
                GizmoAxisDescription {
                    axis_vec: Vec3::Z,
                    rotation: Quat::from_axis_angle(Vec3::X, f32::consts::FRAC_PI_2),
                    material: PbrMaterialDescriptor::from_color(get_color_for_axis(Vec3::Z)),
                },
            ),
        ]);

        Self {
            selected_object_id: None,
            gizmo_parts_drawn: HashMap::new(),
            gizmo_position: None,
            hovered_gizmo_part_id: None,
            gizmo_part_descriptions: gizmo_part_configs,
        }
    }

    pub fn get_axis_with_id(&self, id: u32) -> Option<&Vec3> {
        self.gizmo_parts_drawn.get(&id)
    }

    fn restore_hovered_gizmo_material_if_any(&self, world: &mut World) {
        if let Some(hovered_gizmo_part_id) = self.hovered_gizmo_part_id {
            if let Some(object) = world.get_object_mut(hovered_gizmo_part_id) {
                if let Some(axis) = self.gizmo_parts_drawn.get(&hovered_gizmo_part_id) {
                    let color = get_color_for_axis(*axis);
                    object.update_material(&PbrMaterialDescriptor::Flat(PbrParameters::new(
                        color, 1.0, 0.0,
                    )));
                }
            }
        }
    }

    pub fn set_hovered_object_id(&mut self, hovered_object_id: Option<u32>, world: &mut World) {
        // Nothing to do if we are already up to date with the hovered object
        if self.hovered_gizmo_part_id == hovered_object_id {
            return;
        }

        self.restore_hovered_gizmo_material_if_any(world);

        // We have a new valid hovered object
        if let Some(hovered_gizmo_part_id) = hovered_object_id {
            if self.gizmo_parts_drawn.contains_key(&hovered_gizmo_part_id) {
                self.hovered_gizmo_part_id = hovered_object_id;
                if let Some(object) = world.get_object_mut(hovered_gizmo_part_id) {
                    object.update_material(&PbrMaterialDescriptor::from_color(HOVERED_GIZMO_COLOR));
                }
            } else {
                self.hovered_gizmo_part_id = None;
            }
        } else {
            // The new hovered object ID is None -> nothing is hovered
            self.hovered_gizmo_part_id = None;
        };
    }

    pub fn update_with_new_object_id(
        &mut self,
        object_id: Option<u32>,
        world: &mut World,
    ) -> GizmoUpdateResult {
        // Clean up old gizmo, if necessary. If a gizmo is selected, we don't want to remove it from the world
        if object_id.is_none() || !self.gizmo_parts_drawn.contains_key(&object_id.unwrap()) {
            match self.selected_object_id {
                Some(id) => {
                    if object_id.is_none() || object_id.unwrap() != id {
                        for (gizmo_id, _) in self.gizmo_parts_drawn.drain() {
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
                        self.selected_object_id = None;
                        GizmoUpdateResult::Nothing
                    }
                } else {
                    if let Some(object) = world.get_object(object_id) {
                        self.selected_object_id = Some(object_id);
                        let selected_object_transform = object.get_transform();
                        let arrow_source = MeshSource::FromFile(
                            PathBuf::from_str("./assets/models/arrow/arrow.obj").unwrap(),
                        );

                        self.gizmo_position = Some(selected_object_transform.position);

                        for (_axis, gizmo_description) in &self.gizmo_part_descriptions {
                            let gizmo_transform = TransformComponent {
                                position: selected_object_transform.position,
                                scale: Vec3::splat(1.0),
                                rotation: gizmo_description.rotation,
                            };
                            let gizmo_id = world.add_object(WorldObject::new(
                                ObjectWithMaterial {
                                    material_descriptor: gizmo_description.material.clone(),
                                    mesh_source: arrow_source.clone(),
                                },
                                Some(gizmo_transform),
                                true,
                                ModelRenderingOptions {
                                    pass: RenderingPass::ForceForwardAfterDeferred,
                                    use_depth_test: false,
                                    cast_shadows: false,
                                    needs_projection: false,
                                },
                            ));
                            self.gizmo_parts_drawn
                                .insert(gizmo_id, gizmo_description.axis_vec);
                        }

                        GizmoUpdateResult::GizmoAddedWithPosition
                    } else {
                        self.selected_object_id = None;
                        GizmoUpdateResult::Nothing
                    }
                }
            }
            None => {
                self.selected_object_id = object_id;
                GizmoUpdateResult::Nothing
            }
        }
    }

    pub fn update_position(&mut self, new_position: Vec3, world: &mut World) {
        self.gizmo_position = Some(new_position);
        for (id, _axis) in &self.gizmo_parts_drawn {
            let object = world.get_object_mut(*id).unwrap();
            object.set_location(new_position);
        }
    }
}
