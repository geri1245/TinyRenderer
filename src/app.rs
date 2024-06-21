use crate::camera_controller::CameraController;
use crate::gui::{Gui, GuiEvent};
use crate::input_actions::RenderingAction;
use crate::light_controller::LightController;
use crate::lights::{DirectionalLight, Light, PointLight};
use crate::player_controller::PlayerController;
use crate::resource_loader::ResourceLoader;
use crate::world::World;
use crate::world_renderer::WorldRenderer;
use crate::{frame_timer::BasicTimer, renderer::Renderer};
use crossbeam_channel::{unbounded, Receiver};
use glam::Vec3;
use std::time::Duration;
use wgpu::TextureViewDescriptor;
use winit::event::{DeviceEvent, ElementState, KeyEvent, MouseButton, WindowEvent};
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
    player_controller: PlayerController,

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

        let gui = Gui::new(&window, &renderer.device, gui_event_sender);

        let mut world = World::new(&renderer.device, &mut resource_loader).await;
        world.add_light(Light::Point(PointLight::new(
            Vec3::new(10.0, 20.0, 0.0),
            Vec3::new(2.0, 5.0, 4.0),
        )));
        world.add_light(Light::Directional(DirectionalLight {
            direction: Vec3::new(0.0, -1.0, 0.0).normalize(),
            color: Vec3::new(1.0, 1.0, 1.0),
        }));

        let player_controller = PlayerController::new();
        let mut world_renderer: WorldRenderer =
            WorldRenderer::new(&renderer, &mut resource_loader).await;
        // Initial environment cubemap generation from the equirectangular map
        world_renderer.add_action(RenderingAction::GenerateCubeMapFromEquirectangular);

        let camera_controller = CameraController::new(
            &renderer.device,
            renderer.config.width as f32 / renderer.config.height as f32,
        );
        let light_controller = LightController::new(&renderer.device).await;

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
            player_controller,
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

    pub fn handle_event(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::WindowEvent,
    ) {
        self.gui.handle_event(window, event);
    }

    pub fn handle_device_event(&mut self, window: &Window, event: &DeviceEvent) {
        self.gui.handle_device_event(window, event);

        self.camera_controller.process_device_events(event);
    }

    pub fn handle_window_event(&mut self, event: WindowEvent) -> WindowEventHandlingResult {
        match event {
            WindowEvent::CloseRequested => return WindowEventHandlingResult::RequestExit,

            WindowEvent::KeyboardInput { event, .. } => self.handle_keyboard_event(event),

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

    fn handle_keyboard_event(&mut self, key_event: KeyEvent) {
        if key_event.state == ElementState::Pressed {
            if let PhysicalKey::Code(key) = key_event.physical_key {
                match key {
                    KeyCode::KeyF => self.toggle_should_draw_gui(),
                    KeyCode::KeyI => self
                        .world_renderer
                        .add_action(RenderingAction::SaveDiffuseIrradianceMapToFile),
                    _ => {}
                }
            }
        }
    }

    pub fn render(&mut self, window: &winit::window::Window) -> Result<(), wgpu::SurfaceError> {
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
            self.world.get_meshes(),
            &self.light_controller,
            &self.camera_controller,
        )?;

        self.renderer.queue.submit(Some(encoder.finish()));

        if self.should_draw_gui {
            self.gui.render(
                &window,
                &self.renderer.device,
                &self.renderer.queue,
                &self.renderer.config,
                &current_frame_texture_view,
            );
        }

        current_frame_texture.present();

        Ok(())
    }

    pub fn handle_events_received_from_gui(&mut self) {
        while let Ok(event) = self.gui_event_receiver.try_recv() {
            match event {
                GuiEvent::RecompileShaders => self.try_recompile_shaders(),
                _ => {
                    _ = self
                        .player_controller
                        .handle_gui_events(&event, &mut self.world)
                }
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
                .set_shader_compilation_result(&vec!["Success!".into()]);
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

        {
            self.light_controller.update(
                delta,
                &self.renderer.queue,
                &self.renderer.device,
                &self.world,
            );
            self.world.set_lights_udpated();
        }
    }

    pub fn toggle_should_draw_gui(&mut self) {
        self.should_draw_gui = !self.should_draw_gui
    }
}
