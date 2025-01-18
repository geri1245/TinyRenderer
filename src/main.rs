#[macro_use]
mod world_object;

mod actions;
mod app;
mod bind_group_layout_descriptors;
mod buffer;
mod buffer_content;
mod camera;
mod camera_controller;
mod components;
mod cubemap_helpers;
mod custom_event;
mod diffuse_irradiance_renderer;
mod equirectangular_to_cubemap_renderer;
mod file_loader;
mod forward_renderer;
mod frame_timer;
mod gbuffer_geometry_renderer;
mod gizmo;
mod gizmo_handler;
mod global_params;
mod gpu_buffer;
mod gui;
mod gui_helpers;
mod gui_settable_value;
mod light_controller;
mod light_render_data;
mod light_rendering_gpu_data;
mod lights;
mod mainloop;
mod mappable_gpu_buffer;
mod material;
mod math;
mod mipmap_generator;
mod model;
mod object_picker;
mod pipelines;
mod player_controller;
mod pollable_gpu_buffer;
mod post_process_manager;
mod primitive_shapes;
mod render_pipeline;
mod render_pipeline_layout;
mod renderer;
mod resource_loader;
mod skybox;
mod texture;
mod vertex;
mod world;
mod world_loader;
mod world_renderer;

fn main() {
    mainloop::run_main_loop();
}
