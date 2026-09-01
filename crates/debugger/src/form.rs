//! What the widgets are holding, and how it becomes JSON.
//!
//! Kept deliberately free of gpui: the widgets write plain values in here keyed
//! by path, and assembly is a pure function over them. That means the part most
//! likely to be wrong — turning a half-filled form into arguments — is testable
//! without a window.
#![allow(dead_code)] // wired up by place/compose.rs in this same phase


use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::schema::{Field, FieldError, Kind};

/// Raw widget state, keyed by the same paths `validate` reports errors against.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Raw {
    /// Text, number, integer and raw-JSON fields all hold text.
    pub text: BTreeMap<String, String>,
    pub bools: BTreeMap<String, bool>,
    /// Index into the field's `Kind::Choice` options.
    pub choices: BTreeMap<String, usize>,
    /// How many rows a list currently shows.
    pub lists: BTreeMap<String, usize>,
    /// Paths whose disclosure is open. `""` is the schema panel.
    pub open: std::collections::BTreeSet<String>,
}

impl Raw {
    pub fn text_at(&self, path: &str) -> &str {
        self.text.get(path).map(String::as_str).unwrap_or("").trim()
    }

    pub fn is_open(&self, path: &str) -> bool {
        self.open.contains(path)
    }

    pub fn toggle(&mut self, path: &str) {
        if !self.open.remove(path) {
            self.open.insert(path.to_string());
        }
    }

    /// Rows in a list, defaulting to one so a list is never a dead end.
    pub fn list_len(&self, path: &str) -> usize {
        self.lists.get(path).copied().unwrap_or(1)
    }

    /// Remove row `dropped` of `list`, renumbering every path behind it.
    /// Nested objects, booleans and choices move too, not just plain text rows.
    pub fn drop_row(&mut self, list: &str, dropped: usize) {
        let count = self.list_len(list);
        if dropped >= count {
            return;
        }
        self.text = rekey_map(std::mem::take(&mut self.text), list, dropped);
        self.bools = rekey_map(std::mem::take(&mut self.bools), list, dropped);
        self.choices = rekey_map(std::mem::take(&mut self.choices), list, dropped);
        self.lists = rekey_map(std::mem::take(&mut self.lists), list, dropped);
        self.open = std::mem::take(&mut self.open)
            .iter()
            .filter_map(|path| rekey_after_drop(path, list, dropped))
            .collect();
        self.lists.insert(list.to_string(), count - 1);
    }
}

/// Where `key` lands once row `dropped` of `list` is gone: `None` for the row
/// itself, renumbered for rows behind it, untouched for everything else.
pub fn rekey_after_drop(key: &str, list: &str, dropped: usize) -> Option<String> {
    let unchanged = Some(key.to_string());
    let Some(rest) = key.strip_prefix(list).and_then(|rest| rest.strip_prefix('[')) else {
        return unchanged;
    };
    let Some((digits, tail)) = rest.split_once(']') else {
        return unchanged;
    };
    let Ok(index) = digits.parse::<usize>() else {
        return unchanged;
    };
    if !(tail.is_empty() || tail.starts_with('.') || tail.starts_with('[')) {
        return unchanged;
    }
    if index == dropped {
        return None;
    }
    if index > dropped {
        return Some(format!("{list}[{}]{tail}", index - 1));
    }
    unchanged
}

/// `rekey_after_drop` over a whole map, dropping the removed row's entries.
pub fn rekey_map<V>(map: BTreeMap<String, V>, list: &str, dropped: usize) -> BTreeMap<String, V> {
    map.into_iter()
        .filter_map(|(key, value)| rekey_after_drop(&key, list, dropped).map(|key| (key, value)))
        .collect()
}

pub fn child_path(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}.{name}")
    }
}

/// Turn widget state into arguments.
///
/// Returns the value plus any problem assembly itself found — a number that will
/// not parse, or raw JSON that will not. Everything else is `schema::validate`'s
/// job, so the two never disagree about the same thing.
pub fn assemble(fields: &[Field], raw: &Raw, prefix: &str) -> (Value, Vec<FieldError>) {
    let mut out = Map::new();
    let mut errors = Vec::new();
    for field in fields {
        let path = child_path(prefix, &field.name);
        if let Some(value) = value_of(&field.kind, field, &path, raw, &mut errors) {
            out.insert(field.name.clone(), value);
        }
    }
    (Value::Object(out), errors)
}

fn value_of(
    kind: &Kind,
    field: &Field,
    path: &str,
    raw: &Raw,
    errors: &mut Vec<FieldError>,
) -> Option<Value> {
    let mut fail = |message: &str| {
        errors.push(FieldError {
            path: path.to_string(),
            message: message.to_string(),
        });
    };
    match kind {
        Kind::Boolean => Some(Value::Bool(raw.bools.get(path).copied().unwrap_or(false))),
        Kind::Text { .. } => {
            let text = raw.text_at(path);
            if text.is_empty() {
                return None;
            }
            Some(Value::String(text.to_string()))
        }
        Kind::Integer { .. } => {
            let text = raw.text_at(path);
            if text.is_empty() {
                return None;
            }
            match text.parse::<i64>() {
                Ok(number) => Some(Value::from(number)),
                Err(_) => {
                    fail("must be a whole number");
                    None
                }
            }
        }
        Kind::Number { .. } => {
            let text = raw.text_at(path);
            if text.is_empty() {
                return None;
            }
            match text.parse::<f64>().ok().and_then(serde_json::Number::from_f64) {
                Some(number) => Some(Value::Number(number)),
                None => {
                    fail("must be a finite number");
                    None
                }
            }
        }
        Kind::Choice { options } => {
            let picked = raw
                .choices
                .get(path)
                .and_then(|index| options.get(*index))
                .cloned();
            picked.or_else(|| field.default.clone())
        }
        Kind::List { item, .. } => {
            let mut items = Vec::new();
            for index in 0..raw.list_len(path) {
                let item_path = format!("{path}[{index}]");
                // A list row carries no name of its own; it borrows the field's
                // so that `default` and `required` do not leak into rows.
                let row = Field {
                    name: field.name.clone(),
                    title: None,
                    description: None,
                    required: false,
                    default: None,
                    kind: (**item).clone(),
                };
                if let Some(value) = value_of(item, &row, &item_path, raw, errors) {
                    items.push(value);
                }
            }
            Some(Value::Array(items))
        }
        Kind::Group { fields } => {
            let (value, mut nested) = assemble(fields, raw, path);
            errors.append(&mut nested);
            let empty = value.as_object().is_some_and(Map::is_empty);
            if empty && !field.required {
                return None;
            }
            Some(value)
        }
        Kind::Raw { .. } => {
            let text = raw.text_at(path);
            if text.is_empty() {
                return None;
            }
            match serde_json::from_str::<Value>(text) {
                Ok(value) => Some(value),
                Err(_) => {
                    fail("must be valid JSON");
                    None
                }
            }
        }
    }
}

/// Every focusable field path, in the order they appear on screen.
///
/// Tab has to follow the eye, and `inputs` is a map sorted by path — which puts
/// `customer.name` before `items[0]` regardless of how the schema declared them.
/// This walks the tree in declaration order instead, descending only into groups
/// the operator has actually opened.
pub fn ordered_paths(fields: &[Field], raw: &Raw, prefix: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for field in fields {
        let path = child_path(prefix, &field.name);
        match &field.kind {
            Kind::Group { fields } => {
                if raw.is_open(&path) {
                    paths.extend(ordered_paths(fields, raw, &path));
                }
            }
            Kind::List { item, .. } => {
                for index in 0..raw.list_len(&path) {
                    let row = format!("{path}[{index}]");
                    if matches!(**item, Kind::Group { .. } | Kind::List { .. }) {
                        continue;
                    }
                    if !matches!(**item, Kind::Boolean | Kind::Choice { .. }) {
                        paths.push(row);
                    }
                }
            }
            // Only kinds that own a text field can take focus.
            Kind::Boolean | Kind::Choice { .. } => {}
            _ => paths.push(path),
        }
    }
    paths
}

/// Seed widget state from the schema's own defaults.
pub fn seed(fields: &[Field], prefix: &str, raw: &mut Raw) {
    for field in fields {
        let path = child_path(prefix, &field.name);
        match &field.kind {
            Kind::Choice { options } => {
                let index = field
                    .default
                    .as_ref()
                    .and_then(|value| options.iter().position(|option| option == value));
                if let Some(index) = index {
                    raw.choices.insert(path, index);
                }
            }
            Kind::Boolean => {
                if let Some(Value::Bool(value)) = field.default {
                    raw.bools.insert(path, value);
                }
            }
            Kind::Group { fields } => seed(fields, &path, raw),
            _ => {
                if let Some(default) = &field.default {
                    let text = match default {
                        Value::String(text) => text.clone(),
                        other => other.to_string(),
                    };
                    raw.text.insert(path, text);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::form_from_schema;

    fn schema() -> Value {
        serde_json::json!({
            "type": "object",
            "required": ["items", "customer"],
            "properties": {
                "priority": { "enum": ["low", "normal", "urgent"], "default": "normal" },
                "items": { "type": "array", "minItems": 1, "items": { "type": "string" } },
                "quantity": { "type": "integer", "minimum": 1 },
                "gift_wrap": { "type": "boolean" },
                "customer": {
                    "type": "object",
                    "required": ["name"],
                    "properties": { "name": { "type": "string" } }
                },
                "metadata": { "oneOf": [{ "type": "string" }] }
            }
        })
    }

    fn fields() -> Vec<Field> {
        form_from_schema(&schema()).fields().to_vec()
    }

    #[test]
    fn a_filled_form_assembles_the_payload_the_tool_expects() {
        let mut raw = Raw::default();
        raw.choices.insert("priority".into(), 2);
        raw.lists.insert("items".into(), 2);
        raw.text.insert("items[0]".into(), "SKU-4471".into());
        raw.text.insert("items[1]".into(), "SKU-9920".into());
        raw.text.insert("quantity".into(), "12".into());
        raw.bools.insert("gift_wrap".into(), true);
        raw.text.insert("customer.name".into(), "Ada Lovelace".into());
        raw.text.insert("metadata".into(), r#"{"kind":"promo"}"#.into());

        let (value, errors) = assemble(&fields(), &raw, "");
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(
            value,
            serde_json::json!({
                "priority": "urgent",
                "items": ["SKU-4471", "SKU-9920"],
                "quantity": 12,
                "gift_wrap": true,
                "customer": { "name": "Ada Lovelace" },
                "metadata": { "kind": "promo" }
            })
        );
    }

    #[test]
    fn empty_optional_fields_are_omitted_not_sent_as_empty_strings() {
        let mut raw = Raw::default();
        raw.lists.insert("items".into(), 0);
        raw.text.insert("customer.name".into(), "Ada".into());
        let (value, errors) = assemble(&fields(), &raw, "");
        assert!(errors.is_empty());
        let object = value.as_object().unwrap();
        assert!(!object.contains_key("quantity"), "empty number must not be sent");
        assert!(!object.contains_key("metadata"), "empty raw must not be sent");
        // A boolean is always meaningful, so it is always sent.
        assert_eq!(object["gift_wrap"], Value::Bool(false));
    }

    #[test]
    fn a_choice_falls_back_to_the_schema_default() {
        let (value, _) = assemble(&fields(), &Raw::default(), "");
        assert_eq!(value["priority"], Value::from("normal"));
    }

    #[test]
    fn seeding_prefills_from_defaults() {
        let mut raw = Raw::default();
        seed(&fields(), "", &mut raw);
        assert_eq!(raw.choices.get("priority"), Some(&1), "index of \"normal\"");
    }

    #[test]
    fn unparseable_input_is_reported_against_its_own_field() {
        let mut raw = Raw::default();
        raw.text.insert("quantity".into(), "twelve".into());
        raw.text.insert("metadata".into(), "{not json".into());
        raw.text.insert("customer.name".into(), "Ada".into());
        let (_, errors) = assemble(&fields(), &raw, "");
        let by = |path: &str| errors.iter().find(|e| e.path == path).map(|e| e.message.clone());
        assert_eq!(by("quantity").as_deref(), Some("must be a whole number"));
        assert_eq!(by("metadata").as_deref(), Some("must be valid JSON"));
    }

    #[test]
    fn a_blank_optional_group_is_omitted_but_a_required_one_is_not() {
        let (value, _) = assemble(&fields(), &Raw::default(), "");
        // `customer` is required, so it is sent even when empty — which is what
        // lets validate report `customer.name is required` instead of staying silent.
        assert!(value.as_object().unwrap().contains_key("customer"));
    }

    #[test]
    fn assembly_and_validation_agree_on_a_half_filled_form() {
        let form = form_from_schema(&schema());
        let mut raw = Raw::default();
        raw.lists.insert("items".into(), 0);
        let (value, assembly_errors) = assemble(form.fields(), &raw, "");
        let validation = crate::schema::validate(&form, &value);
        assert!(assembly_errors.is_empty());
        let paths: Vec<&str> = validation.iter().map(|e| e.path.as_str()).collect();
        assert!(paths.contains(&"items"), "minItems must be caught: {paths:?}");
        assert!(paths.contains(&"customer.name"), "nested required: {paths:?}");
    }

    #[test]
    fn tab_order_follows_the_schema_not_the_alphabet() {
        let mut raw = Raw::default();
        raw.lists.insert("items".into(), 2);
        let paths = ordered_paths(&fields(), &raw, "");
        // Declaration order: priority (no field), items rows, quantity, gift_wrap
        // (no field), customer (collapsed), metadata.
        assert_eq!(paths, vec!["items[0]", "items[1]", "quantity", "metadata"]);
        // Sorted order would have put customer/metadata first — the bug this avoids.
        let mut sorted = paths.clone();
        sorted.sort();
        assert_ne!(paths, sorted, "path order must not be alphabetical");
    }

    #[test]
    fn a_collapsed_group_is_skipped_and_an_open_one_is_walked() {
        let mut raw = Raw::default();
        raw.lists.insert("items".into(), 0);
        assert!(!ordered_paths(&fields(), &raw, "").contains(&"customer.name".to_string()));
        raw.toggle("customer");
        assert!(ordered_paths(&fields(), &raw, "").contains(&"customer.name".to_string()));
    }

    #[test]
    fn kinds_without_a_text_field_never_take_focus() {
        let mut raw = Raw::default();
        raw.lists.insert("items".into(), 0);
        let paths = ordered_paths(&fields(), &raw, "");
        assert!(!paths.contains(&"priority".to_string()), "a choice has no text field");
        assert!(!paths.contains(&"gift_wrap".to_string()), "a boolean has no text field");
    }

    #[test]
    fn dropping_a_row_renumbers_everything_behind_it() {
        let mut raw = Raw::default();
        raw.lists.insert("items".into(), 3);
        raw.text.insert("items[0].name".into(), "a".into());
        raw.text.insert("items[1].name".into(), "b".into());
        raw.text.insert("items[2].name".into(), "c".into());
        raw.bools.insert("items[2].gift".into(), true);
        raw.choices.insert("items[1].tier".into(), 1);
        raw.lists.insert("items[2].tags".into(), 4);
        raw.open.insert("items[2]".into());
        raw.text.insert("items2[0]".into(), "keep".into());

        raw.drop_row("items", 0);

        assert_eq!(raw.list_len("items"), 2);
        assert_eq!(raw.text.get("items[0].name").map(String::as_str), Some("b"));
        assert_eq!(raw.text.get("items[1].name").map(String::as_str), Some("c"));
        assert!(!raw.text.contains_key("items[2].name"));
        assert_eq!(raw.bools.get("items[1].gift"), Some(&true));
        assert_eq!(raw.choices.get("items[0].tier"), Some(&1));
        assert_eq!(raw.list_len("items[1].tags"), 4);
        assert!(raw.is_open("items[1]"));
        assert_eq!(
            raw.text.get("items2[0]").map(String::as_str),
            Some("keep"),
            "a list whose name merely starts the same is not touched"
        );
    }

    #[test]
    fn dropping_the_last_row_or_a_missing_one_is_safe() {
        let mut raw = Raw::default();
        raw.text.insert("items[0]".into(), "only".into());
        raw.drop_row("items", 5);
        assert_eq!(raw.list_len("items"), 1, "out of range must not touch anything");
        raw.drop_row("items", 0);
        assert_eq!(raw.list_len("items"), 0);
        assert!(raw.text.is_empty());
    }

    #[test]
    fn disclosure_toggles_and_lists_start_usable() {
        let mut raw = Raw::default();
        assert!(!raw.is_open("customer"));
        raw.toggle("customer");
        assert!(raw.is_open("customer"));
        raw.toggle("customer");
        assert!(!raw.is_open("customer"));
        // A list with no rows would be a dead end with no way to add one.
        assert_eq!(raw.list_len("items"), 1);
    }
}
