use core::f32;
use std::{ops::RangeInclusive, str::from_utf8, time::Duration};

use crossbeam_channel::Sender;
use egui::{Button, FontId, Label, Separator, Ui, Widget};
use egui_wgpu::ScreenDescriptor;
use wgpu::{CommandEncoder, TextureFormat};

use crate::gui_helpers::EguiRenderer;

pub enum GuiButton {
    SaveLevel,
}

pub enum GuiUpdateEvent {
    ShaderCompilationResult(anyhow::Result<()>),
    LevelSaveResult(anyhow::Result<()>),
}

pub enum GuiEvent {
    RecompileShaders,
    LightPositionChanged { new_position: [f32; 3] },
    ButtonClicked(GuiButton),
    FieldValueChanged(String, f32),
}

struct GuiNotification {
    notification_text: String,
    auto_remove_after_time: bool,
    screen_time: f32,
    max_screen_time: f32,
}

impl GuiNotification {
    fn from_result(result: anyhow::Result<()>, category_string: String) -> Self {
        let result_as_string = match &result {
            Ok(_) => "Success!".into(),
            Err(error) => error.to_string(),
        };

        let final_message = category_string + &result_as_string;

        // If the result was success, then we remove it from the UI after some time. If we had an error, we keep it on the
        // screen, as in that case we expect the user to take some action and retry whatever action resulted in errors
        GuiNotification {
            notification_text: from_utf8(final_message.as_bytes()).unwrap().into(),
            screen_time: 0.0,
            max_screen_time: 3.0,
            auto_remove_after_time: result.is_ok(),
        }
    }

    fn progress_screen_time(&mut self, delta: f32) {
        self.screen_time += delta;
    }

    fn should_remove_from_ui(&self) -> bool {
        self.auto_remove_after_time && self.screen_time >= self.max_screen_time
    }
}

struct AppInfo {
    recent_notification: Option<GuiNotification>,
    frame_time: f32,
    fps_counter: u32,
}

#[derive(Default)]
pub struct GuiParams {
    pub point_light_position: [f32; 3],
    gui_size: [f32; 2],
    pub random_parameter: f32,
    pub tone_mapping_method: u32,
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
            random_parameter: 1.0,
            scale_factor: 1.0,
            gui_size: [500.0, 300.0],
            tone_mapping_method: 1,
        };

        Gui {
            sender,
            gui_params,
            renderer: egui_renderer,
            app_info: AppInfo {
                recent_notification: None,
                frame_time: 0.0,
                fps_counter: 0,
            },
        }
    }

    fn add_float_slider_with_change_notification(
        ui: &mut Ui,
        value: &mut f32,
        name: String,
        range: RangeInclusive<f32>,
        sender: &mut Sender<GuiEvent>,
    ) {
        ui.horizontal(|ui| {
            ui.add(Label::new(&name));
            ui.add(Separator::default().vertical());
            let slider_response = ui.add(egui::Slider::new(value, range).smart_aim(false));

            if slider_response.changed() {
                sender
                    .try_send(GuiEvent::FieldValueChanged(name, *value))
                    .unwrap();
            }
        });
    }

    fn add_integer_slider_with_change_notification(
        ui: &mut Ui,
        value: &mut u32,
        name: String,
        range: RangeInclusive<u32>,
        sender: &mut Sender<GuiEvent>,
    ) {
        ui.horizontal(|ui| {
            ui.add(Label::new(&name));
            ui.add(Separator::default().vertical());
            let slider_response = ui.add(egui::Slider::new(value, range).integer());

            if slider_response.changed() {
                sender
                    .try_send(GuiEvent::FieldValueChanged(name, *value as f32))
                    .unwrap();
            }
        });
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
            &mut |ctx| {
                egui::Window::new("Settings page").show(&ctx, |ui| {
                    let frame_time_string = self.app_info.frame_time.to_string();
                    let fps_string = self.app_info.fps_counter.to_string();
                    ui.label(format!("Frame time: {frame_time_string}"));
                    ui.label(format!("FPS: {fps_string}"));

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

                    Self::add_float_slider_with_change_notification(
                        ui,
                        &mut self.gui_params.random_parameter,
                        "random parameter".into(),
                        0.0..=5.0,
                        &mut self.sender,
                    );

                    Self::add_integer_slider_with_change_notification(
                        ui,
                        &mut self.gui_params.tone_mapping_method,
                        "tone mapping method".into(),
                        0..=5,
                        &mut self.sender,
                    );

                    ui.add(Separator::default().horizontal());

                    if Button::new("Save current level").ui(ui).clicked() {
                        let _ = self
                            .sender
                            .try_send(GuiEvent::ButtonClicked(GuiButton::SaveLevel));
                    }

                    if let Some(result) = &self.app_info.recent_notification {
                        let color = if result.auto_remove_after_time {
                            egui::Color32::from_rgb(112, 200, 128)
                        } else {
                            egui::Color32::from_rgb(255, 166, 166)
                        };
                        ui.label(
                            egui::RichText::new(&result.notification_text)
                                .color(color)
                                .font(FontId {
                                    size: 14.0,
                                    family: egui::FontFamily::Monospace,
                                }),
                        );
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

    pub fn update(&mut self, delta: Duration) {
        if let Some(operation_result) = &mut self.app_info.recent_notification {
            operation_result.progress_screen_time(delta.as_secs_f32());
            if operation_result.should_remove_from_ui() {
                self.app_info.recent_notification = None;
            }
        }
    }

    pub fn push_update(&mut self, update: GuiUpdateEvent) {
        match update {
            GuiUpdateEvent::ShaderCompilationResult(result) => {
                self.app_info.recent_notification = Some(GuiNotification::from_result(
                    result,
                    "Shader compilation result: ".into(),
                ));
            }
            GuiUpdateEvent::LevelSaveResult(result) => {
                self.app_info.recent_notification = Some(GuiNotification::from_result(
                    result,
                    "Saving level result: ".into(),
                ));
            }
        };
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
