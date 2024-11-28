use glam::{Vec2, Vec3};
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, MouseButton, WindowEvent},
};

use crate::{
    camera_controller::CameraController,
    gizmo::{Gizmo, GizmoUpdateResult},
    math::Line,
    world::World,
};

const GIZMO_DRAG_SQUARAED_DISTANCE_THRESHOLD: f32 = 25.0;

fn squared_distance(pos1: &PhysicalPosition<f64>, pos2: &PhysicalPosition<f64>) -> f32 {
    let pos1 = Vec2::new(pos1.x as f32, pos1.y as f32);
    let pos2 = Vec2::new(pos2.x as f32, pos2.y as f32);

    pos1.distance_squared(pos2)
}

fn get_world_position_from_screen_position(
    camera_controller: &CameraController,
    screen_position: &PhysicalPosition<f64>,
) -> Vec3 {
    camera_controller.deproject_screen_to_world(Vec3::new(
        screen_position.x as f32,
        screen_position.y as f32,
        0.5,
    ))
}

#[derive(Debug, Copy, Clone)]
struct GizmoMoveInfo {
    /// Represents the starting point of the gizmo interaction and the axis of it
    gizmo_movement_axis: Line,
    /// Contains the difference between the interaction start point and the object position
    /// This is needed, so we can calculate the final object position from the gizmo position in each frame
    gizmo_interaction_and_object_position_difference: Vec3,
}

#[derive(Debug, Copy, Clone)]
enum GizmoInteractionState {
    Idle,
    WaitingForThresholdAfterPress(PhysicalPosition<f64>, GizmoMoveInfo),
    Moving(GizmoMoveInfo),
}

pub struct GizmoHandler {
    gizmo: Gizmo,
    interaction_state: GizmoInteractionState,
    cursor_position: Option<PhysicalPosition<f64>>,
}

impl GizmoHandler {
    pub fn new() -> Self {
        Self {
            gizmo: Gizmo::new(),
            interaction_state: GizmoInteractionState::Idle,
            cursor_position: None,
        }
    }

    pub fn remove_object_selection(&mut self, world: &mut World) {
        self.gizmo.update_with_new_object_id(None, world);
    }

    pub fn update(&mut self, world: &mut World) {
        self.gizmo.update(world);
    }

    pub fn handle_window_event(&mut self, event: &WindowEvent, world: &mut World) -> bool {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position = Some(*position);

                match self.interaction_state {
                    GizmoInteractionState::WaitingForThresholdAfterPress(
                        interaction_start_position,
                        gizmo_move_info,
                    ) => {
                        if self.cursor_position.is_some()
                            && squared_distance(
                                &interaction_start_position,
                                &self.cursor_position.unwrap(),
                            ) >= GIZMO_DRAG_SQUARAED_DISTANCE_THRESHOLD
                        {
                            self.interaction_state = GizmoInteractionState::Moving(gizmo_move_info);

                            self.perform_move(world, position, &gizmo_move_info);
                        }
                    }
                    GizmoInteractionState::Moving(gizmo_move_info) => {
                        self.perform_move(world, position, &gizmo_move_info);
                    }
                    GizmoInteractionState::Idle => {
                        if let Some(pos) = self.cursor_position {
                            let hovered_object_id =
                                world.get_object_id_at(pos.x as u32, pos.y as u32);
                            self.gizmo.set_hovered_object_id(hovered_object_id, world);
                        }
                    }
                }

                // Pretend we didn't handle this event, so others will get it as well and can update the position
                false
            }
            WindowEvent::MouseInput { state, button, .. } => match button {
                MouseButton::Right => {
                    let result = self.gizmo.update_with_new_object_id(None, world);
                    matches!(result, GizmoUpdateResult::GizmoRemoved)
                }
                MouseButton::Left => {
                    match state {
                        ElementState::Pressed => {
                            if let Some(pos) = self.cursor_position {
                                let selected_object_id =
                                    world.get_object_id_at(pos.x as u32, pos.y as u32);

                                match self
                                    .gizmo
                                    .update_with_new_object_id(selected_object_id, world)
                                {
                                    GizmoUpdateResult::GizmoSelectedWithAxis(gizmo_axis_line) => {
                                        if let Some(cursor_position) = self.cursor_position {
                                            let position_on_camera_ray =
                                                get_world_position_from_screen_position(
                                                    &world.camera_controller,
                                                    &cursor_position,
                                                );
                                            let camera_line = Line {
                                                position: world.camera_controller.camera.position,
                                                direction: (position_on_camera_ray
                                                    - world.camera_controller.camera.position)
                                                    .normalize(),
                                            };

                                            let (
                                                gizmo_axis_line_closest_point,
                                                _camera_line_closest_point,
                                            ) = gizmo_axis_line.distance(&camera_line);

                                            self.interaction_state =
                                        GizmoInteractionState::WaitingForThresholdAfterPress(
                                            self.cursor_position.unwrap(),
                                            GizmoMoveInfo { gizmo_movement_axis: Line{position: gizmo_axis_line_closest_point, direction: gizmo_axis_line.direction }, gizmo_interaction_and_object_position_difference:  self.gizmo.gizmo_position.unwrap() - gizmo_axis_line_closest_point }
                                        );
                                        }
                                    }
                                    _ => {}
                                }
                            } else {
                                self.gizmo.update_with_new_object_id(None, world);
                            }
                        }
                        ElementState::Released => {
                            self.interaction_state = GizmoInteractionState::Idle;
                        }
                    }

                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn perform_move(
        &mut self,
        world: &mut World,
        screen_position: &PhysicalPosition<f64>,
        gizmo_move_info: &GizmoMoveInfo,
    ) {
        let camera_ray_world_position =
            get_world_position_from_screen_position(&world.camera_controller, screen_position);
        let camera_ray_direction =
            camera_ray_world_position - world.camera_controller.camera.position;
        let camera_ray = Line {
            position: world.camera_controller.camera.position,
            direction: camera_ray_direction.normalize(),
        };

        let (gizmo_axis_point, _camera_axis_point) =
            gizmo_move_info.gizmo_movement_axis.distance(&camera_ray);

        let object = world
            .get_object_mut(self.gizmo.selected_object_id.unwrap())
            .unwrap();

        let new_position =
            gizmo_axis_point + gizmo_move_info.gizmo_interaction_and_object_position_difference;
        object.set_location(new_position);

        self.gizmo.update_position(new_position, world);
    }

    pub fn get_active_onject_id(&self) -> Option<u32> {
        self.gizmo.selected_object_id
    }
}
