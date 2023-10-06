use std::{f32::consts, rc::Rc};

use glam::{Quat, Vec3};
use wgpu::util::DeviceExt;

use crate::{
    bind_group_layout_descriptors,
    instance::{self, Instance},
    model::{Material, Mesh, Model},
    primitive_shapes,
    renderer::Renderer,
    resources, texture,
};

const NUM_INSTANCES_PER_ROW: u32 = 10;

pub struct World {
    pub obj_model: Model,
    pub instances: Vec<Instance>,
    pub instance_buffer: wgpu::Buffer,
    pub square: Mesh,
    pub square_instance_buffer: wgpu::Buffer,
}

impl World {
    pub async fn new(renderer: &Renderer) -> Self {
        let tree_texture_raw = include_bytes!("../assets/happy-tree.png");

        let tree_texture = texture::Texture::from_bytes(
            &renderer.device,
            &renderer.queue,
            tree_texture_raw,
            "treeTexture",
        )
        .unwrap();
        const SPACE_BETWEEN: f32 = 4.0;
        const SCALE: Vec3 = Vec3::new(1.0, 1.0, 1.0);
        let instances = (0..NUM_INSTANCES_PER_ROW)
            .flat_map(|z| {
                (0..NUM_INSTANCES_PER_ROW).map(move |x| {
                    let x = SPACE_BETWEEN * (x as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);
                    let z = SPACE_BETWEEN * (z as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);

                    let position = Vec3 { x, y: 0.0, z };

                    let rotation = if position == Vec3::ZERO {
                        Quat::from_axis_angle(Vec3::Z, 0.0)
                    } else {
                        Quat::from_axis_angle(position.normalize(), consts::FRAC_PI_4)
                    };

                    Instance {
                        position,
                        rotation,
                        scale: SCALE,
                    }
                })
            })
            .collect::<Vec<_>>();

        let instance_data = instances
            .iter()
            .map(instance::Instance::to_raw)
            .collect::<Vec<_>>();
        let instance_buffer =
            renderer
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Instance Buffer"),
                    contents: bytemuck::cast_slice(&instance_data),
                    usage: wgpu::BufferUsages::VERTEX,
                });

        let obj_model = resources::load_model("cube.obj", &renderer.device, &renderer.queue)
            .await
            .unwrap();

        let texture_bind_group = renderer
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &renderer
                    .device
                    .create_bind_group_layout(&bind_group_layout_descriptors::DIFFUSE_TEXTURE),
                entries: &[
                    tree_texture.get_texture_bind_group_entry(0),
                    tree_texture.get_sampler_bind_group_entry(1),
                ],
                label: Some("diffuse_bind_group"),
            });

        let square_material = Some(Rc::new(Material {
            name: "Tree texture material".into(),
            diffuse_texture: tree_texture,
            bind_group: texture_bind_group,
        }));

        let square = primitive_shapes::square(&renderer.device, square_material);

        let square_instances = vec![Instance {
            position: Vec3::new(0.0, -10.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: 100.0_f32
                * Vec3 {
                    x: 1.0_f32,
                    y: 1.0,
                    z: 1.0,
                },
        }];

        let square_instance_raw = square_instances
            .iter()
            .map(|instance| instance.to_raw())
            .collect::<Vec<_>>();
        let square_instance_buffer =
            renderer
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Square Instance Buffer"),
                    contents: bytemuck::cast_slice(&square_instance_raw),
                    usage: wgpu::BufferUsages::VERTEX,
                });

        World {
            obj_model,
            instances,
            instance_buffer,
            square,
            square_instance_buffer,
        }
    }
}
