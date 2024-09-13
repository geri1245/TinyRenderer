use crossbeam_channel::Receiver;
use log::warn;
use wgpu::{BufferAsyncError, Device, Extent3d, TextureFormat};

use crate::buffer_capture::OutputBuffer;

pub struct ReadableBuffer {
    pub mapable_buffer: OutputBuffer,
    position: (u32, u32),
    receiver: Receiver<Result<(), BufferAsyncError>>,
}

impl ReadableBuffer {
    pub fn new(
        device: &wgpu::Device,
        texture_extent: &Extent3d,
        format: &TextureFormat,
        x: u32,
        y: u32,
    ) -> Self {
        let buffer = OutputBuffer::new(device, texture_extent, format);
        let (sender, receiver) = crossbeam_channel::bounded(4);
        let buffer_slice = buffer.buffer.slice(..);
        // Sets the buffer up for mapping, sending over the result of the mapping back to us when it is finished.
        buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

        Self {
            mapable_buffer: buffer,
            receiver,
            position: (x, y),
        }
    }

    pub fn get_value_at_position<T>(&self, device: &Device) -> Option<u32> {
        device.poll(wgpu::MaintainBase::Poll);
        match self.receiver.try_recv() {
            Ok(result) => {
                if result.is_ok() {
                    // If the buffer is laid out in a single dimension, what is the element index that we need?
                    let padded_buffer = self.mapable_buffer.buffer.slice(..).get_mapped_range();

                    let index =
                        self.position.1 * self.mapable_buffer.padded_row_size + self.position.0;
                    let u32data: &[u32] = bytemuck::cast_slice(&*padded_buffer);
                    let return_value = u32data[index as usize];

                    drop(padded_buffer);

                    self.mapable_buffer.buffer.unmap();

                    Some(return_value)
                } else {
                    warn!("We got an error: {result:?}");
                    None
                }
            }
            Err(error) => {
                warn!("We got an error: {error:?}");
                None
            }
        }
    }
}
