pub struct DisplayNumberToUiDescription<NumberType> {
    pub value: NumberType,
    pub min: NumberType,
    pub max: NumberType,
}

pub enum UiDisplayDescription {
    Float(DisplayNumberToUiDescription<f32>),
    UInt(DisplayNumberToUiDescription<u32>),
}

pub struct SetNumberFromUiDescription<NumberType> {
    pub value: NumberType,
}

pub enum SetPropertyFromUiDescription {
    Float(SetNumberFromUiDescription<f32>),
    UInt(SetNumberFromUiDescription<u32>),
}

pub struct SetPropertyFromUiParams {
    pub name: String,
    pub value: SetPropertyFromUiDescription,
}

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
