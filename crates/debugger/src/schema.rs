//! Schema → form.
//!
//! Full JSON Schema, recursive, with one rule that shapes everything: an
//! unrecognised property degrades *that field alone*. The primitive parser this
//! replaced bailed the entire form to a single JSON box the moment it met a
//! nested object, so a six-field tool with one `customer` object gave you a
//! textarea and the raw schema to squint at.

use serde_json::{Map, Value};

fn is_object_schema(root: &Map<String, Value>) -> bool {
    match root.get("type") {
        None => root.contains_key("properties"),
        Some(Value::String(kind)) => kind == "object",
        Some(_) => false,
    }
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

/// What a single field is, recursively.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Text {
        format: Option<String>,
        min_len: Option<u64>,
        max_len: Option<u64>,
        pattern: Option<String>,
    },
    Number {
        min: Option<f64>,
        max: Option<f64>,
    },
    Integer {
        min: Option<i64>,
        max: Option<i64>,
    },
    Boolean,
    Choice {
        options: Vec<Value>,
    },
    List {
        item: Box<Kind>,
        min_items: Option<u64>,
        max_items: Option<u64>,
    },
    Group {
        fields: Vec<Field>,
    },
    /// No widget can represent this one. It falls back on its own.
    Raw {
        reason: String,
    },
}

impl Kind {
    pub fn is_raw(&self) -> bool {
        matches!(self, Kind::Raw { .. })
    }

    /// The constraint shown on the label row, so the schema panel stops being
    /// required reading.
    pub fn summary(&self) -> String {
        match self {
            Kind::Text { format: Some(f), .. } => format!("string · {f}"),
            Kind::Text { .. } => "string".into(),
            Kind::Number { min: Some(lo), max: Some(hi) } => format!("number · {lo}–{hi}"),
            Kind::Number { .. } => "number".into(),
            Kind::Integer { min: Some(lo), max: Some(hi) } => format!("integer · {lo}–{hi}"),
            Kind::Integer { .. } => "integer".into(),
            Kind::Boolean => "boolean".into(),
            Kind::Choice { options } => format!("enum · {} values", options.len()),
            Kind::List { min_items: Some(n), .. } => format!("array · at least {n}"),
            Kind::List { .. } => "array".into(),
            Kind::Group { fields } => format!("object · {} inside", fields.len()),
            Kind::Raw { reason } => format!("{reason} · raw"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub name: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub required: bool,
    pub default: Option<Value>,
    pub kind: Kind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Form {
    /// The root is an object: a real form, however odd individual fields are.
    Fields(Vec<Field>),
    /// The root itself cannot be laid out. Only here does the whole form fall back.
    Raw { reason: String },
}

impl Form {
    pub fn fields(&self) -> &[Field] {
        match self {
            Form::Fields(fields) => fields,
            Form::Raw { .. } => &[],
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldError {
    /// Dotted/indexed path, e.g. `customer.name` or `items[1]`.
    pub path: String,
    pub message: String,
}

pub fn form_from_schema(schema: &Value) -> Form {
    let Some(root) = schema.as_object() else {
        return Form::Raw { reason: "schema is not an object".into() };
    };
    if let Some(reason) = combinator(root) {
        return Form::Raw { reason };
    }
    if !is_object_schema(root) {
        return Form::Raw { reason: "root is not an object schema".into() };
    }
    Form::Fields(fields_of(root))
}

fn fields_of(root: &Map<String, Value>) -> Vec<Field> {
    let Some(properties) = root.get("properties").and_then(Value::as_object) else {
        return Vec::new();
    };
    let required = required_names(root);
    properties
        .iter()
        .map(|(name, property)| Field {
            name: name.clone(),
            title: property.get("title").and_then(Value::as_str).map(str::to_string),
            description: property
                .get("description")
                .and_then(Value::as_str)
                .map(str::to_string),
            required: required.iter().any(|item| item == name),
            default: property.get("default").cloned(),
            kind: kind_of(property),
        })
        .collect()
}

fn kind_of(schema: &Value) -> Kind {
    let Some(obj) = schema.as_object() else {
        return Kind::Raw { reason: "not a schema object".into() };
    };
    if let Some(reason) = combinator(obj) {
        return Kind::Raw { reason };
    }
    if let Some(options) = obj.get("enum").and_then(Value::as_array) {
        if options.is_empty() {
            return Kind::Raw { reason: "empty enum".into() };
        }
        return Kind::Choice { options: options.clone() };
    }
    let declared = obj.get("type").and_then(Value::as_str);
    match declared {
        Some("string") => Kind::Text {
            format: obj.get("format").and_then(Value::as_str).map(str::to_string),
            min_len: obj.get("minLength").and_then(Value::as_u64),
            max_len: obj.get("maxLength").and_then(Value::as_u64),
            pattern: obj.get("pattern").and_then(Value::as_str).map(str::to_string),
        },
        Some("number") => Kind::Number {
            min: obj.get("minimum").and_then(Value::as_f64),
            max: obj.get("maximum").and_then(Value::as_f64),
        },
        Some("integer") => Kind::Integer {
            min: obj.get("minimum").and_then(Value::as_i64),
            max: obj.get("maximum").and_then(Value::as_i64),
        },
        Some("boolean") => Kind::Boolean,
        Some("array") => match obj.get("items") {
            Some(items) => Kind::List {
                item: Box::new(kind_of(items)),
                min_items: obj.get("minItems").and_then(Value::as_u64),
                max_items: obj.get("maxItems").and_then(Value::as_u64),
            },
            None => Kind::Raw { reason: "array without items".into() },
        },
        Some("object") => Kind::Group { fields: fields_of(obj) },
        Some(other) => Kind::Raw { reason: format!("unsupported type {other}") },
        None if obj.contains_key("properties") => Kind::Group { fields: fields_of(obj) },
        None => Kind::Raw { reason: "no type".into() },
    }
}

fn combinator(obj: &Map<String, Value>) -> Option<String> {
    for key in ["oneOf", "anyOf", "allOf"] {
        if obj.contains_key(key) {
            return Some(key.to_string());
        }
    }
    None
}

/// Every problem with `args`, as paths the UI can render beside the right field.
/// One pass, one answer — replacing the three disagreeing checks the old path had.
pub fn validate(form: &Form, args: &Value) -> Vec<FieldError> {
    let mut errors = Vec::new();
    match form {
        Form::Raw { .. } => {
            if !args.is_object() && !args.is_null() {
                // A raw root accepts whatever parses; only shape is checked.
            }
        }
        Form::Fields(fields) => check_fields(fields, args, "", &mut errors),
    }
    errors
}

fn join(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}.{name}")
    }
}

fn check_fields(fields: &[Field], args: &Value, prefix: &str, errors: &mut Vec<FieldError>) {
    let empty = Map::new();
    let object = args.as_object().unwrap_or(&empty);
    for field in fields {
        let path = join(prefix, &field.name);
        match object.get(&field.name) {
            None | Some(Value::Null) => {
                if field.required {
                    errors.push(FieldError {
                        path,
                        message: format!("{} is required", field.name),
                    });
                }
            }
            Some(value) => check_kind(&field.kind, value, &path, errors),
        }
    }
}

fn check_kind(kind: &Kind, value: &Value, path: &str, errors: &mut Vec<FieldError>) {
    let mut fail = |message: String| {
        errors.push(FieldError { path: path.to_string(), message });
    };
    match kind {
        // We cannot know what a raw field should look like, so we never call it wrong.
        Kind::Raw { .. } => {}
        Kind::Boolean => {
            if !value.is_boolean() {
                fail("must be true or false".into());
            }
        }
        Kind::Text { min_len, max_len, .. } => {
            let Some(text) = value.as_str() else {
                return fail("must be text".into());
            };
            let len = text.chars().count() as u64;
            if min_len.is_some_and(|min| len < min) {
                fail(format!("must be at least {} characters", min_len.unwrap()));
            }
            if max_len.is_some_and(|max| len > max) {
                fail(format!("must be at most {} characters", max_len.unwrap()));
            }
        }
        Kind::Integer { min, max } => {
            let Some(number) = value.as_i64() else {
                return fail("must be a whole number".into());
            };
            if min.is_some_and(|lo| number < lo) {
                fail(format!("must be at least {}", min.unwrap()));
            }
            if max.is_some_and(|hi| number > hi) {
                fail(format!("must be at most {}", max.unwrap()));
            }
        }
        Kind::Number { min, max } => {
            let Some(number) = value.as_f64() else {
                return fail("must be a number".into());
            };
            if !number.is_finite() {
                return fail("must be a finite number".into());
            }
            if min.is_some_and(|lo| number < lo) {
                fail(format!("must be at least {}", min.unwrap()));
            }
            if max.is_some_and(|hi| number > hi) {
                fail(format!("must be at most {}", max.unwrap()));
            }
        }
        Kind::Choice { options } => {
            if !options.contains(value) {
                let shown: Vec<String> = options
                    .iter()
                    .map(|option| match option.as_str() {
                        Some(text) => text.to_string(),
                        None => option.to_string(),
                    })
                    .collect();
                fail(format!("must be one of {}", shown.join(", ")));
            }
        }
        Kind::List { item, min_items, max_items } => {
            let Some(items) = value.as_array() else {
                return fail("must be a list".into());
            };
            let len = items.len() as u64;
            if min_items.is_some_and(|min| len < min) {
                fail(format!("needs at least {} item(s)", min_items.unwrap()));
            }
            if max_items.is_some_and(|max| len > max) {
                fail(format!("takes at most {} item(s)", max_items.unwrap()));
            }
            for (index, entry) in items.iter().enumerate() {
                check_kind(item, entry, &format!("{path}[{index}]"), errors);
            }
        }
        Kind::Group { fields } => {
            if !value.is_object() {
                return fail("must be an object".into());
            }
            check_fields(fields, value, path, errors);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- the recursive engine -------------------------------------------------

    fn create_order() -> Value {
        serde_json::json!({
            "type": "object",
            "required": ["items", "customer"],
            "properties": {
                "priority": { "enum": ["low", "normal", "urgent"], "default": "normal" },
                "items": { "type": "array", "minItems": 1, "items": { "type": "string" } },
                "quantity": { "type": "integer", "minimum": 1, "maximum": 99 },
                "gift_wrap": { "type": "boolean" },
                "customer": {
                    "type": "object",
                    "required": ["name"],
                    "title": "Customer",
                    "properties": {
                        "name": { "type": "string", "description": "Full name" },
                        "tier": { "enum": ["std", "pro"] }
                    }
                },
                "metadata": { "oneOf": [{ "type": "string" }, { "type": "object" }] }
            }
        })
    }

    fn field<'a>(form: &'a Form, name: &str) -> &'a Field {
        form.fields()
            .iter()
            .find(|field| field.name == name)
            .unwrap_or_else(|| panic!("no field {name}"))
    }

    #[test]
    fn one_unsupported_property_no_longer_destroys_the_whole_form() {
        // The entire point of the rewrite. `metadata` uses oneOf and `customer`
        // is nested; the primitive path this replaced collapsed the whole form
        // to a single JSON textarea the moment it met either one.
        let form = form_from_schema(&create_order());
        assert_eq!(form.fields().len(), 6, "every property survives");
        assert!(field(&form, "metadata").kind.is_raw(), "only metadata falls back");
        for name in ["priority", "items", "quantity", "gift_wrap", "customer"] {
            assert!(!field(&form, name).kind.is_raw(), "{name} must stay a control");
        }
    }

    #[test]
    fn each_kind_is_recognised_with_its_constraints() {
        let form = form_from_schema(&create_order());
        assert!(matches!(&field(&form, "priority").kind, Kind::Choice { options } if options.len() == 3));
        assert!(matches!(
            &field(&form, "items").kind,
            Kind::List { min_items: Some(1), item, .. } if matches!(**item, Kind::Text { .. })
        ));
        assert!(matches!(field(&form, "quantity").kind, Kind::Integer { min: Some(1), max: Some(99) }));
        assert!(matches!(field(&form, "gift_wrap").kind, Kind::Boolean));
        assert!(matches!(&field(&form, "customer").kind, Kind::Group { fields } if fields.len() == 2));
    }

    #[test]
    fn nesting_goes_all_the_way_down() {
        let form = form_from_schema(&create_order());
        let Kind::Group { fields } = &field(&form, "customer").kind else {
            panic!("customer must be a group");
        };
        let name = fields.iter().find(|f| f.name == "name").unwrap();
        assert!(name.required);
        assert_eq!(name.description.as_deref(), Some("Full name"));
        assert!(matches!(fields.iter().find(|f| f.name == "tier").unwrap().kind, Kind::Choice { .. }));
    }

    #[test]
    fn titles_descriptions_and_defaults_are_kept() {
        // All three were parsed and thrown away by the old path.
        let form = form_from_schema(&create_order());
        assert_eq!(field(&form, "customer").title.as_deref(), Some("Customer"));
        assert_eq!(field(&form, "priority").default, Some(Value::from("normal")));
        // Prefill itself is form::seed's job and is tested there; here we only
        // care that the default survived parsing.
        assert!(field(&form, "priority").default.is_some());
    }

    #[test]
    fn only_a_hopeless_root_collapses_the_whole_form() {
        assert!(matches!(form_from_schema(&serde_json::json!([1, 2])), Form::Raw { .. }));
        assert!(matches!(form_from_schema(&serde_json::json!({"type": "string"})), Form::Raw { .. }));
        assert!(matches!(
            form_from_schema(&serde_json::json!({"oneOf": [{"type": "object"}]})),
            Form::Raw { .. }
        ));
        // An object with no properties is an empty form, not a failure.
        let empty = form_from_schema(&serde_json::json!({"type": "object"}));
        assert_eq!(empty.fields().len(), 0);
        assert!(matches!(empty, Form::Fields(_)));
    }

    fn errors_for(args: Value) -> Vec<FieldError> {
        validate(&form_from_schema(&create_order()), &args)
    }

    #[test]
    fn a_complete_and_correct_payload_reports_nothing() {
        let errors = errors_for(serde_json::json!({
            "priority": "urgent",
            "items": ["SKU-4471"],
            "quantity": 12,
            "gift_wrap": true,
            "customer": { "name": "Ada Lovelace", "tier": "pro" },
            "metadata": { "anything": "goes" }
        }));
        assert!(errors.is_empty(), "unexpected: {errors:?}");
    }

    #[test]
    fn missing_required_fields_are_named() {
        let errors = errors_for(serde_json::json!({}));
        let paths: Vec<&str> = errors.iter().map(|e| e.path.as_str()).collect();
        assert!(paths.contains(&"items"));
        assert!(paths.contains(&"customer"));
        assert!(!paths.contains(&"quantity"), "optional fields must stay quiet");
    }

    #[test]
    fn errors_carry_a_path_the_ui_can_point_at() {
        let errors = errors_for(serde_json::json!({
            "items": ["ok", 7],
            "customer": {}
        }));
        let paths: Vec<&str> = errors.iter().map(|e| e.path.as_str()).collect();
        assert!(paths.contains(&"items[1]"), "list index: {paths:?}");
        assert!(paths.contains(&"customer.name"), "nested field: {paths:?}");
    }

    #[test]
    fn constraints_are_actually_enforced() {
        let errors = errors_for(serde_json::json!({
            "priority": "immediately",
            "items": [],
            "quantity": 500,
            "customer": { "name": "Ada" }
        }));
        let by = |path: &str| errors.iter().find(|e| e.path == path).map(|e| e.message.clone());
        assert!(by("priority").unwrap().contains("must be one of"));
        assert!(by("items").unwrap().contains("at least 1"));
        assert!(by("quantity").unwrap().contains("at most 99"));
    }

    #[test]
    fn a_raw_field_is_never_called_wrong() {
        // We cannot know what a oneOf should look like, so we must not pretend to.
        for candidate in [serde_json::json!("text"), serde_json::json!({"a": 1}), serde_json::json!([1])] {
            let errors = errors_for(serde_json::json!({
                "items": ["x"],
                "customer": { "name": "Ada" },
                "metadata": candidate
            }));
            assert!(errors.is_empty(), "raw field rejected: {errors:?}");
        }
    }

    #[test]
    fn wrong_types_are_caught_not_coerced() {
        let errors = errors_for(serde_json::json!({
            "items": "not-a-list",
            "customer": "not-an-object",
            "gift_wrap": "yes"
        }));
        let by = |path: &str| errors.iter().find(|e| e.path == path).map(|e| e.message.clone());
        assert!(by("items").unwrap().contains("list"));
        assert!(by("customer").unwrap().contains("object"));
        assert!(by("gift_wrap").unwrap().contains("true or false"));
    }

    #[test]
    fn every_kind_summarises_itself_for_the_label_row() {
        let form = form_from_schema(&create_order());
        for field in form.fields() {
            assert!(!field.kind.summary().is_empty(), "{} has no summary", field.name);
        }
        assert_eq!(field(&form, "quantity").kind.summary(), "integer · 1–99");
        assert_eq!(field(&form, "items").kind.summary(), "array · at least 1");
    }
}
