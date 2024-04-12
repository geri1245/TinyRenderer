use std::{collections::HashMap, f32::consts::FRAC_PI_2, rc::Rc};

use glam::{Quat, Vec3};

use crate::{
    instance::SceneComponent,
    model::{InstancedRenderableMesh, InstancedTexturedRenderableMesh, RenderableMesh},
    primitive_shapes,
    resource_loader::ResourceLoader,
    world_renderer::MeshType,
};

#[derive(Eq, PartialEq, Hash)]
pub enum PrimitiveMeshes {
    Cube,
}

pub struct World {
    pub meshes: Vec<MeshType>,
    debug_objects: HashMap<PrimitiveMeshes, Rc<RenderableMesh>>,
    pending_meshes: Vec<(PrimitiveMeshes, SceneComponent)>,
}

impl World {
    pub async fn new(device: &wgpu::Device, resource_loader: &mut ResourceLoader) -> Self {
        let cube_instances = vec![
            SceneComponent {
                position: Vec3::new(10.0, 10.0, 0.0),
                scale: Vec3::splat(3.0),
                rotation: Quat::from_axis_angle(Vec3::ZERO, 0.0),
            },
            SceneComponent {
                position: Vec3::new(-20.0, 10.0, 0.0),
                scale: Vec3::splat(2.0),
                rotation: Quat::from_axis_angle(Vec3::ZERO, 0.0),
            },
            SceneComponent {
                position: Vec3::new(0.0, 10.0, 30.0),
                scale: Vec3::splat(2.0),
                rotation: Quat::from_axis_angle(Vec3::ZERO, 0.0),
            },
            SceneComponent {
                position: Vec3::new(30.0, 20.0, 10.0),
                scale: Vec3::splat(2.0),
                rotation: Quat::from_axis_angle(Vec3::ZERO, 0.0),
            },
            SceneComponent {
                position: Vec3::new(25.0, 10.0, 20.0),
                scale: Vec3::splat(1.5),
                rotation: Quat::from_axis_angle(Vec3::ZERO, 0.0),
            },
        ];

        let square_instances = vec![
            // Bottom
            SceneComponent {
                position: Vec3::new(0.0, -10.0, 0.0),
                rotation: Quat::IDENTITY,
                scale: 100.0_f32
                    * Vec3 {
                        x: 1.0_f32,
                        y: 1.0,
                        z: 1.0,
                    },
            },
            // Top
            // SceneComponent {
            //     position: Vec3::new(0.0, 40.0, 0.0),
            //     rotation: Quat::from_axis_angle(Vec3::X, PI),
            //     scale: 100.0_f32
            //         * Vec3 {
            //             x: 1.0_f32,
            //             y: 1.0,
            //             z: 1.0,
            //         },
            // },
            // +X
            // SceneComponent {
            //     position: Vec3::new(-40.0, 0.0, 0.0),
            //     rotation: Quat::from_axis_angle(Vec3::Z, -FRAC_PI_2),
            //     scale: 100.0_f32
            //         * Vec3 {
            //             x: 1.0_f32,
            //             y: 1.0,
            //             z: 1.0,
            //         },
            // },
            // -X
            // SceneComponent {
            //     position: Vec3::new(40.0, 0.0, 0.0),
            //     rotation: Quat::from_axis_angle(Vec3::Z, FRAC_PI_2),
            //     scale: 100.0_f32
            //         * Vec3 {
            //             x: 1.0_f32,
            //             y: 1.0,
            //             z: 1.0,
            //         },
            // },
            // -Z
            SceneComponent {
                position: Vec3::new(0.0, 0.0, -40.0),
                rotation: Quat::from_axis_angle(Vec3::X, FRAC_PI_2),
                scale: 100.0_f32
                    * Vec3 {
                        x: 1.0_f32,
                        y: 1.0,
                        z: 1.0,
                    },
            },
            // // Z
            // SceneComponent {
            //     position: Vec3::new(0.0, 0.0, 40.0),
            //     rotation: Quat::from_axis_angle(Vec3::X, -FRAC_PI_2),
            //     scale: 100.0_f32
            //         * Vec3 {
            //             x: 1.0_f32,
            //             y: 1.0,
            //             z: 1.0,
            //         },
            // },
        ];

        let (cube_model, material_loading_id) = resource_loader
            .load_asset_file("cube", device)
            .await
            .unwrap();
        let cube = Rc::new(cube_model);
        let cube_instanced = InstancedRenderableMesh::new(cube_instances, device, cube.clone());

        let square = Rc::new(primitive_shapes::square(device));
        let square_instanced = InstancedRenderableMesh::new(square_instances, device, square);

        let meshes = vec![
            MeshType::TexturedMesh(InstancedTexturedRenderableMesh {
                material: resource_loader.get_default_material(),
                mesh: square_instanced,
                material_id: None,
            }),
            MeshType::TexturedMesh(InstancedTexturedRenderableMesh {
                material: resource_loader.get_default_material(),
                mesh: cube_instanced,
                material_id: Some(material_loading_id),
            }),
        ];

        let mut debug_objects = HashMap::new();
        debug_objects.insert(PrimitiveMeshes::Cube, cube);

        World {
            meshes,
            debug_objects,
            pending_meshes: vec![],
        }
    }

    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resource_loader: &mut ResourceLoader,
    ) {
        for (object_type, scene_component) in self.pending_meshes.drain(..) {
            let renderable_mesh = self.debug_objects.get(&object_type).unwrap();
            self.meshes
                .push(MeshType::DebugMesh(InstancedRenderableMesh::new(
                    vec![scene_component],
                    device,
                    renderable_mesh.clone(),
                )));
        }
        let materials_loaded = resource_loader.poll_loaded_textures(device, queue);

        for (id, material) in materials_loaded {
            for mesh in &mut self.meshes {
                if let MeshType::TexturedMesh(mesh) = mesh {
                    if let Some(pending_material_id) = mesh.material_id {
                        if pending_material_id == id {
                            mesh.material = material.clone();
                        }
                    }
                }
            }
        }
    }

    pub fn add_debug_object(
        &mut self,
        debug_mesh: PrimitiveMeshes,
        scene_component: &SceneComponent,
    ) {
        self.pending_meshes
            .push((debug_mesh, scene_component.clone()))
    }
}
