use serde_json::{Map, Value};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldKind {
    String,
    Number,
    Integer,
    Boolean,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormField {
    pub name: String,
    pub kind: FieldKind,
    pub required: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormSpec {
    Primitive { fields: Vec<FormField> },
    JsonFallback,
}

impl FieldKind {
    fn from_type_name(name: &str) -> Option<Self> {
        match name {
            "string" => Some(Self::String),
            "number" => Some(Self::Number),
            "integer" => Some(Self::Integer),
            "boolean" => Some(Self::Boolean),
            _ => None,
        }
    }

    pub fn placeholder(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Number => "number",
            Self::Integer => "integer",
            Self::Boolean => "boolean",
        }
    }
}

pub fn form_spec_from_schema(schema: &Value) -> FormSpec {
    let Some(root) = schema.as_object() else {
        return FormSpec::JsonFallback;
    };
    if has_unsupported_combinator(root) {
        return FormSpec::JsonFallback;
    }
    if !is_object_schema(root) {
        return FormSpec::JsonFallback;
    }
    let Some(properties) = root.get("properties") else {
        return FormSpec::Primitive { fields: Vec::new() };
    };
    let Some(properties) = properties.as_object() else {
        return FormSpec::JsonFallback;
    };

    let required = required_names(root);
    let mut fields = Vec::with_capacity(properties.len());
    for (name, property) in properties {
        let Some(kind) = property_kind(property) else {
            return FormSpec::JsonFallback;
        };
        fields.push(FormField {
            name: name.clone(),
            kind,
            required: required.iter().any(|item| item == name),
        });
    }
    FormSpec::Primitive { fields }
}

pub fn arguments_from_primitive(
    fields: &[FormField],
    strings: &Map<String, Value>,
    bools: &Map<String, Value>,
) -> Result<Value, String> {
    let mut out = Map::new();
    for field in fields {
        match field.kind {
            FieldKind::Boolean => {
                let value = bools
                    .get(&field.name)
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                out.insert(field.name.clone(), Value::Bool(value));
            }
            FieldKind::String => {
                let raw = string_field(strings, &field.name);
                if raw.is_empty() {
                    if field.required {
                        return Err(format!("{} is required", field.name));
                    }
                    continue;
                }
                out.insert(field.name.clone(), Value::String(raw));
            }
            FieldKind::Integer => {
                let raw = string_field(strings, &field.name);
                if raw.is_empty() {
                    if field.required {
                        return Err(format!("{} is required", field.name));
                    }
                    continue;
                }
                let parsed: i64 = raw
                    .parse()
                    .map_err(|_| format!("{} must be an integer", field.name))?;
                out.insert(field.name.clone(), Value::Number(parsed.into()));
            }
            FieldKind::Number => {
                let raw = string_field(strings, &field.name);
                if raw.is_empty() {
                    if field.required {
                        return Err(format!("{} is required", field.name));
                    }
                    continue;
                }
                let parsed: f64 = raw
                    .parse()
                    .map_err(|_| format!("{} must be a number", field.name))?;
                let number = serde_json::Number::from_f64(parsed)
                    .ok_or_else(|| format!("{} must be a finite number", field.name))?;
                out.insert(field.name.clone(), Value::Number(number));
            }
        }
    }
    Ok(Value::Object(out))
}

pub fn required_fields_filled(
    spec: &FormSpec,
    strings: &Map<String, Value>,
    json_text: &str,
) -> bool {
    match spec {
        FormSpec::Primitive { fields } => fields.iter().all(|field| match field.kind {
            FieldKind::Boolean => true,
            FieldKind::String | FieldKind::Number | FieldKind::Integer => {
                if !field.required {
                    return true;
                }
                !string_field(strings, &field.name).is_empty()
            }
        }),
        FormSpec::JsonFallback => json_text.trim().is_empty() || serde_json::from_str::<Value>(json_text).is_ok(),
    }
}

fn string_field(strings: &Map<String, Value>, name: &str) -> String {
    strings
        .get(name)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn is_object_schema(root: &Map<String, Value>) -> bool {
    match root.get("type") {
        None => root.contains_key("properties"),
        Some(Value::String(kind)) => kind == "object",
        Some(_) => false,
    }
}

fn has_unsupported_combinator(obj: &Map<String, Value>) -> bool {
    obj.contains_key("oneOf") || obj.contains_key("anyOf") || obj.contains_key("allOf")
}

fn property_kind(property: &Value) -> Option<FieldKind> {
    let obj = property.as_object()?;
    if has_unsupported_combinator(obj)
        || obj.contains_key("items")
        || obj.contains_key("properties")
        || obj.contains_key("enum")
    {
        return None;
    }
    let type_name = obj.get("type")?.as_str()?;
    FieldKind::from_type_name(type_name)
}

fn required_names(root: &Map<String, Value>) -> Vec<String> {
    root.get("required")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn search_products_is_a_required_string_field() {
        let spec = form_spec_from_schema(&json!({
            "type": "object",
            "properties": { "query": { "type": "string" } },
            "required": ["query"]
        }));
        match spec {
            FormSpec::Primitive { fields } => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].name, "query");
                assert_eq!(fields[0].kind, FieldKind::String);
                assert!(fields[0].required);
            }
            FormSpec::JsonFallback => panic!("expected primitive form"),
        }
    }

    #[test]
    fn empty_object_schema_has_no_fields() {
        let spec = form_spec_from_schema(&json!({
            "type": "object",
            "properties": {}
        }));
        assert_eq!(spec, FormSpec::Primitive { fields: Vec::new() });
        assert!(required_fields_filled(
            &spec,
            &Map::new(),
            ""
        ));
    }

    #[test]
    fn nested_object_falls_back_to_json() {
        let spec = form_spec_from_schema(&json!({
            "type": "object",
            "properties": {
                "user": {
                    "type": "object",
                    "properties": { "name": { "type": "string" } }
                }
            }
        }));
        assert_eq!(spec, FormSpec::JsonFallback);
    }

    #[test]
    fn required_string_blocks_execute_until_filled() {
        let spec = form_spec_from_schema(&json!({
            "type": "object",
            "properties": { "query": { "type": "string" } },
            "required": ["query"]
        }));
        assert!(!required_fields_filled(&spec, &Map::new(), ""));
        let mut values = Map::new();
        values.insert("query".into(), json!("gpui"));
        assert!(required_fields_filled(&spec, &values, ""));
        let args = arguments_from_primitive(
            match &spec {
                FormSpec::Primitive { fields } => fields,
                FormSpec::JsonFallback => panic!("expected primitive form"),
            },
            &values,
            &Map::new(),
        )
        .unwrap();
        assert_eq!(args, json!({"query": "gpui"}));
    }

    #[test]
    fn field_kind_placeholder_is_exhaustive() {
        for kind in [
            FieldKind::String,
            FieldKind::Number,
            FieldKind::Integer,
            FieldKind::Boolean,
        ] {
            assert!(!kind.placeholder().is_empty());
        }
    }
}
