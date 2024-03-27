mod app;
mod bind_group_layout_descriptors;
mod buffer;
mod buffer_capture;
mod buffer_content;
mod camera;
mod camera_controller;
mod color;
mod diffuse_irradiance_renderer;
mod equirec_to_cubemap_renderer;
mod equirectangular_to_cubemap_renderer;
mod file_loader;
mod forward_renderer;
mod frame_timer;
mod gbuffer_geometry_renderer;
mod gui;
mod instance;
mod light_controller;
mod lights;
mod mainloop;
mod model;
mod pipelines;
mod post_process_manager;
mod primitive_shapes;
mod render_pipeline_layout;
mod renderer;
mod resource_loader;
mod shader_manager;
mod skybox;
mod texture;
mod vertex;
mod world;
mod world_renderer;

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    const EQUIREC_TO_CUBE_COMMAND_LINE_ARG_NAME: &str = "--equirec_to_cube";

    if let Some(arg_pos) = args
        .iter()
        .position(|item| item == EQUIREC_TO_CUBE_COMMAND_LINE_ARG_NAME)
    {
        if args.len() < arg_pos + 1 {
            panic!("Please specify the name of the equirectangular map to convert!");
        }

        async_std::task::block_on(equirec_to_cubemap_renderer::render_equirec_to_cubemap(
            &args[arg_pos + 1],
        ));
    }

    async_std::task::block_on(mainloop::run_main_loop());
}
