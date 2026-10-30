use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{DeriveInput, Error, punctuated::Punctuated};

use crate::attr::DbAttr;

pub fn validate_struct_attrs<T: ToTokens>(tokens: T, attrs: &[DbAttr]) -> Option<TokenStream> {
    validate_attrs(tokens, attrs, "err_text", |attr| match attr {
        DbAttr::Group(_) | DbAttr::Rename(_) => true,
        _ => false,
    })
}

pub fn validate_field_attrs<T: ToTokens>(tokens: T, attrs: &[DbAttr]) -> Option<TokenStream> {
    validate_attrs(tokens, attrs, "err_text", |attr| match attr {
        DbAttr::Rename(_)
        | DbAttr::PrimaryKey(_)
        | DbAttr::ForeignKey(_)
        | DbAttr::Index(_)
        | DbAttr::Unique(_)
        | DbAttr::Update(_) => true,
        _ => false,
    })
}

fn validate_attrs<T, F>(tokens: T, attrs: &[DbAttr], ty: &str, f: F) -> Option<TokenStream>
where
    T: ToTokens,
    F: Fn(&DbAttr) -> bool,
{
    let mut set = HashSet::new();
    for attr in attrs {
        let attr_str = attr.as_ref();
        if !set.insert(attr_str) {
            return Some(
                Error::new_spanned(
                    tokens,
                    format!("{ty} doesn't support multiple {attr_str} attributes!"),
                )
                .into_compile_error(),
            );
        }
        if !f(attr) {
            return Some(
                Error::new_spanned(
                    tokens,
                    format!("{ty} doesn't support {attr_str} attribute!"),
                )
                .into_compile_error(),
            );
        }
    }

    None
}

pub fn validate_enum(
    item: &DeriveInput,
) -> syn::Result<&Punctuated<syn::Variant, syn::token::Comma>> {
    match &item.data {
        syn::Data::Enum(data_enum) => Ok(&data_enum.variants),
        _ => Err(Error::new_spanned(item, "Only enum is supported")),
    }
}
