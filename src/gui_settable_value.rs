use std::ops::{Deref, DerefMut};

use crossbeam_channel::{unbounded, Receiver};
use ui_item::UiDisplayDescription;
use winit::event_loop::EventLoopProxy;

use crate::{
    custom_event::{CustomEvent, GuiRegistrationEvent},
    gui::SetItemFromUiParams,
};

pub struct GuiSettableValue<T> {
    data: T,
    category: String,
    receiver: Receiver<SetItemFromUiParams>,
}

impl<T> GuiSettableValue<T> {
    pub fn new(
        data: T,
        category: String,
        event_loop_proxy: &EventLoopProxy<CustomEvent>,
        items: UiDisplayDescription,
    ) -> Self {
        let (sender, receiver) = unbounded::<SetItemFromUiParams>();
        let _ = event_loop_proxy.send_event(CustomEvent::GuiRegistration(GuiRegistrationEvent {
            register: true,
            items,
            category: category.clone(),
            sender,
        }));

        Self {
            data,
            receiver,
            category,
        }
    }

    pub fn handle_gui_changes(&mut self) -> Vec<SetItemFromUiParams> {
        let mut changes = Vec::new();

        while let Ok(value_changed_params) = self.receiver.try_recv() {
            if value_changed_params.category == self.category {
                changes.push(value_changed_params);
            }
        }

        changes
    }
}

impl<T> Deref for GuiSettableValue<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> DerefMut for GuiSettableValue<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}
