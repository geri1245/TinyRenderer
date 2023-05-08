pub trait BasicRenderable {
    fn get_vertex_buffer(&self) -> wgpu::Buffer;
    fn get_index_buffer(&self) -> wgpu::Buffer;
    fn get_instance_buffer(&self) -> wgpu::Buffer;

    fn get_index_count(&self) -> u32;
}
