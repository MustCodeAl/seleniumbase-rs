//! Strongly typed automated HTML form filling.
//!
//! Provides [`FormData`] to serialize typed Rust structs into field-name key-value pairs,
//! and helpers to inspect or fill form fields in a web page.

use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

use crate::error::SeleniumBaseError;

/// Serialized form field values ready to be dispatched to form inputs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FormData {
    fields: BTreeMap<String, String>,
}

impl FormData {
    /// Creates an empty `FormData`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Serializes any serde-compatible struct into a `FormData` map.
    ///
    /// Top-level struct field names become form input names, and values are converted to strings.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Json`] if JSON serialization fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::extract::FormData;
    /// use serde::Serialize;
    ///
    /// #[derive(Serialize)]
    /// struct LoginForm<'a> {
    ///     username: &'a str,
    ///     remember_me: bool,
    /// }
    ///
    /// let form = LoginForm { username: "admin", remember_me: true };
    /// let data = FormData::from_serializable(&form).expect("valid form");
    /// assert_eq!(data.get("username"), Some("admin"));
    /// assert_eq!(data.get("remember_me"), Some("true"));
    /// ```
    pub fn from_serializable<T: Serialize>(value: &T) -> Result<Self, SeleniumBaseError> {
        let json_val = serde_json::to_value(value)?;
        let mut fields = BTreeMap::new();

        if let Value::Object(map) = json_val {
            for (k, v) in map {
                let str_val = match v {
                    Value::String(s) => s,
                    Value::Bool(b) => b.to_string(),
                    Value::Number(n) => n.to_string(),
                    Value::Null => String::new(),
                    other => other.to_string(),
                };
                fields.insert(k, str_val);
            }
        }

        Ok(Self { fields })
    }

    /// Inserts or replaces a field value.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.fields.insert(key.into(), value.into());
    }

    /// Returns a reference to a field's value if present.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }

    /// Returns an iterator over all form fields and values.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.fields.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Returns the number of fields in the form data.
    #[must_use]
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Returns `true` if there are no fields.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize)]
    struct ContactForm {
        email: String,
        age: u32,
        subscribe: bool,
    }

    #[test]
    fn serializes_struct_to_form_data() {
        let form = ContactForm {
            email: "test@example.com".to_string(),
            age: 28,
            subscribe: true,
        };

        let data = FormData::from_serializable(&form).unwrap();
        assert_eq!(data.get("email"), Some("test@example.com"));
        assert_eq!(data.get("age"), Some("28"));
        assert_eq!(data.get("subscribe"), Some("true"));
    }
}
