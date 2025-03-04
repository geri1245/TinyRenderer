#![feature(prelude_import)]
#[prelude_import]
use std::prelude::rust_2021::*;
#[macro_use]
extern crate std;
use std::path::PathBuf;
use ui_item_derive::{UiDisplayable, UiSettableNew};
struct Embedded {
    #[ui_param(max = "25.0")]
    member1: u32,
    #[ui_param(valid_file_extensions = "jpg,png")]
    path: PathBuf,
}
#[automatically_derived]
impl ui_item::UiDisplayable for Embedded {
    fn get_ui_description(&self) -> ui_item::UiDisplayDescription {
        let mut ui_params: Vec<ui_item::UiDisplayParam> = Vec::new();
        let ui_desc = self.member1.get_ui_description();
        let field_params = ui_item::FieldAttributes {
            min: None,
            max: Some("25.0".to_string()),
            valid_file_extensions: None,
            file_description: None,
        };
        ui_params
            .push(
                ui_item::UiDisplayParam::new(
                    "member1".to_string(),
                    ui_desc,
                    &field_params,
                ),
            );
        let ui_desc = self.path.get_ui_description();
        let field_params = ui_item::FieldAttributes {
            min: None,
            max: None,
            valid_file_extensions: Some("jpg,png".to_string()),
            file_description: None,
        };
        ui_params
            .push(
                ui_item::UiDisplayParam::new("path".to_string(), ui_desc, &field_params),
            );
        ui_item::UiDisplayDescription::Struct(ui_params)
    }
}
#[automatically_derived]
impl ui_item::UiSettableNew for Embedded {
    fn set_value_from_ui(&mut self, desc: &[ui_item::SetPropertyFromUiDescription]) {
        if let ui_item::SetPropertyFromUiDescription::Struct(struct_params) = &desc[0] {
            match struct_params.field_name.as_str() {
                "member1" => self.member1.set_value_from_ui(&desc[1..]),
                "path" => self.path.set_value_from_ui(&desc[1..]),
                _ => {
                    ::core::panicking::panic_fmt(
                        format_args!("Failed to find member for description"),
                    );
                }
            }
        } else {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "Trying to set a struct, but not struct setting params were provided!",
                    ),
                );
            };
        }
    }
}
#[automatically_derived]
impl ::core::default::Default for Embedded {
    #[inline]
    fn default() -> Embedded {
        Embedded {
            member1: ::core::default::Default::default(),
            path: ::core::default::Default::default(),
        }
    }
}
struct Test {
    #[ui_param(min = "12.0", max = "25.0")]
    member1: f32,
    #[ui_param(min = "12", max = "25")]
    member2: i32,
    embedded: Embedded,
}
#[automatically_derived]
impl ui_item::UiDisplayable for Test {
    fn get_ui_description(&self) -> ui_item::UiDisplayDescription {
        let mut ui_params: Vec<ui_item::UiDisplayParam> = Vec::new();
        let ui_desc = self.member1.get_ui_description();
        let field_params = ui_item::FieldAttributes {
            min: Some("12.0".to_string()),
            max: Some("25.0".to_string()),
            valid_file_extensions: None,
            file_description: None,
        };
        ui_params
            .push(
                ui_item::UiDisplayParam::new(
                    "member1".to_string(),
                    ui_desc,
                    &field_params,
                ),
            );
        let ui_desc = self.member2.get_ui_description();
        let field_params = ui_item::FieldAttributes {
            min: Some("12".to_string()),
            max: Some("25".to_string()),
            valid_file_extensions: None,
            file_description: None,
        };
        ui_params
            .push(
                ui_item::UiDisplayParam::new(
                    "member2".to_string(),
                    ui_desc,
                    &field_params,
                ),
            );
        let ui_desc = self.embedded.get_ui_description();
        let field_params = ui_item::FieldAttributes {
            min: None,
            max: None,
            valid_file_extensions: None,
            file_description: None,
        };
        ui_params
            .push(
                ui_item::UiDisplayParam::new(
                    "embedded".to_string(),
                    ui_desc,
                    &field_params,
                ),
            );
        ui_item::UiDisplayDescription::Struct(ui_params)
    }
}
#[automatically_derived]
impl ui_item::UiSettableNew for Test {
    fn set_value_from_ui(&mut self, desc: &[ui_item::SetPropertyFromUiDescription]) {
        if let ui_item::SetPropertyFromUiDescription::Struct(struct_params) = &desc[0] {
            match struct_params.field_name.as_str() {
                "member1" => self.member1.set_value_from_ui(&desc[1..]),
                "member2" => self.member2.set_value_from_ui(&desc[1..]),
                "embedded" => self.embedded.set_value_from_ui(&desc[1..]),
                _ => {
                    ::core::panicking::panic_fmt(
                        format_args!("Failed to find member for description"),
                    );
                }
            }
        } else {
            {
                ::core::panicking::panic_fmt(
                    format_args!(
                        "Trying to set a struct, but not struct setting params were provided!",
                    ),
                );
            };
        }
    }
}
#[automatically_derived]
impl ::core::default::Default for Test {
    #[inline]
    fn default() -> Test {
        Test {
            member1: ::core::default::Default::default(),
            member2: ::core::default::Default::default(),
            embedded: ::core::default::Default::default(),
        }
    }
}
enum Alma {
    Variant1(Test),
    Variant2,
}
#[automatically_derived]
impl ui_item::UiDisplayable for Alma {
    fn get_ui_description(&self) -> ui_item::UiDisplayDescription {
        let mut variant_names = Vec::new();
        let mut active_variant_name = None;
        variant_names.push("Variant1".to_string());
        variant_names.push("Variant2".to_string());
        let active_variant_item_desc = match self {
            Alma::Variant1(variant_data) => {
                active_variant_name = Some("Variant1".to_string());
                Some(variant_data.get_ui_description())
            }
            Alma::Variant2 => {
                active_variant_name = Some("Variant2".to_string());
                None
            }
        };
        let enum_desc = ui_item::DisplayEnumOnUiDescription {
            variants: variant_names,
            active_variant: active_variant_name.unwrap(),
            active_variant_item_desc,
        };
        ui_item::UiDisplayDescription::Enum(Box::new(enum_desc))
    }
}
fn main() {
    {
        ::std::io::_print(format_args!("Hello, world!\n"));
    };
}
