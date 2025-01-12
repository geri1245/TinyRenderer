use crate::actions::RenderingAction;
use crate::bind_group_layout_descriptors;
use crate::buffer::GpuBufferCreationOptions;
use crate::camera_controller::CameraController;
use crate::custom_event::CustomEvent;
use crate::global_params::{GlobalCPUParams, GlobalGPUParams};
use crate::gpu_buffer::GpuBuffer;
use crate::gui::{Gui, GuiButton, GuiEvent, GuiUpdateEvent};
use crate::gui_settable_value::GuiSettableValue;
use crate::light_controller::LightController;
use crate::mipmap_generator::MipMapGenerator;
use crate::player_controller::PlayerController;
use crate::resource_loader::ResourceLoader;
use crate::world::World;
use crate::world_loader::{load_level, save_level};
use crate::world_renderer::WorldRenderer;
use crate::{frame_timer::BasicTimer, renderer::Renderer};
use async_std::task::block_on;
use crossbeam_channel::{unbounded, Receiver};
use std::path::Path;
use std::time::Duration;
use ui_item::{UiDisplayable, UiSettable};
use wgpu::{Queue, TextureViewDescriptor};
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::EventLoopProxy;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::Window;

pub enum WindowEventHandlingAction {
    Exit,
    RecompileShaders,
}

pub enum WindowEventHandlingResult {
    Handled,
    Unhandled,
    RequestAction(WindowEventHandlingAction),
}

pub struct App {
    renderer: Renderer,
    resource_loader: ResourceLoader,
    frame_timer: BasicTimer,
    gui: Gui,
    player_controller: PlayerController,
    gpu_params: GuiSettableValue<GpuBuffer<GlobalGPUParams>, Queue>,
    cpu_rendering_params: GlobalCPUParams,

    light_controller: LightController,
    mip_map_generator: MipMapGenerator,

    pub world: World,
    should_draw_gui: bool,
    gui_event_receiver: Receiver<GuiEvent>,
}

impl App {
    pub fn new(window: &Window, event_loop_proxy: EventLoopProxy<CustomEvent>) -> Self {
        let renderer = Renderer::new(window);
        let (gui_event_sender, gui_event_receiver) = unbounded::<GuiEvent>();
        let mut resource_loader = ResourceLoader::new(&renderer.device, &renderer.queue);

        let gui = Gui::new(&window, &renderer.device, gui_event_sender);

        let world_renderer: WorldRenderer = WorldRenderer::new(&renderer, &mut resource_loader);

        let camera_controller = CameraController::new(
            &renderer.device,
            renderer.config.width,
            renderer.config.height,
        );

        let mut world = World::new(world_renderer, camera_controller);

        load_level(&mut world, Path::new("levels/test.lvl")).unwrap();

        let player_controller = PlayerController::new();

        let light_controller = LightController::new(&renderer.device);

        let frame_timer = BasicTimer::new();

        let gpu_params = GpuBuffer::new(
            GlobalGPUParams::default(),
            &renderer.device,
            &GpuBufferCreationOptions {
                bind_group_layout_descriptor:
                    &bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE,
                usages: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                label: "Global GPU params".into(),
            },
        );

        let ui_items = gpu_params.get_ui_description();
        let gpu_params = GuiSettableValue::new(
            gpu_params,
            "gpu_params".to_owned(),
            Box::new(|data, set_property_params, queue| {
                data.get_mut_data(queue)
                    .try_set_value_from_ui(set_property_params.clone());
            }),
            &event_loop_proxy,
            ui_items,
        );

        let mip_map_generator = MipMapGenerator::new(&renderer.device);

        Self {
            renderer,
            frame_timer,
            gui,
            should_draw_gui: true,
            gui_event_receiver,
            world,
            light_controller,
            resource_loader,
            player_controller,
            cpu_rendering_params: GlobalCPUParams::default(),
            gpu_params,
            mip_map_generator,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 && new_size != self.renderer.size {
            self.resize_unchecked(new_size);
        }
    }

    fn resize_unchecked(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.renderer.resize(new_size);
        self.world
            .handle_size_changed(&self.renderer, new_size.width, new_size.height);
    }

    pub fn handle_custom_event(&mut self, event: &CustomEvent) {
        match event {
            CustomEvent::GuiRegistration(gui_registration_event) => {
                if gui_registration_event.register {
                    self.gui.register_item(
                        &gui_registration_event.category,
                        gui_registration_event.items.clone(),
                        gui_registration_event.sender.clone(),
                    );
                }
            }
        }
    }

    pub fn handle_window_event(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::WindowEvent,
    ) -> WindowEventHandlingResult {
        if self.gui.handle_event(window, event) {
            return WindowEventHandlingResult::Handled;
        }

        match self
            .player_controller
            .handle_window_event(&event, &mut self.world)
        {
            WindowEventHandlingResult::RequestAction(action) => {
                if matches!(action, WindowEventHandlingAction::RecompileShaders) {
                    self.recompile_shaders();
                    return WindowEventHandlingResult::Handled;
                } else {
                    return WindowEventHandlingResult::RequestAction(action);
                }
            }
            WindowEventHandlingResult::Handled => return WindowEventHandlingResult::Handled,
            WindowEventHandlingResult::Unhandled => {}
        }

        match event {
            WindowEvent::KeyboardInput { event, .. } => self.handle_keyboard_event(&event),
            WindowEvent::Resized(new_size) => {
                self.resize(*new_size);
                WindowEventHandlingResult::Handled
            }
            WindowEvent::CloseRequested => {
                WindowEventHandlingResult::RequestAction(WindowEventHandlingAction::Exit)
            }
            // WindowEvent::ScaleFactorChanged {
            //     scale_factor,
            //     inner_size_writer,
            // } => todo!(),
            WindowEvent::RedrawRequested => {
                match self.run_frame(window) {
                    Ok(_) => WindowEventHandlingResult::Handled,
                    // Reconfigure the surface if lost
                    Err(wgpu::SurfaceError::Lost) => {
                        self.resize_unchecked(self.renderer.size);
                        WindowEventHandlingResult::Handled
                    }
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory) => {
                        WindowEventHandlingResult::RequestAction(WindowEventHandlingAction::Exit)
                    }
                    // All other errors (Outdated, Timeout) should be resolved by the next frame
                    Err(e) => {
                        eprintln!("{:?}", e);
                        WindowEventHandlingResult::RequestAction(WindowEventHandlingAction::Exit)
                    }
                }
            }
            _ => WindowEventHandlingResult::Unhandled,
        }
    }

    fn handle_keyboard_event(&mut self, key_event: &KeyEvent) -> WindowEventHandlingResult {
        if key_event.state == ElementState::Pressed {
            if let PhysicalKey::Code(key) = key_event.physical_key {
                match key {
                    KeyCode::KeyF => {
                        self.toggle_should_draw_gui();
                        WindowEventHandlingResult::Handled
                    }
                    KeyCode::KeyI => {
                        self.world
                            .add_action(RenderingAction::SaveDiffuseIrradianceMapToFile);
                        WindowEventHandlingResult::Handled
                    }
                    _ => WindowEventHandlingResult::Unhandled,
                }
            } else {
                WindowEventHandlingResult::Unhandled
            }
        } else {
            WindowEventHandlingResult::Unhandled
        }
    }

    pub fn run_frame(&mut self, window: &winit::window::Window) -> Result<(), wgpu::SurfaceError> {
        let delta = self.frame_timer.get_delta_and_reset_timer();
        self.update(delta);

        let mut encoder = self.renderer.get_encoder();
        let current_frame_texture = self.renderer.get_current_frame_texture()?;
        let current_frame_texture_view = current_frame_texture
            .texture
            .create_view(&TextureViewDescriptor::default());

        self.world.render(
            &self.renderer,
            &mut encoder,
            &current_frame_texture,
            &self.light_controller,
            &self.gpu_params.bind_group,
        )?;

        if self.should_draw_gui {
            let frame_time = delta.as_secs_f32();
            self.gui.update_frame_time(frame_time);
            self.gui.render(
                &window,
                &self.renderer.device,
                &self.renderer.queue,
                &self.renderer.config,
                &current_frame_texture_view,
                &mut encoder,
            );
        }

        self.renderer.queue.submit(Some(encoder.finish()));

        current_frame_texture.present();

        self.world.post_render();

        Ok(())
    }

    fn handle_gui_button_pressed(&mut self, button: GuiButton) {
        match button {
            GuiButton::SaveLevel => {
                let result = save_level(&self.world, "test.lvl");
                self.gui
                    .push_display_info_update(GuiUpdateEvent::LevelSaveResult(result));
            }
        };
    }

    fn handle_gpu_params_changed_events(&mut self) {
        self.gpu_params.handle_gui_changes(&self.renderer.queue);
    }

    fn handle_events_received_from_gui(&mut self) {
        while let Ok(event) = self.gui_event_receiver.try_recv() {
            match event {
                GuiEvent::RecompileShaders => self.recompile_shaders(),
                GuiEvent::ButtonClicked(button) => self.handle_gui_button_pressed(button),
            }
        }
    }

    async fn recompile_shaders_internal(&mut self) -> anyhow::Result<()> {
        self.light_controller
            .try_recompile_shaders(&self.renderer.device)?;

        self.mip_map_generator
            .try_recompile_shader(&self.renderer.device)?;

        self.world
            .recompile_shaders_if_needed(&self.renderer.device)
    }

    fn recompile_shaders(&mut self) {
        let result = block_on(self.recompile_shaders_internal());
        self.gui
            .push_display_info_update(GuiUpdateEvent::ShaderCompilationResult(result));
    }

    pub fn update(&mut self, delta: Duration) {
        self.handle_events_received_from_gui();
        self.handle_gpu_params_changed_events();

        self.player_controller.update(&mut self.world);

        self.light_controller.update(
            delta,
            &self.renderer.queue,
            &self.renderer.device,
            &self.world,
        );

        self.world.update(
            delta,
            &self.renderer.device,
            &self.renderer.queue,
            &self.resource_loader,
            &self.mip_map_generator,
        );

        self.gui.update(delta);
    }

    pub fn toggle_should_draw_gui(&mut self) {
        self.should_draw_gui = !self.should_draw_gui
    }
}
