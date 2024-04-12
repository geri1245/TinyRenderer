use std::{fs::File, io::Write};

use wgpu::Device;

struct BufferDimensions {
    width: usize,
    height: usize,
    unpadded_bytes_per_row: usize,
    padded_bytes_per_row: usize,
}

impl BufferDimensions {
    fn new(width: usize, height: usize) -> Self {
        let bytes_per_pixel = 2 * 4;
        let unpadded_bytes_per_row = width * bytes_per_pixel;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
        let padded_bytes_per_row_padding = (align - unpadded_bytes_per_row % align) % align;
        let padded_bytes_per_row = unpadded_bytes_per_row + padded_bytes_per_row_padding;
        Self {
            width,
            height,
            unpadded_bytes_per_row,
            padded_bytes_per_row,
        }
    }
}

pub struct OutputBuffer {
    dimensions: BufferDimensions,
    pub buffer: wgpu::Buffer,
    pub texture_extent: wgpu::Extent3d,
}

impl OutputBuffer {
    pub fn get_bytes_per_row(&self) -> u32 {
        self.dimensions.padded_bytes_per_row as u32
    }

    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        // It is a WebGPU requirement that ImageCopyBuffer.layout.bytes_per_row % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT == 0
        // So we calculate padded_bytes_per_row by rounding unpadded_bytes_per_row
        // up to the next multiple of wgpu::COPY_BYTES_PER_ROW_ALIGNMENT.
        // https://en.wikipedia.org/wiki/Data_structure_alignment#Computing_padding
        let dimensions = BufferDimensions::new(width as usize, height as usize);
        // The output buffer lets us retrieve the data as an array
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Buffer to copy frame content into"),
            size: (6 * dimensions.padded_bytes_per_row * dimensions.height) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let texture_extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 6,
        };

        OutputBuffer {
            dimensions,
            buffer,
            texture_extent,
        }
    }

    pub async fn save_buffer_to_file(
        &self,
        output_path: &str,
        submission_index: wgpu::SubmissionIndex,
        device: &Device,
    ) {
        let buffer_slice = self.buffer.slice(..);
        // Sets the buffer up for mapping, sending over the result of the mapping back to us when it is finished.
        let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

        // TODO: Either Poll without blocking or move the blocking polling to another thread
        device.poll(wgpu::Maintain::WaitForSubmissionIndex(submission_index));

        let has_file_system_available = cfg!(not(target_arch = "wasm32"));
        if !has_file_system_available {
            return;
        }

        if let Some(Ok(())) = receiver.receive().await {
            let padded_buffer = buffer_slice.get_mapped_range();
            let mut file = File::create(output_path).unwrap();

            // let mut png_encoder = png::Encoder::new(
            //     File::create(png_output_path).unwrap(),
            //     frame_content_copy_dest.dimensions.width as u32,
            //     frame_content_copy_dest.dimensions.height as u32,
            // );
            // png_encoder.set_depth(png::BitDepth::Eight);
            // png_encoder.set_color(png::ColorType::Rgba);
            // let mut png_writer = png_encoder
            //     .write_header()
            //     .unwrap()
            //     .into_stream_writer_with_size(frame_content_copy_dest.dimensions.unpadded_bytes_per_row)
            //     .unwrap();

            // from the padded_buffer we write just the unpadded bytes into the image
            file.write_all(&padded_buffer).unwrap();

            // With the current interface, we have to make sure all mapped views are
            // dropped before we unmap the buffer.
            drop(padded_buffer);

            self.buffer.unmap();
        }
    }
}
