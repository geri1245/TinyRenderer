use ui_item::{UiDisplayable, UiSettableNew};
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::EventLoopProxy,
    keyboard::{KeyCode, ModifiersState, PhysicalKey},
};

use crate::{
    app::{WindowEventHandlingAction, WindowEventHandlingResult},
    components::{RenderableComponent, SceneComponentType, TransformComponent},
    custom_event::CustomEvent,
    gizmo_handler::GizmoHandler,
    gui_settable_value::GuiSettableValue,
    material::PbrMaterialDescriptor,
    model::{MeshDescriptor, ModelRenderingOptions, PbrParameters},
    object_picker::ObjectPickManager,
    world::World,
    world_object::WorldObject,
};

const SELECTED_OBJECT_GUI_CATEGORY: &str = "Selected object";

pub struct PlayerController {
    cursor_position: Option<PhysicalPosition<f64>>,
    is_left_button_pressed: bool,
    gizmo_handler: GizmoHandler,
    modifiers: ModifiersState,

    // TODO: This should be refactored into something like a selection controller and the gizmo andler and this struct
    // should both be using the new selection controller
    selected_object: Option<u32>,
    gui_registered_object: Option<GuiSettableValue<u32>>,
}

impl PlayerController {
    pub fn new() -> Self {
        Self {
            cursor_position: None,
            is_left_button_pressed: false,
            gizmo_handler: GizmoHandler::new(),
            modifiers: ModifiersState::empty(),
            selected_object: None,
            gui_registered_object: None,
        }
    }

    fn update_registered_object(
        &mut self,
        world: &mut World,
        event_loop_proxy: &mut EventLoopProxy<CustomEvent>,
    ) {
        if let Some(new_selected_object_id) = self.gizmo_handler.get_active_object_id() {
            if let Some(current_registered_object) = &self.gui_registered_object {
                if **current_registered_object == new_selected_object_id {
                    return;
                }
            }

            if let Some(world_object) = world.get_world_object(&new_selected_object_id) {
                let ui_desc = world_object.get_ui_description();
                self.gui_registered_object = Some(GuiSettableValue::new(
                    new_selected_object_id,
                    SELECTED_OBJECT_GUI_CATEGORY.to_string(),
                    event_loop_proxy,
                    ui_desc,
                ));
            }
        } else {
            self.gui_registered_object = None;
        }
    }

    pub fn update(
        &mut self,
        world: &mut World,
        event_loop_proxy: &mut EventLoopProxy<CustomEvent>,
    ) {
        self.gizmo_handler.update(world);

        self.update_registered_object(world, event_loop_proxy);

        if let Some(selected_object_id) = &mut self.gui_registered_object {
            if let Some(world_object) = world.get_world_object_mut(selected_object_id) {
                let changes = selected_object_id.get_gui_changes();
                for change in changes {
                    world_object.set_value_from_ui(&change);
                }
            }
        }
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
