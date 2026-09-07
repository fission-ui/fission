use anyhow::{bail, Result};
use std::collections::BTreeMap;

/// Cardinality and browser-value policy for a submitted server form field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerFormFieldKind {
    /// Zero or one UTF-8 text value.
    Text,
    /// An HTML checkbox value; absence is normalized to `false`.
    Boolean,
    /// Zero or more UTF-8 values with the same field name.
    Repeated,
}

/// One explicitly allowed field in a typed server action form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerFormField {
    pub(crate) name: String,
    pub(crate) kind: ServerFormFieldKind,
    pub(crate) required: bool,
    pub(crate) max_length: Option<usize>,
}

impl ServerFormField {
    /// Declares a scalar UTF-8 text field.
    pub fn text(name: impl Into<String>) -> Self {
        Self::new(name, ServerFormFieldKind::Text)
    }

    /// Declares a checkbox field. An absent successful control becomes `false`.
    pub fn boolean(name: impl Into<String>) -> Self {
        Self::new(name, ServerFormFieldKind::Boolean)
    }

    /// Declares a field that deliberately accepts repeated values.
    pub fn repeated(name: impl Into<String>) -> Self {
        Self::new(name, ServerFormFieldKind::Repeated)
    }

    fn new(name: impl Into<String>, kind: ServerFormFieldKind) -> Self {
        let name = name.into();
        assert!(!name.is_empty(), "server form field names cannot be empty");
        assert_ne!(name, "token", "`token` is reserved by server action forms");
        Self {
            name,
            kind,
            required: false,
            max_length: None,
        }
    }

    /// Requires at least one non-empty submitted value.
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Limits each submitted value by Unicode scalar count.
    pub fn max_length(mut self, max_length: usize) -> Self {
        self.max_length = Some(max_length);
        self
    }
}

/// Explicit allow-list and cardinality schema for one server action form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerFormSchema {
    pub(crate) id: String,
    fields: BTreeMap<String, ServerFormField>,
}

impl ServerFormSchema {
    /// Creates a schema matching the logical form id assigned to its controls
    /// and submit button.
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        assert!(!id.is_empty(), "server form ids cannot be empty");
        Self {
            id,
            fields: BTreeMap::new(),
        }
    }

    /// Adds one allowed field. Duplicate declarations are rejected immediately.
    pub fn field(mut self, field: ServerFormField) -> Self {
        let name = field.name.clone();
        assert!(
            self.fields.insert(name.clone(), field).is_none(),
            "server form field `{name}` was declared more than once"
        );
        self
    }

    pub(crate) fn normalize(
        &self,
        submitted: &[(String, String)],
    ) -> Result<Vec<(String, String)>> {
        let mut values = BTreeMap::<&str, Vec<&str>>::new();
        for (name, value) in submitted {
            let Some(field) = self.fields.get(name) else {
                bail!(
                    "server form field `{name}` is not declared by schema `{}`",
                    self.id
                );
            };
            if field.kind != ServerFormFieldKind::Repeated && values.contains_key(name.as_str()) {
                bail!("server form field `{name}` cannot be repeated");
            }
            if let Some(max_length) = field.max_length {
                if value.chars().count() > max_length {
                    bail!("server form field `{name}` exceeds its maximum length");
                }
            }
            values.entry(name.as_str()).or_default().push(value);
        }

        let mut normalized = submitted.to_vec();
        for field in self.fields.values() {
            let submitted_values = values.get(field.name.as_str());
            if field.required
                && submitted_values
                    .map_or(true, |values| values.iter().all(|value| value.is_empty()))
            {
                bail!("required server form field `{}` is missing", field.name);
            }
            if field.kind == ServerFormFieldKind::Boolean {
                match submitted_values {
                    None => normalized.push((field.name.clone(), "false".to_string())),
                    Some(values) if matches!(values.as_slice(), [value] if matches!(*value, "on" | "true" | "false")) =>
                        {}
                    Some(_) => bail!("server form field `{}` is not a boolean", field.name),
                }
            }
        }
        Ok(normalized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> ServerFormSchema {
        ServerFormSchema::new("search")
            .field(ServerFormField::text("query").required().max_length(20))
            .field(ServerFormField::boolean("archived"))
            .field(ServerFormField::repeated("tag"))
    }

    #[test]
    fn schema_accepts_repeated_values_and_normalizes_absent_boolean() {
        let values = schema()
            .normalize(&[
                ("query".into(), "ocean".into()),
                ("tag".into(), "water".into()),
                ("tag".into(), "physics".into()),
            ])
            .unwrap();

        assert!(values.contains(&("archived".into(), "false".into())));
        assert_eq!(values.iter().filter(|(name, _)| name == "tag").count(), 2);
    }

    #[test]
    fn schema_rejects_unknown_duplicate_invalid_and_oversized_values() {
        assert!(schema().normalize(&[("other".into(), "x".into())]).is_err());
        assert!(schema()
            .normalize(&[
                ("query".into(), "one".into()),
                ("query".into(), "two".into())
            ])
            .is_err());
        assert!(schema()
            .normalize(&[
                ("query".into(), "ocean".into()),
                ("archived".into(), "yes".into())
            ])
            .is_err());
        assert!(schema()
            .normalize(&[("query".into(), "this query is far too long".into())])
            .is_err());
    }
}
