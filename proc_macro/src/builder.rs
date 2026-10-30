use std::collections::HashMap;

use crate::{
    attr::{DbAttr, ForeignKeyAttr, GroupAttr, IndexAttr, PrimaryKeyAttr, UniqueAttr, UpdateAttr},
    parser::{
        Constraints, KeyName, MacroCondition, NamedPair, parse_constraints, parse_name, parse_tt,
    },
    util::get_crate_path,
};
use convert_case::ccase;
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Ident, Type};

pub fn build_enitity_id(attrs: &[DbAttr], ident: &Ident) -> TokenStream {
    let crate_path = get_crate_path();

    let name = parse_name(ident, attrs);

    if let Some(group_attr) = DbAttr::query::<GroupAttr>(attrs) {
        let group = group_attr.name;
        quote! {
            #crate_path :: EntityId :: new_with_group( #group, #name )
        }
    } else {
        quote! {
            #crate_path :: EntityId :: new( #name )
        }
    }
}

pub fn build_scheme(
    attrs: &[DbAttr],
    ident: &Ident,
    fields: &[(Ident, Type, Vec<DbAttr>)],
) -> TokenStream {
    let crate_path = get_crate_path();

    let name = parse_name(ident, attrs);
    let entity_id = build_enitity_id(attrs, ident);
    let constraints = parse_constraints(fields);

    let mut constrs = Vec::<TokenStream>::new();

    constrs.push(build_pk_scheme(&entity_id, constraints.primary_keys));

    for named in constraints.foreign_keys {
        let (key_name, keys) = named.into_inner();

        constrs.push(build_fk_scheme(&entity_id, key_name, keys));
    }
    for named in constraints.unique {
        let (key_name, keys) = named.into_inner();

        constrs.push(build_unique_scheme(&entity_id, key_name, keys));
    }
    for named in constraints.indexes {
        let (key_name, keys) = named.into_inner();

        constrs.push(build_index_scheme(&entity_id, key_name, keys));
    }

    let fields: Vec<_> = fields
        .iter()
        .map(|(ident, ty, attrs)| build_field_scheme(ident, ty, attrs))
        .collect();

    let group = if let Some(group_attr) = DbAttr::query::<GroupAttr>(attrs) {
        let name = group_attr.name;
        quote! { .group(#name) }
    } else {
        quote! {}
    };

    quote! {
        #crate_path :: Scheme :: builder()
            #group
            .name(#name)
            .fields([#(#fields),*])
            .constraints([#(#constrs),*])
            .build()
            .expect("Failed to build scheme")
    }
}

fn build_pk_scheme(name: &TokenStream, keys: Vec<NamedPair<PrimaryKeyAttr>>) -> TokenStream {
    let crate_path = get_crate_path();
    let keys: Vec<_> = keys.iter().map(|x| x.name()).collect();

    quote! {
        #crate_path :: Constraint::primary_key(#name)
            #(.add_key(#keys))*

    }
}

fn build_fk_scheme(
    name: &TokenStream,
    key_name: KeyName,
    keys: Vec<NamedPair<ForeignKeyAttr>>,
) -> TokenStream {
    let crate_path = get_crate_path();
    let Some(first_key) = keys.first() else {
        return syn::Error::new_spanned(name, "foreign key must contain at least one field")
            .to_compile_error();
    };
    let entity = &first_key.value().entity;
    let entity_keys: Vec<_> = keys.iter().map(|x| x.value().key.to_string()).collect();
    let keys: Vec<_> = keys.iter().map(|x| x.name()).collect();

    quote! {
        #crate_path :: Constraint::foreign_key_builder(#name, #key_name, #crate_path :: pd :: <#entity> ())
            #(.add_key(<#entity as #crate_path :: Entity> :: get_key_name(
                #entity_keys
            )))*
            .build()
            #(.add_key(#keys))*

    }
}

fn build_index_scheme(
    name: &TokenStream,
    key_name: KeyName,
    keys: Vec<NamedPair<IndexAttr>>,
) -> TokenStream {
    let crate_path = get_crate_path();
    let keys: Vec<_> = keys.iter().map(|x| x.name()).collect();

    quote! {
        #crate_path :: Constraint::index(#name, #key_name)
            #(.add_key(#keys))*

    }
}

fn build_unique_scheme(
    name: &TokenStream,
    key_name: KeyName,
    keys: Vec<NamedPair<UniqueAttr>>,
) -> TokenStream {
    let crate_path = get_crate_path();
    let keys: Vec<_> = keys.iter().map(|x| x.name()).collect();

    quote! {
        #crate_path :: Constraint::unique(#name, #key_name)
            #(.add_key(#keys))*

    }
}

pub fn build_field_scheme(ident: &Ident, ty: &Type, attrs: &[DbAttr]) -> TokenStream {
    let crate_path = get_crate_path();

    let name = parse_name(ident, attrs);

    quote! {
        #crate_path :: FieldScheme :: builder()
            .name(#name)
            .ty_ext::<#ty>()
            .build()
            .expect("Failed to build field scheme")
    }
}

pub fn build_to_entry(fields: &[(Ident, Type, Vec<DbAttr>)], clone: bool) -> TokenStream {
    let crate_path = get_crate_path();

    let clone = if clone {
        quote! {.clone()}
    } else {
        quote! {}
    };

    let fields: Vec<_> = fields
        .iter()
        .map(|(ident, _ty, attrs)| {
            let name = parse_name(ident, attrs);
            quote! {(
                String::from(#name),
                #crate_path :: FieldType :: to_value(self. #ident #clone)
            )}
        })
        .collect();

    quote! {
        [
            #(#fields,)*
        ].into()
    }
}

pub fn build_from_entry(fields: &[(Ident, Type, Vec<DbAttr>)]) -> TokenStream {
    let idents: Vec<_> = fields.iter().map(|x| &x.0).collect();

    let names: Vec<_> = fields
        .iter()
        .map(|(ident, _ty, attrs)| parse_name(ident, attrs))
        .collect();

    quote! {
        Ok(Self {
            #(#idents: entry.get(#names)?,)*
        })
    }
}

pub fn build_update_fields(fields: &[(Ident, Type, Vec<DbAttr>)]) -> TokenStream {
    let fields = fields.iter().filter_map(|(ident, _ty, attrs)| {
        DbAttr::query::<UpdateAttr>(attrs).map(|UpdateAttr { func }| {
            quote! {
                self. #ident = #func ();
            }
        })
    });
    quote! {
        #(#fields)*
    }
}

pub fn build_builder_struct(ident: &Ident, fields: &[(Ident, Type, Vec<DbAttr>)]) -> TokenStream {
    let crate_path = get_crate_path();

    let idents: Vec<_> = fields.iter().map(|x| &x.0).collect();
    let types: Vec<_> = fields.iter().map(|x| &x.1).collect();

    quote! {
        #[derive(Default)]
        pub struct Builder {
            #(#idents: #types,)*
        }

        impl Builder {
            #(
                pub fn #idents <I: Into<<#types as #crate_path :: FieldType> :: OriginalType>>(mut self, #idents: I) -> Self {
                    self. #idents = #idents .into().into();
                    self
                }
            )*

            pub fn build(self) -> #ident {
                let Self {
                    #(#idents,)*
                } = self;

                #ident {
                    #(#idents,)*
                }
            }
        }
    }
}

pub fn build_condition_builder_struct(fields: &[(Ident, Type, Vec<DbAttr>)]) -> TokenStream {
    let crate_path = get_crate_path();

    let idents: Vec<_> = fields.iter().map(|x| &x.0).collect();
    let types: Vec<_> = fields.iter().map(|x| &x.1).collect();
    let names: Vec<_> = fields
        .iter()
        .map(|(ident, _ty, attrs)| parse_name(ident, attrs))
        .collect();

    quote! {
        #[derive(Default)]
        pub struct ConditionBuilder;

        impl ConditionBuilder {
            #(
                pub fn #idents (&self) -> #crate_path :: FieldConditionBuilder<#types> {
                    #crate_path :: FieldConditionBuilder::<#types>::new(#names)
                }
            )*
        }
    }
}

pub fn build_order_by_builder_struct(fields: &[(Ident, Type, Vec<DbAttr>)]) -> TokenStream {
    let crate_path = get_crate_path();

    let idents: Vec<_> = fields.iter().map(|x| &x.0).collect();
    let names: Vec<_> = fields
        .iter()
        .map(|(ident, _ty, attrs)| parse_name(ident, attrs))
        .collect();

    quote! {
        #[derive(Default)]
        pub struct OrderByBuilder;

        impl OrderByBuilder {
            #(
                pub fn #idents (&self, ordering: #crate_path :: Ordering) -> (String, #crate_path :: Ordering) {
                    (#names .into(), ordering)
                }
            )*
        }
    }
}
pub fn build_get_key_name(fields: &[(Ident, Type, Vec<DbAttr>)]) -> TokenStream {
    let idents: Vec<_> = fields.iter().map(|x| x.0.to_string()).collect();
    let names: Vec<_> = fields
        .iter()
        .map(|(ident, _ty, attrs)| parse_name(ident, attrs))
        .collect();

    quote! {
        match field {
            #(#idents => #names,)*
            _ => "",
        }
    }
}

pub fn build_macro_condition(condition: MacroCondition) -> TokenStream {
    match condition {
        MacroCondition::Between(path, expr1, expr2) => quote! {
            #path () .between(#expr1, #expr2)
        },
        MacroCondition::In(path, exprs) => quote! {
            #path () .in_(vec![#(#exprs),*])
        },
        MacroCondition::Like(path, expr) => quote! {
            #path () .like(#expr)
        },
        MacroCondition::Eq(path, expr) => quote! {
            #path () .eq(#expr)
        },
        MacroCondition::Neq(path, expr) => quote! {
            #path () .neq(#expr)
        },
        MacroCondition::Lt(path, expr) => quote! {
            #path () .lt(#expr)
        },
        MacroCondition::Lte(path, expr) => quote! {
            #path () .lte(#expr)
        },
        MacroCondition::Gt(path, expr) => quote! {
            #path () .gt(#expr)
        },
        MacroCondition::Gte(path, expr) => quote! {
            #path () .gte(#expr)
        },
    }
}

pub(crate) fn build_join_impl(
    ident: &Ident,
    fields: &[(Ident, Type, Vec<DbAttr>)],
    util_mod: &TokenStream,
) -> (TokenStream, TokenStream) {
    let crate_path = get_crate_path();

    let mut join_impl = quote! {};
    let mut mod_join_impl = quote! {};

    let Constraints { foreign_keys, .. } = parse_constraints(fields);

    let mut map = HashMap::<String, Vec<(String, Vec<NamedPair<ForeignKeyAttr>>)>>::new();
    for fk in foreign_keys {
        let (key_name, keys) = fk.into_inner();
        let Some(first_key) = keys.first() else {
            continue;
        };
        let entity = first_key.value().entity.to_token_stream().to_string();

        if let KeyName::String(name) = key_name {
            map.entry(entity).or_default().push((name, keys));
        }
    }

    for (entity, fks) in map {
        let entity_ident = parse_tt(&entity);

        let ident_fkb = parse_tt(&format!("{entity}FKB"));
        let ident_fk = parse_tt(&format!("{entity}FK"));

        let mut fields = Vec::with_capacity(fks.len());
        let mut variants = Vec::with_capacity(fks.len());
        let mut keys = Vec::with_capacity(fks.len());

        for (name, fk) in fks {
            fields.push(parse_tt(&name));
            variants.push(parse_tt(&ccase!(pascal, name)));
            let fk: Vec<_> = fk
                .into_iter()
                .map(|x| {
                    let key_from = x.name();
                    let key_to = x.value().key.to_string();

                    quote! {
                        (
                            #key_from .into(),
                            <#entity_ident as #crate_path :: Entity> :: get_key_name(
                                #key_to
                            ).into(),
                        )
                    }
                })
                .collect();
            keys.push(quote! {vec![#(#fk),*]});
        }

        mod_join_impl.extend(quote! {
            #[derive(PartialEq)]
            pub enum #ident_fk {
                #(#variants),*
            }

            pub struct #ident_fkb {
                #(pub #fields : #ident_fk),*
            }

            impl Default for #ident_fkb {
                fn default() -> Self {
                    Self {
                        #(#fields : #ident_fk :: #variants),*
                    }
                }
            }
        });

        join_impl.extend(quote! {
            impl #crate_path :: JoinBuilder <#entity_ident> for #ident {
                type FKB = #util_mod :: #ident_fkb;
                type FK = #util_mod :: #ident_fk;

                fn get_pairs(fk: Self::FK) -> Vec<(String, String)> {
                    match fk {
                        #(#util_mod :: #ident_fk :: #variants => #keys),*
                    }
                }
            }
        });
    }

    (join_impl, mod_join_impl)
}

pub fn build_util_mod(ident: &Ident) -> TokenStream {
    parse_tt(&format!(
        "{}_utils",
        ccase!(pascal -> snake, ident.to_string())
    ))
}
