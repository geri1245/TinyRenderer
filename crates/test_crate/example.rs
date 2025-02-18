#![feature(prelude_import)]
#[prelude_import]
use std::prelude::rust_2021::*;
#[macro_use]
extern crate std;
use std::path::PathBuf;
use ui_item::*;
use ui_item_derive::{UiDisplayable, UiSettableNew};
struct Embedded {
    #[ui_param(fmax = 25.0, getter = "get_member1")]
    member1: u32,
    #[ui_param(valid_file_extensions = "jpg,png")]
    path: PathBuf,
}
#[automatically_derived]
impl UiDisplayable for Embedded {
    fn get_ui_description(&self) -> UiDisplayDescription {
        let mut ui_params: Vec<UiDisplayParam> = Vec::new();
        let ui_desc = self.get_member1();
        let field_params = ui_item::FieldAttributes {
            min: None,
            max: None,
            fmin: None,
            fmax: Some(25f32),
            valid_file_extensions: None,
            file_description: None,
        };
        ui_params
            .push(UiDisplayParam::new("member1".to_string(), ui_desc, &field_params));
        let ui_desc = self.path.get_ui_description();
        let field_params = ui_item::FieldAttributes {
            min: None,
            max: None,
            fmin: None,
            fmax: None,
            valid_file_extensions: Some("jpg,png".to_string()),
            file_description: None,
        };
        ui_params.push(UiDisplayParam::new("path".to_string(), ui_desc, &field_params));
        UiDisplayDescription::Struct(ui_params)
    }
}
#[automatically_derived]
impl UiSettableNew for Embedded {
    fn set_value_from_ui(&mut self, desc: &[SetPropertyFromUiDescription]) {
        if let SetPropertyFromUiDescription::Struct(struct_params) = &desc[0] {
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
impl Embedded {
    pub fn set_me_please(&mut self, value: &[SetPropertyFromUiDescription]) {}
    pub fn get_member1(&self) -> UiDisplayDescription {
        UiDisplayDescription::SliderInt(DisplayNumberOnUiDescription {
            value: self.member1 as i32,
            min: 1,
            max: 8,
        })
    }
}
struct Test {
    #[ui_param(fmin = 12.0, fmax = 25.0)]
    member1: f32,
    #[ui_param(min = 12, max = 25)]
    member2: i32,
    #[ui_set(setter = "set_me_please")]
    embedded: Embedded,
}
#[automatically_derived]
impl UiDisplayable for Test {
    fn get_ui_description(&self) -> UiDisplayDescription {
        let mut ui_params: Vec<UiDisplayParam> = Vec::new();
        let ui_desc = self.member1.get_ui_description();
        let field_params = ui_item::FieldAttributes {
            min: None,
            max: None,
            fmin: Some(12f32),
            fmax: Some(25f32),
            valid_file_extensions: None,
            file_description: None,
        };
        ui_params
            .push(UiDisplayParam::new("member1".to_string(), ui_desc, &field_params));
        let ui_desc = self.member2.get_ui_description();
        let field_params = ui_item::FieldAttributes {
            min: Some(12i32),
            max: Some(25i32),
            fmin: None,
            fmax: None,
            valid_file_extensions: None,
            file_description: None,
        };
        ui_params
            .push(UiDisplayParam::new("member2".to_string(), ui_desc, &field_params));
        let ui_desc = self.embedded.get_ui_description();
        let field_params = ui_item::FieldAttributes {
            min: None,
            max: None,
            fmin: None,
            fmax: None,
            valid_file_extensions: None,
            file_description: None,
        };
        ui_params
            .push(UiDisplayParam::new("embedded".to_string(), ui_desc, &field_params));
        UiDisplayDescription::Struct(ui_params)
    }
}
#[automatically_derived]
impl UiSettableNew for Test {
    fn set_value_from_ui(&mut self, desc: &[SetPropertyFromUiDescription]) {
        if let SetPropertyFromUiDescription::Struct(struct_params) = &desc[0] {
            match struct_params.field_name.as_str() {
                "member1" => self.member1.set_value_from_ui(&desc[1..]),
                "member2" => self.member2.set_value_from_ui(&desc[1..]),
                "embedded" => self.embedded.set_me_please(&desc[1..]),
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
impl UiDisplayable for Alma {
    fn get_ui_description(&self) -> UiDisplayDescription {
        let mut variant_names = Vec::new();
        variant_names.push("Variant1".to_string());
        variant_names.push("Variant2".to_string());
        let active_variant_item_desc = match self {
            Alma::Variant1(variant_data) => Some(variant_data.get_ui_description()),
            Alma::Variant2 => None,
        };
        let enum_desc = DisplayEnumOnUiDescription {
            variants: variant_names,
            active_variant: "".to_string(),
            active_variant_item_desc,
        };
        UiDisplayDescription::Enum(Box::new(enum_desc))
    }
}
fn main() {
    {
        ::std::io::_print(format_args!("Hello, world!\n"));
    };
}
