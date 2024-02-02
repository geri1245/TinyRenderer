use crate::gui::Gui;
use crate::world::World;
use crate::{frame_timer::FrameTimer, renderer::Renderer};
use std::time::Duration;
use wgpu::TextureViewDescriptor;
use winit::event::{DeviceEvent, ElementState, MouseButton, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::Window;

pub enum WindowEventHandlingResult {
    RequestExit,
    Handled,
}

pub struct App {
    pub renderer: Renderer,
    pub frame_timer: FrameTimer,
    gui: crate::gui::Gui,
    world: World,
    should_draw_gui: bool,
}

impl App {
    pub async fn new(window: &Window) -> Self {
        let renderer = Renderer::new(window).await;
        let gui = Gui::new(
            &window,
            &renderer.device,
            &renderer.queue,
            renderer.surface_texture_format,
        );

        let world: World = World::new(&renderer).await;

        let frame_timer = FrameTimer::new();

        Self {
            renderer,
            frame_timer,
            world,
            gui,
            should_draw_gui: true,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 && new_size != self.renderer.size {
            self.renderer.resize(new_size);
            self.world
                .resize_main_camera(&self.renderer, new_size.width, new_size.height);
        }
    }

    pub fn handle_event<'a, T>(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::Event<T>,
    ) {
        self.gui.handle_event(window, event);
    }

    pub fn handle_device_event(
        &mut self,
        window: &Window,
        device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        self.gui.handle_event(
            window,
            &winit::event::Event::DeviceEvent::<()> {
                device_id,
                event: event.clone(),
            },
        );

        self.world.camera_controller.process_device_events(event);
    }

    pub fn handle_window_event(&mut self, event: WindowEvent) -> WindowEventHandlingResult {
        match event {
            WindowEvent::CloseRequested => return WindowEventHandlingResult::RequestExit,

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed
                    && event.physical_key == PhysicalKey::Code(KeyCode::KeyF)
                {
                    self.toggle_should_draw_gui();
                }
            }

            WindowEvent::Resized(new_size) => {
                self.resize(new_size);
            }
            // WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
            // self.resize(); // TODO Handle scale factor change
            // }
            WindowEvent::MouseInput { state, button, .. } if button == MouseButton::Right => {
                self.world
                    .camera_controller
                    .set_is_movement_enabled(state == ElementState::Pressed);
            }
            _ => {}
        };

        WindowEventHandlingResult::Handled
    }

    pub fn request_redraw(
        &mut self,
        window: &winit::window::Window,
    ) -> Result<(), wgpu::SurfaceError> {
        let delta = self.frame_timer.get_delta_and_reset_timer();
        self.update(delta);

        let mut encoder = self.renderer.begin_frame();
        let current_frame_texture = self.renderer.get_current_frame_texture()?;
        let current_frame_texture_view = current_frame_texture
            .texture
            .create_view(&TextureViewDescriptor::default());

        self.world
            .render(&self.renderer, &mut encoder, &current_frame_texture_view)?;

        self.renderer.queue.submit(Some(encoder.finish()));

        if self.should_draw_gui {
            self.gui.render(
                &window,
                &self.renderer.device,
                &self.renderer.queue,
                delta,
                &current_frame_texture_view,
            );
        }

        self.renderer.end_frame(current_frame_texture);

        Ok(())
    }

    pub fn update(&mut self, delta: Duration) {
        self.world.update(delta, &self.renderer.queue);
    }

    pub fn toggle_should_draw_gui(&mut self) {
        self.should_draw_gui = !self.should_draw_gui
    }
}
