use ui_item_derive::{UiDisplayable, UiSettableNew};

/// These will be transferred to the GPU and can be used there
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, UiDisplayable, UiSettableNew)]
pub struct GlobalGPUParams {
    #[ui_param(fmin = 0.0, fmax = 5.0)]
    pub random_param: f32,
    #[ui_param(min = 0, max = 3)]
    pub tone_mapping_type: u32,
    #[ui_param(fmin = 0.01, fmax = 0.1)]
    pub ssr_thickness: f32,
}

impl Default for GlobalGPUParams {
    fn default() -> Self {
        Self {
            random_param: 1.0,
            tone_mapping_type: 1,
            ssr_thickness: 0.01,
        }
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
