use winit::{
    dpi::LogicalSize,
    event::{DeviceEvent, Event},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

use crate::app::{self};

pub async fn run_main_loop() {
    simple_logger::init_with_level(log::Level::Warn).unwrap();
    let event_loop = EventLoop::new().unwrap();

    let window = WindowBuilder::new()
        .with_inner_size(LogicalSize::new(1200.0, 800.0))
        .build(&event_loop)
        .unwrap();
    // window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
    window.set_title("Awesome application");

    let mut app = app::App::new(&window).await;

    event_loop
        .run(move |event, control_flow| {
            match event {
                Event::WindowEvent { event, window_id } if window_id == window.id() => {
                    app.handle_window_event(&window, &event);
                }
                // TODO: instead of doing this from here, trigger the redraw from inside the app
                // using the limited framerate
                Event::AboutToWait => {
                    window.request_redraw();
                }
                Event::DeviceEvent { event, .. } => {
                    if let DeviceEvent::Key(ref input) = event {
                        if input.physical_key == PhysicalKey::Code(KeyCode::Escape) {
                            control_flow.exit();
                            return;
                        }
                    }
                }
                _ => {}
            }
        })
        .unwrap();
}
