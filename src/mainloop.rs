use async_std::task::block_on;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{DeviceEvent, DeviceId, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

use crate::app::{App, WindowEventHandlingAction, WindowEventHandlingResult};

// This event can be posted from inside the app and can be handled in fn user_event
// Add members later if needed
enum CustomEvent {
    RecompileShaders,
}

#[derive(Default)]
struct MainApplicationState {
    // Use an `Option` to allow the window to not be available until the
    // application is properly running.
    window: Option<Window>,
    app: Option<App>,
    frame_number: i32,
}

impl ApplicationHandler<CustomEvent> for MainApplicationState {
    /// This method is the entry point, this is where the creation logic should be
    // TODO: probably this won't handle multiple initializations gracefully, which doesn't seem to be a problem on
    // Windows for now, as this event only arrives once on startup, but we definitely should handle it!
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let new_window = event_loop
            .create_window(
                WindowAttributes::default()
                    .with_inner_size(PhysicalSize::new(1200, 800))
                    .with_title("Rendering is fun!"),
            )
            .unwrap();
        let app = block_on(App::new(&new_window));
        self.window = Some(new_window);
        self.app = Some(app);
    }
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // `unwrap` is fine, the window will always be available when receiving a window event.
        let window = self.window.as_ref().unwrap();
        let result = self
            .app
            .as_mut()
            .unwrap()
            .handle_window_event(&window, &event);

        if let WindowEventHandlingResult::RequestAction(WindowEventHandlingAction::Exit) = result {
            event_loop.exit();
        }
    }
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: CustomEvent) {
        todo!()
    }
    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if let DeviceEvent::Key(ref input) = event {
            if input.physical_key == PhysicalKey::Code(KeyCode::Escape) {
                event_loop.exit();
                return;
            }
        }
    }
    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
            self.frame_number += 1;
        }
    }
}

pub async fn run_main_loop() {
    simple_logger::init_with_level(log::Level::Warn).unwrap();
    let event_loop = EventLoop::<CustomEvent>::with_user_event().build().unwrap();
    let mut app_state = MainApplicationState::default();

    event_loop.run_app(&mut app_state).unwrap();
}
