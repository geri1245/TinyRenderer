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
use crate::object_picker::ObjectPickManager;
use crate::player_controller::PlayerController;
use crate::resource_loader::ResourceLoader;
use crate::world::World;
use crate::world_loader::{load_level, save_level};
use crate::world_renderer::WorldRenderer;
use crate::{frame_timer::BasicTimer, renderer::Renderer};
use crossbeam_channel::{unbounded, Receiver};
use std::path::Path;
use std::time::Duration;
use ui_item::{UiDisplayable, UiSettableNew};
use wgpu::TextureViewDescriptor;
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
    pub world: World,

    renderer: Renderer,
    world_renderer: WorldRenderer,
    object_picker: ObjectPickManager,
    resource_loader: ResourceLoader,
    frame_timer: BasicTimer,
    gui: Gui,
    player_controller: PlayerController,
    gpu_params: GuiSettableValue<GpuBuffer<GlobalGPUParams>>,
    cpu_rendering_params: GlobalCPUParams,

    light_controller: LightController,

    should_draw_gui: bool,
    gui_event_receiver: Receiver<GuiEvent>,
}

impl App {
    pub fn new(window: &Window, event_loop_proxy: EventLoopProxy<CustomEvent>) -> Self {
        let renderer = Renderer::new(window);
        let (gui_event_sender, gui_event_receiver) = unbounded::<GuiEvent>();
        let mut resource_loader = ResourceLoader::new(&renderer);

        let gui = Gui::new(&window, &renderer.device, gui_event_sender);

        let mut world_renderer: WorldRenderer = WorldRenderer::new(&renderer, &mut resource_loader);

        let camera_controller = CameraController::new(
            &renderer.device,
            renderer.config.width,
            renderer.config.height,
        );

        let mut world = World::new(camera_controller);

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
            &event_loop_proxy,
            ui_items,
        );

        let object_picker = ObjectPickManager::new(&renderer);

        // Initial environment cubemap generation from the equirectangular map
        world_renderer.add_action(RenderingAction::GenerateCubeMapFromEquirectangular);

        Self {
            renderer,
            world_renderer,
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
            object_picker,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 && new_size != self.renderer.size {
            self.resize_unchecked(new_size);
        }
    }

    fn resize_unchecked(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.renderer.resize(new_size);
        self.world_renderer.handle_size_changed(&self.renderer);
        self.world
            .handle_size_changed(new_size.width, new_size.height);
        self.object_picker.resize(&self.renderer);
    }

    pub fn handle_custom_event(&mut self, event: &CustomEvent) {
        match event {
            CustomEvent::GuiRegistration(gui_registration_event) => {
                if !self.gui.register_item(
                    &gui_registration_event.category,
                    gui_registration_event.items.clone(),
                    gui_registration_event.sender.clone(),
                ) {
                    let name = gui_registration_event.category.clone();
                    log::warn!("Failed to register gui item with category {name:?}");
                }
            }
            CustomEvent::GuiDeregistration(gui_deregistration_event) => {
                if !self.gui.deregister_item(&gui_deregistration_event.category) {
                    let category = &gui_deregistration_event.category;
                    log::warn!("Failed to deregister item with category {category:?}");
                }
            }
        }
    }

    pub fn handle_window_event(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::WindowEvent,
        event_loop_proxy: &mut EventLoopProxy<CustomEvent>,
    ) -> WindowEventHandlingResult {
        if self.gui.handle_event(window, event) {
            return WindowEventHandlingResult::Handled;
        }

        match self.player_controller.handle_window_event(
            &event,
            &mut self.world,
            &self.object_picker,
        ) {
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
                match self.run_frame(window, event_loop_proxy) {
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
                        self.world_renderer
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

    pub fn render(&mut self, window: &winit::window::Window) -> Result<(), wgpu::SurfaceError> {
        let mut encoder = self.renderer.get_encoder();
        let current_frame_texture = self.renderer.get_current_frame_texture()?;
        let current_frame_texture_view = current_frame_texture
            .texture
            .create_view(&TextureViewDescriptor::default());

        self.world_renderer.render(
            &self.renderer,
            &mut encoder,
            &current_frame_texture,
            &self.light_controller,
            &self.world.camera_controller,
            &self.gpu_params.bind_group,
            &mut self.object_picker,
        )?;

        if self.should_draw_gui {
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

        Ok(())
    }

    pub fn on_end_frame(&mut self) {
        self.world.on_end_frame();
        self.object_picker.on_end_frame();
    }

    pub fn run_frame(
        &mut self,
        window: &winit::window::Window,
        event_loop_proxy: &mut EventLoopProxy<CustomEvent>,
    ) -> Result<(), wgpu::SurfaceError> {
        let delta = self.frame_timer.get_delta_and_reset_timer();

        self.update(delta, event_loop_proxy);

        self.render(window)?;

        self.on_end_frame();

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
        let changes = self.gpu_params.get_gui_changes();
        for change in changes {
            self.gpu_params
                .get_mut_data(&self.renderer.queue)
                .set_value_from_ui(&change);
        }
    }

    fn handle_events_received_from_gui(&mut self) {
        while let Ok(event) = self.gui_event_receiver.try_recv() {
            match event {
                GuiEvent::RecompileShaders => self.recompile_shaders(),
                GuiEvent::ButtonClicked(button) => self.handle_gui_button_pressed(button),
            }
        }
    }

    fn recompile_shaders_internal(&mut self) -> anyhow::Result<()> {
        self.light_controller
            .try_recompile_shaders(&self.renderer.device)?;

        self.renderer.try_recompile_shaders()?;

        self.world_renderer
            .recompile_shaders_if_needed(&self.renderer.device)?;

        self.object_picker
            .try_recompile_shader(&self.renderer.device)?;

        Ok(())
    }

    fn recompile_shaders(&mut self) {
        let result = self.recompile_shaders_internal();
        self.gui
            .push_display_info_update(GuiUpdateEvent::ShaderCompilationResult(result));
    }

    fn update(&mut self, delta: Duration, event_loop_proxy: &mut EventLoopProxy<CustomEvent>) {
        self.handle_events_received_from_gui();
        self.handle_gpu_params_changed_events();

        self.player_controller
            .update(&mut self.world, event_loop_proxy);

        // Light controller might add light debug objects to the world, so we update it before the world
        self.light_controller
            .update(delta, &self.renderer, &mut self.world);

        self.world.update(delta, &self.renderer);

        self.world_renderer
            .update(&self.renderer, &self.world, &self.resource_loader);

        self.object_picker.update();

        self.gui.update(delta);
    }

    fn toggle_should_draw_gui(&mut self) {
        self.should_draw_gui = !self.should_draw_gui
    }
}
