use winit::{
    event::{DeviceEvent, Event, WindowEvent},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

use crate::app::{self, WindowEventHandlingResult};

pub async fn run_main_loop() {
    simple_logger::init_with_level(log::Level::Warn).unwrap();
    let event_loop = EventLoop::new().unwrap();

    let window = WindowBuilder::new().build(&event_loop).unwrap();
    // window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
    window.set_title("Awesome application");

    let mut app = app::App::new(&window).await;

    event_loop
        .run(move |event, control_flow| {
            match event {
                Event::WindowEvent { event, window_id } if window_id == window.id() => {
                    app.handle_event(&window, &event);
                    match event {
                        WindowEvent::Resized(new_size) => app.resize(new_size),
                        WindowEvent::CloseRequested => control_flow.exit(),
                        // WindowEvent::ScaleFactorChanged {
                        //     scale_factor,
                        //     inner_size_writer,
                        // } => todo!(),
                        WindowEvent::RedrawRequested => {
                            match app.run_frame(&window) {
                                Ok(_) => (),
                                // Reconfigure the surface if lost
                                Err(wgpu::SurfaceError::Lost) => app.reconfigure(),
                                // The system is out of memory, we should probably quit
                                Err(wgpu::SurfaceError::OutOfMemory) => control_flow.exit(),
                                // All other errors (Outdated, Timeout) should be resolved by the next frame
                                Err(e) => eprintln!("{:?}", e),
                            }
                        }
                        _ => {}
                    }
                    if let WindowEventHandlingResult::RequestExit = app.handle_window_event(event) {
                        control_flow.exit();
                    }
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
