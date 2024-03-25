use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    BindGroup, Buffer, BufferDescriptor,
};

pub struct BufferBindGroupCreationOptions<'a> {
    pub bind_group_layout_descriptor: &'a wgpu::BindGroupLayoutDescriptor<'a>,
    pub num_of_items: u64,
    pub usages: wgpu::BufferUsages,
    pub label: &'a str,
}

pub struct BufferInitBindGroupCreationOptions<'a> {
    pub bind_group_layout_descriptor: &'a wgpu::BindGroupLayoutDescriptor<'a>,
    pub usages: wgpu::BufferUsages,
    pub label: &'a str,
}

pub fn create_bind_group_from_buffer_entire_binding<Type>(
    device: &wgpu::Device,
    options: &BufferBindGroupCreationOptions,
) -> (Buffer, BindGroup) {
    let type_size = core::mem::size_of::<Type>() as wgpu::BufferAddress;
    let buffer_label = options.label.to_string() + " buffer";

    let buffer = device.create_buffer(&BufferDescriptor {
        label: Some(&buffer_label),
        size: type_size * options.num_of_items,
        usage: options.usages,
        mapped_at_creation: false,
    });

    let bind_group = create_bind_group(
        device,
        &buffer,
        &options.label,
        &options.bind_group_layout_descriptor,
    );

    (buffer, bind_group)
}

pub fn create_bind_group_from_buffer_entire_binding_init(
    device: &wgpu::Device,
    options: &BufferInitBindGroupCreationOptions,
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
    );

    (buffer, bind_group)
}

fn create_bind_group(
    device: &wgpu::Device,
    buffer: &Buffer,
    label: &str,
    bind_group_layout_descriptor: &wgpu::BindGroupLayoutDescriptor,
) -> BindGroup {
    let bind_group_label = label.to_string() + " bind group";

    device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &device.create_bind_group_layout(bind_group_layout_descriptor),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
        label: Some(&bind_group_label),
    })
}
