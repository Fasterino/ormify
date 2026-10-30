use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

use crate::{FieldType, IsString, Value};

/// A composable, serializable database predicate.
///
/// Values are kept separately from generated SQL and should be bound by the
/// backend driver. `Raw` is an explicit trusted-SQL escape hatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Condition {
    Raw(String),
    Not(Box<Self>),
    And(Box<Self>, Box<Self>),
    Or(Box<Self>, Box<Self>),
    Between(String, Value, Value),
    In(String, Vec<Value>),
    Like(String, String),
    Eq(String, Value),
    Neq(String, Value),
    Lt(String, Value),
    Lte(String, Value),
    Gt(String, Value),
    Gte(String, Value),
}

impl Condition {
    /// Construct a raw predicate. Only use trusted SQL fragments.
    pub fn raw<I: Into<String>>(value: I) -> Self {
        Self::Raw(value.into())
    }

    /// Construct an inclusive SQL `BETWEEN` predicate.
    pub fn between<I1, I2, I3>(field: I1, from: I2, to: I3) -> Self
    where
        I1: Into<String>,
        I2: Into<Value>,
        I3: Into<Value>,
    {
        Self::Between(field.into(), from.into(), to.into())
    }

    /// Construct an SQL `IN` predicate.
    pub fn in_<I1, I2>(field: I1, values: Vec<I2>) -> Self
    where
        I1: Into<String>,
        I2: Into<Value>,
    {
        Self::In(field.into(), values.into_iter().map(Into::into).collect())
    }

    /// Construct a SQL `LIKE` predicate.
    pub fn like<I1, I2>(field: I1, str: I2) -> Self
    where
        I1: Into<String>,
        I2: Into<String>,
    {
        Self::Like(field.into(), str.into())
    }

    /// Construct an equality predicate.
    pub fn eq<I1, I2>(field: I1, value: I2) -> Self
    where
        I1: Into<String>,
        I2: Into<Value>,
    {
        Self::Eq(field.into(), value.into())
    }

    /// Construct a not-equal predicate.
    pub fn neq<I1, I2>(field: I1, value: I2) -> Self
    where
        I1: Into<String>,
        I2: Into<Value>,
    {
        Self::Neq(field.into(), value.into())
    }

    /// Construct a less-than predicate.
    pub fn lt<I1, I2>(field: I1, value: I2) -> Self
    where
        I1: Into<String>,
        I2: Into<Value>,
    {
        Self::Lt(field.into(), value.into())
    }

    /// Construct a less-than-or-equal predicate.
    pub fn lte<I1, I2>(field: I1, value: I2) -> Self
    where
        I1: Into<String>,
        I2: Into<Value>,
    {
        Self::Lte(field.into(), value.into())
    }

    /// Construct a greater-than predicate.
    pub fn gt<I1, I2>(field: I1, value: I2) -> Self
    where
        I1: Into<String>,
        I2: Into<Value>,
    {
        Self::Gt(field.into(), value.into())
    }

    /// Construct a greater-than-or-equal predicate.
    pub fn gte<I1, I2>(field: I1, value: I2) -> Self
    where
        I1: Into<String>,
        I2: Into<Value>,
    {
        Self::Gte(field.into(), value.into())
    }
}

impl FromIterator<(String, Value)> for Condition {
    fn from_iter<T: IntoIterator<Item = (String, Value)>>(iter: T) -> Self {
        let mut iter = iter.into_iter();
        let Some((key, value)) = iter.next() else {
            // An empty key set is the identity for a conjunction. Keep this
            // conversion total; callers that need a real predicate should
            // validate the collection before constructing a condition.
            return Condition::Raw("TRUE".to_owned());
        };

        let mut cond = Condition::Eq(key, value);

        for (key, value) in iter {
            cond = cond & Condition::Eq(key, value);
        }

        cond
    }
}
#[cfg(test)]
mod tests {
    use super::Condition;
    use crate::Value;

    #[test]
    fn empty_condition_collection_is_true() {
        let condition = std::iter::empty::<(String, Value)>().collect::<Condition>();
        assert!(matches!(condition, Condition::Raw(value) if value == "TRUE"));
    }
}

impl std::ops::Not for Condition {
    type Output = Self;

    fn not(self) -> Self {
        Self::Not(Box::new(self))
    }
}

impl std::ops::BitAnd for Condition {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self {
        Self::And(Box::new(self), Box::new(rhs))
    }
}

impl std::ops::BitOr for Condition {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self::Or(Box::new(self), Box::new(rhs))
    }
}

/// Type-constrained condition constructors for one entity field.
pub struct FieldConditionBuilder<T: FieldType> {
    name: String,
    _pd: PhantomData<T>,
}

impl<T: FieldType> FieldConditionBuilder<T> {
    pub fn new<I: Into<String>>(name: I) -> Self {
        Self {
            name: name.into(),
            _pd: Default::default(),
        }
    }

    pub fn between<I1, I2>(self, from: I1, to: I2) -> Condition
    where
        I1: Into<T>,
        I2: Into<T>,
    {
        Condition::between(self.name, from.into(), to.into())
    }

    pub fn in_<I>(self, values: Vec<I>) -> Condition
    where
        I: Into<T>,
    {
        Condition::in_(self.name, values.into_iter().map(Into::into).collect())
    }

    pub fn like<I>(self, str: I) -> Condition
    where
        T: IsString,
        I: Into<String>,
    {
        Condition::like(self.name, str)
    }

    pub fn eq<I>(self, value: I) -> Condition
    where
        I: Into<T>,
    {
        Condition::eq(self.name, value.into())
    }

    pub fn neq<I>(self, value: I) -> Condition
    where
        I: Into<T>,
    {
        Condition::neq(self.name, value.into())
    }

    pub fn lt<I>(self, value: I) -> Condition
    where
        I: Into<T>,
    {
        Condition::lt(self.name, value.into())
    }

    pub fn lte<I>(self, value: I) -> Condition
    where
        I: Into<T>,
    {
        Condition::lte(self.name, value.into())
    }

    pub fn gt<I>(self, value: I) -> Condition
    where
        I: Into<T>,
    {
        Condition::gt(self.name, value.into())
    }

    pub fn gte<I>(self, value: I) -> Condition
    where
        I: Into<T>,
    {
        Condition::gte(self.name, value.into())
    }
}

/// Query limit or offset value.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LimitLike(usize);

impl LimitLike {
    pub fn new(value: usize) -> Self {
        Self(value)
    }

    pub fn change_value(&mut self, value: usize) {
        self.0 = value;
    }

    pub fn value(&self) -> usize {
        self.0
    }
}

/// Direction for an order-by field.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Ordering {
    Ascending,
    Descending,
}

/// Ordered field/direction pairs used by a select query.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct OrderBy(Vec<(String, Ordering)>);

impl From<Vec<(String, Ordering)>> for OrderBy {
    fn from(value: Vec<(String, Ordering)>) -> Self {
        Self(value)
    }
}

impl OrderBy {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn to_sql(&self) -> String {
        self.0
            .iter()
            .map(|(field, ordering)| {
                let direction = match ordering {
                    Ordering::Ascending => "ASC",
                    Ordering::Descending => "DESC",
                };
                format!("\"{}\" {direction}", field.replace('"', "\"\""))
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn extend(&mut self, mut values: Vec<(String, Ordering)>) {
        self.0.append(&mut values);
    }
}
