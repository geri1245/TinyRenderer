use crate::gui::{Gui, GuiEvent};
use crate::world::World;
use crate::{frame_timer::BasicTimer, renderer::Renderer};
use crossbeam_channel::{unbounded, Receiver};
use std::time::Duration;
use wgpu::{Extent3d, TextureViewDescriptor};
use winit::event::{DeviceEvent, ElementState, MouseButton, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::Window;

pub enum WindowEventHandlingResult {
    RequestExit,
    Handled,
}

pub struct App {
    pub renderer: Renderer,
    pub frame_timer: BasicTimer,
    gui: Gui,
    world: World,
    should_draw_gui: bool,
    gui_event_receiver: Receiver<GuiEvent>,
}

impl App {
    pub async fn new(window: &Window) -> Self {
        let renderer = Renderer::new(window).await;
        let (gui_event_sender, gui_event_receiver) = unbounded::<GuiEvent>();

        let gui = Gui::new(&window, &renderer.device, &renderer.queue, gui_event_sender);

        let world: World = World::new(&renderer).await;

        let frame_timer = BasicTimer::new();

        Self {
            renderer,
            frame_timer,
            world,
            gui,
            should_draw_gui: true,
            gui_event_receiver,
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

        self.world.render(
            &self.renderer,
            &mut encoder,
            &self.renderer.full_screen_render_target_ping_pong_textures[0].view,
        )?;

        encoder.copy_texture_to_texture(
            self.renderer.full_screen_render_target_ping_pong_textures[1]
                .texture
                .as_image_copy(),
            current_frame_texture.texture.as_image_copy(),
            Extent3d {
                depth_or_array_layers: 1,
                width: self.renderer.config.width,
                height: self.renderer.config.height,
            },
        );

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

    pub fn handle_gui_events(&mut self) {
        while let Ok(event) = self.gui_event_receiver.try_recv() {
            match event {
                GuiEvent::RecompileShaders => match self
                    .world
                    .recompile_shaders_if_needed(&self.renderer.device)
                {
                    Ok(_) => self.gui.set_shader_compilation_result("Sucess!".into()),
                    Err(error) => self.gui.set_shader_compilation_result(error.to_string()),
                },
                GuiEvent::LightPositionChanged { new_position } => self
                    .world
                    .light_controller
                    .set_light_position(new_position.into()),
            }
        }
    }

    pub fn update(&mut self, delta: Duration) {
        self.handle_gui_events();
        self.world
            .update(delta, &self.renderer.device, &self.renderer.queue);
    }

    pub fn toggle_should_draw_gui(&mut self) {
        self.should_draw_gui = !self.should_draw_gui
    }
}
