use darling::FromField;
use proc_macro::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::Data;

#[derive(Debug, FromField)]
#[darling(attributes(ui_param))]
pub struct UiParamFieldAttributes {
    #[darling(default)]
    pub min: Option<i32>,
    #[darling(default)]
    pub max: Option<i32>,
    #[darling(default)]
    pub fmin: Option<f32>,
    #[darling(default)]
    pub fmax: Option<f32>,
    #[darling(default)]
    pub valid_file_extensions: Option<String>,
    #[darling(default)]
    pub file_description: Option<String>,
    #[darling(default)]
    pub getter: Option<String>,
    #[darling(default)]
    pub skip: Option<bool>,
}

fn quote_option<T: ToTokens>(option: Option<T>) -> proc_macro2::TokenStream {
    if let Some(value) = option {
        quote! {Some(#value)}
    } else {
        quote! {None}
    }
}

fn quote_option_string(option: Option<String>) -> proc_macro2::TokenStream {
    if let Some(value) = option {
        quote! {Some(#value.to_string())}
    } else {
        quote! {None}
    }
}

pub fn derive_ui_displayable_type(item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);

    let type_name = &input.ident;

    let function_body_implementation = match &input.data {
        Data::Struct(syn::DataStruct { fields, .. }) => {
            let mut implementation = proc_macro2::TokenStream::new();

            for field in fields {
                let identifier = field.ident.as_ref().unwrap();

                if let syn::Type::Path(_type_path) = &field.ty {
                    let field_params = match UiParamFieldAttributes::from_field(field) {
                        Ok(v) => v,
                        Err(e) => {
                            return TokenStream::from(e.write_errors());
                        }
                    };

                    if let Some(skip) = field_params.skip {
                        if skip {
                            continue;
                        }
                    }

                    let field_param_min = quote_option(field_params.min);
                    let field_param_max = quote_option(field_params.max);
                    let field_param_fmin = quote_option(field_params.fmin);
                    let field_param_fmax = quote_option(field_params.fmax);
                    let field_param_valid_file_types =
                        quote_option_string(field_params.valid_file_extensions);
                    let field_param_file_description =
                        quote_option_string(field_params.file_description);

                    let get_item_value = if let Some(getter_function) = field_params.getter {
                        // Calling the getter function specified by the getter attribute
                        let getter_function_ident = format_ident!("{}", getter_function);
                        quote! { self.#getter_function_ident() }
                    } else {
                        // Calling the standard getter function of the trait
                        quote! { self.#identifier.get_ui_description() }
                    };

                    implementation.extend(quote! {
                        let ui_desc = #get_item_value;
                        let field_params = ui_item::FieldAttributes{
                            min: #field_param_min,
                            max: #field_param_max,
                            fmin: #field_param_fmin,
                            fmax: #field_param_fmax,
                            valid_file_extensions: #field_param_valid_file_types,
                            file_description: #field_param_file_description,
                        };
                        ui_params.push(ui_item::UiDisplayParam::new(stringify!(#identifier).to_string(), ui_desc, &field_params));
                        });
                } else {
                    continue;
                };
            }

            quote! {
                let mut ui_params: Vec<ui_item::UiDisplayParam> = Vec::new();

                #implementation

                ui_item::UiDisplayDescription::Struct(ui_params)
            }
        }
        Data::Enum(enum_data) => {
            let mut cases = Vec::new();
            let mut variant_names = Vec::new();

            for variant in &enum_data.variants {
                let variant_name = &variant.ident;
                variant_names.push(variant_name.clone());

                let case = if variant.fields.is_empty() {
                    quote! {#type_name::#variant_name => { None }}
                } else {
                    quote! {#type_name::#variant_name(variant_data) => { Some(variant_data.get_ui_description()) }}
                };
                cases.push(case);
            }

            let implementation = quote! {
                let mut variant_names = Vec::new();

                #(variant_names.push(stringify!(#variant_names).to_string());)*

                let active_variant_item_desc = match self {
                    #(#cases)*
                };

                let enum_desc = ui_item::DisplayEnumOnUiDescription{
                    variants: variant_names,
                    active_variant: "".to_string(),
                    active_variant_item_desc,
                };

                ui_item::UiDisplayDescription::Enum(Box::new(enum_desc))
            };

            implementation
        }
        _ => unimplemented!(),
    };

    quote! {
        #[automatically_derived]
        impl ui_item::UiDisplayable for #type_name {
            fn get_ui_description(&self) -> ui_item::UiDisplayDescription {
                #function_body_implementation
            }
        }
    }
    .into()
}
