use wgpu::{BindGroup, Buffer, BufferUsages, Device, Queue};

use crate::{
    bind_group_layout_descriptors,
    buffer::{
        create_bind_group_from_buffer_entire_binding,
        create_bind_group_from_buffer_entire_binding_fixed_size,
        create_bind_group_from_buffer_entire_binding_init, BufferBindGroupCreationOptions,
        GpuBufferCreationOptions,
    },
    light_render_data::CUBE_FACE_COUNT,
    lights::{DirectionalLightData, LightRaw, PointLightData},
    renderer::Renderer,
};

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct LightCountRaw {
    point: u32,
    directional: u32,
}

pub struct LightCount {
    pub point: usize,
    pub directional: usize,
}

impl LightCount {
    fn to_raw(&self) -> LightCountRaw {
        LightCountRaw {
            point: self.point as u32,
            directional: self.directional as u32,
        }
    }
}

pub struct LightRenderData {
    /// Contains the actual data about the lights, eg. position, direction
    light_uniform_buffer: Buffer,
    pub light_bind_group: BindGroup,

    /// Contains parameters about the lights in general, eg. count of point lights
    light_parameters_uniform_buffer: Buffer,
    pub light_parameters_bind_group: BindGroup,

    /// This is used for creating the shadow maps - contains the viewproj matrices of the lights
    /// Here we are using dynamic offsets into the buffer, so the data needs to be aligned properly
    light_viewproj_only_uniform_buffer: Buffer,
    pub light_bind_group_viewproj_only: BindGroup,

    // This is the alignment for the uniform buffer with dynamic offsets - the viewproj buffers use dynamic offsets
    pub uniform_buffer_alignment: u64,
}

impl LightRenderData {
    pub fn new(device: &Device, uniform_buffer_alignment: u64) -> Self {
        // Fake some default numbers, so we can create the initial assets
        let light_count = LightCount {
            directional: 1,
            point: 1,
        };

        let (light_parameters_uniform_buffer, light_parameters_bind_group) =
            create_bind_group_from_buffer_entire_binding_init(
                device,
                &GpuBufferCreationOptions {
                    bind_group_layout_descriptor:
                        &bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE,
                    usages: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                    label: "Light parameters".into(),
                },
                bytemuck::cast_slice(&[light_count.to_raw()]),
            );

        let (light_uniform_buffer, light_bind_group) =
            Self::create_light_parameters_buffer_and_bgroup(device, &light_count);

        let (light_viewproj_only_uniform_buffer, light_bind_group_viewproj_only) =
            Self::create_light_viewproj_buffer_and_bgroup(
                device,
                &light_count,
                uniform_buffer_alignment,
            );

        Self {
            light_uniform_buffer,
            light_bind_group,
            light_parameters_uniform_buffer,
            light_parameters_bind_group,
            light_bind_group_viewproj_only,
            light_viewproj_only_uniform_buffer,
            uniform_buffer_alignment,
        }
    }

    fn create_light_parameters_buffer_and_bgroup(
        device: &Device,
        light_count: &LightCount,
    ) -> (Buffer, BindGroup) {
        // Actual data of the lights is contained here (position, color, etc.)
        // The data is copied in the update shadow assets function

        create_bind_group_from_buffer_entire_binding::<LightRaw>(
            device,
            &BufferBindGroupCreationOptions {
                bind_group_layout_descriptor: &bind_group_layout_descriptors::LIGHT,
                num_of_items: light_count.point + light_count.directional,
                usages: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                label: "Light".into(),
                binding_size: None,
            },
        )
    }

    fn create_light_viewproj_buffer_and_bgroup(
        device: &Device,
        light_count: &LightCount,
        uniform_alignment: u64,
    ) -> (Buffer, BindGroup) {
        create_bind_group_from_buffer_entire_binding_fixed_size(
            device,
            &BufferBindGroupCreationOptions {
                bind_group_layout_descriptor:
                    &bind_group_layout_descriptors::BUFFER_WITH_DYNAMIC_OFFSET,
                num_of_items: CUBE_FACE_COUNT * light_count.point + light_count.directional,
                usages: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                label: "Light projection matrix only".into(),
                binding_size: Some(uniform_alignment),
            },
            uniform_alignment,
        )
    }

    pub fn update(
        &mut self,
        renderer: &Renderer,
        light_count: &LightCount,
        point_lights: Vec<&PointLightData>,
        directional_lights: Vec<&DirectionalLightData>,
    ) {
        let (light_uniform_buffer, light_bind_group) =
            Self::create_light_parameters_buffer_and_bgroup(&renderer.device, &light_count);

        let (light_viewproj_only_uniform_buffer, light_bind_group_viewproj_only) =
            Self::create_light_viewproj_buffer_and_bgroup(
                &renderer.device,
                &light_count,
                self.uniform_buffer_alignment,
            );

        self.light_bind_group = light_bind_group;
        self.light_uniform_buffer = light_uniform_buffer;
        self.light_bind_group_viewproj_only = light_bind_group_viewproj_only;
        self.light_viewproj_only_uniform_buffer = light_viewproj_only_uniform_buffer;

        self.update_gpu_data(
            &renderer.queue,
            light_count,
            point_lights,
            directional_lights,
        );
    }

    fn update_gpu_data(
        &self,
        queue: &Queue,
        light_count: &LightCount,
        point_lights: Vec<&PointLightData>,
        directional_lights: Vec<&DirectionalLightData>,
    ) {
        let mut light_raws = point_lights
            .iter()
            .map(|light| light.to_raw())
            .collect::<Vec<_>>();

        for directional_light in &directional_lights {
            light_raws.push(directional_light.to_raw())
        }

        queue.write_buffer(
            &self.light_uniform_buffer,
            0,
            bytemuck::cast_slice(&light_raws),
        );

        queue.write_buffer(
            &self.light_parameters_uniform_buffer,
            0,
            bytemuck::cast_slice(&[light_count.to_raw()]),
        );

        for (light_index, point_light) in point_lights.iter().enumerate() {
            let raw_viewprojs = point_light.get_viewprojs_raw();
            for (face_index, raw_data) in raw_viewprojs.iter().enumerate() {
                queue.write_buffer(
                    &self.light_viewproj_only_uniform_buffer,
                    (light_index * CUBE_FACE_COUNT + face_index) as u64
                        * self.uniform_buffer_alignment,
                    bytemuck::cast_slice(&[*raw_data]),
                );
            }
        }

        let base_offset_after_point_lights =
            CUBE_FACE_COUNT * point_lights.len() * self.uniform_buffer_alignment as usize;

        for (directional_light_index, directional_light) in directional_lights.iter().enumerate() {
            queue.write_buffer(
                &self.light_viewproj_only_uniform_buffer,
                base_offset_after_point_lights as u64
                    + self.uniform_buffer_alignment * (directional_light_index as u64),
                bytemuck::cast_slice(&[directional_light.get_viewprojs_raw()]),
            );
        }
    }
}
