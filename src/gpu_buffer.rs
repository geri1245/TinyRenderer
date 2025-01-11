use std::ops::{Deref, DerefMut};

use wgpu::{BindGroup, Buffer, Device, Queue};

use crate::buffer::{create_bind_group_from_buffer_entire_binding_init, GpuBufferCreationOptions};

pub struct GpuBuffer<T>
where
    T: bytemuck::Pod + bytemuck::Zeroable,
{
    data: T,
    pub bind_group: BindGroup,
    buffer: Buffer,
}

impl<T> Deref for GpuBuffer<T>
where
    T: bytemuck::Pod + bytemuck::Zeroable,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

pub struct GpuBufferUpdateGuard<'buffer, T>
where
    T: bytemuck::Pod + bytemuck::Zeroable,
{
    gpu_buffer: &'buffer mut GpuBuffer<T>,
    queue: &'buffer Queue,
}

impl<'buffer, T> Drop for GpuBufferUpdateGuard<'buffer, T>
where
    T: bytemuck::Pod + bytemuck::Zeroable,
{
    fn drop(&mut self) {
        self.gpu_buffer.update_gpu_data(self.queue);
    }
}

impl<'buffer, T> Deref for GpuBufferUpdateGuard<'buffer, T>
where
    T: bytemuck::Pod + bytemuck::Zeroable,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.gpu_buffer.data
    }
}

impl<'buffer, T> DerefMut for GpuBufferUpdateGuard<'buffer, T>
where
    T: bytemuck::Pod + bytemuck::Zeroable,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.gpu_buffer.data
    }
}

impl<T> GpuBuffer<T>
where
    T: bytemuck::Pod + bytemuck::Zeroable,
{
    pub fn new(data: T, device: &Device, creation_options: &GpuBufferCreationOptions) -> Self {
        let (buffer, bind_group) = create_bind_group_from_buffer_entire_binding_init(
            device,
            creation_options,
            bytemuck::cast_slice(&[data]),
        );
        Self {
            data,
            bind_group,
            buffer,
        }
    }

    pub fn get_mut_data<'buffer>(
        &'buffer mut self,
        queue: &'buffer Queue,
    ) -> GpuBufferUpdateGuard<'buffer, T> {
        GpuBufferUpdateGuard {
            gpu_buffer: self,
            queue,
        }
    }

    fn update_gpu_data(&self, queue: &Queue) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[self.data]));
    }
}
