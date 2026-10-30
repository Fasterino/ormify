use serde::{Deserialize, Serialize};

use crate::{Constraint, Error, FieldType, Type};

/// Database table metadata, including its optional PostgreSQL schema/group.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Scheme {
    pub group: Option<String>,
    pub name: String,
    pub fields: Vec<FieldScheme>,
    pub constraints: Vec<Constraint>,
}

impl Scheme {
    /// Start building a validated schema description.
    pub fn builder() -> SchemeBuilder {
        SchemeBuilder::default()
    }
}

#[derive(Default)]
/// Fluent builder for validating and constructing [`Scheme`] metadata.
pub struct SchemeBuilder {
    pub group: Option<String>,
    pub name: Option<String>,
    pub fields: Option<Vec<FieldScheme>>,
    pub constraints: Option<Vec<Constraint>>,
}

impl SchemeBuilder {
    /// Set the table name.
    pub fn name<I: Into<String>>(mut self, name: I) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the optional schema/group name.
    pub fn group<I: Into<String>>(mut self, group: I) -> Self {
        self.group = Some(group.into());
        self
    }

    /// Set all field definitions.
    pub fn fields<I: Into<Vec<FieldScheme>>>(mut self, fields: I) -> Self {
        self.fields = Some(fields.into());
        self
    }
    /// Set the table constraints; an empty collection is valid.
    pub fn constraints<I: Into<Vec<Constraint>>>(mut self, constraints: I) -> Self {
        self.constraints = Some(constraints.into());
        self
    }

    /// Validate and construct the schema.
    pub fn build(self) -> crate::Result<Scheme> {
        let Self {
            group,
            name,
            fields,
            constraints,
        } = self;

        let name = name
            .ok_or("Name isn't defined")
            .map_err(Error::make_build_error)?;
        if name.trim().is_empty() {
            return Err(Error::make_invalid_schema_error("Name cannot be empty"));
        }

        let fields = fields
            .ok_or("Fields aren't defined")
            .map_err(Error::make_build_error)?;
        if fields.is_empty() {
            return Err(Error::make_invalid_schema_error(
                "Scheme must define at least one field",
            ));
        }

        let constraints = constraints
            .ok_or("Constraints aren't defined")
            .map_err(Error::make_build_error)?;

        let mut field_names = std::collections::HashSet::with_capacity(fields.len());
        for field in &fields {
            if field.name.trim().is_empty() {
                return Err(Error::make_invalid_schema_error(
                    "Field names cannot be empty",
                ));
            }
            if !field_names.insert(&field.name) {
                return Err(Error::make_invalid_schema_error(format!(
                    "Duplicate field: {}",
                    field.name
                )));
            }
        }

        Ok(Scheme {
            group,
            name,
            fields,
            constraints,
        })
    }
}

/// Metadata for one database field.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FieldScheme {
    pub name: String,
    pub r#type: Type,
    pub nullable: bool,
}

impl FieldScheme {
    /// Start building field metadata.
    pub fn builder() -> FieldSchemeBuilder {
        FieldSchemeBuilder::default()
    }
}

#[derive(Default)]
/// Fluent builder for [`FieldScheme`] metadata.
pub struct FieldSchemeBuilder {
    pub name: Option<String>,
    pub r#type: Option<Type>,
    pub nullable: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::{FieldScheme, Scheme};
    use crate::Type;

    fn field(name: &str) -> FieldScheme {
        FieldScheme::builder()
            .name(name)
            .ty(Type::I32)
            .nullable(false)
            .build()
            .unwrap()
    }

    #[test]
    fn rejects_empty_scheme_name() {
        let result = Scheme::builder()
            .name(" ")
            .fields(vec![field("id")])
            .constraints(Vec::new())
            .build();
        assert!(matches!(result, Err(crate::Error::InvalidSchema(_))));
    }

    #[test]
    fn rejects_duplicate_fields() {
        let result = Scheme::builder()
            .name("users")
            .fields(vec![field("id"), field("id")])
            .constraints(Vec::new())
            .build();
        assert!(matches!(result, Err(crate::Error::InvalidSchema(_))));
    }
}

impl FieldSchemeBuilder {
    /// Set the database column name.
    pub fn name<I: Into<String>>(mut self, name: I) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set type and nullability from a field's [`FieldType`] implementation.
    pub fn ty_ext<T: FieldType>(self) -> Self {
        self.ty(T::get_type()).nullable(T::is_nullable())
    }

    /// Set the database type explicitly.
    pub fn ty(mut self, r#type: Type) -> Self {
        self.r#type = Some(r#type);
        self
    }

    /// Set whether this column accepts SQL `NULL`.
    pub fn nullable(mut self, nullable: bool) -> Self {
        self.nullable = Some(nullable);
        self
    }

    /// Validate and construct the field metadata.
    pub fn build(self) -> crate::Result<FieldScheme> {
        let Self {
            name,
            r#type,
            nullable,
        } = self;

        let name = name
            .ok_or("Name isn't defined")
            .map_err(Error::make_build_error)?;
        if name.trim().is_empty() {
            return Err(Error::make_invalid_schema_error("Field name cannot be empty"));
        }
        let r#type = r#type
            .ok_or("Type isn't defined")
            .map_err(Error::make_build_error)?;
        let nullable = nullable
            .ok_or("Nullable isn't defined")
            .map_err(Error::make_build_error)?;

        Ok(FieldScheme {
            name,
            r#type,
            nullable,
        })
    }
}
