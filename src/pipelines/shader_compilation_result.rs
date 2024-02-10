use wgpu::ShaderModule;

pub struct CompiledShader {
    pub shader_module: ShaderModule,
    pub last_write_time: u64,
}

pub enum PipelineRecreationResult<Pipeline> {
    AlreadyUpToDate,
    Success(Pipeline),
    Failed(anyhow::Error),
}
