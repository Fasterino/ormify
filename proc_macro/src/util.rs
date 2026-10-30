use quote::quote;

pub fn get_crate_path() -> proc_macro2::TokenStream {
    quote! { ormify }
}
