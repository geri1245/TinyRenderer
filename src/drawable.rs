use std::ops::Range;

pub trait Drawable<'a, 'b> {
    fn draw(&'a self, render_pass: &mut wgpu::RenderPass<'b>) {
        self.draw_instanced(render_pass, 0..1);
    }

    fn draw_instanced(&'a self, render_pass: &mut wgpu::RenderPass<'b>, instances: Range<u32>);
}
