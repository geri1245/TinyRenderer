use crate::{gui::GuiEvent, world::World};

pub struct PlayerController {}

impl PlayerController {
    pub fn new() -> Self {
        Self {}
    }

    pub fn handle_gui_events(&mut self, gui_event: &GuiEvent, world: &mut World) -> bool {
        // This pick shouldn't be here, the selected object should be part of the struct
        let selected_object_handle = Some(world.pick());

        match gui_event {
            GuiEvent::LightPositionChanged { new_position } => {
                if let Some(handle) = &selected_object_handle {
                    if let Some(light) = world.get_light(handle) {
                        match light {
                            crate::lights::Light::Point(point_light) => {
                                point_light.transform.position = (*new_position).into();
                            }
                            crate::lights::Light::Directional(_) => {}
                        }
                    }
                }
            }
            GuiEvent::RecompileShaders => {}
        }

        return false;
    }
}
