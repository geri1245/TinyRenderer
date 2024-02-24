use anyhow::anyhow;
use async_std::fs;
use std::{borrow::Cow, os::windows::fs::MetadataExt};
use wgpu::Device;

use super::shader_compilation_result::CompiledShader;

pub trait RenderPipelineBase {
    async fn compile_shader_if_needed(
        source: &str,
        device: &Device,
    ) -> anyhow::Result<CompiledShader> {
        let shader_contents = fs::read_to_string(source).await?;
        let shader_desc = wgpu::ShaderModuleDescriptor {
            label: Some(source.split("/").last().unwrap()),
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
            let last_write_time = match fs::metadata(source).await {
                Ok(metadata) => metadata.last_write_time(),
                // If we can't get the last write time, not a big deal, the compilation is what matters
                Err(_) => 0u64,
            };

            Ok(CompiledShader {
                shader_module: shader,
                last_write_time,
            })
        }
    }

    async fn need_recompile_shader(shader_path: &str, last_compile_time: u64) -> bool {
        let metadata = fs::metadata(shader_path).await.unwrap();
        metadata.last_write_time() > last_compile_time
    }
}
