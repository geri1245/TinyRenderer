use crossbeam_channel::{Receiver, Sender};

use crate::gui::GuiEvent;

enum _GuiShaderCompilationMessage {
    Successful(String),
    Failed(String, String),
}

struct _ShaderManager {
    gui_message_receiver: Receiver<GuiEvent>,
    gui_compilation_result_sender: Sender<_GuiShaderCompilationMessage>,
}
