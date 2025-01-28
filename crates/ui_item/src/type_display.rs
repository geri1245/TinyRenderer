use core::f32;
use glam::{Quat, Vec3};
use std::{fmt::Debug, path::PathBuf};

#[derive(Debug, Clone)]
pub struct DisplayNumberOnUiDescription<NumberType> {
    pub value: NumberType,
    pub min: NumberType,
    pub max: NumberType,
}

#[derive(Debug, Clone)]
pub struct DisplayPathOnUiDescription {
    pub path: PathBuf,
    /// This will be displayed in the type of the file picker dialog
    pub file_type_description: String,
    /// Files with these extensions are accepted when trying to update the item from the UI
    /// Added as a comma separated string sequence
    pub valid_file_extensions: String,
}

#[derive(Debug, Clone)]
pub struct DisplayEnumOnUiDescription {
    pub variants: Vec<String>,
    pub active_variant: String,
    pub active_variant_item_desc: Option<UiDisplayDescription>,
}

#[derive(Debug, Clone)]
pub struct DisplayRotationOnUiParams {
    pub angle: DisplayNumberOnUiDescription<f32>,
    pub axis: DisplayNumberOnUiDescription<Vec3>,
}

pub struct FieldAttributes {
    pub min: Option<i32>,
    pub max: Option<i32>,
    pub fmin: Option<f32>,
    pub fmax: Option<f32>,
    pub valid_file_extensions: Option<String>,
    pub file_description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UiDisplayParam {
    pub name: String,
    pub display: UiDisplayDescription,
}

fn get_or_warn<T>(value: &Option<T>, name: &String, attribute: &str, default: T) -> T
where
    T: Debug + Clone,
{
    if let Some(unwrapped_value) = value {
        unwrapped_value.clone()
    } else {
        log::warn!(
            "Attribute {attribute:?} wasn't set for item {name:?}. Using default value {default:?}"
        );
        default
    }
}

impl UiDisplayParam {
    pub fn new(
        name: String,
        mut ui_display: UiDisplayDescription,
        ui_params: &FieldAttributes,
    ) -> Self {
        match &mut ui_display {
            UiDisplayDescription::SliderFloat(display_number_on_ui_description) => {
                display_number_on_ui_description.min =
                    get_or_warn(&ui_params.fmin, &name, "fmin", 0.0);
                display_number_on_ui_description.max =
                    get_or_warn(&ui_params.fmax, &name, "fmax", 1.0);
            }
            UiDisplayDescription::SliderInt(display_number_on_ui_description) => {
                display_number_on_ui_description.min = get_or_warn(&ui_params.min, &name, "min", 0);
                display_number_on_ui_description.max = get_or_warn(&ui_params.max, &name, "max", 5);
            }
            UiDisplayDescription::Path(display_path_on_ui_description) => {
                display_path_on_ui_description.file_type_description = get_or_warn(
                    &ui_params.file_description,
                    &name,
                    "file_description",
                    "Dummy file".to_string(),
                );
                display_path_on_ui_description.valid_file_extensions = get_or_warn(
                    &ui_params.valid_file_extensions,
                    &name,
                    "valid_file_extensions",
                    "png".to_string(),
                );
            }
            UiDisplayDescription::Vec3(display_vec3_on_ui_description) => {
                let fmin = get_or_warn(&ui_params.fmin, &name, "fmin", 0.0);
                let fmax = get_or_warn(&ui_params.fmax, &name, "fmax", 1.0);
                display_vec3_on_ui_description.min = Vec3::splat(fmin);
                display_vec3_on_ui_description.max = Vec3::splat(fmax);
            }

            // These types don't need to set anything from the UI, only the primitive types will have things set
            UiDisplayDescription::Struct(_) => {}
            UiDisplayDescription::Enum(_) => {}
            UiDisplayDescription::Rotation(_) => {}
            UiDisplayDescription::Vector(_) => {}
            UiDisplayDescription::Bool(_) => {}
        }

        Self {
            name,
            display: ui_display,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SliderParams<NumberType> {
    pub min: NumberType,
    pub max: NumberType,
}

#[derive(Debug, Clone)]
pub enum UiDisplayDescription {
    SliderFloat(DisplayNumberOnUiDescription<f32>),
    SliderInt(DisplayNumberOnUiDescription<i32>),
    Path(DisplayPathOnUiDescription),
    Bool(bool),

    Vec3(DisplayNumberOnUiDescription<Vec3>),
    Rotation(DisplayRotationOnUiParams),

    Vector(Vec<UiDisplayDescription>),

    Struct(Vec<UiDisplayParam>),
    Enum(Box<DisplayEnumOnUiDescription>),
}

pub trait UiDisplayable {
    fn get_ui_description(&self) -> UiDisplayDescription;
}

impl UiDisplayable for f32 {
    fn get_ui_description(&self) -> UiDisplayDescription {
        UiDisplayDescription::SliderFloat(DisplayNumberOnUiDescription {
            value: *self,
            min: 0.0,
            max: 1.0,
        })
    }
}

impl UiDisplayable for i32 {
    fn get_ui_description(&self) -> UiDisplayDescription {
        UiDisplayDescription::SliderInt(DisplayNumberOnUiDescription {
            value: *self,
            min: 0,
            max: 10,
        })
    }
}

impl UiDisplayable for u32 {
    fn get_ui_description(&self) -> UiDisplayDescription {
        UiDisplayDescription::SliderInt(DisplayNumberOnUiDescription {
            value: *self as i32,
            min: 0,
            max: 10,
        })
    }
}

impl UiDisplayable for bool {
    fn get_ui_description(&self) -> UiDisplayDescription {
        UiDisplayDescription::Bool(*self)
    }
}

impl UiDisplayable for PathBuf {
    fn get_ui_description(&self) -> UiDisplayDescription {
        UiDisplayDescription::Path(DisplayPathOnUiDescription {
            path: self.clone(),
            file_type_description: "".to_owned(),
            valid_file_extensions: "".to_owned(),
        })
    }
}

impl UiDisplayable for Vec3 {
    fn get_ui_description(&self) -> UiDisplayDescription {
        UiDisplayDescription::Vec3(DisplayNumberOnUiDescription {
            value: *self,
            min: Vec3::splat(0.0),
            max: Vec3::splat(0.0),
        })
    }
}

impl UiDisplayable for Quat {
    fn get_ui_description(&self) -> UiDisplayDescription {
        let (axis, angle) = self.to_axis_angle();
        UiDisplayDescription::Rotation(DisplayRotationOnUiParams {
            angle: DisplayNumberOnUiDescription {
                value: math_helpers::normalize_to_interval(angle, 0.0..=(2.0 * f32::consts::PI)),
                min: 0.0,
                max: 2.0 * f32::consts::PI,
            },
            axis: DisplayNumberOnUiDescription {
                value: axis,
                min: Vec3::splat(-1.0),
                max: Vec3::splat(1.0),
            },
        })
    }
}

impl<T: UiDisplayable> UiDisplayable for Vec<T> {
    fn get_ui_description(&self) -> UiDisplayDescription {
        UiDisplayDescription::Vector(
            self.iter()
                .map(|item| item.get_ui_description())
                .collect::<Vec<_>>(),
        )
    }
}

impl<T: UiDisplayable> UiDisplayable for &T {
    fn get_ui_description(&self) -> UiDisplayDescription {
        (*self).get_ui_description()
    }
}
