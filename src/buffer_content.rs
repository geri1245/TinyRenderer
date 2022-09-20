pub trait BufferContent {
    fn buffer_layout<'a>() -> wgpu::VertexBufferLayout<'a>;
}
