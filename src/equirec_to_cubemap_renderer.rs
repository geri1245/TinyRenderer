use std::fs::File;
use std::io::BufReader;

use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

use crate::renderer::Renderer;

pub async fn render_equirec_to_cubemap(file_name: &str) {
    let event_loop = EventLoop::new().unwrap();

    let window = WindowBuilder::new().build(&event_loop).unwrap();

    let renderer = Renderer::new(&window);

    let file = File::open(file_name).expect("Failed to open specified file");
    let reader = BufReader::new(file);
    let image = radiant::load(reader).expect("Failed to load image data");

    let mut raw_data = Vec::new();
    raw_data.reserve(image.data.len() * 3);
    for rgb in image.data {
        raw_data.push(rgb.r);
        raw_data.push(rgb.g);
        raw_data.push(rgb.b);
    }
}
