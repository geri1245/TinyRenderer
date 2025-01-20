use crate::{
    components::{
        LightObjectComponent, OmnipresentComponentType, RenderableComponent, SceneComponentType,
        TransformComponent,
    },
    lights::DirectionalLight,
    material::PbrMaterialDescriptor,
    model::{MeshDescriptor, ModelRenderingOptions, PbrRenderingType, RenderingPass},
    resource_loader::PrimitiveShape,
    texture::{MaterialSource, TextureSourceDescriptor, TextureUsage},
};

/// Describes an object in the world. Used for object that have a 3D position (eg. rendered meshes, lights)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorldObject {
    pub components: Vec<SceneComponentType>,

    pub transform: TransformComponent,
}

/// Describes an aspect of the world (something that exists, but doesn't have a position, eg. directional light, skybox)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OmnipresentObject {
    pub components: Vec<OmnipresentComponentType>,
}

impl WorldObject {
    pub fn new(components: Vec<SceneComponentType>, transform: TransformComponent) -> Self {
        Self {
            components,
            transform,
        }
    }

    fn get_light_debug_object() -> RenderableComponent {
        let texture_source_descriptor = TextureSourceDescriptor {
            source: MaterialSource::FromFile("assets/textures/defaults/lightbulb.png".to_owned()),
            usage: TextureUsage::Albedo,
        };

        let rendering_options = ModelRenderingOptions {
            pass: RenderingPass::DeferredMain,
            use_depth_test: true,
            cast_shadows: false,
            pbr_resource_type: PbrRenderingType::Textures,
        };

        RenderableComponent::new(
            MeshDescriptor::PrimitiveInCode(PrimitiveShape::Square),
            PbrMaterialDescriptor::Texture(vec![texture_source_descriptor]),
            rendering_options,
            true,
        )
    }

    pub fn add_light_debug_object(&mut self) {
        self.components.push(SceneComponentType::Renderable(
            Self::get_light_debug_object(),
        ));
    }

    pub fn on_end_frame(&mut self) {
        self.transform.is_transform_dirty = false;
        for component in &mut self.components {
            component.reset_dirty_state();
        }
    }

    // TODO: general method for getting any component?
    pub fn get_renderable_component(&self) -> Option<&RenderableComponent> {
        for component in &self.components {
            match component {
                SceneComponentType::Renderable(renderable_component) => {
                    return Some(&renderable_component)
                }
                _ => {}
            }
        }

        None
    }

    pub fn get_renderable_component_mut(&mut self) -> Option<&mut RenderableComponent> {
        for component in &mut self.components {
            match component {
                SceneComponentType::Renderable(renderable_component) => {
                    return Some(renderable_component)
                }
                _ => {}
            }
        }

        None
    }

    pub fn get_light_component(&self) -> Option<&LightObjectComponent> {
        for component in &self.components {
            match component {
                SceneComponentType::LightObject(light_component) => return Some(&light_component),
                _ => {}
            }
        }

        None
    }

    pub fn get_light_component_mut(&mut self) -> Option<&mut LightObjectComponent> {
        for component in &mut self.components {
            match component {
                SceneComponentType::LightObject(light_component) => return Some(light_component),
                _ => {}
            }
        }

        None
    }
}

macro_rules! get_component {
    ( $object_with_components:expr, $component_type:expr ) => {{
        let mut maybe_component = None;
        for component in &object_with_components.components {
            match component {
                $component_type(component) => maybe_component = Some(&component),
                _ => {}
            }
        }
        maybe_component
    }};
}

impl OmnipresentObject {
    pub fn new(components: Vec<OmnipresentComponentType>) -> Self {
        Self { components }
    }

    pub fn on_end_frame(&mut self) {
        for component in &mut self.components {
            component.reset_dirty_state();
        }
    }

    pub fn get_light_component(&self) -> Option<&DirectionalLight> {
        for component in &self.components {
            match component {
                OmnipresentComponentType::DirectionalLight(directional_light) => {
                    return Some(directional_light)
                }
            }
        }

        None
    }

    pub fn get_light_component_mut(&mut self) -> Option<&mut DirectionalLight> {
        for component in &mut self.components {
            match component {
                OmnipresentComponentType::DirectionalLight(directional_light) => {
                    return Some(directional_light)
                }
            }
        }

        None
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Object {
    World(WorldObject),
    Omnipresent(OmnipresentObject),
}
