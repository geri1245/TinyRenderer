use anyhow::anyhow;
use async_std::task::block_on;
use std::{borrow::Cow, fs, os::windows::fs::MetadataExt};
use wgpu::{Device, ShaderModule};

pub enum ShaderCompilationResult {
    AlreadyUpToDate,
    Success(ShaderModule),
}

#[derive(Debug, PartialEq, Eq)]
pub enum ShaderCompilationSuccess {
    AlreadyUpToDate,
    Recompiled,
}

pub struct ShaderCompiler {
    last_compile_time: u64,
    shader_source: String,
}

impl ShaderCompiler {
    pub fn new(source_path: String) -> Self {
        Self {
            last_compile_time: 0,
            shader_source: source_path,
        }
    }

    pub fn compile_shader_if_needed(
        &mut self,
        device: &Device,
    ) -> anyhow::Result<ShaderCompilationResult> {
        let last_write_time = match fs::metadata(&self.shader_source) {
            Ok(metadata) => metadata.last_write_time(),
            // If we can't get the last write time, let's just recompile the shader
            Err(_) => 0u64,
        };

        if last_write_time <= self.last_compile_time {
            return Ok(ShaderCompilationResult::AlreadyUpToDate);
        }

        let shader_contents = fs::read_to_string(&self.shader_source)?;
        let shader_desc = wgpu::ShaderModuleDescriptor {
            label: Some(self.shader_source.split("/").last().unwrap()),
            source: wgpu::ShaderSource::Wgsl(Cow::from(shader_contents)),
        };
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = device.create_shader_module(shader_desc);
        if let Some(error) = block_on(device.pop_error_scope()) {
            match error {
                wgpu::Error::OutOfMemory { .. } => Err(anyhow!("Out of memory")),
                wgpu::Error::Validation { description, .. } => Err(anyhow!(description)),
                wgpu::Error::Internal { description, .. } => Err(anyhow!(description)),
            }
        } else {
            let last_write_time = match fs::metadata(&self.shader_source) {
                Ok(metadata) => metadata.last_write_time(),
                // If we can't get the last write time, not a big deal, the compilation is what matters
                Err(_) => 0u64,
            };

            self.last_compile_time = last_write_time;

            Ok(ShaderCompilationResult::Success(shader))
        }
    }
}
