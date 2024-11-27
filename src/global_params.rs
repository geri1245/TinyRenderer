/// These will be transferred to the GPU and can be used there
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlobalGPUParams {
    pub random_param: f32,
}

impl Default for GlobalGPUParams {
    fn default() -> Self {
        Self { random_param: 1.0 }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct GlobalCPUParams {
    pub shadow_bias: f32,
    pub scale: f32,
}

impl Default for GlobalCPUParams {
    fn default() -> Self {
        Self {
            shadow_bias: 1.0,
            scale: Default::default(),
        }
    }
}
