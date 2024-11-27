use std::num::NonZeroU64;

use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    BindGroup, BindingResource, Buffer, BufferDescriptor,
};

use crate::bind_group_layout_descriptors;

pub struct BufferBindGroupCreationOptions<'a> {
    pub bind_group_layout_descriptor: &'a wgpu::BindGroupLayoutDescriptor<'a>,
    pub num_of_items: u64,
    pub usages: wgpu::BufferUsages,
    pub label: &'a str,
    /// If None, then uses the entire buffer, else the given size
    pub binding_size: Option<u64>,
}

pub struct GpuBufferCreationOptions<'a> {
    pub bind_group_layout_descriptor: &'a wgpu::BindGroupLayoutDescriptor<'a>,
    pub usages: wgpu::BufferUsages,
    pub label: &'a str,
}

impl<'a> Default for GpuBufferCreationOptions<'a> {
    fn default() -> Self {
        Self {
            bind_group_layout_descriptor: &bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE,
            usages: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            label: "Anonymous buffer",
        }
    }
}

pub fn create_bind_group_from_buffer_entire_binding_fixed_size(
    device: &wgpu::Device,
    options: &BufferBindGroupCreationOptions,
    size: u64,
) -> (Buffer, BindGroup) {
    let buffer_label = options.label.to_string() + " buffer";

    let buffer = device.create_buffer(&BufferDescriptor {
        label: Some(&buffer_label),
        size: size * options.num_of_items,
        usage: options.usages,
        mapped_at_creation: false,
    });

    let bind_group = create_bind_group(
        device,
        &buffer,
        &options.label,
        &options.bind_group_layout_descriptor,
        options.binding_size,
    );

    (buffer, bind_group)
}

pub fn create_bind_group_from_buffer_entire_binding<Type>(
    device: &wgpu::Device,
    options: &BufferBindGroupCreationOptions,
) -> (Buffer, BindGroup) {
    let type_size = core::mem::size_of::<Type>() as wgpu::BufferAddress;

    create_bind_group_from_buffer_entire_binding_fixed_size(device, options, type_size)
}

pub fn create_bind_group_from_buffer_entire_binding_init(
    device: &wgpu::Device,
    options: &GpuBufferCreationOptions,
    contents: &[u8],
) -> (Buffer, BindGroup) {
    let buffer_label = options.label.to_string() + " buffer";

    let buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some(&buffer_label),
        usage: options.usages,
        contents,
    });

    let bind_group = create_bind_group(
        device,
        &buffer,
        options.label,
        options.bind_group_layout_descriptor,
        None,
    );

    (buffer, bind_group)
}

fn create_bind_group(
    device: &wgpu::Device,
    buffer: &Buffer,
    label: &str,
    bind_group_layout_descriptor: &wgpu::BindGroupLayoutDescriptor,
    binding_size: Option<u64>,
) -> BindGroup {
    let bind_group_label = label.to_string() + " bind group";

    device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &device.create_bind_group_layout(bind_group_layout_descriptor),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: if binding_size.is_some() {
                BindingResource::Buffer(wgpu::BufferBinding {
                    buffer,
                    offset: 0,
                    size: NonZeroU64::new(binding_size.unwrap()),
                })
            } else {
                buffer.as_entire_binding()
            },
        }],
        label: Some(&bind_group_label),
    })
}
