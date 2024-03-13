pub const fn _wgpu_color_to_f32_array_rgba(color: wgpu::Color) -> [f32; 4] {
    [
        color.r as f32,
        color.g as f32,
        color.b as f32,
        color.a as f32,
    ]
}

pub const fn _f32_array_rgba_to_wgpu_color(color_array: [f32; 4]) -> wgpu::Color {
    wgpu::Color {
        r: color_array[0] as f64,
        g: color_array[1] as f64,
        b: color_array[2] as f64,
        a: color_array[3] as f64,
    }
}
