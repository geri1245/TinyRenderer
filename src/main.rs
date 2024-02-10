use app::WindowEventHandlingResult;
use winit::{
    event::*,
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

mod app;
mod basic_renderable;
mod bind_group_layout_descriptors;
mod buffer_content;
mod camera;
mod camera_controller;
mod color;
mod frame_timer;
mod gui;
mod instance;
mod light_controller;
mod lights;
mod model;
mod pipelines;
mod primitive_shapes;
mod render_pipeline_layout;
mod renderer;
mod resource_map;
mod resources;
mod skybox;
mod texture;
mod vertex;
mod world;

const CLEAR_COLOR: wgpu::Color = wgpu::Color {
    r: 0.1,
    g: 0.2,
    b: 0.3,
    a: 1.0,
};

pub async fn run() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();

    let window = WindowBuilder::new().build(&event_loop).unwrap();
    // window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
    window.set_title("Awesome application");

    let mut app = app::App::new(&window).await;

    event_loop
        .run(move |event, control_flow| {
            app.handle_event(&window, &event);

            match event {
                Event::WindowEvent { event, window_id } if window_id == window.id() => {
                    match event {
                        WindowEvent::Resized(new_size) => app.resize(new_size),
                        WindowEvent::CloseRequested => control_flow.exit(),
                        // WindowEvent::ScaleFactorChanged {
                        //     scale_factor,
                        //     inner_size_writer,
                        // } => todo!(),
                        WindowEvent::RedrawRequested => {
                            match app.request_redraw(&window) {
                                Ok(_) => (),
                                // Reconfigure the surface if lost
                                Err(wgpu::SurfaceError::Lost) => app.resize(app.renderer.size),
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
                Event::AboutToWait => {
                    window.request_redraw();
                }
                Event::DeviceEvent {
                    event, device_id, ..
                } => {
                    if let DeviceEvent::Key(ref input) = event {
                        if input.physical_key == PhysicalKey::Code(KeyCode::Escape) {
                            control_flow.exit();
                            return;
                        }
                    }

                    app.handle_device_event(&window, device_id, event);
                }
                _ => {}
            }
        })
        .unwrap();
}

fn main() {
    async_std::task::block_on(run());
}
