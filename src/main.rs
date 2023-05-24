use app::WindowEventHandlingResult;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

mod app;
mod basic_renderable;
mod bind_group_layout_descriptors;
mod buffer_content;
mod camera;
mod camera_controller;
mod color;
mod drawable;
mod frame_timer;
mod gbuffer;
mod imgui;
mod instance;
mod light_controller;
mod model;
mod primitive_shapes;
mod render_pipeline;
mod render_pipeline_layout;
mod renderer;
mod resource_map;
mod resources;
mod shadow_pipeline;
mod skybox_pipeline;
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
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    window.set_title("Awesome application");
    let mut app = app::App::new(&window).await;

    event_loop.run(move |event, _, control_flow| {
        app.handle_event(&window, &event);

        match event {
            Event::WindowEvent { event, window_id } if window_id == window.id() => {
                if let WindowEventHandlingResult::RequestExit = app.handle_window_event(event) {
                    *control_flow = ControlFlow::Exit;
                }
            }
            Event::RedrawRequested(window_id) if window_id == window.id() => {
                match app.request_redraw(&window) {
                    Ok(_) => (),
                    // Reconfigure the surface if lost
                    Err(wgpu::SurfaceError::Lost) => app.resize(app.renderer.size),
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    // All other errors (Outdated, Timeout) should be resolved by the next frame
                    Err(e) => eprintln!("{:?}", e),
                }
            }
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::DeviceEvent {
                event, device_id, ..
            } => {
                app.handle_device_event(&window, device_id, event);
            }
            _ => {}
        }
    });
}

fn main() {
    async_std::task::block_on(run());
}
