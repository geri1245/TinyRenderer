use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor, BindingResource,
    CompareFunction, Device, Extent3d, TextureAspect, TextureView, TextureViewDescriptor,
    TextureViewDimension,
};

use crate::{
    bind_group_layout_descriptors,
    texture::{SampledTexture, SamplingType},
};

pub const SHADOW_SIZE: Extent3d = Extent3d {
    width: 1024,
    height: 1024,
    depth_or_array_layers: crate::renderer::MAX_LIGHTS as u32,
};

pub const CUBE_FACE_COUNT: usize = 6;

struct LightRenderingResources<const DEPTH_TARGET_FACE_COUNT: usize> {
    /// The backing texture for the members below
    depth_texture: SampledTexture,
    /// A vector of render targets - one cube texture view for each light to render the depth into
    depth_render_target_views: Vec<[TextureView; DEPTH_TARGET_FACE_COUNT]>,
    /// Cube array view to read the light shadow maps as a cube array
    /// This will contain every single view of the depth_render_target_views as a texture array
    depth_view: TextureView,
    bind_group: BindGroup,
}

impl<const DEPTH_TARGET_FACE_COUNT: usize> LightRenderingResources<DEPTH_TARGET_FACE_COUNT> {
    fn get_texture_view_type() -> TextureViewDimension {
        if DEPTH_TARGET_FACE_COUNT == 1 {
            TextureViewDimension::D2Array
        } else {
            TextureViewDimension::CubeArray
        }
    }

    fn get_bind_group_layout_descriptor() -> &'static BindGroupLayoutDescriptor<'static> {
        if DEPTH_TARGET_FACE_COUNT == 1 {
            &bind_group_layout_descriptors::DEPTH_TEXTURE_ARRAY
        } else {
            &bind_group_layout_descriptors::DEPTH_TEXTURE_CUBE_ARRAY
        }
    }

    pub fn new(device: &Device, point_light_count: usize) -> Self {
        let depth_texture = SampledTexture::create_depth_texture(
            device,
            Extent3d {
                depth_or_array_layers: (DEPTH_TARGET_FACE_COUNT * point_light_count) as u32,
                ..SHADOW_SIZE
            },
            Some(CompareFunction::Greater),
            SamplingType::Nearest,
            "Point shadow texture",
        );

        // Map through each light index and through each cube face for each light and create
        // the depth target views to render the shadow map into
        let depth_render_target_views = (0..point_light_count)
            .map(|light_index| {
                (0..DEPTH_TARGET_FACE_COUNT)
                    .map(|face_index| {
                        depth_texture.texture.create_view(&TextureViewDescriptor {
                            label: Some("shadow cubemap texture view single face"),
                            format: Some(SampledTexture::DEPTH_FORMAT),
                            dimension: Some(TextureViewDimension::D2),
                            aspect: TextureAspect::All,
                            base_mip_level: 0,
                            mip_level_count: None,
                            base_array_layer: (light_index * DEPTH_TARGET_FACE_COUNT + face_index)
                                as u32,
                            array_layer_count: Some(1),
                        })
                    })
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap()
            })
            .collect::<Vec<[_; DEPTH_TARGET_FACE_COUNT]>>();

        // Create the cube array view for reading the shadow map of the point lights
        let depth_view = depth_texture.texture.create_view(&TextureViewDescriptor {
            array_layer_count: Some((DEPTH_TARGET_FACE_COUNT * point_light_count) as u32),
            dimension: Some(Self::get_texture_view_type()),
            aspect: TextureAspect::DepthOnly,
            ..Default::default()
        });

        let bind_group = Self::create_bind_group(device, &depth_texture, &depth_view);

        Self {
            depth_texture,
            depth_render_target_views,
            depth_view,
            bind_group,
        }
    }

    fn create_bind_group(
        device: &Device,
        depth_texture: &SampledTexture,
        depth_texture_view: &TextureView,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            layout: &device.create_bind_group_layout(Self::get_bind_group_layout_descriptor()),
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(depth_texture_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&depth_texture.sampler),
                },
            ],
            label: None,
        })
    }
}

pub struct GeneralLightRenderData<const DEPTH_TARGET_FACE_COUNT: usize> {
    render_resources: LightRenderingResources<DEPTH_TARGET_FACE_COUNT>,
    /// The number of point lights for which we have space. If we don't have any more space and a new
    /// light is added, then we need to allocate some more space
    light_count: usize,
    /// If a light was deleted, we just take a note here, we don't shrink to textures
    /// If a new light is added later, then these free texture views are distributed before new space is allocated
    free_indices: Vec<usize>,
}

impl<const DEPTH_TARGET_FACE_COUNT: usize> GeneralLightRenderData<DEPTH_TARGET_FACE_COUNT> {
    pub fn new(device: &Device) -> Self {
        let initial_light_count = 1;
        let render_resources = LightRenderingResources::new(device, initial_light_count);
        Self {
            light_count: initial_light_count,
            free_indices: Vec::new(),
            render_resources,
        }
    }

    pub fn make_resources_for_new_light(&mut self, device: &Device) -> usize {
        if let Some(index) = self.free_indices.pop() {
            index
        } else {
            self.render_resources = LightRenderingResources::new(device, self.light_count + 1);
            self.light_count
        }
    }

    pub fn get_bind_group(&self) -> &BindGroup {
        &self.render_resources.bind_group
    }

    pub fn get_depth_target_view(&self, index: usize) -> &[TextureView; DEPTH_TARGET_FACE_COUNT] {
        &self.render_resources.depth_render_target_views[index]
    }
}
