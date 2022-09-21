use cgmath::Rotation3;
use cgmath::Vector3;
use wgpu::util::DeviceExt;

pub struct PointLight {
    pub position: [f32; 3],
    pub color: [f32; 3],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PointLightRaw {
    pub position: [f32; 3],
    // Due to uniforms requiring 16 byte (4 float) spacing, we need to use a padding field here
    _padding: u32,
    pub color: [f32; 3],
    // Due to uniforms requiring 16 byte (4 float) spacing, we need to use a padding field here
    _padding2: u32,
}

pub struct LightController {
    pub light: PointLight,
    pub uniform_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}

impl LightController {
    pub fn new(
        light: PointLight,
        render_device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> LightController {
        let buffer = render_device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light VB"),
            contents: bytemuck::cast_slice(&[Self::to_raw(&light)]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = render_device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
            label: Some("Light bind group"),
        });

        Self {
            light,
            uniform_buffer: buffer,
            bind_group,
        }
    }

    pub fn update(&mut self, delta_time: std::time::Duration, render_queue: &wgpu::Queue) {
        let old_light_position = Vector3::from(self.light.position);
        self.light.position = (cgmath::Quaternion::from_axis_angle(
            (0.0, 1.0, 0.0).into(),
            cgmath::Deg(60.0 * delta_time.as_secs_f32()),
        ) * old_light_position)
            .into();

        render_queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[Self::to_raw(&self.light)]),
        );
    }

    fn to_raw(light: &PointLight) -> PointLightRaw {
        PointLightRaw {
            position: light.position,
            _padding: 0,
            color: light.color,
            _padding2: 0,
        }
    }
}
