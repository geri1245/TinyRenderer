use crossbeam_channel::Sender;
use ui_item::{SetPropertyFromUiParams, UiDisplayParam};

pub struct GuiRegistrationEvent {
    /// Register or deregister?
    pub register: bool,
    /// What items should be registered?
    pub items: Vec<UiDisplayParam>,
    /// The category under which the items will be displayed
    pub category: String,
    /// The channel through which the events can be sent from the gui
    pub sender: Sender<(String, SetPropertyFromUiParams)>,
}

/// Events that can be posted from inside the app. These are also handled in the event loop
pub enum CustomEvent {
    GuiRegistration(GuiRegistrationEvent),
}
