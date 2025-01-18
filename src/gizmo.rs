use core::f32;
use std::{collections::HashMap, path::PathBuf, str::FromStr};

use glam::{Quat, Vec3};

use crate::{
    components::{RenderableComponent, SceneComponentType, TransformComponent},
    material::PbrMaterialDescriptor,
    math::Line,
    model::{MeshDescriptor, ModelRenderingOptions, PbrParameters, RenderingPass},
    world::World,
    world_object::WorldObject,
};

const X_AXIS_COLOR: [f32; 3] = [1.0, 0.0, 0.0];
const Y_AXIS_COLOR: [f32; 3] = [0.0, 1.0, 0.0];
const Z_AXIS_COLOR: [f32; 3] = [0.0, 0.0, 1.0];
const HOVERED_GIZMO_COLOR: [f32; 3] = [0.9, 0.9, 0.0];
const GIZMO_DISTANCE_SCALE: f32 = 0.06;

pub enum GizmoUpdateResult {
    Nothing,
    GizmoAddedWithPosition,
    GizmoSelectedWithAxis(Line),
    GizmoRemoved,
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

    fn calculate_gizmo_scale(camera_position: Vec3, selected_object_position: Vec3) -> f32 {
        camera_position.distance(selected_object_position) * GIZMO_DISTANCE_SCALE
    }

    pub fn update(&mut self, world: &mut World) {
        if let Some(selected_object_id) = self.selected_object_id {
            let maybe_selected_object_position =
                if let Some(selected_object) = world.get_world_object(&selected_object_id) {
                    Some(selected_object.transform.get_position())
                } else {
                    None
                };

            if let Some(selected_object_position) = maybe_selected_object_position {
                let camera_position = world.camera_controller.camera.get_position();
                let gizmo_scale =
                    Self::calculate_gizmo_scale(selected_object_position, camera_position);

                for (gizmo_object_id, _axis) in &self.gizmo_parts_drawn {
                    if let Some(gizmo_object) = world.get_world_object_mut(gizmo_object_id) {
                        gizmo_object.transform.set_scale(gizmo_scale);
                    }
                }
            }
        }
    }

    fn restore_hovered_gizmo_material_if_any(&self, world: &mut World) {
        if let Some(hovered_gizmo_part_id) = self.hovered_gizmo_part_id {
            if let Some(object) = world.get_world_object_mut(&hovered_gizmo_part_id) {
                if let Some(axis) = self.gizmo_parts_drawn.get(&hovered_gizmo_part_id) {
                    if let Some(renderable) = object.get_renderable_component_mut() {
                        let color = get_color_for_axis(*axis);
                        renderable.update_material(PbrMaterialDescriptor::Flat(
                            PbrParameters::new(color, 1.0, 0.0),
                        ));
                    }
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
                if let Some(object) = world.get_world_object_mut(&hovered_gizmo_part_id) {
                    if let Some(renderable) = object.get_renderable_component_mut() {
                        renderable.update_material(PbrMaterialDescriptor::from_color(
                            HOVERED_GIZMO_COLOR,
                        ));
                    }
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
        new_selected_object_id: Option<u32>,
        world: &mut World,
    ) -> GizmoUpdateResult {
        // Clean up old gizmo, if necessary. If a gizmo is selected, we don't want to remove it from the world
        let removed_old_gizmo_now = if new_selected_object_id.is_none()
            || !self
                .gizmo_parts_drawn
                .contains_key(&new_selected_object_id.unwrap())
        {
            if let Some(selected_object_id) = self.selected_object_id {
                if new_selected_object_id.is_none()
                    || new_selected_object_id.unwrap() != selected_object_id
                {
                    for (gizmo_id, _) in self.gizmo_parts_drawn.drain() {
                        world.remove_world_object(gizmo_id);
                    }
                    self.gizmo_position = None;
                }

                true
            } else {
                false
            }
        } else {
            false
        };

        // Add new gizmo
        match new_selected_object_id {
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
                    if let Some(object) = world.get_world_object(&object_id) {
                        self.selected_object_id = Some(object_id);
                        let selected_object_transform = object.transform;
                        let arrow_source = MeshDescriptor::FromFile(
                            PathBuf::from_str("./assets/models/arrow/arrow.obj").unwrap(),
                        );

                        self.gizmo_position = Some(selected_object_transform.get_position());

                        for (_axis, gizmo_description) in &self.gizmo_part_descriptions {
                            let gizmo_transform = TransformComponent::new(
                                selected_object_transform.get_position(),
                                Vec3::splat(Self::calculate_gizmo_scale(
                                    world.camera_controller.camera.get_position(),
                                    selected_object_transform.get_position(),
                                )),
                                gizmo_description.rotation,
                            );

                            let renderable_component = RenderableComponent::new(
                                arrow_source.clone(),
                                gizmo_description.material.clone(),
                                ModelRenderingOptions {
                                    pass: RenderingPass::ForceForwardAfterDeferred,
                                    use_depth_test: false,
                                    cast_shadows: false,
                                },
                                true,
                            );

                            let world_object = WorldObject::new(
                                vec![SceneComponentType::Renderable(renderable_component)],
                                gizmo_transform,
                            );

                            let gizmo_id = world.add_world_object(world_object);
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
                self.selected_object_id = new_selected_object_id;
                if removed_old_gizmo_now {
                    GizmoUpdateResult::GizmoRemoved
                } else {
                    GizmoUpdateResult::Nothing
                }
            }
        }
    }

    pub fn update_position(&mut self, new_position: Vec3, world: &mut World) {
        self.gizmo_position = Some(new_position);
        for (id, _axis) in &self.gizmo_parts_drawn {
            if let Some(object) = world.get_world_object_mut(id) {
                object.transform.set_location(new_position);
            }
        }
    }
}
