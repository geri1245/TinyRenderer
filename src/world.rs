use std::{collections::HashMap, f32::consts::FRAC_PI_2, rc::Rc};

use glam::{Quat, Vec3};

use crate::{
    instance::SceneComponent,
    lights::Light,
    material::Material,
    model::{InstanceData, PbrParameters, RenderableMesh, RenderableObject},
    primitive_shapes,
    resource_loader::ResourceLoader,
};

#[derive(Eq, PartialEq, Hash)]
pub enum PrimitiveMeshes {
    Cube,
}

pub struct ObjectHandle {
    id: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DirtyState {
    /// No changes, nothing needs to be updated
    NothingChanged,
    /// In this case we might have to regenerate the buffers, as the number of items might have changed
    ItemsChanged,
    /// In this case it's enough to copy the new data to the existing buffers,
    /// as the number/structure of items remains the same
    ItemPropertiesChanged,
}

pub struct World {
    meshes: Vec<RenderableObject>,
    lights: Vec<Light>,
    lights_dirty_state: DirtyState,

    debug_meshes: HashMap<PrimitiveMeshes, Rc<RenderableMesh>>,
    pending_lights: Vec<Light>,
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

        let mut example_instances = Vec::with_capacity(100);
        for i in 0..11 {
            for j in 0..11 {
                example_instances.push(SceneComponent {
                    position: Vec3::new(i as f32 * 5.0 - 25.0, j as f32 * 5.0 - 25.0, 0.0),
                    scale: Vec3::splat(1.0),
                    rotation: Quat::from_axis_angle(Vec3::ZERO, 0.0),
                });
            }
        }

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
        let cube_instances = InstanceData::new(cube_instances, device);
        let cube_instances2 = InstanceData::new(example_instances, device);

        let square = Rc::new(primitive_shapes::square(device));
        let square_instances = InstanceData::new(square_instances, device);

        let meshes = vec![
            RenderableObject {
                material: resource_loader.get_default_material(),
                mesh: square,
                material_id: None,
                instance_data: square_instances,
            },
            RenderableObject {
                material: resource_loader.get_default_material(),
                instance_data: cube_instances,
                material_id: Some(material_loading_id),
                mesh: cube.clone(),
            },
            RenderableObject {
                material: Rc::new(Material::from_flat_parameters(
                    device,
                    &PbrParameters::new([0.2, 0.6, 0.8], 0.7, 0.0),
                )),
                instance_data: cube_instances2,
                material_id: None,
                mesh: cube.clone(),
            },
        ];

        let mut debug_objects = HashMap::new();
        debug_objects.insert(PrimitiveMeshes::Cube, cube);

        World {
            meshes,
            lights: vec![],
            lights_dirty_state: DirtyState::ItemsChanged,
            debug_meshes: debug_objects,
            pending_lights: vec![],
        }
    }

    pub fn add_light(&mut self, light: Light) -> usize {
        self.lights.push(light);

        if let Light::Point(_) = light {
            self.pending_lights.push(light);
        }

        self.lights.len()
    }

    pub fn get_light(&mut self, handle: &ObjectHandle) -> Option<&mut Light> {
        if handle.id < self.lights.len() {
            self.lights_dirty_state = DirtyState::ItemPropertiesChanged;
            Some(&mut self.lights[handle.id])
        } else {
            None
        }
    }

    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resource_loader: &mut ResourceLoader,
    ) {
        for light in self.pending_lights.drain(..) {
            if let Light::Point(point_light) = light {
                let instance_data = InstanceData::new(vec![point_light.transform], device);
                self.meshes.push(RenderableObject {
                    material: Rc::new(Material::from_flat_parameters(
                        device,
                        &PbrParameters::fully_rough(point_light.color.into()),
                    )),
                    mesh: self
                        .debug_meshes
                        .get(&PrimitiveMeshes::Cube)
                        .unwrap()
                        .clone(),
                    material_id: None,
                    instance_data,
                });
            }
        }

        let materials_loaded = resource_loader.poll_loaded_textures(device, queue);

        for (id, material) in materials_loaded {
            for mesh in &mut self.meshes {
                if let Some(pending_material_id) = mesh.material_id {
                    if pending_material_id == id {
                        mesh.material = material.clone();
                    }
                }
            }
        }
    }

    // This will be raypicing in the future
    pub fn pick(&self) -> ObjectHandle {
        return ObjectHandle { id: 0 };
    }

    pub fn get_lights_dirty_state(&self) -> DirtyState {
        self.lights_dirty_state
    }

    pub fn set_lights_udpated(&mut self) {
        self.lights_dirty_state = DirtyState::NothingChanged;
    }

    pub fn get_lights(&self) -> &Vec<Light> {
        &self.lights
    }

    pub fn get_meshes(&self) -> &Vec<RenderableObject> {
        &self.meshes
    }
}
