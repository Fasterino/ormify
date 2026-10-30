use serde::{Deserialize, Serialize};

use crate::{Execution, ExecutionResponse, Query, QueryResponse, Uuid};

/// Serializable request envelope for dispatching a query or transaction.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Request {
    id: Uuid,
    inner: RequestInner,
}

impl Request {
    /// Create an envelope with a caller-chosen correlation ID.
    pub fn new(id: Uuid, inner: RequestInner) -> Self {
        Self { id, inner }
    }

    /// Return the correlation ID.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Borrow the carried operation.
    pub fn inner(&self) -> &RequestInner {
        &self.inner
    }

    pub(crate) fn into_inner(self) -> (Uuid, RequestInner) {
        (self.id, self.inner)
    }
}

/// Operation carried by a [`Request`].
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum RequestInner {
    Query(Query),
    Transaction(Vec<Execution>),
}

/// Serializable response envelope paired with a request ID.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Response {
    id: Uuid,
    inner: ResponseInner,
}

impl Response {
    /// Pair a response result with its originating correlation ID.
    pub fn new(id: Uuid, inner: ResponseInner) -> Self {
        Self { id, inner }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn inner(&self) -> &ResponseInner {
        &self.inner
    }

    pub fn into_inner(self) -> (Uuid, ResponseInner) {
        (self.id, self.inner)
    }
}

/// Result carried by a [`Response`].
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ResponseInner {
    Query(crate::Result<QueryResponse>),
    Execution(crate::Result<ExecutionResponse>),
}
