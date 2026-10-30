use std::marker::PhantomData;

use crate::{FieldType, VarChar};

/// Marks input values accepted by generated entity builder setters.
///
/// This is used to support ergonomic conversions such as `&str` into
/// `VarChar<N>` while retaining the field's concrete ORM type.
pub trait DbValue<T: FieldType>: Into<T> {}

pub fn db_value<T1: DbValue<T2>, T2: FieldType>(value: T1) -> T2 {
    value.into()
}

impl<const N: usize> DbValue<VarChar<N>> for &str {}
impl<const N: usize> DbValue<VarChar<N>> for String {}

pub fn pd<T>() -> PhantomData<T> {
    Default::default()
}

/// Encode a serializable value as CBOR bytes.
pub fn cbor_encode<T: serde::Serialize>(value: &T) -> crate::Result<Vec<u8>> {
    serde_cbor::to_vec(value).map_err(|e| crate::Error::make_serialization_error(e.to_string()))
}

/// Decode a value previously encoded with [`cbor_encode`].
pub fn cbor_decode<T: serde::de::DeserializeOwned>(data: &[u8]) -> crate::Result<T> {
    serde_cbor::from_slice(data).map_err(|e| crate::Error::make_serialization_error(e.to_string()))
}
