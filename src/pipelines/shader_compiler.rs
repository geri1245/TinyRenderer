use anyhow::anyhow;
use async_std::fs;
use std::{borrow::Cow, os::windows::fs::MetadataExt};
use wgpu::{Device, ShaderModule};

pub enum ShaderCompilationResult {
    AlreadyUpToDate,
    Success(ShaderModule),
}

pub struct ShaderCompiler {
    last_compile_time: u64,
    shader_source: &'static str,
}

impl ShaderCompiler {
    pub fn new(source_path: &'static str) -> Self {
        Self {
            last_compile_time: 0,
            shader_source: source_path,
        }
    }

    pub async fn compile_shader_if_needed(
        &mut self,
        device: &Device,
    ) -> anyhow::Result<ShaderCompilationResult> {
        let last_write_time = match fs::metadata(&self.shader_source).await {
            Ok(metadata) => metadata.last_write_time(),
            // If we can't get the last write time, not a big deal, the compilation is what matters
            Err(_) => 0u64,
        };

        if last_write_time <= self.last_compile_time {
            return Ok(ShaderCompilationResult::AlreadyUpToDate);
        }

        let shader_contents = fs::read_to_string(&self.shader_source).await?;
        let shader_desc = wgpu::ShaderModuleDescriptor {
            label: Some(self.shader_source.split("/").last().unwrap()),
            source: wgpu::ShaderSource::Wgsl(Cow::from(shader_contents)),
        };
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = device.create_shader_module(shader_desc);
        if let Some(error) = device.pop_error_scope().await {
            match error {
                wgpu::Error::OutOfMemory { .. } => todo!(),
                wgpu::Error::Validation { description, .. } => Err(anyhow!(description)),
            }
        } else {
            let last_write_time = match fs::metadata(&self.shader_source).await {
                Ok(metadata) => metadata.last_write_time(),
                // If we can't get the last write time, not a big deal, the compilation is what matters
                Err(_) => 0u64,
            };

            self.last_compile_time = last_write_time;

            Ok(ShaderCompilationResult::Success(shader))
        }
    }
}
