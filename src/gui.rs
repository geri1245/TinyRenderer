use std::{path::PathBuf, str::FromStr, time::Duration};

use crossbeam_channel::Sender;
use imgui::MouseCursor;
use imgui_wgpu::{Renderer, RendererConfig};
use imgui_winit_support::WinitPlatform;

pub enum GuiEvent {
    RecompileShaders,
    LightPositionChanged { new_position: [f32; 3] },
}

#[derive(Default)]
pub struct GuiParams {
    pub point_light_position: [f32; 3],
    pub fov_x: f32,
    pub fov_y: f32,
}

pub struct Gui {
    context: imgui::Context,
    renderer: Renderer,
    platform: WinitPlatform,
    is_ui_open: bool,
    last_cursor_position: Option<MouseCursor>,
    sender: Sender<GuiEvent>,
    gui_params: GuiParams,
    shader_error: String,
}

impl Gui {
    pub fn new(
        window: &winit::window::Window,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sender: Sender<GuiEvent>,
    ) -> Self {
        let mut context = imgui::Context::create();
        let mut platform = imgui_winit_support::WinitPlatform::init(&mut context);

        platform.attach_window(
            context.io_mut(),
            &window,
            imgui_winit_support::HiDpiMode::Default,
        );
        context.set_ini_filename(Some(PathBuf::from_str("imgui_config.ini").unwrap()));

        let hidpi_factor = window.scale_factor();
        let font_size = (13.0 * hidpi_factor) as f32;
        context.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

        context
            .fonts()
            .add_font(&[imgui::FontSource::DefaultFontData {
                config: Some(imgui::FontConfig {
                    oversample_h: 1,
                    pixel_snap_h: true,
                    size_pixels: font_size,
                    ..Default::default()
                }),
            }]);

        let renderer_config = RendererConfig::new_srgb();

        let renderer = Renderer::new(&mut context, &device, &queue, renderer_config);

        let gui_params = GuiParams {
            point_light_position: [10.0, 20.0, 0.0],
            fov_x: 90.0,
            fov_y: 45.0,
        };

        Gui {
            context,
            renderer,
            platform,
            is_ui_open: true,
            last_cursor_position: None,
            sender,
            gui_params,
            shader_error: "".into(),
        }
    }

    pub fn render(
        &mut self,
        window: &winit::window::Window,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        delta: Duration,
        current_frame_texture_view: &wgpu::TextureView,
    ) {
        let light_position = self.gui_params.point_light_position;

        self.context.io_mut().update_delta_time(delta);

        self.platform
            .prepare_frame(self.context.io_mut(), window)
            .expect("Failed to prepare frame");
        let ui = self.context.frame();

        {
            let window = ui.window("Render parameter configurations");
            window
                .size([300.0, 100.0], imgui::Condition::FirstUseEver)
                .build(|| {
                    ui.text("Hello world!");

                    if ui.button("Recompile shaders") {
                        self.sender.send(GuiEvent::RecompileShaders).unwrap_or(());
                    }

                    if !self.shader_error.is_empty() {
                        ui.text(&self.shader_error)
                    }

                    ui.separator();
                    ui.slider("FOV (vertical)", 40.0, 50.0, &mut self.gui_params.fov_x);
                    const MIN: f32 = -30.0;
                    const MAX: f32 = 30.0;
                    ui.slider(
                        "Point light position x",
                        MIN,
                        MAX,
                        &mut self.gui_params.point_light_position[0],
                    );
                    ui.slider(
                        "Point light position y",
                        MIN,
                        MAX,
                        &mut self.gui_params.point_light_position[1],
                    );
                    ui.slider(
                        "Point light position z",
                        MIN,
                        MAX,
                        &mut self.gui_params.point_light_position[2],
                    );
                });

            ui.show_demo_window(&mut self.is_ui_open);

            if light_position != self.gui_params.point_light_position {
                self.sender
                    .try_send(GuiEvent::LightPositionChanged {
                        new_position: self.gui_params.point_light_position,
                    })
                    .unwrap();
            }
        }

        let mut encoder: wgpu::CommandEncoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        if self.last_cursor_position != ui.mouse_cursor() {
            self.last_cursor_position = ui.mouse_cursor();
            self.platform.prepare_render(ui, &window);
        }

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &current_frame_texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.renderer
                .render(self.context.render(), &queue, &device, &mut rpass)
                .expect("Rendering failed");
        }

        queue.submit(Some(encoder.finish()));
    }

    pub fn set_shader_compilation_result(&mut self, result: &Vec<String>) {
        self.shader_error = result.join("\n");
    }

    pub fn handle_event<'a, T>(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::Event<T>,
    ) {
        self.platform
            .handle_event(self.context.io_mut(), window, event);
    }
}
