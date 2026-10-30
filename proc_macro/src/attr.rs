use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use strum::AsRefStr;
use syn::{
    Ident, LitInt, Path, Token,
    parse::{Parse, ParseStream},
};

#[derive(Clone)]
pub struct GroupAttr {
    pub name: proc_macro2::TokenStream,
}

#[derive(Clone)]
pub struct RenameAttr {
    pub name: proc_macro2::TokenStream,
}

#[derive(Clone)]
pub struct PrimaryKeyAttr;

#[derive(Clone)]
pub struct ForeignKeyAttr {
    pub name: Option<String>,
    pub entity: Path,
    pub key: Ident,
}

#[derive(Clone)]
pub struct IndexAttr {
    pub name: Option<String>,
}

#[derive(Clone)]
pub struct UniqueAttr {
    pub name: Option<String>,
}

#[derive(Clone)]
pub struct UpdateAttr {
    pub func: Path,
}

#[derive(AsRefStr)]
pub enum DbAttr {
    Group(GroupAttr),
    Rename(RenameAttr),
    PrimaryKey(PrimaryKeyAttr),
    ForeignKey(ForeignKeyAttr),
    Index(IndexAttr),
    Unique(UniqueAttr),
    Update(UpdateAttr),
}

impl DbAttr {
    pub fn query<T>(attrs: &[DbAttr]) -> Option<T>
    where
        DbAttr: SetAttr<T>,
    {
        let mut res = None;

        for attr in attrs {
            attr.set_attr(&mut res);
        }

        res
    }
}

macro_rules! set_attr {
    ($($from:ident => $into:ident),* $(,)?) => {
        $(
            impl SetAttr<$into> for DbAttr {
                fn set_attr(& self, opt: &mut Option<$into>) {
                    match self {
                        DbAttr::$from(value) => opt.replace(value.clone()),
                        _ => return,
                    };
                }
            }
        )*
    };
}

pub trait SetAttr<T> {
    fn set_attr(&self, opt: &mut Option<T>);
}

set_attr! {
    Group => GroupAttr,
    Rename => RenameAttr,
    PrimaryKey => PrimaryKeyAttr,
    ForeignKey => ForeignKeyAttr,
    Index => IndexAttr,
    Unique => UniqueAttr,
    Update => UpdateAttr,
}

impl Parse for DbAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: syn::Ident = input.parse()?;

        match ident.to_string().as_str() {
            "group" => {
                input.parse::<Token![=]>()?;
                let name = input.parse()?;
                Ok(Self::Group(GroupAttr { name }))
            }
            "rename" => {
                input.parse::<Token![=]>()?;
                let name = input.parse()?;
                Ok(Self::Rename(RenameAttr { name }))
            }
            "pk" => Ok(Self::PrimaryKey(PrimaryKeyAttr)),
            "fk" => {
                let name = if input.peek(Token![:]) {
                    input.parse::<Token![:]>()?;
                    Some(input.parse::<Ident>()?.to_string())
                } else {
                    None
                };
                input.parse::<Token![->]>()?;
                let key = input.parse()?;
                input.parse::<Token![in]>()?;
                let entity = input.parse()?;
                Ok(Self::ForeignKey(ForeignKeyAttr { name, entity, key }))
            }
            "index" => {
                let name = if input.peek(Token![:]) {
                    input.parse::<Token![:]>()?;
                    Some(input.parse::<Ident>()?.to_string())
                } else {
                    None
                };
                Ok(Self::Index(IndexAttr { name }))
            }
            "unique" => {
                let name = if input.peek(Token![:]) {
                    input.parse::<Token![:]>()?;
                    Some(input.parse::<Ident>()?.to_string())
                } else {
                    None
                };
                Ok(Self::Unique(UniqueAttr { name }))
            }
            "update" => {
                input.parse::<Token![=]>()?;
                let func = input.parse()?;
                Ok(Self::Update(UpdateAttr { func }))
            }
            _ => Err(syn::Error::new(
                ident.span(),
                format!("Unknown constraint type: {ident}"),
            )),
        }
    }
}

pub struct AttrParser<T: Parse>(Vec<T>);

impl<T: Parse> AttrParser<T> {
    pub fn inner_mut(&mut self) -> &mut Vec<T> {
        &mut self.0
    }
}

impl<T: Parse> Parse for AttrParser<T> {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut attrs = Vec::new();
        loop {
            attrs.push(T::parse(input)?);

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            } else {
                break;
            }
        }
        Ok(Self(attrs))
    }
}

#[derive(Clone)]
pub enum FieldTypeAttr {
    I16,
    I32,
    I64,
    VarChar(LitInt),
    Blob,
}
impl FieldTypeAttr {
    pub fn get_type_and_value_variant(&self) -> (TokenStream, TokenStream) {
        match self {
            FieldTypeAttr::I16 => (quote! {i16}, quote! {I16}),
            FieldTypeAttr::I32 => (quote! {i32}, quote! {I32}),
            FieldTypeAttr::I64 => (quote! {i64}, quote! {I64}),
            FieldTypeAttr::VarChar(_) => (quote! {todo!()}, quote! {todo!()}),
            FieldTypeAttr::Blob => (quote! {todo!()}, quote! {todo!()}),
        }
    }
}

impl Parse for FieldTypeAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Проверяем, что идет на входе: если это VarChar, то за ним будут круглые скобки.
        // Поэтому для VarChar парсим сначала только имя (Ident).
        // Для всех остальных типов (включая Vec<u8>) парсим как полный путь (Path).

        let path: Path = input.parse()?;

        // Пытаемся получить простой идентификатор (работает для u32, i32 и т.д.)
        if let Some(ident) = path.get_ident() {
            let ident_str = ident.to_string();
            match ident_str.as_str() {
                "i16" => return Ok(Self::I16),
                "i32" => return Ok(Self::I32),
                "i64" => return Ok(Self::I64),
                "VarChar" => {
                    let content;
                    syn::parenthesized!(content in input);
                    let len_lit: LitInt = content.parse()?;
                    return Ok(Self::VarChar(len_lit));
                }
                _ => {}
            }
        }

        let path_str: String = path
            .segments
            .iter()
            .map(|seg| {
                let ident = seg.ident.to_string();
                if seg.arguments.is_empty() {
                    ident
                } else {
                    // Преобразуем аргументы (например, <u8>) в строку
                    let args = seg.arguments.to_token_stream().to_string();
                    format!("{}{}", ident, args)
                }
            })
            .collect::<Vec<_>>()
            .join("::")
            .replace(" ", "");

        if path_str == "Vec<u8>" || path_str == "std::vec::Vec<u8>" {
            return Ok(Self::Blob);
        }

        Err(syn::Error::new_spanned(path, "Unsupported type"))
    }
}
