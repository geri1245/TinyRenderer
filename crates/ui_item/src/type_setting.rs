use std::path::PathBuf;

use glam::{Quat, Vec3};

#[derive(Debug, Clone)]
pub struct SetNumberFromUiDescription<NumberType> {
    pub value: NumberType,
}

#[derive(Debug, Clone)]
pub struct SetPathFromUiDescription {
    pub value: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SetVecFromUiDescription {
    pub index: usize,
}

#[derive(Debug, Clone)]
pub struct SetEnumFromTheUiDescription {
    pub variant_name: String,
}

#[derive(Debug, Clone)]
pub struct SetStructFromUiDesc {
    pub field_name: String,
}

#[derive(Debug, Clone)]
pub enum SetPropertyFromUiDescription {
    Float(SetNumberFromUiDescription<f32>),
    Int(SetNumberFromUiDescription<i32>),
    Bool(bool),
    Path(SetPathFromUiDescription),
    Vec3(Vec3),
    Rotation(Quat),

    Vec(SetVecFromUiDescription),

    Struct(SetStructFromUiDesc),
    Enum(SetEnumFromTheUiDescription),
}

/// If a custom setter is used for setting a value from the UI, this trait must be implemented for it
pub trait CustomUiSettablePrimitive
where
    Self: Sized,
{
    fn get_raw_value(params: &[SetPropertyFromUiDescription]) -> Self;
}

pub trait UiSettableNew {
    fn set_value_from_ui(&mut self, params: &[SetPropertyFromUiDescription]);
}

impl UiSettableNew for f32 {
    fn set_value_from_ui(&mut self, value: &[SetPropertyFromUiDescription]) {
        *self = Self::get_raw_value(value);
    }
}

impl CustomUiSettablePrimitive for f32 {
    fn get_raw_value(value: &[SetPropertyFromUiDescription]) -> Self {
        if let SetPropertyFromUiDescription::Float(params) = &value[0] {
            params.value
        } else {
            panic!("Wrong type!")
        }
    }
}

impl UiSettableNew for u32 {
    fn set_value_from_ui(&mut self, value: &[SetPropertyFromUiDescription]) {
        if let SetPropertyFromUiDescription::Int(params) = &value[0] {
            *self = params.value as u32;
        } else {
            panic!("Wrong type!")
        }
    }
}

impl CustomUiSettablePrimitive for u32 {
    fn get_raw_value(value: &[SetPropertyFromUiDescription]) -> Self {
        if let SetPropertyFromUiDescription::Int(params) = &value[0] {
            params.value as u32
        } else {
            panic!("Wrong type!")
        }
    }
}

impl UiSettableNew for i32 {
    fn set_value_from_ui(&mut self, value: &[SetPropertyFromUiDescription]) {
        if let SetPropertyFromUiDescription::Int(params) = &value[0] {
            *self = params.value;
        } else {
            panic!("Wrong type!")
        }
    }
}

impl CustomUiSettablePrimitive for i32 {
    fn get_raw_value(value: &[SetPropertyFromUiDescription]) -> Self {
        if let SetPropertyFromUiDescription::Int(params) = &value[0] {
            params.value as i32
        } else {
            panic!("Wrong type!")
        }
    }
}

impl UiSettableNew for bool {
    fn set_value_from_ui(&mut self, value: &[SetPropertyFromUiDescription]) {
        if let SetPropertyFromUiDescription::Bool(new_value) = &value[0] {
            *self = *new_value;
        } else {
            panic!("Wrong type!")
        }
    }
}

impl CustomUiSettablePrimitive for bool {
    fn get_raw_value(value: &[SetPropertyFromUiDescription]) -> Self {
        if let SetPropertyFromUiDescription::Bool(param) = &value[0] {
            *param
        } else {
            panic!("Wrong type!")
        }
    }
}

impl UiSettableNew for Vec3 {
    fn set_value_from_ui(&mut self, value: &[SetPropertyFromUiDescription]) {
        if let SetPropertyFromUiDescription::Vec3(params) = &value[0] {
            *self = *params;
        } else {
            panic!("Wrong type!")
        }
    }
}

impl CustomUiSettablePrimitive for Vec3 {
    fn get_raw_value(value: &[SetPropertyFromUiDescription]) -> Self {
        if let SetPropertyFromUiDescription::Vec3(vec) = &value[0] {
            *vec
        } else {
            panic!("Wrong type!")
        }
    }
}

impl UiSettableNew for Quat {
    fn set_value_from_ui(&mut self, value: &[SetPropertyFromUiDescription]) {
        if let SetPropertyFromUiDescription::Rotation(quat) = &value[0] {
            *self = *quat;
        } else {
            panic!("Wrong type!")
        }
    }
}

impl CustomUiSettablePrimitive for Quat {
    fn get_raw_value(value: &[SetPropertyFromUiDescription]) -> Self {
        if let SetPropertyFromUiDescription::Rotation(quat) = &value[0] {
            *quat
        } else {
            panic!("Wrong type!")
        }
    }
}

impl UiSettableNew for PathBuf {
    fn set_value_from_ui(&mut self, value: &[SetPropertyFromUiDescription]) {
        if let SetPropertyFromUiDescription::Path(params) = &value[0] {
            *self = params.value.clone();
        } else {
            panic!("Wrong type!")
        }
    }
}

impl CustomUiSettablePrimitive for PathBuf {
    fn get_raw_value(value: &[SetPropertyFromUiDescription]) -> Self {
        if let SetPropertyFromUiDescription::Path(params) = &value[0] {
            params.value.clone()
        } else {
            panic!("Wrong type!")
        }
    }
}

impl<T: UiSettableNew> UiSettableNew for Vec<T> {
    fn set_value_from_ui(&mut self, value: &[SetPropertyFromUiDescription]) {
        if let SetPropertyFromUiDescription::Vec(params) = &value[0] {
            if self.len() > params.index {
                self[params.index].set_value_from_ui(&value[1..]);
            } else {
                panic!("Trying to set a vector value from the UI that is larger than the vector length");
            }
        } else {
            panic!("Wrong type!")
        }
    }
}
