use std::collections::HashMap;

use wgpu::util::align_to;
use wgpu::{BindGroup, BufferAddress, CommandEncoder, Device};

use crate::light_render_data::{GeneralLightRenderData, CUBE_FACE_COUNT};
use crate::light_rendering_gpu_data::{LightCount, LightRenderData};
use crate::lights::{DirectionalLight, DirectionalLightData};
use crate::renderer::Renderer;
use crate::world::{self, ObjectModificationType};
use crate::{
    lights::{Light, LightRawSmall, PointLight, PointLightData},
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

    fn add_point_light(&mut self, device: &Device, id: u32, light: PointLight) {
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

    pub fn add_light(&mut self, renderer: &Renderer, id: u32, light: Light) {
        match light {
            Light::Point(point_light) => self.add_point_light(&renderer.device, id, point_light),
            Light::Directional(directional_light) => {
                self.add_directional_light(&renderer.device, id, directional_light)
            }
        }

        self.update_shadow_assets(renderer);
    }

    pub fn remove_light(&mut self, device: &Device, id: u32) {
        if let Some(lightData) = self.point_lights.remove(&id) {}
    }

    fn create_shadow_assets(device: &Device) -> ShadowAssets {
        let point_light_render_data = GeneralLightRenderData::new(device);
        let directional_light_render_data = GeneralLightRenderData::new(device);

        ShadowAssets {
            point_light_render_data,
            directional_light_render_data,
        }
    }

    pub fn update(&mut self, _delta_time: std::time::Duration, renderer: &Renderer, world: &World) {
        for modification in &world.dirty_objects {
            if let ObjectModificationType::Light(light_modification) =
                &modification.modification_type
            {
                match light_modification {
                    world::ModificationType::Added => {
                        if let Some(light) = world.get_light(&modification.id) {
                            self.add_light(renderer, modification.id, light.clone());
                        }
                    }
                    world::ModificationType::Removed => todo!(),
                    world::ModificationType::TransformModified(transform_component) => {
                        todo!()
                    }
                    world::ModificationType::MaterialModified(pbr_material_descriptor) => {
                        unreachable!()
                    }
                }
            }
        }
    }

    pub fn render<'a, T>(&self, encoder: &mut CommandEncoder, renderables: T)
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
