use log::warn;
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, MouseButton, WindowEvent},
};

use crate::{gui::GuiEvent, world::World};

enum ObjectManipulationState {
    Hovered(u32),
    Moved(u32),
    Scaled(u32),
    Rotated(u32),
    Selected(u32),
}

pub struct PlayerController {
    hovered_object: Option<u32>,
    cursor_position: Option<PhysicalPosition<f64>>,
    is_left_button_pressed: bool,
}

impl PlayerController {
    pub fn new() -> Self {
        Self {
            hovered_object: None,
            cursor_position: None,
            is_left_button_pressed: false,
        }
    }

    pub fn handle_window_event(&mut self, window_event: &WindowEvent, world: &mut World) -> bool {
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
                            if let Some(pos) = self.cursor_position {
                                let id = world.get_object_id_at(pos.x as u32, pos.y as u32);
                                warn!("{id:?}");
                            }
                        }
                        ElementState::Released => self.is_left_button_pressed = false,
                    }

                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub fn handle_gui_events(&mut self, gui_event: &GuiEvent, world: &mut World) -> bool {
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

        return false;
    }

    pub fn set_selected_object(&mut self, selected_object_id: Option<u32>) {
        self.hovered_object = selected_object_id;
    }
}
