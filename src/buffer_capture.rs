use std::{fs::File, io::Write};

use wgpu::{Device, Extent3d, SubmissionIndex, TextureFormat};

fn get_bytes_per_pixel(format: &TextureFormat) -> u32 {
    match format {
        TextureFormat::Rgba16Float => 2 * 4,
        TextureFormat::R32Uint => 4,
        _ => unimplemented!(
            "Capturing images with format {:?} is not yet supported.
            Add the bytes per pixel value here and it will work!",
            format
        ),
    }
}

fn calculate_padded_size_for_image_copy_buffer(width: u32, format: &TextureFormat) -> u32 {
    let bytes_per_pixel = get_bytes_per_pixel(format);

    let unpadded_bytes_per_row = (width * bytes_per_pixel) as usize;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
    let padded_bytes_per_row_padding = (align - unpadded_bytes_per_row % align) % align;
    (unpadded_bytes_per_row + padded_bytes_per_row_padding) as u32
}

pub struct OutputBuffer {
    /// Size of each row, padded to `wgpu::COPY_BYTES_PER_ROW_ALIGNMENT`, as that is a requirement
    /// of ImageCopyBuffer
    pub padded_row_size: u32,
    pub buffer: wgpu::Buffer,
    pub texture_extent: Extent3d,
    pub texture_format: TextureFormat,
}

impl OutputBuffer {
    pub fn new(device: &wgpu::Device, texture_extent: &Extent3d, format: &TextureFormat) -> Self {
        // It is a WebGPU requirement that ImageCopyBuffer.layout.bytes_per_row % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT == 0
        // So we calculate padded_bytes_per_row by rounding unpadded_bytes_per_row
        // up to the next multiple of wgpu::COPY_BYTES_PER_ROW_ALIGNMENT.
        // https://en.wikipedia.org/wiki/Data_structure_alignment#Computing_padding
        let padded_row_size =
            calculate_padded_size_for_image_copy_buffer(texture_extent.width, format);

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Buffer to copy frame content into"),
            size: (texture_extent.depth_or_array_layers * padded_row_size * texture_extent.height)
                as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        OutputBuffer {
            padded_row_size,
            buffer,
            texture_extent: texture_extent.clone(),
            texture_format: *format,
        }
    }

    pub async fn save_buffer_to_file(
        &self,
        output_path: &str,
        submission_index: Option<SubmissionIndex>,
        device: &Device,
    ) {
        let buffer_slice = self.buffer.slice(..);
        // Sets the buffer up for mapping, sending over the result of the mapping back to us when it is finished.
        let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

        if let Some(submission_index) = submission_index {
            device.poll(wgpu::Maintain::WaitForSubmissionIndex(submission_index));
        }

        if let Some(Ok(())) = receiver.receive().await {
            let padded_buffer = buffer_slice.get_mapped_range();
            let mut file = File::create(output_path).unwrap();

            file.write_all(&padded_buffer).unwrap();

            // With the current interface, we have to make sure all mapped views are
            // dropped before we unmap the buffer.
            drop(padded_buffer);

            self.buffer.unmap();
        }
    }
}
