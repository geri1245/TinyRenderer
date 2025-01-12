use std::ops::Deref;

use crossbeam_channel::{unbounded, Receiver};
use ui_item::{SetPropertyFromUiParams, UiDisplayParam};
use winit::event_loop::EventLoopProxy;

use crate::custom_event::{CustomEvent, GuiRegistrationEvent};

type UpdateFunType<T, ExtraParam> = Box<dyn Fn(&mut T, &SetPropertyFromUiParams, &ExtraParam)>;

pub struct GuiSettableValue<T, ExtraParam> {
    data: T,
    category: String,
    receiver: Receiver<(String, SetPropertyFromUiParams)>,
    update_fun: UpdateFunType<T, ExtraParam>,
}

impl<T, ExtraParam> GuiSettableValue<T, ExtraParam> {
    pub fn new(
        data: T,
        category: String,
        update_fun: UpdateFunType<T, ExtraParam>,
        event_loop_proxy: &EventLoopProxy<CustomEvent>,
        items: Vec<UiDisplayParam>,
    ) -> Self {
        let (sender, receiver) = unbounded::<(String, SetPropertyFromUiParams)>();
        let _ = event_loop_proxy.send_event(CustomEvent::GuiRegistration(GuiRegistrationEvent {
            register: true,
            items,
            category: category.clone(),
            sender,
        }));

        Self {
            data,
            receiver,
            update_fun,
            category,
        }
    }

    pub fn handle_gui_changes(&mut self, extra_param: &ExtraParam) {
        while let Ok((category, value_changed_params)) = self.receiver.try_recv() {
            if category == self.category {
                (self.update_fun)(&mut self.data, &value_changed_params, extra_param);
            }
        }
    }
}

impl<T, ExtraParam> Deref for GuiSettableValue<T, ExtraParam> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}
