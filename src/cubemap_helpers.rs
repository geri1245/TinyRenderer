use std::f32::consts;

use glam::{Mat4, Vec3};
use wgpu::{Device, Texture};

use crate::{
    bind_group_layout_descriptors,
    buffer::{
        create_bind_group_from_buffer_entire_binding_init, BufferInitBindGroupCreationOptions,
    },
};

pub struct RenderingIntoCubemapResources {
    pub cube_face_viewproj_bind_group: wgpu::BindGroup,
    pub cube_target_texture_view: wgpu::TextureView,
}

pub fn create_cubemap_face_rendering_parameters(
    device: &Device,
    cube_target_texture: &Texture,
) -> Vec<RenderingIntoCubemapResources> {
    let proj = glam::Mat4::perspective_rh(consts::FRAC_PI_2, 1.0, 0.1, 2.0);

    const DIFF_AND_UP_VECTORS: [(Vec3, Vec3); 6] = [
        (Vec3::X, Vec3::Y),
        (Vec3::NEG_X, Vec3::Y),
        (Vec3::Y, Vec3::NEG_Z),
        (Vec3::NEG_Y, Vec3::Z),
        (Vec3::Z, Vec3::Y),
        (Vec3::NEG_Z, Vec3::Y),
    ];

    DIFF_AND_UP_VECTORS
        .iter()
        .enumerate()
        .map(|(index, &(diff, up))| {
            let view = Mat4::look_at_rh(Vec3::ZERO, diff, up);
            let matrix_data = (proj * view).to_cols_array();

            let (_buffer, bind_group) = create_bind_group_from_buffer_entire_binding_init(
                device,
                &BufferInitBindGroupCreationOptions {
                    bind_group_layout_descriptor:
                        &bind_group_layout_descriptors::BUFFER_VISIBLE_EVERYWHERE,
                    usages: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    label: "Equirectangular projection viewprojs".into(),
                },
                bytemuck::cast_slice(&matrix_data),
            );

            let view = cube_target_texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("HDR cubemap target view"),
                base_array_layer: index as u32,
                array_layer_count: Some(1),
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_mip_level: 0,
                mip_level_count: None,
                ..Default::default()
            });

            RenderingIntoCubemapResources {
                cube_target_texture_view: view,
                cube_face_viewproj_bind_group: bind_group,
            }
        })
        .collect()
}
