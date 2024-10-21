use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, MouseButton, WindowEvent},
};

use crate::{
    gizmo_handler::GizmoHandler,
    material::PbrMaterialDescriptor,
    model::{MeshSource, ObjectWithMaterial, PbrParameters, WorldObject},
    world::World,
};

pub struct PlayerController {
    cursor_position: Option<PhysicalPosition<f64>>,
    is_left_button_pressed: bool,
    gizmo_handler: GizmoHandler,
}

impl PlayerController {
    pub fn new() -> Self {
        Self {
            cursor_position: None,
            is_left_button_pressed: false,
            gizmo_handler: GizmoHandler::new(),
        }
    }

    pub fn handle_window_event(&mut self, window_event: &WindowEvent, world: &mut World) -> bool {
        if self.gizmo_handler.handle_window_event(window_event, world) {
            return true;
        }

        if world.camera_controller.process_window_event(window_event) {
            return true;
        }

        match window_event {
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position = Some(*position);

                // Pretend we didn't handle this event, so others will get it as well and can update the position
                false
            }
            WindowEvent::CursorLeft { .. } => {
                self.cursor_position = None;
                false
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if *button == MouseButton::Left {
                    match state {
                        ElementState::Pressed => {
                            self.is_left_button_pressed = true;
                        }
                        ElementState::Released => self.is_left_button_pressed = false,
                    }

                    true
                } else {
                    false
                }
            }
            WindowEvent::DroppedFile(path) => {
                let object = WorldObject::new(
                    ObjectWithMaterial {
                        mesh_source: MeshSource::FromFile(path.clone()),
                        material_descriptor: PbrMaterialDescriptor::Flat(PbrParameters::default()),
                    },
                    None,
                );

                world.add_object(object);

                true
            }
            _ => false,
        }
    }

    // pub fn handle_gui_events(&mut self, gui_event: &GuiEvent, world: &mut World) -> bool {
    // match gui_event {
    //     GuiEvent::LightPositionChanged { new_position } => {
    //         if let Some(handle) = &selected_object_handle {
    //             if let Some(light) = world.get_light(handle) {
    //                 match light {
    //                     crate::lights::Light::Point(point_light) => {
    //                         point_light.transform.position = (*new_position).into();
    //                     }
    //                     crate::lights::Light::Directional(_) => {}
    //                 }
    //             }
    //         }
    //     }
    //     _ => {}
    // }

    //     return false;
    // }
}
