use crossbeam_channel::{Receiver, Sender};
use wgpu::{BufferAsyncError, Extent3d, TextureFormat};

use crate::mappable_gpu_buffer::MapableGpuBuffer;

/// A buffer written by the GPU, whose contents can be read back to the CPU
pub struct PollableGpuBuffer {
    pub mapable_buffer: MapableGpuBuffer,
    receiver: Receiver<Result<(), BufferAsyncError>>,
    sender: Sender<Result<(), BufferAsyncError>>,
}

impl PollableGpuBuffer {
    pub fn new(device: &wgpu::Device, texture_extent: &Extent3d, format: &TextureFormat) -> Self {
        let buffer = MapableGpuBuffer::new(device, texture_extent, format);
        let (sender, receiver) = crossbeam_channel::bounded(1);

        Self {
            mapable_buffer: buffer,
            receiver,
            sender,
        }
    }

    pub fn post_render(&self) {
        let buffer_slice = self.mapable_buffer.buffer.slice(..);
        let sender = self.sender.clone();
        // Sets the buffer up for mapping, sending over the result of the mapping back to us when it is finished.
        buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());
    }

    pub fn poll_mapped_buffer(&self, result_vec: &mut Vec<u32>) -> Option<u32> {
        match self.receiver.try_recv() {
            Ok(result) => {
                if result.is_ok() {
                    // If the buffer is laid out in a single dimension, what is the element index that we need?
                    let padded_buffer = self.mapable_buffer.buffer.slice(..).get_mapped_range();

                    let u32data: &[u32] = bytemuck::cast_slice(&*padded_buffer);
                    result_vec.clear();
                    result_vec.extend_from_slice(u32data);

                    drop(padded_buffer);

                    self.mapable_buffer.buffer.unmap();

                    Some(self.mapable_buffer.padded_row_size / 4)
                } else {
                    None
                }
            }
            Err(_error) => None,
        }
    }
}
