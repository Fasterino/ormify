use std::vec::IntoIter;

use serde::{Deserialize, Serialize};

use crate::{Condition, Entity, EntityId, Entry, LimitLike, OrderBy, Ordering, Scheme};

/// A schema initialization, insert, update, or delete operation.
///
/// For initialization with related entities, submit all relevant `Init`
/// operations together so the engine can initialize foreign-key targets first.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Execution {
    /// Register the entity schema and create its database table.
    Init {
        entity_id: EntityId,
        scheme: Scheme,
    },
    /// Insert one serialized entity row.
    Insert {
        entity_id: EntityId,
        entry: Entry,
    },
    /// Update the fields in `entry` for rows matching `condition`.
    Update {
        entity_id: EntityId,
        entry: Entry,
        condition: Condition,
    },
    /// Delete rows matching `condition`.
    Delete {
        entity_id: EntityId,
        condition: Condition,
    },
}

/// Aggregate number of rows affected by a transaction.
pub type ExecutionResponse = u64;

/// A select request identified by an entity and its filters.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Query {
    pub(crate) entity_id: EntityId,
    pub(crate) filters: Filters,
}

/// A list of transport-neutral rows, each paired with one nested response
/// per declared join.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct QueryResponse(pub(crate) Vec<(Entry, Vec<QueryResponse>)>);

impl IntoIterator for QueryResponse {
    type Item = (Entry, Vec<QueryResponse>);

    type IntoIter = IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl QueryResponse {
    /// Construct an empty response.
    pub fn empty() -> Self {
        Self(Default::default())
    }

    /// Return the number of rows in this response.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Borrow the first row and its nested responses, if present.
    pub fn first(&self) -> Option<&(Entry, Vec<QueryResponse>)> {
        self.0.first()
    }

    /// Consume the response and return its first row, if present.
    pub fn into_first(self) -> Option<(Entry, Vec<QueryResponse>)> {
        self.0.into_iter().next()
    }

    /// Return whether this response contains no rows.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterate over rows and their nested responses without consuming them.
    pub fn iter(&self) -> impl Iterator<Item = &(Entry, Vec<QueryResponse>)> {
        self.0.iter()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
/// Filtering, pagination, ordering, and joins associated with a [`Query`].
pub struct Filters {
    pub(crate) condition: Option<Condition>,
    pub(crate) limit: LimitLike,
    pub(crate) offset: LimitLike,
    pub(crate) order_by: OrderBy,
    pub(crate) joins: Vec<Join>,
}

/// Fluent builder for a typed `SELECT` query.
pub struct SelectBuilder<E: Entity + Sized> {
    condition_builder: E::ConditionBuilder,
    condition: Option<Condition>,
    order_by_builder: E::OrderByBuilder,
    order_by: OrderBy,
    limit: LimitLike,
    offset: LimitLike,
    joins: Vec<Join>,
}

/// A nested select for a related entity and its foreign-key pair mapping.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Join {
    pub(crate) entity_id: EntityId,
    pub(crate) filters: Filters,
    pub(crate) key_pairs: Vec<(String, String)>,
}

impl<E: Entity + Sized> SelectBuilder<E> {
    pub fn new() -> Self {
        Self {
            condition_builder: Default::default(),
            condition: Default::default(),
            order_by_builder: Default::default(),
            order_by: Default::default(),
            limit: Default::default(),
            offset: Default::default(),
            joins: Default::default(),
        }
    }

    /// Add a predicate; repeated calls are combined with logical AND.
    pub fn where_<F: FnOnce(&E::ConditionBuilder) -> Condition>(mut self, f: F) -> Self {
        let condition = f(&self.condition_builder);
        if let Some(old_condition) = self.condition {
            self.condition = Some(old_condition & condition);
        } else {
            self.condition = Some(condition);
        }
        self
    }
    /// Append one or more ordering expressions.
    pub fn order_by<F: FnOnce(&E::OrderByBuilder) -> Vec<(String, Ordering)>>(
        mut self,
        f: F,
    ) -> Self {
        self.order_by.extend(f(&self.order_by_builder));
        self
    }
    /// Limit the maximum number of root rows returned.
    pub fn limit(mut self, value: usize) -> Self {
        self.limit.change_value(value);
        self
    }
    /// Skip this many root rows before returning results.
    pub fn offset(mut self, value: usize) -> Self {
        self.offset.change_value(value);
        self
    }
    /// Add a typed relationship join. Child filters are configured in `f`.
    pub fn join<T: JoinBuilder<E>>(
        mut self,
        f: impl FnOnce(T::FKB, SelectBuilder<T>) -> (T::FK, SelectBuilder<T>),
    ) -> Self {
        let (fk, s) = f(Default::default(), SelectBuilder::new());
        self.joins.push(T::build(fk, s));
        self
    }

    /// Consume the builder and create a [`Query`].
    pub fn build_query(self) -> Query {
        let filters = self.build_filters();

        let entity_id = E::entity_id();

        Query { entity_id, filters }
    }

    pub fn build_filters(self) -> Filters {
        let Self {
            condition,
            order_by,
            limit,
            offset,
            joins: join,
            ..
        } = self;

        Filters {
            condition,
            order_by,
            limit,
            offset,
            joins: join,
        }
    }
}

impl<E: Entity> Into<Query> for SelectBuilder<E> {
    fn into(self) -> Query {
        self.build_query()
    }
}

/// Generated relationship metadata connecting this entity to parent `T`.
pub trait JoinBuilder<T: Entity>: Entity + Sized {
    type FKB: Default;
    type FK: Sized + PartialEq;

    /// Return `(child_field, parent_field)` names for the selected FK relation.
    fn get_pairs(fk: Self::FK) -> Vec<(String, String)>;

    fn build(fk: Self::FK, s: SelectBuilder<Self>) -> Join {
        let entity_id = Self::entity_id();
        let filters = s.build_filters();
        let key_pairs = Self::get_pairs(fk);

        Join {
            entity_id,
            filters,
            key_pairs,
        }
    }
}
