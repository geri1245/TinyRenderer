use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, MouseButton, WindowEvent},
    keyboard::{KeyCode, ModifiersState, PhysicalKey},
};

use crate::{
    app::{WindowEventHandlingAction, WindowEventHandlingResult},
    gizmo_handler::GizmoHandler,
    material::PbrMaterialDescriptor,
    model::{
        MeshDescriptor, ModelRenderingOptions, ModelDescriptor, PbrParameters, WorldObject,
    },
    world::World,
};

pub struct PlayerController {
    cursor_position: Option<PhysicalPosition<f64>>,
    is_left_button_pressed: bool,
    gizmo_handler: GizmoHandler,
    modifiers: ModifiersState,
}

impl PlayerController {
    pub fn new() -> Self {
        Self {
            cursor_position: None,
            is_left_button_pressed: false,
            gizmo_handler: GizmoHandler::new(),
            modifiers: ModifiersState::empty(),
        }
    }

    pub fn update(&mut self, world: &mut World) {
        self.gizmo_handler.update(world);
    }

    pub fn handle_window_event(
        &mut self,
        window_event: &WindowEvent,
        world: &mut World,
    ) -> WindowEventHandlingResult {
        if self.gizmo_handler.handle_window_event(window_event, world) {
            return WindowEventHandlingResult::Handled;
        }

        if world.camera_controller.process_window_event(window_event) {
            return WindowEventHandlingResult::Handled;
        }

        match window_event {
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position = Some(*position);

                // Pretend we didn't handle this event, so others will get it as well and can update the position
                return WindowEventHandlingResult::Unhandled;
            }
            WindowEvent::CursorLeft { .. } => {
                self.cursor_position = None;
                WindowEventHandlingResult::Unhandled
            }
            WindowEvent::MouseInput { state, button, .. } => match button {
                MouseButton::Left => {
                    match state {
                        ElementState::Pressed => {
                            self.is_left_button_pressed = true;
                        }
                        ElementState::Released => self.is_left_button_pressed = false,
                    }

                    return WindowEventHandlingResult::Handled;
                }
                _ => WindowEventHandlingResult::Unhandled,
            },
            WindowEvent::KeyboardInput { event, .. } => match event.physical_key {
                PhysicalKey::Code(KeyCode::Delete) => {
                    if let Some(id) = self.gizmo_handler.get_active_onject_id() {
                        world.remove_object(id);
                        self.gizmo_handler.remove_object_selection(world);
                        WindowEventHandlingResult::Handled
                    } else {
                        WindowEventHandlingResult::Unhandled
                    }
                }
                PhysicalKey::Code(KeyCode::KeyR) => {
                    if self.modifiers.contains(ModifiersState::CONTROL) {
                        WindowEventHandlingResult::RequestAction(
                            WindowEventHandlingAction::RecompileShaders,
                        )
                    } else {
                        WindowEventHandlingResult::Unhandled
                    }
                }
                PhysicalKey::Code(KeyCode::KeyW) => {
                    if self.modifiers.contains(ModifiersState::CONTROL) {
                        WindowEventHandlingResult::RequestAction(WindowEventHandlingAction::Exit)
                    } else {
                        WindowEventHandlingResult::Unhandled
                    }
                }
                _ => WindowEventHandlingResult::Unhandled,
            },
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();

                WindowEventHandlingResult::Unhandled
            }
            WindowEvent::DroppedFile(path) => {
                let object = WorldObject::new(
                    ModelDescriptor {
                        mesh_descriptor: MeshDescriptor::FromFile(path.clone()),
                        material_descriptor: PbrMaterialDescriptor::Flat(PbrParameters::default()),
                    },
                    None,
                    false,
                    ModelRenderingOptions::default(),
                );

                world.add_object(object);

                WindowEventHandlingResult::Handled
            }
            _ => WindowEventHandlingResult::Unhandled,
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
