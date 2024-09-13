use crossbeam_channel::Sender;
use egui::{Button, Separator, Widget};
use egui_wgpu::ScreenDescriptor;
use wgpu::{CommandEncoder, TextureFormat};

use crate::gui_helpers::EguiRenderer;

pub enum GuiButton {
    SaveLevel,
}

pub enum GuiEvent {
    RecompileShaders,
    LightPositionChanged { new_position: [f32; 3] },
    ButtonClicked(GuiButton),
}

struct AppInfo {
    shader_error: String,
    frame_time: f32,
    fps_counter: u32,
}

#[derive(Default)]
pub struct GuiParams {
    pub point_light_position: [f32; 3],
    gui_size: [f32; 2],
    pub fov_x: f32,
    pub fov_y: f32,
    pub scale_factor: f32,
}

pub struct Gui {
    renderer: EguiRenderer,
    sender: Sender<GuiEvent>,
    gui_params: GuiParams,
    app_info: AppInfo,
}

impl Gui {
    pub fn new(
        window: &winit::window::Window,
        device: &wgpu::Device,
        sender: Sender<GuiEvent>,
    ) -> Self {
        let egui_renderer = EguiRenderer::new(&device, TextureFormat::Rgba8Unorm, None, 1, &window);

        let gui_params = GuiParams {
            point_light_position: [10.0, 20.0, 0.0],
            fov_x: 90.0,
            fov_y: 45.0,
            scale_factor: 1.0,
            gui_size: [500.0, 300.0],
        };

        Gui {
            sender,
            gui_params,
            renderer: egui_renderer,
            app_info: AppInfo {
                shader_error: "".into(),
                frame_time: 0.0,
                fps_counter: 0,
            },
        }
    }

    pub fn render(
        &mut self,
        window: &winit::window::Window,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: &wgpu::SurfaceConfiguration,
        current_frame_texture_view: &wgpu::TextureView,
        encoder: &mut CommandEncoder,
    ) {
        // let mut encoder: wgpu::CommandEncoder =
        //     device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        //         label: Some("UI encoder"),
        //     });

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [config.width, config.height],
            pixels_per_point: window.scale_factor() as f32,
        };

        let light_position = self.gui_params.point_light_position;

        self.renderer.draw(
            device,
            queue,
            encoder,
            window,
            current_frame_texture_view,
            screen_descriptor,
            |ctx| {
                egui::Window::new("Settings page").show(&ctx, |ui| {
                    let frame_time_string = self.app_info.frame_time.to_string();
                    let fps_string = self.app_info.fps_counter.to_string();
                    ui.label(format!("Frame time: {frame_time_string}"));
                    ui.label(format!("FPS: {fps_string}"));

                    ui.label(&self.app_info.shader_error);

                    if ui.button("Recompile shaders").clicked() {
                        let _ = self.sender.try_send(GuiEvent::RecompileShaders);
                    }
                    ui.style_mut().spacing.slider_width = 300.0;
                    ui.add(
                        egui::Slider::new(
                            &mut self.gui_params.point_light_position[0],
                            -30.0..=30.0,
                        )
                        .smart_aim(false),
                    );
                    ui.add(
                        egui::Slider::new(
                            &mut self.gui_params.point_light_position[1],
                            -30_f32..=30.0,
                        )
                        .smart_aim(false),
                    );
                    ui.add(
                        egui::Slider::new(
                            &mut self.gui_params.point_light_position[2],
                            -30.0..=30.0,
                        )
                        .smart_aim(false),
                    );

                    ui.separator();
                    ui.label("Gui size");
                    ui.add(egui::Slider::new(
                        &mut self.gui_params.gui_size[0],
                        0_f32..=2000.0,
                    ));
                    ui.add(egui::Slider::new(
                        &mut self.gui_params.gui_size[1],
                        0.0..=2000.0,
                    ));

                    ui.add(Separator::default().horizontal());

                    if Button::new("Save current level").ui(ui).clicked() {
                        let _ = self
                            .sender
                            .try_send(GuiEvent::ButtonClicked(GuiButton::SaveLevel));
                        let mut file_dialog = egui_file::FileDialog::open_file(None);
                        if file_dialog.show(ctx).selected() {
                            if let Some(file) = file_dialog.path() {
                                Some(file.to_path_buf());
                            }
                        }
                    }

                    // ui.horizontal(|ui| {
                    //     ui.label(format!("Pixels per point: {}", ctx.pixels_per_point()));
                    //     if ui.button("-").clicked() {
                    //         scale_factor = (scale_factor - 0.1).max(0.3);
                    //     }
                    //     if ui.button("+").clicked() {
                    //         scale_factor = (scale_factor + 0.1).min(3.0);
                    //     }
                    // });
                });
            },
        );

        if light_position != self.gui_params.point_light_position {
            self.sender
                .try_send(GuiEvent::LightPositionChanged {
                    new_position: self.gui_params.point_light_position,
                })
                .unwrap();
        }
    }

    pub fn set_shader_compilation_result(&mut self, result: &Vec<String>) {
        self.app_info.shader_error = result.join("\n");
    }

    pub fn update_frame_time(&mut self, frame_time: f32) {
        self.app_info.frame_time = frame_time;
        self.app_info.fps_counter = frame_time.recip() as u32;
    }

    pub fn handle_event(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::WindowEvent,
    ) -> bool {
        let response = self.renderer.handle_input(window, event);
        response.consumed
    }
}
