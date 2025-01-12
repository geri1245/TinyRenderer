use std::path::PathBuf;

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
    pub valid_extensions: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum UiDisplayDescription {
    Float(DisplayNumberOnUiDescription<f32>),
    UInt(DisplayNumberOnUiDescription<u32>),
    Path(DisplayPathOnUiDescription),
}

#[derive(Debug, Clone)]
pub struct SetNumberFromUiDescription<NumberType> {
    pub value: NumberType,
}

#[derive(Debug, Clone)]
pub enum SetPropertyFromUiDescription {
    Float(SetNumberFromUiDescription<f32>),
    UInt(SetNumberFromUiDescription<u32>),
}

#[derive(Debug, Clone)]
pub struct SetPropertyFromUiParams {
    pub name: String,
    pub value: SetPropertyFromUiDescription,
}

#[derive(Debug, Clone)]
pub struct UiDisplayParam {
    pub name: String,
    pub value: UiDisplayDescription,
}

pub trait UiSettable {
    fn try_set_value_from_ui(&mut self, params: SetPropertyFromUiParams) -> bool;
}

pub trait UiDisplayable {
    fn get_ui_description(&self) -> Vec<UiDisplayParam>;
}
