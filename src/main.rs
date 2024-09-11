mod app;
mod bind_group_layout_descriptors;
mod buffer;
mod buffer_capture;
mod buffer_content;
mod camera;
mod camera_controller;
mod color;
mod cubemap_helpers;
mod diffuse_irradiance_renderer;
mod equirectangular_to_cubemap_renderer;
mod file_loader;
mod forward_renderer;
mod frame_timer;
mod gbuffer_geometry_renderer;
mod gui;
mod gui_helpers;
mod input_actions;
mod instance;
mod light_controller;
mod lights;
mod mainloop;
mod material;
mod model;
mod object_picker;
mod pipelines;
mod player_controller;
mod post_process_manager;
mod primitive_shapes;
mod render_pipeline_layout;
mod renderer;
mod resource_loader;
mod shader_manager;
mod skybox;
mod super_hash_map;
mod texture;
mod vertex;
mod world;
mod world_loader;
mod world_renderer;

fn main() {
    async_std::task::block_on(mainloop::run_main_loop());
}
