use crate::actions::{RenderingAction, UserInputAction};
use crate::camera_controller::CameraController;
use crate::gui::{Gui, GuiButton, GuiEvent};
use crate::instance::SceneComponent;
use crate::light_controller::LightController;
use crate::lights::{DirectionalLight, Light, PointLight};
use crate::material::PbrMaterialDescriptor;
use crate::model::{MeshSource, ObjectWithMaterial, PbrParameters, WorldObject};
use crate::player_controller::PlayerController;
use crate::resource_loader::{PrimitiveShape, ResourceLoader};
use crate::texture::{MaterialSource, TextureSourceDescriptor, TextureUsage};
use crate::world::World;
use crate::world_loader::WorldLoader;
use crate::world_renderer::WorldRenderer;
use crate::{frame_timer::BasicTimer, renderer::Renderer};
use crossbeam_channel::{unbounded, Receiver};
use glam::{Quat, Vec3};
use std::f32::consts::FRAC_PI_2;
use std::time::Duration;
use wgpu::{MaintainBase, TextureViewDescriptor};
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
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

    pub world: World,
    should_draw_gui: bool,
    gui_event_receiver: Receiver<GuiEvent>,
    user_input_action_receiver: Receiver<UserInputAction>,
}

impl App {
    pub async fn new(window: &Window) -> Self {
        let renderer = Renderer::new(window).await;
        let (gui_event_sender, gui_event_receiver) = unbounded::<GuiEvent>();
        let (user_input_action_sender, user_input_action_receiver) = unbounded::<UserInputAction>();
        let mut resource_loader = ResourceLoader::new(&renderer.device, &renderer.queue).await;

        let gui = Gui::new(&window, &renderer.device, gui_event_sender);

        let world_renderer: WorldRenderer =
            WorldRenderer::new(&renderer, &mut resource_loader, user_input_action_sender).await;

        let mut world = World::new(world_renderer);
        world.add_light(Light::Point(PointLight::new(
            Vec3::new(10.0, 20.0, 0.0),
            Vec3::new(2.0, 5.0, 4.0),
        )));
        world.add_light(Light::Directional(DirectionalLight {
            direction: Vec3::new(0.0, -1.0, 0.0).normalize(),
            color: Vec3::new(1.0, 1.0, 1.0),
        }));

        Self::init_world_objects(&mut world);

        let player_controller = PlayerController::new();

        let camera_controller = CameraController::new(
            &renderer.device,
            renderer.config.width as f32 / renderer.config.height as f32,
        );
        let light_controller = LightController::new(&renderer.device).await;

        let frame_timer = BasicTimer::new();

        Self {
            renderer,
            frame_timer,
            gui,
            should_draw_gui: true,
            gui_event_receiver,
            world,
            camera_controller,
            light_controller,
            resource_loader,
            player_controller,
            user_input_action_receiver,
        }
    }

    fn init_world_objects(world: &mut World) {
        let big_cube_instances = vec![
            SceneComponent {
                position: Vec3::new(10.0, 10.0, 0.0),
                scale: Vec3::splat(3.0),
                rotation: Quat::from_axis_angle(Vec3::ZERO, 0.0),
            },
            SceneComponent {
                position: Vec3::new(-20.0, 10.0, 0.0),
                scale: Vec3::splat(2.0),
                rotation: Quat::from_axis_angle(Vec3::ZERO, 0.0),
            },
            SceneComponent {
                position: Vec3::new(0.0, 10.0, 30.0),
                scale: Vec3::splat(2.0),
                rotation: Quat::from_axis_angle(Vec3::ZERO, 0.0),
            },
            SceneComponent {
                position: Vec3::new(30.0, 20.0, 10.0),
                scale: Vec3::splat(2.0),
                rotation: Quat::from_axis_angle(Vec3::ZERO, 0.0),
            },
            SceneComponent {
                position: Vec3::new(25.0, 10.0, 20.0),
                scale: Vec3::splat(1.5),
                rotation: Quat::from_axis_angle(Vec3::ZERO, 0.0),
            },
        ];

        let mut small_cube_instances = Vec::with_capacity(100);
        for i in 0..11 {
            for j in 0..11 {
                small_cube_instances.push(SceneComponent {
                    position: Vec3::new(i as f32 * 5.0 - 25.0, j as f32 * 5.0 - 25.0, 0.0),
                    scale: Vec3::splat(1.0),
                    rotation: Quat::from_axis_angle(Vec3::ZERO, 0.0),
                });
            }
        }

        let square_instances = vec![
            // Bottom
            SceneComponent {
                position: Vec3::new(0.0, -10.0, 0.0),
                rotation: Quat::IDENTITY,
                scale: 100.0_f32
                    * Vec3 {
                        x: 1.0_f32,
                        y: 1.0,
                        z: 1.0,
                    },
            },
            // Top
            // SceneComponent {
            //     position: Vec3::new(0.0, 40.0, 0.0),
            //     rotation: Quat::from_axis_angle(Vec3::X, PI),
            //     scale: 100.0_f32
            //         * Vec3 {
            //             x: 1.0_f32,
            //             y: 1.0,
            //             z: 1.0,
            //         },
            // },
            // +X
            // SceneComponent {
            //     position: Vec3::new(-40.0, 0.0, 0.0),
            //     rotation: Quat::from_axis_angle(Vec3::Z, -FRAC_PI_2),
            //     scale: 100.0_f32
            //         * Vec3 {
            //             x: 1.0_f32,
            //             y: 1.0,
            //             z: 1.0,
            //         },
            // },
            // -X
            // SceneComponent {
            //     position: Vec3::new(40.0, 0.0, 0.0),
            //     rotation: Quat::from_axis_angle(Vec3::Z, FRAC_PI_2),
            //     scale: 100.0_f32
            //         * Vec3 {
            //             x: 1.0_f32,
            //             y: 1.0,
            //             z: 1.0,
            //         },
            // },
            // -Z
            SceneComponent {
                position: Vec3::new(0.0, 0.0, -40.0),
                rotation: Quat::from_axis_angle(Vec3::X, FRAC_PI_2),
                scale: 100.0_f32
                    * Vec3 {
                        x: 1.0_f32,
                        y: 1.0,
                        z: 1.0,
                    },
            },
            // // Z
            // SceneComponent {
            //     position: Vec3::new(0.0, 0.0, 40.0),
            //     rotation: Quat::from_axis_angle(Vec3::X, -FRAC_PI_2),
            //     scale: 100.0_f32
            //         * Vec3 {
            //             x: 1.0_f32,
            //             y: 1.0,
            //             z: 1.0,
            //         },
            // },
        ];

        let small_cube = ObjectWithMaterial {
            mesh_source: MeshSource::FromFile("assets/models/cube/cube.obj".into()),
            material_descriptor: PbrMaterialDescriptor::Flat(PbrParameters::new(
                [0.2, 0.5, 1.0],
                1.0,
                0.0,
            )),
        };

        let big_cube = ObjectWithMaterial {
            mesh_source: MeshSource::FromFile("assets/models/cube/cube.obj".into()),
            material_descriptor: PbrMaterialDescriptor::Texture(vec![
                TextureSourceDescriptor {
                    source: MaterialSource::FromFile(
                        "assets/textures/brick_wall_basic/albedo.jpg".into(),
                    ),
                    usage: TextureUsage::Albedo,
                },
                TextureSourceDescriptor {
                    source: MaterialSource::FromFile(
                        "assets/textures/brick_wall_basic/normal.jpg".into(),
                    ),
                    usage: TextureUsage::Normal,
                },
            ]),
        };

        let square = ObjectWithMaterial {
            mesh_source: MeshSource::PrimitiveInCode(PrimitiveShape::Square),
            material_descriptor: PbrMaterialDescriptor::Texture(vec![]),
        };

        for transform in big_cube_instances {
            world.add_object(WorldObject {
                object: big_cube.clone(),
                transform,
            });
        }

        for transform in small_cube_instances {
            world.add_object(WorldObject {
                object: small_cube.clone(),
                transform,
            });
        }

        for transform in square_instances {
            world.add_object(WorldObject {
                object: square.clone(),
                transform,
            });
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
        self.world
            .handle_size_changed(&self.renderer, new_size.width, new_size.height);
    }

    pub fn handle_event(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::WindowEvent,
    ) {
        self.gui.handle_event(window, event);
    }

    pub fn handle_window_event(&mut self, event: WindowEvent) -> WindowEventHandlingResult {
        self.camera_controller.process_window_event(&event);

        match event {
            WindowEvent::CloseRequested => return WindowEventHandlingResult::RequestExit,

            WindowEvent::KeyboardInput { event, .. } => {
                self.handle_keyboard_event(&event);
            }

            WindowEvent::Resized(new_size) => {
                self.resize(new_size);
            }
            // WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
            // self.resize(); // TODO Handle scale factor change
            // }
            _ => {}
        };

        WindowEventHandlingResult::Handled
    }

    fn handle_keyboard_event(&mut self, key_event: &KeyEvent) {
        if key_event.state == ElementState::Pressed {
            if let PhysicalKey::Code(key) = key_event.physical_key {
                match key {
                    KeyCode::KeyF => self.toggle_should_draw_gui(),
                    KeyCode::KeyI => self
                        .world
                        .add_action(RenderingAction::SaveDiffuseIrradianceMapToFile),
                    _ => {}
                }
            }
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
            &self.camera_controller,
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

        self.world
            .world_renderer
            .object_picker
            .update(&self.renderer.device);
        self.renderer.queue.submit(Some(encoder.finish()));

        current_frame_texture.present();

        Ok(())
    }

    fn handle_gui_button_pressed(&self, button: GuiButton) {
        match button {
            GuiButton::SaveLevel => WorldLoader::save_level(&self.world, "test.lvl"),
        }
        .unwrap();
    }

    fn handle_events_received_from_gui(&mut self) {
        while let Ok(event) = self.gui_event_receiver.try_recv() {
            match event {
                GuiEvent::RecompileShaders => self.try_recompile_shaders(),
                GuiEvent::ButtonClicked(button) => self.handle_gui_button_pressed(button),
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
            self.world
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

        self.world.update(
            &self.renderer.device,
            &self.renderer.queue,
            &self.resource_loader,
        );
    }

    pub fn toggle_should_draw_gui(&mut self) {
        self.should_draw_gui = !self.should_draw_gui
    }
}
