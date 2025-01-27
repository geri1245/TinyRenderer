use darling::FromField;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::Data;

#[derive(Debug, FromField)]
#[darling(attributes(ui_set))]
pub struct UiSetFieldAttributes {
    #[darling(default)]
    pub setter: Option<String>,
    #[darling(default)]
    pub skip: Option<bool>,
}

pub fn derive_ui_settable_helper(item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);

    let type_name = &input.ident;

    let function_body = match &input.data {
        Data::Struct(syn::DataStruct { fields, .. }) => {
            let mut field_names_with_params = Vec::new();
            for field in fields {
                let field_params = match UiSetFieldAttributes::from_field(field) {
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

                field_names_with_params.push((field.ident.clone().unwrap(), field_params));
            }

            let mut cases = quote! {};
            for (field_name, field_params) in field_names_with_params {
                if let Some(setter_function) = field_params.setter {
                    let setter_function_ident = format_ident!("{}", setter_function);
                    cases.extend(
                        quote! {
                            stringify!(#field_name) => self.#field_name.#setter_function_ident(&desc[1..]),
                        },
                    );
                } else {
                    cases.extend(quote! {
                        stringify!(#field_name) => self.#field_name.set_value_from_ui(&desc[1..]),
                    });
                };
            }

            quote! {
                if let ui_item::SetPropertyFromUiDescription::Struct(struct_params) = &desc[0] {
                    match struct_params.field_name.as_str() {
                         #cases
                        _ => panic!(stringify!(Failed to find member for description)),
                    }
                } else {
                    panic!("Trying to set a struct, but not struct setting params were provided!");
                }
            }
        }
        Data::Enum(enum_data) => {
            let mut variant_names = Vec::new();
            let mut cases = Vec::new();

            for variant in &enum_data.variants {
                let variant_name = &variant.ident;
                variant_names.push(variant_name.clone());

                let what_to_do = if variant.fields.is_empty() {
                    quote! {
                        // We already have the same enum variant that we want to set. Set the inner data if any
                        if let Self::#variant_name = *self {
                        } else {
                            *self = Self::#variant_name;
                        }
                    }
                } else {
                    quote! {
                        // We already have the same enum variant that we want to set. Set the inner data if any
                        if let Self::#variant_name(inner_data) = self {
                            inner_data.set_value_from_ui(&desc[1..]);
                        } else {
                            // If the variants are different, we can't set the inner data immediately.
                            // That should come in the next change event, we just set the new variant now
                            *self = Self::#variant_name(Default::default());
                        }
                    }
                };

                let case = quote! {
                    stringify!(#variant_name) => {
                        #what_to_do
                    }
                };
                cases.push(case);
            }

            quote! {
                if let ui_item::SetPropertyFromUiDescription::Enum(enum_param) = &desc[0] {
                    match enum_param.variant_name.as_str() {
                        #(#cases)*,
                        _ => {},
                    }
                }
                println!("This is not implemented yet!");
            }
        }
        _ => unimplemented!(),
    };

    quote! {
        #[automatically_derived]
        impl ui_item::UiSettableNew for #type_name {
            fn set_value_from_ui(&mut self, desc: &[ui_item::SetPropertyFromUiDescription]) {
                #function_body
            }
        }
    }
    .into()
}
