use core::f32;
use std::{
    collections::HashMap,
    ops::RangeInclusive,
    path::PathBuf,
    str::from_utf8,
    time::{Duration, Instant},
};

use crossbeam_channel::Sender;
use egui::{
    Button, CollapsingHeader, FontId, Label, SelectableLabel, Separator, Slider, Ui, Widget,
};
use egui_wgpu::ScreenDescriptor;
use glam::{Quat, Vec3};
use rfd::FileDialog;
use ui_item::{
    DisplayNumberOnUiDescription, SetEnumFromTheUiDescription, SetNumberFromUiDescription,
    SetPathFromUiDescription, SetPropertyFromUiDescription, SetStructFromUiDesc,
    SetVecFromUiDescription, UiDisplayDescription,
};
use wgpu::{CommandEncoder, TextureFormat};
use winit::event::WindowEvent;

use crate::gui_helpers::EguiRenderer;

const LABEL_SIZE: [f32; 2] = [100.0, 10.0];

pub enum GuiButton {
    SaveLevel,
}

pub enum GuiUpdateEvent {
    ShaderCompilationResult(anyhow::Result<()>),
    LevelSaveResult(anyhow::Result<()>),
}

pub enum GuiEvent {
    RecompileShaders,
    ButtonClicked(GuiButton),
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

/// This is kind of a hacky solution.
/// When dropping a file, we have to save it, so we can handle it in the next render loop (unfortunately we don't have
/// both the file and the hovered element at one place, so we save the dropped file and when checking the hover, we
/// also check the saved dropped file as well)
/// The problem is that the hover even will sometimes happen a few frames after the drop, so we have to keep the dropped
/// file alive for <i>a short</i> period of time after the drop event
struct DroppedFileHandler {
    dropped_file: Option<PathBuf>,
    drop_time: Instant,
    keepalive_time: Duration,
}

// Keep the dropped file alive for half a sec. It's highly unlikely that we will get another drop event in that time
const DROPPED_FILE_KEEPALIVE_TIME_MS: u64 = 500;

impl DroppedFileHandler {
    fn update(&mut self) {
        if self.dropped_file.is_some() {
            if Instant::now() >= self.drop_time + self.keepalive_time {
                self.dropped_file = None;
            }
        }
    }

    fn add_dropped_file(&mut self, file: &PathBuf) {
        self.dropped_file = Some(file.clone());
        self.drop_time = Instant::now();
    }
}

#[derive(Debug, Clone)]
pub struct SetItemFromUiParams {
    pub category: String,
    pub item_setting_breadcrumbs: Vec<SetPropertyFromUiDescription>,
}

impl SetItemFromUiParams {
    fn add_breadcrumb(&self, new_breadcrumb: SetPropertyFromUiDescription) -> Self {
        let mut new_breadcrumbs = self.item_setting_breadcrumbs.clone();
        new_breadcrumbs.push(new_breadcrumb);

        Self {
            category: self.category.clone(),
            item_setting_breadcrumbs: new_breadcrumbs,
        }
    }

    fn get_last_category(&self) -> String {
        if let Some(last_item) = self.item_setting_breadcrumbs.last() {
            match last_item {
                SetPropertyFromUiDescription::Float(_)
                | SetPropertyFromUiDescription::Int(_)
                | SetPropertyFromUiDescription::Bool(_)
                | SetPropertyFromUiDescription::Vec3(_)
                | SetPropertyFromUiDescription::Rotation(_)
                | SetPropertyFromUiDescription::Path(_) => self.category.clone(),
                SetPropertyFromUiDescription::Struct(set_struct_from_ui_desc) => {
                    set_struct_from_ui_desc.field_name.clone()
                }
                SetPropertyFromUiDescription::Enum(set_enum_from_the_ui_description) => {
                    set_enum_from_the_ui_description.variant_name.clone()
                }
                SetPropertyFromUiDescription::Vec(set_vec_from_ui_description) => {
                    set_vec_from_ui_description.index.to_string()
                }
            }
        } else {
            self.category.clone()
        }
    }
}

pub struct Gui {
    renderer: EguiRenderer,
    sender: Sender<GuiEvent>,
    app_info: AppInfo,
    dropped_file_handler: DroppedFileHandler,
    registered_items: HashMap<String, (UiDisplayDescription, Sender<SetItemFromUiParams>)>,
}

impl Gui {
    pub fn new(
        window: &winit::window::Window,
        device: &wgpu::Device,
        sender: Sender<GuiEvent>,
    ) -> Self {
        let egui_renderer = EguiRenderer::new(&device, TextureFormat::Rgba8Unorm, None, 1, &window);
        Gui {
            sender,
            renderer: egui_renderer,
            app_info: AppInfo {
                recent_notification: None,
                frame_time: 0.0,
                fps_counter: 0,
            },
            registered_items: HashMap::new(),
            dropped_file_handler: DroppedFileHandler {
                dropped_file: None,
                drop_time: std::time::Instant::now(),
                keepalive_time: Duration::from_millis(DROPPED_FILE_KEEPALIVE_TIME_MS),
            },
        }
    }

    pub fn register_item(
        &mut self,
        category: &String,
        item: UiDisplayDescription,
        sender: Sender<SetItemFromUiParams>,
    ) -> bool {
        let insertion_result = self
            .registered_items
            .insert(category.clone(), (item, sender));
        insertion_result.is_none()
    }

    pub fn deregister_item(&mut self, category: &String) -> bool {
        self.registered_items.remove(category).is_some()
    }

    fn add_float_slider(
        ui: &mut Ui,
        slider_label: String,
        value: &mut f32,
        range: RangeInclusive<f32>,
    ) -> bool {
        let slider_response = ui
            .horizontal(|ui| {
                ui.add_sized(
                    LABEL_SIZE,
                    Label::new(slider_label).wrap_mode(egui::TextWrapMode::Truncate),
                );
                ui.add(Slider::new(value, range).smart_aim(false))
            })
            .inner;

        slider_response.changed()
    }

    fn add_vec3(
        ui: &mut Ui,
        slider_label: String,
        vec: &mut DisplayNumberOnUiDescription<Vec3>,
    ) -> bool {
        let mut any_component_changed = false;
        any_component_changed =
            Self::add_float_slider(ui, "x".to_owned(), &mut vec.value.x, vec.min.x..=vec.max.x)
                || any_component_changed;

        any_component_changed =
            Self::add_float_slider(ui, "y".to_owned(), &mut vec.value.y, vec.min.y..=vec.max.y)
                || any_component_changed;

        any_component_changed =
            Self::add_float_slider(ui, "z".to_owned(), &mut vec.value.z, vec.min.z..=vec.max.z)
                || any_component_changed;

        any_component_changed
    }

    fn add_item_to_ui(
        item: &mut UiDisplayDescription,
        ui: &mut Ui,
        breadcrumbs: SetItemFromUiParams,
        sender: &mut Sender<SetItemFromUiParams>,
        dropped_file: &mut Option<PathBuf>,
    ) {
        match item {
            UiDisplayDescription::SliderFloat(float_desc) => {
                let slider_changed = Self::add_float_slider(
                    ui,
                    breadcrumbs.get_last_category(),
                    &mut float_desc.value,
                    float_desc.min..=float_desc.max,
                );

                if slider_changed {
                    sender
                        .try_send(
                            breadcrumbs.add_breadcrumb(SetPropertyFromUiDescription::Float(
                                SetNumberFromUiDescription {
                                    value: float_desc.value,
                                },
                            )),
                        )
                        .unwrap();
                }
            }
            UiDisplayDescription::SliderInt(uint_desc) => {
                let slider_response = ui.horizontal(|ui| {
                    ui.add_sized(
                        LABEL_SIZE,
                        Label::new(breadcrumbs.get_last_category())
                            .wrap_mode(egui::TextWrapMode::Truncate),
                    );

                    ui.add(
                        egui::Slider::new(&mut uint_desc.value, uint_desc.min..=uint_desc.max)
                            .smart_aim(false),
                    )
                });

                if slider_response.inner.changed() {
                    sender
                        .try_send(
                            breadcrumbs.add_breadcrumb(SetPropertyFromUiDescription::Int(
                                SetNumberFromUiDescription {
                                    value: uint_desc.value,
                                },
                            )),
                        )
                        .unwrap();
                }
            }
            UiDisplayDescription::Path(path_desc) => {
                let button_response = Button::new(
                    path_desc
                        .path
                        .as_os_str()
                        .to_str()
                        .unwrap_or("Failed to get path"),
                )
                .ui(ui);
                if button_response.clicked() {
                    if let Some(file) = FileDialog::new()
                        .add_filter(
                            &path_desc.file_type_description,
                            &path_desc
                                .valid_file_extensions
                                .split(',')
                                .collect::<Vec<_>>(),
                        )
                        .pick_file()
                    {
                        sender
                            .try_send(breadcrumbs.add_breadcrumb(
                                SetPropertyFromUiDescription::Path(SetPathFromUiDescription {
                                    value: file,
                                }),
                            ))
                            .unwrap();
                    }
                } else if button_response.hovered() && dropped_file.is_some() {
                    {
                        let file_path = dropped_file.as_ref().unwrap();
                        sender
                            .try_send(breadcrumbs.add_breadcrumb(
                                SetPropertyFromUiDescription::Path(SetPathFromUiDescription {
                                    value: file_path.clone(),
                                }),
                            ))
                            .unwrap();
                    }
                    *dropped_file = None;
                }
            }
            UiDisplayDescription::Vec3(vec) => {
                let any_component_changed =
                    Self::add_vec3(ui, breadcrumbs.get_last_category(), vec);

                if any_component_changed {
                    sender
                        .try_send(
                            breadcrumbs
                                .add_breadcrumb(SetPropertyFromUiDescription::Vec3(vec.value)),
                        )
                        .unwrap();
                }
            }
            UiDisplayDescription::Vector(vec) => {
                for (index, desc) in vec.iter_mut().enumerate() {
                    let new_breadcrumb = breadcrumbs.add_breadcrumb(
                        SetPropertyFromUiDescription::Vec(SetVecFromUiDescription { index }),
                    );
                    Self::add_item_to_ui(desc, ui, new_breadcrumb, sender, dropped_file);
                }
            }
            UiDisplayDescription::Struct(display_params) => {
                ui.add(Separator::default().horizontal());

                for item in display_params {
                    Self::add_item_to_ui(
                        &mut item.display,
                        ui,
                        breadcrumbs.add_breadcrumb(SetPropertyFromUiDescription::Struct(
                            SetStructFromUiDesc {
                                field_name: item.name.clone(),
                            },
                        )),
                        sender,
                        dropped_file,
                    );
                }
            }
            UiDisplayDescription::Enum(display_enum_on_ui_description) => {
                let labels = display_enum_on_ui_description
                    .variants
                    .iter()
                    .map(|variant_name| {
                        SelectableLabel::new(
                            *variant_name == display_enum_on_ui_description.active_variant,
                            variant_name,
                        )
                    });
                for label in labels {
                    if ui.add(label).clicked() {}
                }
                if let Some(active_variant_item) =
                    &mut display_enum_on_ui_description.active_variant_item_desc
                {
                    Self::add_item_to_ui(
                        active_variant_item,
                        ui,
                        breadcrumbs.add_breadcrumb(SetPropertyFromUiDescription::Enum(
                            SetEnumFromTheUiDescription {
                                variant_name: display_enum_on_ui_description.active_variant.clone(),
                            },
                        )),
                        sender,
                        dropped_file,
                    );
                }
            }
            UiDisplayDescription::Rotation(display_rotation_on_ui_params) => {
                let mut any_component_changed = Self::add_vec3(
                    ui,
                    breadcrumbs.get_last_category(),
                    &mut display_rotation_on_ui_params.axis,
                );

                any_component_changed = Self::add_float_slider(
                    ui,
                    breadcrumbs.get_last_category(),
                    &mut display_rotation_on_ui_params.angle.value,
                    display_rotation_on_ui_params.angle.min
                        ..=display_rotation_on_ui_params.angle.max,
                ) || any_component_changed;

                if any_component_changed {
                    sender
                        .try_send(breadcrumbs.add_breadcrumb(
                            SetPropertyFromUiDescription::Rotation(Quat::from_axis_angle(
                                display_rotation_on_ui_params.axis.value,
                                display_rotation_on_ui_params.angle.value,
                            )),
                        ))
                        .unwrap();
                }
            }
            UiDisplayDescription::Bool(value) => {
                if ui
                    .checkbox(value, breadcrumbs.get_last_category())
                    .changed()
                {
                    sender
                        .try_send(
                            breadcrumbs.add_breadcrumb(SetPropertyFromUiDescription::Bool(*value)),
                        )
                        .unwrap();
                }
            }
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
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [config.width, config.height],
            pixels_per_point: window.scale_factor() as f32,
        };

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

                    ui.add(Separator::default().horizontal());

                    for (category, (item, sender)) in &mut self.registered_items {
                        CollapsingHeader::new(category)
                            .default_open(true)
                            .show(ui, |ui| {
                                Self::add_item_to_ui(
                                    item,
                                    ui,
                                    SetItemFromUiParams {
                                        category: category.clone(),
                                        item_setting_breadcrumbs: vec![],
                                    },
                                    sender,
                                    &mut self.dropped_file_handler.dropped_file,
                                );
                            });
                    }

                    ui.add(Separator::default().horizontal());

                    let button_response = Button::new("Change skybox").ui(ui);
                    if button_response.clicked() {
                        if let Some(file) = FileDialog::new()
                            .add_filter("hdr environment map", &["hdr"])
                            .pick_file()
                        {
                            println!("Some file was picked: {file:?}");
                        }
                    } else if button_response.hovered() {
                        if self.dropped_file_handler.dropped_file.is_some() {
                            let file_path =
                                self.dropped_file_handler.dropped_file.as_ref().unwrap();
                            println!("Some file was fropped: {file_path:?}");
                        }
                    }

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
                });
            },
        );

        // We don't want to use the dropped file anymore, we only keep it alive for one frame
        self.dropped_file_handler.dropped_file = None;
    }

    pub fn update(&mut self, delta: Duration) {
        self.dropped_file_handler.update();

        if let Some(operation_result) = &mut self.app_info.recent_notification {
            operation_result.progress_screen_time(delta.as_secs_f32());
            if operation_result.should_remove_from_ui() {
                self.app_info.recent_notification = None;
            }
        }

        self.app_info.frame_time = delta.as_secs_f32();
        self.app_info.fps_counter = self.app_info.frame_time.recip() as u32;
    }

    pub fn push_display_info_update(&mut self, update: GuiUpdateEvent) {
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

    pub fn handle_event(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::WindowEvent,
    ) -> bool {
        let response = self.renderer.handle_input(window, event);

        if !response.consumed {
            if let WindowEvent::DroppedFile(file_path) = &event {
                self.dropped_file_handler.add_dropped_file(file_path);
                return true;
            }
        }

        response.consumed
    }
}
