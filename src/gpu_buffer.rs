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

    pub fn get_data(&self) -> T {
        self.data
    }

    pub fn update_data(&mut self, queue: &Queue, new_data: T) {
        self.data = new_data;
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[new_data]));
    }
}
