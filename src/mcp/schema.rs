//! A small builder for the JSON Schema of a tool's arguments.

use std::sync::Arc;

use serde_json::{json, Map, Value};

/// One argument of a tool.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::mcp::{Prop, Schema};
///
/// let schema = Schema::new()
///     .required("selector", Prop::string("CSS or XPath selector"))
///     .optional("timeout", Prop::number("Seconds to wait").min(0.0).default(5));
/// let json = schema.to_json();
/// assert_eq!(json["required"][0], "selector");
/// assert_eq!(json["properties"]["timeout"]["default"], 5);
/// ```
#[derive(Debug, Clone)]
pub struct Prop {
    schema: Map<String, Value>,
}

impl Prop {
    fn of(kind: &str, description: &str) -> Self {
        let mut schema = Map::new();
        schema.insert("type".into(), json!(kind));
        schema.insert("description".into(), json!(description));
        Self { schema }
    }

    /// A string.
    #[must_use]
    pub fn string(description: &str) -> Self {
        Self::of("string", description)
    }

    /// A number, integer or not.
    #[must_use]
    pub fn number(description: &str) -> Self {
        Self::of("number", description)
    }

    /// A whole number.
    #[must_use]
    pub fn integer(description: &str) -> Self {
        Self::of("integer", description)
    }

    /// A true or false value.
    #[must_use]
    pub fn boolean(description: &str) -> Self {
        Self::of("boolean", description)
    }

    /// One of a fixed set of strings.
    #[must_use]
    pub fn choice(options: &[&str], description: &str) -> Self {
        let mut prop = Self::of("string", description);
        prop.schema.insert("enum".into(), json!(options));
        prop
    }

    /// The value used when the argument is omitted.
    #[must_use]
    pub fn default(mut self, value: impl Into<Value>) -> Self {
        self.schema.insert("default".into(), value.into());
        self
    }

    /// The smallest allowed value.
    #[must_use]
    pub fn min(mut self, minimum: f64) -> Self {
        self.schema.insert("minimum".into(), json!(minimum));
        self
    }
}

/// The argument schema of one tool.
#[derive(Debug, Clone, Default)]
pub struct Schema {
    properties: Map<String, Value>,
    required: Vec<String>,
}

impl Schema {
    /// An object schema with no arguments yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an argument the caller must supply.
    #[must_use]
    pub fn required(mut self, name: &str, prop: Prop) -> Self {
        self.properties
            .insert(name.to_owned(), Value::Object(prop.schema));
        self.required.push(name.to_owned());
        self
    }

    /// Adds an argument the caller may leave out.
    #[must_use]
    pub fn optional(mut self, name: &str, prop: Prop) -> Self {
        self.properties
            .insert(name.to_owned(), Value::Object(prop.schema));
        self
    }

    /// The schema as the JSON object sent to clients.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut object = Map::new();
        object.insert("type".into(), json!("object"));
        object.insert("properties".into(), Value::Object(self.properties.clone()));
        if !self.required.is_empty() {
            object.insert("required".into(), json!(self.required));
        }
        object.insert("additionalProperties".into(), json!(false));
        Value::Object(object)
    }

    pub(crate) fn build(self) -> Arc<Map<String, Value>> {
        match self.to_json() {
            Value::Object(object) => Arc::new(object),
            _ => Arc::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn required_arguments_are_listed_and_optional_ones_are_not() {
        let schema = Schema::new()
            .required("a", Prop::string("first"))
            .optional("b", Prop::boolean("second"))
            .to_json();

        assert_eq!(schema["type"], "object");
        assert_eq!(schema["required"], json!(["a"]));
        assert_eq!(schema["properties"]["a"]["type"], "string");
        assert_eq!(schema["properties"]["b"]["type"], "boolean");
        assert_eq!(schema["additionalProperties"], false);
    }

    #[test]
    fn a_schema_without_required_arguments_omits_the_key() {
        let schema = Schema::new()
            .optional("x", Prop::integer("count"))
            .to_json();

        assert!(schema.get("required").is_none());
    }

    #[test]
    fn choices_defaults_and_minimums_land_in_the_property() {
        let schema = Schema::new()
            .optional("mode", Prop::choice(&["a", "b"], "which").default("a"))
            .optional("wait", Prop::number("seconds").min(0.0).default(5))
            .to_json();

        assert_eq!(schema["properties"]["mode"]["enum"], json!(["a", "b"]));
        assert_eq!(schema["properties"]["mode"]["default"], "a");
        assert_eq!(schema["properties"]["wait"]["minimum"], 0.0);
        assert_eq!(schema["properties"]["wait"]["default"], 5);
    }

    #[test]
    fn every_property_carries_its_description_for_the_model() {
        let schema = Schema::new()
            .required("url", Prop::string("Address to open"))
            .to_json();

        assert_eq!(
            schema["properties"]["url"]["description"],
            "Address to open"
        );
    }
}
