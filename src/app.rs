use crate::camera_controller::CameraController;
use crate::light_controller::{LightController, PointLight};
use crate::renderer::{BindGroupLayoutType, Renderer};
use std::time;
use winit::event::{
    DeviceEvent, ElementState, KeyboardInput, MouseButton, VirtualKeyCode, WindowEvent,
};
use winit::window::Window;

pub enum WindowEventHandlingResult {
    RequestExit,
    Handled,
}

pub struct App {
    pub renderer: Renderer,
    pub camera_controller: CameraController,
    pub light_controller: LightController,
}

impl App {
    // Creating some of the wgpu types requires async code
    pub async fn new(window: &Window) -> Self {
        let renderer = Renderer::new(window).await;
        let camera_controller = CameraController::new(&renderer);
        let light_controller = LightController::new(
            PointLight {
                position: [20.0, 30.0, 0.0],
                color: [1.0, 1.0, 1.0],
                target: [0.0, 0.0, 0.0],
            },
            &renderer.device,
            &renderer
                .bind_group_layouts
                .get(&BindGroupLayoutType::Light)
                .unwrap(),
        );

        Self {
            renderer,
            camera_controller,
            light_controller,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 && new_size != self.renderer.size {
            self.renderer.resize(new_size);
            self.camera_controller
                .resize(new_size.width as f32 / new_size.height as f32)
        }
    }

    pub fn handle_device_event(&mut self, event: DeviceEvent) {
        self.camera_controller.process_device_events(event);
    }

    pub fn handle_window_event(&mut self, event: &WindowEvent) -> WindowEventHandlingResult {
        match event {
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state: ElementState::Pressed,
                        virtual_keycode: Some(VirtualKeyCode::Escape),
                        ..
                    },
                ..
            } => return WindowEventHandlingResult::RequestExit,

            WindowEvent::Resized(new_size) => {
                self.resize(*new_size);
            }
            WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                self.resize(**new_inner_size);
            }
            WindowEvent::MouseInput { state, button, .. } if *button == MouseButton::Right => {
                self.camera_controller
                    .set_is_movement_enabled(*state == ElementState::Pressed);
            }
            _ => {}
        };

        WindowEventHandlingResult::Handled
    }

    pub fn request_redraw(&mut self) -> Result<(), wgpu::SurfaceError> {
        self.update();
        self.renderer
            .render(&self.camera_controller, &self.light_controller)
    }

    pub fn update(&mut self) {
        self.camera_controller
            .update(time::Duration::from_millis(16), &self.renderer.queue);

        self.light_controller
            .update(time::Duration::from_millis(16), &self.renderer.queue);
    }
}
