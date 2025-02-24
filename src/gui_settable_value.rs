use std::ops::{Deref, DerefMut};

use crossbeam_channel::{unbounded, Receiver};
use ui_item::{SetPropertyFromUiDescription, UiDisplayDescription};
use winit::event_loop::EventLoopProxy;

use crate::{
    custom_event::{CustomEvent, GuiDeregistrationEvent, GuiRegistrationEvent},
    gui::SetItemFromUiParams,
};

pub struct GuiSettableValue<T> {
    data: T,
    category: String,
    receiver: Receiver<SetItemFromUiParams>,
    event_loop_proxy: EventLoopProxy<CustomEvent>,
}

impl<T> GuiSettableValue<T> {
    pub fn new(
        data: T,
        category: String,
        event_loop_proxy: &EventLoopProxy<CustomEvent>,
        item: UiDisplayDescription,
    ) -> Self {
        let (sender, receiver) = unbounded::<SetItemFromUiParams>();
        let _ = event_loop_proxy.send_event(CustomEvent::GuiRegistration(GuiRegistrationEvent {
            items: item,
            category: category.clone(),
            sender,
        }));

        Self {
            data,
            receiver,
            category,
            event_loop_proxy: event_loop_proxy.clone(),
        }
    }

    pub fn get_gui_changes(&mut self) -> Vec<Vec<SetPropertyFromUiDescription>> {
        let mut changes = vec![];

        while let Ok(value_changed_params) = self.receiver.try_recv() {
            if value_changed_params.category == self.category {
                changes.push(value_changed_params.item_setting_breadcrumbs);
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

impl<T> Drop for GuiSettableValue<T> {
    fn drop(&mut self) {
        let _ = self
            .event_loop_proxy
            .send_event(CustomEvent::GuiDeregistration(GuiDeregistrationEvent {
                category: self.category.clone(),
            }));
    }
}
