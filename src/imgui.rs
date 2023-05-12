use std::time::Duration;

use imgui::MouseCursor;
use imgui_wgpu::{Renderer, RendererConfig};
use imgui_winit_support::WinitPlatform;

#[derive(Default)]
pub struct ImguiParams {
    pub clear_color: [f32; 4],
}

pub struct Imgui {
    context: imgui::Context,
    renderer: Renderer,
    platform: WinitPlatform,
    is_ui_open: bool,
    last_cursor_position: Option<MouseCursor>,
}

impl Imgui {
    pub fn new(
        window: &winit::window::Window,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Self {
        let mut context = imgui::Context::create();
        let mut platform = imgui_winit_support::WinitPlatform::init(&mut context);

        platform.attach_window(
            context.io_mut(),
            &window,
            imgui_winit_support::HiDpiMode::Default,
        );
        context.set_ini_filename(None);

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

        let renderer_config = RendererConfig {
            texture_format: format,
            ..Default::default()
        };

        let renderer = Renderer::new(&mut context, &device, &queue, renderer_config);

        Imgui {
            context,
            renderer,
            platform,
            is_ui_open: true,
            last_cursor_position: None,
        }
    }

    pub fn render(
        &mut self,
        window: &winit::window::Window,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        delta: Duration,
        current_frame_texture_view: &wgpu::TextureView,
        params: &mut ImguiParams,
    ) {
        self.context.io_mut().update_delta_time(delta);

        self.platform
            .prepare_frame(self.context.io_mut(), &window)
            .expect("Failed to prepare frame");
        let ui = self.context.frame();

        {
            let window = ui.window("Hello world");
            window
                .size([300.0, 100.0], imgui::Condition::FirstUseEver)
                .build(|| {
                    ui.text("Hello world!");
                    ui.text("This...is...imgui-rs on WGPU!");
                    ui.separator();
                    let mouse_pos = ui.io().mouse_pos;
                    ui.text(format!(
                        "Mouse Position: ({:.1},{:.1})",
                        mouse_pos[0], mouse_pos[1]
                    ));
                    ui.color_picker4("Clear color", &mut params.clear_color);
                });

            let window = ui.window("Hello too");
            window
                .size([400.0, 200.0], imgui::Condition::FirstUseEver)
                .position([400.0, 200.0], imgui::Condition::FirstUseEver)
                .build(|| {
                    ui.text(format!("Frametime: {delta:?}"));
                });

            ui.show_demo_window(&mut self.is_ui_open);
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
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            self.renderer
                .render(self.context.render(), &queue, &device, &mut rpass)
                .expect("Rendering failed");
        }

        queue.submit(Some(encoder.finish()));
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