use proc_macro::TokenStream;

extern crate proc_macro;

mod set_from_ui_derive;
mod ui_display_derive;

#[proc_macro_derive(UiDisplayable, attributes(ui_param))]
pub fn derive_ui_displayable_type(item: TokenStream) -> TokenStream {
    ui_display_derive::derive_ui_displayable_type(item)
}

#[proc_macro_derive(UiSettableNew, attributes(ui_set))]
pub fn derive_ui_settable_type(item: TokenStream) -> TokenStream {
    set_from_ui_derive::derive_ui_settable_helper(item)
}
