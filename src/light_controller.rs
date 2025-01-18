use std::collections::HashMap;

use wgpu::util::align_to;
use wgpu::{BindGroup, BufferAddress, CommandEncoder, Device};

use crate::light_render_data::{GeneralLightRenderData, CUBE_FACE_COUNT};
use crate::light_rendering_gpu_data::{LightCount, LightRenderData};
use crate::lights::{DirectionalLight, DirectionalLightData};
use crate::renderer::Renderer;
use crate::world;
use crate::{
    lights::{Light, LightRawSmall, PointLightData, PointLightRenderData},
    model::Renderable,
    pipelines::{ShaderCompilationSuccess, ShadowRP},
    world::World,
};

struct ShadowAssets {
    point_light_render_data: GeneralLightRenderData<6>,
    directional_light_render_data: GeneralLightRenderData<1>,
}

pub struct LightController {
    shadow_rp: ShadowRP,
    shadow_assets: ShadowAssets,

    point_lights: HashMap<u32, PointLightData>,
    directional_lights: HashMap<u32, DirectionalLightData>,
    light_render_data: LightRenderData,
}

impl LightController {
    pub fn new(device: &Device) -> LightController {
        // Make the `uniform_alignment` >= sizeof`LightRawSmall` and aligned to `min_uniform_buffer_offset_alignment`, as that is a requirement if we want to use dynamic offsets
        let matrix_size4x4 = core::mem::size_of::<LightRawSmall>() as u64;
        let uniform_buffer_alignment = {
            let alignment = device.limits().min_uniform_buffer_offset_alignment as BufferAddress;
            align_to(matrix_size4x4, alignment)
        };

        let shadow_rp = crate::pipelines::ShadowRP::new(&device).unwrap();

        let shadow_assets = Self::create_shadow_assets(device);
        let light_render_data = LightRenderData::new(device, uniform_buffer_alignment);

        Self {
            shadow_rp,
            shadow_assets,
            point_lights: HashMap::new(),
            directional_lights: HashMap::new(),
            light_render_data,
        }
    }

    fn get_light_count(&self) -> LightCount {
        LightCount {
            directional: self.directional_lights.len(),
            point: self.point_lights.len(),
        }
    }

    pub fn get_directional_lights_depth_texture_bgroup(&self) -> &BindGroup {
        &self
            .shadow_assets
            .directional_light_render_data
            .get_bind_group()
    }

    pub fn get_point_lights_depth_texture_bgroup(&self) -> &BindGroup {
        &self.shadow_assets.point_light_render_data.get_bind_group()
    }

    pub fn get_light_bind_group(&self) -> &BindGroup {
        &self.light_render_data.light_bind_group
    }

    pub fn get_light_parameters_bind_group(&self) -> &BindGroup {
        &self.light_render_data.light_parameters_bind_group
    }

    fn update_shadow_assets(&mut self, renderer: &Renderer) {
        let light_count = self.get_light_count();

        self.light_render_data.update(
            renderer,
            &light_count,
            self.point_lights.values().collect::<Vec<_>>(),
            self.directional_lights.values().collect::<Vec<_>>(),
        );
    }

    fn add_point_light(&mut self, device: &Device, id: u32, light: PointLightRenderData) {
        let depth_view_index = self
            .shadow_assets
            .point_light_render_data
            .make_resources_for_new_light(device);

        let point_light_data = PointLightData::new(light, depth_view_index);
        self.point_lights.insert(id, point_light_data);
    }

    fn add_directional_light(&mut self, device: &Device, id: u32, light: DirectionalLight) {
        let depth_view_index = self
            .shadow_assets
            .directional_light_render_data
            .make_resources_for_new_light(device);

        let directional_light_data = DirectionalLightData::new(&light, depth_view_index);
        self.directional_lights.insert(id, directional_light_data);
    }

    fn add_light(&mut self, renderer: &Renderer, id: u32, light: Light) {
        match light {
            Light::Point(point_light) => {
                self.add_point_light(&renderer.device, id, point_light);
            }
            Light::Directional(directional_light) => {
                self.add_directional_light(&renderer.device, id, directional_light);
            }
        }

        self.update_shadow_assets(renderer);
    }

    fn update_light(&mut self, renderer: &Renderer, id: &u32, light: &Light) {
        match light {
            Light::Point(point_light) => {
                if let Some(point_light_render_data) = self.point_lights.get_mut(&id) {
                    point_light_render_data.light = point_light.clone();
                }
            }
            Light::Directional(directional_light) => todo!(),
        }

        self.light_render_data.update_light_gpu_data(
            &renderer.queue,
            &self.point_lights.values().collect(),
            &self.directional_lights.values().collect(),
        );
    }

    pub fn remove_light(&mut self, device: &Device, id: u32) {
        if let Some(light_data) = self.point_lights.remove(&id) {}
    }

    fn create_shadow_assets(device: &Device) -> ShadowAssets {
        let point_light_render_data = GeneralLightRenderData::new(device);
        let directional_light_render_data = GeneralLightRenderData::new(device);

        ShadowAssets {
            point_light_render_data,
            directional_light_render_data,
        }
    }

    fn get_light(world: &World, id: &u32) -> Option<Light> {
        if let Some(world_object) = world.get_world_object(id) {
            Light::from_world_object(world_object)
        } else if let Some(omnipresent_object) = world.get_omnipresent_object(id) {
            Light::from_omnipresent_object(omnipresent_object)
        } else {
            None
        }
    }

    pub fn update(
        &mut self,
        _delta_time: std::time::Duration,
        renderer: &Renderer,
        world: &mut World,
    ) {
        for modification in &world.dirty_objects {
            if let Some(light) = Self::get_light(world, &modification.id) {
                match modification.modification_type {
                    world::ModificationType::Added => {
                        self.add_light(renderer, modification.id, light);
                    }
                    world::ModificationType::Removed => todo!(),
                    world::ModificationType::Modified => {
                        self.update_light(renderer, &modification.id, &light);
                    }
                }
            }
        }
    }

    pub fn render_shadows<'a, T>(&self, encoder: &mut CommandEncoder, renderables: T)
    where
        T: Clone,
        T: Iterator<Item = &'a Renderable>,
    {
        encoder.push_debug_group("Shadow rendering");

        {
            encoder.push_debug_group("Point shadows");

            for (light_index, light) in self.point_lights.values().enumerate() {
                for (face_index, depth_target) in self
                    .shadow_assets
                    .point_light_render_data
                    .get_depth_target_view(light.depth_texture_index)
                    .iter()
                    .enumerate()
                {
                    self.shadow_rp.render(
                        encoder,
                        renderables.clone(),
                        &self.light_render_data.light_bind_group_viewproj_only,
                        depth_target,
                        (CUBE_FACE_COUNT * light_index + face_index) as u32
                            * self.light_render_data.uniform_buffer_alignment as u32,
                    );
                }
            }
            encoder.pop_debug_group();
        }

        let base_offset_after_point_lights = CUBE_FACE_COUNT
            * self.point_lights.len()
            * self.light_render_data.uniform_buffer_alignment as usize;

        {
            encoder.push_debug_group("Directional shadows");

            for (light_index, light) in self.directional_lights.values().enumerate() {
                let target_view = self
                    .shadow_assets
                    .directional_light_render_data
                    .get_depth_target_view(light.depth_texture_index);
                self.shadow_rp.render(
                    encoder,
                    renderables.clone(),
                    &self.light_render_data.light_bind_group_viewproj_only,
                    &target_view[0],
                    (base_offset_after_point_lights
                        + light_index * self.light_render_data.uniform_buffer_alignment as usize)
                        as u32,
                );
            }

            encoder.pop_debug_group();
        }

        encoder.pop_debug_group();
    }

    pub fn try_recompile_shaders(
        &mut self,
        device: &Device,
    ) -> anyhow::Result<ShaderCompilationSuccess> {
        self.shadow_rp.try_recompile_shader(device)
    }
}
