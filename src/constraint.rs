use std::{fmt::Display, marker::PhantomData};

use serde::{Deserialize, Serialize};

use crate::{Entity, EntityId};

#[cfg(doc)]
use crate::Scheme;

/// A named database constraint attached to a [`Scheme`].
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Constraint {
    pub name: String,
    pub inner: ConstraintType,
    pub keys: Vec<String>,
}

/// The constraint kind and, for foreign keys, the referenced entity metadata.
/// Local columns are stored in [`Constraint::keys`]; referenced columns live
/// in the `keys` member of [`ConstraintType::ForeignKey`].
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ConstraintType {
    PrimaryKey,
    Unique,
    Index,
    ForeignKey {
        entity_id: EntityId,
        keys: Vec<String>,
    },
}

impl Constraint {
    /// Start a primary-key constraint for a table.
    pub fn primary_key<D: Display>(table_name: D) -> Self {
        Self {
            name: format!("pk_{}", table_name),
            inner: ConstraintType::PrimaryKey,
            keys: vec![],
        }
    }
    /// Start a unique constraint for a table and logical constraint name.
    pub fn unique<D1: Display, D2: Display>(table_name: D1, name: D2) -> Self {
        Self {
            name: format!("unique_{}_{}", table_name, name),
            inner: ConstraintType::Unique,
            keys: vec![],
        }
    }
    /// Start an index constraint for a table and logical constraint name.
    pub fn index<D1: Display, D2: Display>(table_name: D1, name: D2) -> Self {
        Self {
            name: format!("index_{}_{}", table_name, name),
            inner: ConstraintType::Index,
            keys: vec![],
        }
    }
    /// Start a foreign-key constraint referencing entity `T`.
    pub fn foreign_key_builder<D1: Display, D2: Display, T: Entity>(
        table_name: D1,
        name: D2,
        _entity: PhantomData<T>,
    ) -> ConstraintForeignKeyBuilder {
        ConstraintForeignKeyBuilder::new(format!("fk_{}_{}", table_name, name), T::entity_id())
    }

    /// Add a local key to the constraint.
    pub fn add_key<I: Into<String>>(mut self, key: I) -> Self {
        self.keys.push(key.into());
        self
    }
}

/// Builder for a foreign-key constraint from local fields to an entity's fields.
pub struct ConstraintForeignKeyBuilder {
    name: String,
    entity_id: EntityId,
    keys: Vec<String>,
}

impl ConstraintForeignKeyBuilder {
    fn new(name: String, entity_id: EntityId) -> Self {
        Self {
            name,
            entity_id,
            keys: vec![],
        }
    }

    /// Add a referenced target key. The local key is added to the resulting
    /// [`Constraint`] separately through [`Constraint::add_key`].
    pub fn add_key<I: Into<String>>(mut self, key: I) -> Self {
        self.keys.push(key.into());
        self
    }

    /// Finish the foreign-key constraint definition.
    pub fn build(self) -> Constraint {
        let Self {
            name,
            entity_id,
            keys,
        } = self;
        Constraint {
            name,
            inner: ConstraintType::ForeignKey { entity_id, keys },
            keys: vec![],
        }
    }
}
