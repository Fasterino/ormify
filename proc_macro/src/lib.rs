//! Procedural macros for the `ormify` crate.
//!
//! The macros are re-exported by `ormify` when its `proc_macro` feature is
//! enabled. Most applications should depend on `ormify` rather than this crate
//! directly.
//!
//! # Derives
//!
//! - `#[derive(Entity)]` generates an entity implementation and builder from a
//!   named-field struct. It requires at least one `#[db(pk)]` field.
//! - `#[derive(FieldType)]` implements the core field conversion contract for
//!   enum representations selected with `#[ft(...)]`.
//!
//! # Function-like macro
//!
//! `c!(field.path OP value)` creates a typed condition expression. Supported
//! operators include `==`, `!=`, `<`, `<=`, `>`, `>=`, `in [...]`, ranges with
//! `in [start..=end]`, and string matching with `=?`.
//!
//! See the `ormify` README for supported `db`/`ft` attributes and examples.

use convert_case::ccase;
use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, Fields, ItemStruct, parse_macro_input};

use crate::parser::parse_tt;

pub(crate) mod attr;
pub(crate) mod builder;
pub(crate) mod parser;
pub(crate) mod util;
pub(crate) mod validator;

/// Derive the [`Entity`] implementation for a named-field struct.
///
/// The struct must contain at least one field marked `#[db(pk)]`. Supported
/// field and struct attributes are documented in the `ormify` crate README.
#[proc_macro_derive(Entity, attributes(db))]
pub fn derive_entity(input: TokenStream) -> TokenStream {
    let crate_path = util::get_crate_path();

    let item = parse_macro_input!(input as ItemStruct);
    let ident = &item.ident;
    let vis = &item.vis;

    let attrs = parser::parse_db_attrs(&item.attrs).expect("Failed to parse attributes");

    if let Some(err) = validator::validate_struct_attrs(&ident, &attrs) {
        return err.into();
    }

    let mut fields = Vec::new();
    match &item.fields {
        Fields::Named(fields_named) => {
            for field in &fields_named.named {
                let attrs =
                    parser::parse_db_attrs(&field.attrs).expect("Failed to parse attributes");
                if let Some(err) = validator::validate_field_attrs(field, &attrs) {
                    return err.into();
                }
                fields.push((
                    field.ident.clone().expect("Expected named field"),
                    field.ty.clone(),
                    attrs,
                ))
            }
        }
        Fields::Unnamed(_) | Fields::Unit => {
            return syn::Error::new_spanned(&item, "Entity trait supports only named fields")
                .to_compile_error()
                .into();
        }
    };

    let util_mod = builder::build_util_mod(ident);

    let entity_id = builder::build_enitity_id(&attrs, ident);

    let scheme = builder::build_scheme(&attrs, ident, &fields);

    let builder_struct = builder::build_builder_struct(ident, &fields);
    let condition_builder_struct = builder::build_condition_builder_struct(&fields);
    let order_by_builder_struct = builder::build_order_by_builder_struct(&fields);

    let to_entry = builder::build_to_entry(&fields, false);
    let clone_to_entry = builder::build_to_entry(&fields, true);

    let from_entry = builder::build_from_entry(&fields);

    let update_fields = builder::build_update_fields(&fields);

    let get_key_name = builder::build_get_key_name(&fields);

    let primary_key_fields = parser::parse_primary_key_fields(&fields);

    if primary_key_fields.is_empty() {
        return syn::Error::new_spanned(ident, "Entity must have at least one pk attribute")
            .to_compile_error()
            .into();
    }

    let (join_impl, mod_join_impl) = builder::build_join_impl(ident, &fields, &util_mod);

    let expanded = quote! {
        impl #crate_path :: Entity for #ident {
            type Builder = #util_mod :: Builder;
            type ConditionBuilder = #util_mod :: ConditionBuilder;
            type OrderByBuilder = #util_mod :: OrderByBuilder;

            fn entity_id() -> #crate_path :: EntityId {
                #entity_id
            }

            fn scheme() -> #crate_path :: Scheme {
                #scheme
            }

            fn primary_key_fields() -> Vec<String> {
                vec![#(#primary_key_fields .into()),*]
            }

            fn to_entry(self) -> #crate_path :: Entry {
                #to_entry
            }
            fn clone_to_entry(&self) -> #crate_path :: Entry {
                #clone_to_entry
            }
            fn from_entry(mut entry: #crate_path :: EntryParser) -> #crate_path :: Result<Self> {
                #from_entry
            }
            fn update_fields(&mut self) {
                #update_fields
            }
            fn get_key_name(field: &str) -> &str {
                #get_key_name
            }
        }

        #vis mod #util_mod {
            use super::*;

            #builder_struct
            #condition_builder_struct
            #order_by_builder_struct

            #mod_join_impl
        }

        #join_impl
    };

    expanded.into()
}

/// Derive [`FieldType`] for a supported enum or struct representation.
///
/// Use `#[ft(VarChar(N))]`, `#[ft(i16)]`, `#[ft(i32)]` or `#[ft(i64)]` as appropriate for the enum
/// and `#[ft(Vec<u8>)]` if it is appropriate for the enum or struct. The blob representation
/// requires serde `Serialize` and `Deserialize` implementations.
#[proc_macro_derive(FieldType, attributes(ft))]
pub fn derive_field_type(input: TokenStream) -> TokenStream {
    let crate_path = util::get_crate_path();

    let item = parse_macro_input!(input as DeriveInput);
    let ident = &item.ident;

    let attr = match parser::parse_ft_attr(ident, &item.attrs) {
        Ok(attr) => attr,
        Err(err) => return err.to_compile_error().into(),
    };

    let expanded = match attr {
        attr::FieldTypeAttr::VarChar(n) => {
            let variants = match validator::validate_enum(&item) {
                Ok(v) => v,
                Err(e) => return e.to_compile_error().into(),
            };
            let idents: Vec<_> = variants.iter().map(|x| &x.ident).collect();
            let names: Vec<_> = idents.iter().map(|x| x.to_string()).collect();
            quote! {
                impl #crate_path :: FieldType for #ident {
                    type OriginalType = #ident;

                    fn get_type() -> #crate_path :: Type {
                        #crate_path :: Type::VarChar(#n)
                    }

                    fn to_value(self) -> #crate_path :: Value {
                        #crate_path :: Value::VarChar(
                            match self {
                                #(Self :: #idents => #names,)*
                            }.into()
                        )
                    }

                    fn try_from_value(value: #crate_path :: Value) -> #crate_path :: Result<Self> {
                        if let #crate_path :: Value::VarChar(var_char) = value {
                            match var_char.as_str() {
                                #(#names => Ok(Self :: #idents),)*
                                _ => Err(#crate_path :: Error::TypeMismatch),
                            }
                        } else {
                            Err(#crate_path :: Error::TypeMismatch)
                        }
                    }
                }
            }
        }
        attr::FieldTypeAttr::Blob => quote! {
            impl #crate_path :: FieldType for #ident {
                type OriginalType = #ident;

                fn get_type() -> #crate_path :: Type {
                    #crate_path :: Type::Blob
                }

                fn to_value(self) -> #crate_path :: Value {
                    #crate_path :: cbor_encode(&self)
                        .map(#crate_path :: Value::Blob)
                        .unwrap_or(#crate_path :: Value::Null)
                }

                fn try_from_value(value: #crate_path :: Value) -> #crate_path :: Result<Self> {
                    if let #crate_path :: Value::Blob(blob) = value {
                        #crate_path :: cbor_decode(&blob)
                    } else {
                        Err(#crate_path :: Error::TypeMismatch)
                    }
                }
            }
        },
        int_attr => {
            let variants = match validator::validate_enum(&item) {
                Ok(v) => v,
                Err(e) => return e.to_compile_error().into(),
            };
            let util_mod = builder::build_util_mod(ident);
            let (ty, var) = int_attr.get_type_and_value_variant();
            let idents: Vec<_> = variants.iter().map(|x| &x.ident).collect();
            let idents_caps: Vec<_> = idents
                .iter()
                .map(|x| parse_tt(&ccase!(constant, x.to_string())))
                .collect();
            quote! {
                mod #util_mod {
                    use super :: #ident as Entity;

                    #(pub const #idents_caps : #ty = Entity :: #idents as #ty;)*
                }

                impl #crate_path :: FieldType for #ident {
                    type OriginalType = #ident;

                    fn get_type() -> #crate_path :: Type {
                        #crate_path :: Type :: #var
                    }

                    fn to_value(self) -> #crate_path :: Value {
                        #crate_path :: Value :: #var (self as #ty)
                    }

                    fn try_from_value(value: #crate_path :: Value) -> #crate_path :: Result<Self> {
                        if let #crate_path :: Value :: #var (num) = value {
                            match num {
                                #(#util_mod :: #idents_caps => Ok(Self :: #idents),)*
                                _ => Err(#crate_path :: Error::TypeMismatch),
                            }
                        } else {
                            Err(#crate_path :: Error::TypeMismatch)
                        }
                    }
                }
            }
        }
    };

    expanded.into()
}

/// Build a typed condition from a compact field-expression syntax.
///
/// Examples: `c!(user.id == id)`, `c!(user.age in [18, 21])`,
/// `c!(user.age in [18..=65])`, and `c!(user.name =? "%ada%")`.
#[proc_macro]
pub fn c(stream: TokenStream) -> TokenStream {
    let condition = parse_macro_input!(stream as parser::MacroCondition);

    builder::build_macro_condition(condition).into()
}
