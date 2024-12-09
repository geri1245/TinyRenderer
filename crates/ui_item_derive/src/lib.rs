use proc_macro::TokenStream;
use quote::quote;
use syn::Data;

extern crate proc_macro;

#[proc_macro_derive(UiDisplayable)]
pub fn derive_ui_displayable(item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);

    let struct_identifier = &input.ident;

    match &input.data {
        Data::Struct(syn::DataStruct { fields, .. }) => {
            let mut implementation = quote! {
                let mut ui_params: Vec<UiDisplayParam> = Vec::new();
            };

            for field in fields {
                let identifier = field.ident.as_ref().unwrap();

                if let syn::Type::Path(type_path) = &field.ty {
                    match type_path.path.get_ident().unwrap().to_string().as_str() {
                        "u32" => {
                            implementation.extend(quote! {
                                ui_params.push(UiDisplayParam {
                                    name: stringify!(#identifier).to_string(),
                                    value: UiDisplayDescription::UInt(DisplayNumberToUiDescription {
                                        value: self.#identifier,
                                        min: 0,
                                        max: 5,
                                    }),
                                });
                            });
                        }
                        "f32" => {
                            implementation.extend(quote!{
                        ui_params.push(UiDisplayParam {
                            name: stringify!(#identifier).to_string(),
                            value: UiDisplayDescription::Float(DisplayNumberToUiDescription {
                                value: self.#identifier,
                                min: 0.0,
                                max: 5.0,
                            }),
                        });
                    });
                        }
                        _ => {}
                    }
                } else {
                    continue;
                };
            }

            quote! {
                #[automatically_derived]
                impl UiDisplayable for #struct_identifier {
                    fn get_ui_description(&self) -> Vec<UiDisplayParam> {
                        #implementation

                        ui_params
                    }
                }
            }
        }
        _ => unimplemented!(),
    }
    .into()
}

#[proc_macro_derive(UiSettable)]
pub fn derive_ui_settable(item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);

    let struct_identifier = &input.ident;

    match &input.data {
        Data::Struct(syn::DataStruct { fields, .. }) => {
            let mut implementation = quote! {};

            for field in fields {
                let identifier = field.ident.as_ref().unwrap();

                if let syn::Type::Path(type_path) = &field.ty {
                    match type_path.path.get_ident().unwrap().to_string().as_str() {
                        "u32" => {
                            implementation.extend(quote! {
                                stringify!(#identifier) => {
                                    if let SetPropertyFromUiDescription::UInt(value_set_desc) = params.value {
                                        self.#identifier = value_set_desc.value;
                                        true
                                    } else {
                                        let property_name = params.name;
                                        panic!("Types don't match for {property_name}");
                                    }
                                }
                            });
                        }
                        "f32" => {
                            implementation.extend(quote! {
                                stringify!(#identifier) => {
                                    if let SetPropertyFromUiDescription::Float(value_set_desc) = params.value {
                                        self.#identifier = value_set_desc.value;
                                        true
                                    } else {
                                        let property_name = params.name;
                                        panic!("Types don't match for {property_name}");
                                    }
                                }
                            });
                        }
                        _ => {}
                    }
                } else {
                    continue;
                }
            }

            quote! {
                #[automatically_derived]
                impl UiSettable for #struct_identifier {
                    fn try_set_value_from_ui(&mut self, params: SetPropertyFromUiParams) -> bool {
                        match params.name.as_str() {
                            #implementation
                            _ => false,
                        }
                    }
                }
            }
        }
        _ => unimplemented!(),
    }
    .into()
}
