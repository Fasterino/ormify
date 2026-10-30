use std::{fmt::Debug, ops::Deref};

use serde::{Deserialize, Serialize};

use crate::Error;

/// UTC timestamp value used by ORM fields and PostgreSQL `TIMESTAMPTZ` columns.
///
/// Serialization uses Unix milliseconds; database drivers use the wrapped
/// `chrono::DateTime` when binding values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnixTimeStamp(pub chrono::DateTime<chrono::Utc>);

impl UnixTimeStamp {
    /// Construct a timestamp representing the current UTC time.
    pub fn new() -> Self {
        Default::default()
    }

    /// Construct from milliseconds since the Unix epoch, if representable.
    pub fn from_timestamp_millis(millis: i64) -> Option<Self> {
        chrono::DateTime::from_timestamp_millis(millis).map(Self)
    }
}

impl Default for UnixTimeStamp {
    fn default() -> Self {
        Self(chrono::Utc::now())
    }
}

impl Deref for UnixTimeStamp {
    type Target = chrono::DateTime<chrono::Utc>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Serialize for UnixTimeStamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Serialize::serialize(&self.timestamp_millis(), serializer)
    }
}

impl<'de> Deserialize<'de> for UnixTimeStamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Self::from_timestamp_millis(Deserialize::deserialize(deserializer)?)
            .ok_or_else(|| serde::de::Error::custom("Cannot parse timestamp"))
    }
}

/// UUID value used by ORM entities and database drivers.
///
/// Its serde representation is the UUID's `u128` value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Uuid(pub uuid::Uuid);

impl Uuid {
    /// Generate a random version-4 UUID.
    pub fn new() -> Self {
        Default::default()
    }

    /// Construct from the 128-bit numeric representation.
    pub fn from_u128(v: u128) -> Self {
        Self(uuid::Uuid::from_u128(v))
    }
}

impl Deref for Uuid {
    type Target = uuid::Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for Uuid {
    fn default() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Serialize for Uuid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Serialize::serialize(&self.as_u128(), serializer)
    }
}

impl<'de> Deserialize<'de> for Uuid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self::from_u128(Deserialize::deserialize(deserializer)?))
    }
}

/// A UTF-8 string limited to `N` Unicode scalar values.
///
/// `VarChar<0>` is used for values decoded from an unsized database string.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct VarChar<const N: usize>(String);

impl<const N: usize> VarChar<N> {
    /// Borrow the contained string.
    pub fn get_ref(&self) -> &str {
        &self.0
    }
}

impl VarChar<0> {
    pub fn new_unsized(value: String) -> Self {
        Self(value)
    }
}

impl<const N: usize> From<String> for VarChar<N> {
    fn from(value: String) -> Self {
        // `N` is a character limit, not a byte offset. Using `String::truncate`
        // directly can panic when it splits a multi-byte UTF-8 character.
        let value = value.chars().take(N).collect();
        Self(value)
    }
}

impl<const N: usize> From<&str> for VarChar<N> {
    fn from(value: &str) -> Self {
        value.to_owned().into()
    }
}

impl<const N: usize> ToString for VarChar<N> {
    fn to_string(&self) -> String {
        self.0.clone()
    }
}

/// Database field categories understood by the language and driver layers.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum Type {
    Bool,
    I16,
    I32,
    I64,
    F32,
    F64,
    Uuid,
    UnixTimeStamp,
    VarChar(usize),
    Blob,
}

/// Transport-neutral database value passed between the engine and a driver.
///
/// Values must be bound as parameters by drivers; they should not be
/// interpolated into SQL strings.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub enum Value {
    #[default]
    Null,
    Bool(bool),
    I16(i16),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    Uuid(Uuid),
    UnixTimeStamp(UnixTimeStamp),
    VarChar(String),
    Blob(Vec<u8>),
}

/// Converts an application field type to and from [`Value`] and [`Type`].
///
/// Derive `FieldType` for application-specific enums and wrappers.
pub trait FieldType: Sized + Clone + Default + Debug {
    type OriginalType;

    fn get_type() -> Type;
    fn to_value(self) -> Value;
    fn try_from_value(value: Value) -> crate::Result<Self>;

    fn is_nullable() -> bool {
        false
    }
}

impl<T: FieldType> From<T> for Value {
    fn from(value: T) -> Self {
        value.to_value()
    }
}

pub trait IsString {}

impl<const N: usize> IsString for VarChar<N> {}
impl<const N: usize> IsString for Option<VarChar<N>> {}

macro_rules! impl_field_type {
    ($($path:path => $ident:ident),*$(,)?) => {
        $(
            impl FieldType for $path {
                type OriginalType = $path;
                fn get_type() -> Type {
                    Type::$ident
                }
                fn to_value(self) -> Value {
                    Value::$ident(self)
                }
                fn try_from_value(value: Value) -> crate::Result<Self> {
                    match value {
                        Value::$ident(value) => Ok(value),
                        _ => Err(Error::TypeMismatch),
                    }
                }
            }
        )*
    };
}

impl_field_type! {
    bool => Bool,
    i16 => I16,
    i32 => I32,
    i64 => I64,
    f32 => F32,
    f64 => F64,
    Uuid => Uuid,
    UnixTimeStamp => UnixTimeStamp,
    Vec<u8> => Blob,
}

impl<const N: usize> FieldType for VarChar<N> {
    type OriginalType = VarChar<N>;
    fn get_type() -> Type {
        Type::VarChar(N)
    }
    fn to_value(self) -> Value {
        Value::VarChar(self.to_string())
    }
    fn try_from_value(value: Value) -> crate::Result<Self> {
        match value {
            Value::VarChar(value) => Ok(value.into()),
            _ => Err(Error::TypeMismatch),
        }
    }
}

impl<T: FieldType> FieldType for Option<T> {
    type OriginalType = T;
    fn get_type() -> Type {
        T::get_type()
    }
    fn to_value(self) -> Value {
        if let Some(value) = self {
            value.to_value()
        } else {
            Value::Null
        }
    }
    fn try_from_value(value: Value) -> crate::Result<Self> {
        match value {
            Value::Null => Ok(None),
            value => T::try_from_value(value).map(|x| Some(x)),
        }
    }
    fn is_nullable() -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::VarChar;

    #[test]
    fn varchar_limit_is_utf8_safe() {
        let value: VarChar<1> = "é".into();
        assert_eq!(value.get_ref(), "é");
    }

    #[test]
    fn varchar_limit_counts_characters() {
        let value: VarChar<2> = "абв".into();
        assert_eq!(value.get_ref(), "аб");
    }
}
