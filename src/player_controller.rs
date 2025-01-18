use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, MouseButton, WindowEvent},
    keyboard::{KeyCode, ModifiersState, PhysicalKey},
};

use crate::{
    app::{WindowEventHandlingAction, WindowEventHandlingResult},
    components::{RenderableComponent, SceneComponentType, TransformComponent},
    gizmo_handler::GizmoHandler,
    material::PbrMaterialDescriptor,
    model::{MeshDescriptor, ModelRenderingOptions, PbrParameters},
    object_picker::ObjectPickManager,
    world::World,
    world_object::WorldObject,
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
        object_picker: &ObjectPickManager,
    ) -> WindowEventHandlingResult {
        if self
            .gizmo_handler
            .handle_window_event(window_event, world, object_picker)
        {
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
                    if let Some(id) = self.gizmo_handler.get_active_object_id() {
                        world.remove_world_object(id);
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
                let renderable_component = RenderableComponent::new(
                    MeshDescriptor::FromFile(path.clone()),
                    PbrMaterialDescriptor::Flat(PbrParameters::default()),
                    ModelRenderingOptions::default(),
                    false,
                );

                let object = WorldObject::new(
                    vec![SceneComponentType::Renderable(renderable_component)],
                    TransformComponent::default(),
                );

                world.add_world_object(object);

                WindowEventHandlingResult::Handled
            }
            _ => WindowEventHandlingResult::Unhandled,
        }
    }
}
