use crate::camera_controller::CameraController;
use crate::gui::{Gui, GuiEvent};
use crate::light_controller::LightController;
use crate::resource_loader::ResourceLoader;
use crate::world::World;
use crate::world_renderer::WorldRenderer;
use crate::{frame_timer::BasicTimer, renderer::Renderer};
use crossbeam_channel::{unbounded, Receiver};
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
    renderer: Renderer,
    resource_loader: ResourceLoader,
    frame_timer: BasicTimer,
    gui: Gui,
    camera_controller: CameraController,
    light_controller: LightController,

    world_renderer: WorldRenderer,
    world: World,
    should_draw_gui: bool,
    gui_event_receiver: Receiver<GuiEvent>,
}

impl App {
    pub async fn new(window: &Window) -> Self {
        let renderer = Renderer::new(window).await;
        let (gui_event_sender, gui_event_receiver) = unbounded::<GuiEvent>();
        let mut resource_loader = ResourceLoader::new(&renderer.device, &renderer.queue).await;

        let gui = Gui::new(&window, &renderer.device, &renderer.queue, gui_event_sender);

        let mut world = World::new(&renderer.device, &mut resource_loader).await;
        let world_renderer: WorldRenderer =
            WorldRenderer::new(&renderer, &mut resource_loader).await;

        let camera_controller = CameraController::new(
            &renderer.device,
            renderer.config.width as f32 / renderer.config.height as f32,
        );
        let light_controller = LightController::new(&renderer.device, &mut world).await;

        let frame_timer = BasicTimer::new();

        Self {
            renderer,
            frame_timer,
            world_renderer,
            gui,
            should_draw_gui: true,
            gui_event_receiver,
            world,
            camera_controller,
            light_controller,
            resource_loader,
        }
    }

    pub fn reconfigure(&mut self) {
        self.resize_unchecked(self.renderer.size);
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 && new_size != self.renderer.size {
            self.resize_unchecked(new_size);
        }
    }

    fn resize_unchecked(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.camera_controller
            .resize(new_size.width as f32 / new_size.height as f32);
        self.renderer.resize(new_size);
        self.world_renderer
            .handle_size_changed(&self.renderer, new_size.width, new_size.height);
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

        self.camera_controller.process_device_events(event);
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
                self.camera_controller
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

        let mut encoder = self.renderer.get_encoder();
        let current_frame_texture = self.renderer.get_current_frame_texture()?;
        let current_frame_texture_view = current_frame_texture
            .texture
            .create_view(&TextureViewDescriptor::default());

        self.world_renderer.render(
            &self.renderer,
            &mut encoder,
            &current_frame_texture,
            &self.world.meshes,
            &self.light_controller,
            &self.camera_controller,
        )?;

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

    pub fn handle_events_received_from_gui(&mut self) {
        while let Ok(event) = self.gui_event_receiver.try_recv() {
            match event {
                GuiEvent::RecompileShaders => self.try_recompile_shaders(),
                GuiEvent::LightPositionChanged { new_position } => self
                    .light_controller
                    .set_light_position(new_position.into()),
            }
        }
    }

    fn try_recompile_shaders(&mut self) {
        let results = vec![
            self.light_controller
                .try_recompile_shaders(&self.renderer.device),
            self.world_renderer
                .recompile_shaders_if_needed(&self.renderer.device),
        ];

        let mut errors = Vec::new();
        for result in results {
            if let Err(error) = result {
                errors.push(error.to_string());
            }
        }

        if errors.is_empty() {
            self.gui
                .set_shader_compilation_result(&vec!["Sucess!".into()]);
        } else {
            self.gui.set_shader_compilation_result(&errors);
        }
    }

    pub fn update(&mut self, delta: Duration) {
        self.handle_events_received_from_gui();
        self.world.update(
            &self.renderer.device,
            &self.renderer.queue,
            &mut self.resource_loader,
        );

        self.camera_controller.update(delta, &self.renderer.queue);

        self.light_controller.update(delta, &self.renderer.queue);
    }

    pub fn toggle_should_draw_gui(&mut self) {
        self.should_draw_gui = !self.should_draw_gui
    }
}
