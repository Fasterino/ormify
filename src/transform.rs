use crate::{EntityLike, Error, QueryResponse};

/// Converts a transport-level value into a typed entity, optional entity, or collection.
///
/// The trait is implemented for `T`, `Option<T>`, and `Vec<T>` where `T`
/// implements [`EntityLike`]. For nested joins, use `Joined` as `T` and select
/// a child tuple shape matching the query's join declarations.
pub trait Transform<From: Sized>: Sized {
    /// Convert `value` into the requested application representation.
    fn transform(value: From) -> crate::Result<Self>;
}

impl<T: EntityLike> Transform<QueryResponse> for Vec<T> {
    /// Transform every response row into an entity. An empty response becomes
    /// an empty vector.
    fn transform(data: QueryResponse) -> crate::Result<Self> {
        let mut res = Vec::with_capacity(data.len());

        for (entry, sub_entries) in data {
            res.push(T::from_data(entry, sub_entries)?);
        }

        Ok(res)
    }
}

impl<T: EntityLike> Transform<QueryResponse> for Option<T> {
    /// Transform the first row, returning `None` for an empty response.
    fn transform(data: QueryResponse) -> crate::Result<Self> {
        if let Some((entry, sub_entries)) = data.into_first() {
            T::from_data(entry, sub_entries).map(Some)
        } else {
            Ok(None)
        }
    }
}

impl<T: EntityLike> Transform<QueryResponse> for T {
    /// Transform the first row or return [`Error::EmptyResponse`] if none exists.
    fn transform(data: QueryResponse) -> crate::Result<Self> {
        if let Some((entry, sub_entries)) = data.into_first() {
            T::from_data(entry, sub_entries)
        } else {
            Err(Error::EmptyResponse)
        }
    }
}
