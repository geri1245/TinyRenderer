use crossbeam_channel::{Receiver, Sender};

use crate::gui::GuiEvent;

enum GuiShaderCompilationMessage {
    Successful(String),
    Failed(String, String),
}

struct ShaderManager {
    gui_message_receiver: Receiver<GuiEvent>,
    gui_compilation_result_sender: Sender<GuiShaderCompilationMessage>,
}
