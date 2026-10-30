use crate::attr::{
    AttrParser, DbAttr, FieldTypeAttr, ForeignKeyAttr, IndexAttr, PrimaryKeyAttr, RenameAttr,
    UniqueAttr,
};
use convert_case::ccase;
use proc_macro2::TokenStream;
use quote::{ToTokens, TokenStreamExt, quote};
use syn::{
    Attribute, Error, Expr, Ident, PatRange, RangeLimits, Token, Type,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

pub enum MacroCondition {
    Between(TokenStream, Expr, Expr), // ident in [expr1..=expr2]
    In(TokenStream, Vec<Expr>),       // ident in [expr1, expr2, .., exprN]
    Like(TokenStream, Expr),          // ident =? expr
    Eq(TokenStream, Expr),            // ident == expr
    Neq(TokenStream, Expr),           // ident != expr
    Lt(TokenStream, Expr),            // ident < expr
    Lte(TokenStream, Expr),           // ident <= expr
    Gt(TokenStream, Expr),            // ident > expr
    Gte(TokenStream, Expr),
}

mod custom_tokens {
    syn::custom_punctuation!(EqQuestion, =?);
}

impl Parse for MacroCondition {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident1: Ident = input.parse()?;
        input.parse::<Token![.]>()?;
        let ident2: Ident = input.parse()?;
        let lhs = quote! {#ident1 . #ident2};

        if input.peek(Token![in]) {
            input.parse::<Token![in]>()?;

            let content;
            syn::bracketed!(content in input);

            if content.fork().parse::<PatRange>().is_ok() {
                let expr_range: PatRange = content.parse()?;

                let RangeLimits::Closed(_) = expr_range.limits else {
                    return Err(content.error("Range must be with ..= delimeter"));
                };
                let start = expr_range
                    .start
                    .ok_or_else(|| content.error("Left boundary of the range was expected."))?;
                let end = expr_range
                    .end
                    .ok_or_else(|| content.error("Right boundary of the range was expected."))?;

                Ok(MacroCondition::Between(lhs, *start, *end))
            } else {
                let exprs: Punctuated<Expr, Token![,]> =
                    content.parse_terminated(Expr::parse, Token![,])?;
                Ok(MacroCondition::In(lhs, exprs.into_iter().collect()))
            }
        } else if input.peek(custom_tokens::EqQuestion) {
            input.parse::<custom_tokens::EqQuestion>()?;
            let expr: Expr = input.parse()?;
            Ok(MacroCondition::Like(lhs, expr))
        } else if input.peek(Token![==]) {
            input.parse::<Token![==]>()?;
            let expr: Expr = input.parse()?;
            Ok(MacroCondition::Eq(lhs, expr))
        } else if input.peek(Token![!=]) {
            input.parse::<Token![!=]>()?;
            let expr: Expr = input.parse()?;
            Ok(MacroCondition::Neq(lhs, expr))
        } else if input.peek(Token![<=]) {
            input.parse::<Token![<=]>()?;
            let expr: Expr = input.parse()?;
            Ok(MacroCondition::Lte(lhs, expr))
        } else if input.peek(Token![>=]) {
            input.parse::<Token![>=]>()?;
            let expr: Expr = input.parse()?;
            Ok(MacroCondition::Gte(lhs, expr))
        } else if input.peek(Token![<]) {
            input.parse::<Token![<]>()?;
            let expr: Expr = input.parse()?;
            Ok(MacroCondition::Lt(lhs, expr))
        } else if input.peek(Token![>]) {
            input.parse::<Token![>]>()?;
            let expr: Expr = input.parse()?;
            Ok(MacroCondition::Gt(lhs, expr))
        } else {
            Err(input.error("Comparison operator, `in` or `=?` was expected"))
        }
    }
}

pub struct NamedPair<T>(TokenStream, T);

impl<T> NamedPair<T> {
    pub fn name(&self) -> TokenStream {
        self.0.clone()
    }

    pub fn value(&self) -> &T {
        &self.1
    }
}

pub enum KeyName {
    TokenStream(TokenStream),
    String(String),
}

impl KeyName {
    pub fn new(token_stream: &TokenStream, name_opt: Option<String>) -> Self {
        if let Some(name) = name_opt {
            Self::String(name)
        } else {
            Self::TokenStream(token_stream.clone())
        }
    }
}

impl ToTokens for KeyName {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.append_all(match self {
            KeyName::TokenStream(token_stream) => token_stream.clone(),
            KeyName::String(name) => name.to_token_stream(),
        });
    }
}

impl PartialEq for KeyName {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::String(l), Self::String(r)) => l == r,
            _ => false,
        }
    }
}
pub struct NamedVec<T>(KeyName, Vec<T>);

impl<T> NamedVec<T> {
    pub fn into_inner(self) -> (KeyName, Vec<T>) {
        (self.0, self.1)
    }
}

pub struct Constraints {
    pub primary_keys: Vec<NamedPair<PrimaryKeyAttr>>,
    pub foreign_keys: Vec<NamedVec<NamedPair<ForeignKeyAttr>>>,
    pub unique: Vec<NamedVec<NamedPair<UniqueAttr>>>,
    pub indexes: Vec<NamedVec<NamedPair<IndexAttr>>>,
}

pub fn parse_tt(str: &str) -> TokenStream {
    str.parse().expect("Failed to parse TokenTree")
}

pub fn parse_db_attrs(attrs: &[Attribute]) -> syn::Result<Vec<DbAttr>> {
    let mut data = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("db") {
            continue;
        }

        let mut attrs: AttrParser<DbAttr> = attr.parse_args()?;

        data.append(attrs.inner_mut());
    }

    Ok(data)
}

pub fn parse_ft_attr(ident: &Ident, attrs: &[Attribute]) -> syn::Result<FieldTypeAttr> {
    for attr in attrs {
        if !attr.path().is_ident("ft") {
            continue;
        }

        return attr.parse_args();
    }

    Err(Error::new_spanned(ident, "\"ft\" attribute is not present"))
}

pub fn parse_primary_key_fields(fields: &[(Ident, Type, Vec<DbAttr>)]) -> Vec<TokenStream> {
    fields
        .iter()
        .filter_map(|(ident, _ty, attrs)| {
            if let Some(PrimaryKeyAttr) = DbAttr::query::<PrimaryKeyAttr>(attrs) {
                Some(parse_name(ident, attrs))
            } else {
                None
            }
        })
        .collect()
}

macro_rules! constraint_helper {
    ($name:ident, $attrs:ident, $ident:ident, $ty:ty) => {
        if let Some(mut constraint) = DbAttr::query::<$ty>($attrs) {
            let key_name = KeyName::new(&$name, constraint.name.take());
            let pair = NamedPair($name.clone(), constraint);

            if let Some(named_vec) = $ident
                .iter_mut()
                .find(|NamedVec(name, _)| *name == key_name)
            {
                named_vec.1.push(pair);
            } else {
                $ident.push(NamedVec(key_name, vec![pair]));
            }
        }
    };
}

pub fn parse_constraints(fields: &[(Ident, Type, Vec<DbAttr>)]) -> Constraints {
    let mut primary_keys = Vec::new();
    let mut foreign_keys = Vec::new();
    let mut unique = Vec::new();
    let mut indexes = Vec::new();

    for (ident, _ty, attrs) in fields {
        let name = parse_name(ident, attrs);

        if let Some(constraint) = DbAttr::query::<PrimaryKeyAttr>(attrs) {
            primary_keys.push(NamedPair(name.clone(), constraint));
        }

        if let Some(mut constraint) = DbAttr::query::<ForeignKeyAttr>(attrs) {
            let key_name = KeyName::String(constraint.name.take().unwrap_or_else(|| {
                ccase!(
                    snake,
                    constraint
                        .entity
                        .segments
                        .last()
                        .expect("Error to parse foreign key")
                        .ident
                        .to_string()
                )
            }));
            let pair = NamedPair(name.clone(), constraint);

            if let Some(named_vec) = foreign_keys
                .iter_mut()
                .find(|NamedVec(name, _)| *name == key_name)
            {
                named_vec.1.push(pair);
            } else {
                foreign_keys.push(NamedVec(key_name, vec![pair]));
            }
        }
        constraint_helper!(name, attrs, unique, UniqueAttr);
        constraint_helper!(name, attrs, indexes, IndexAttr);
    }

    Constraints {
        primary_keys,
        foreign_keys,
        unique,
        indexes,
    }
}

pub fn parse_name(ident: &Ident, attrs: &[DbAttr]) -> TokenStream {
    if let Some(rename) = DbAttr::query::<RenameAttr>(attrs) {
        rename.name
    } else {
        ccase!(snake, ident.to_string()).to_token_stream()
    }
}
