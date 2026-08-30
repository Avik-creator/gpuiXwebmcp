use chrono::{TimeZone, Utc};
use serde_json::{Value, json};
use webmcp_protocol::{
    ConnectionStatus, DebuggerState, EventKind, LogEvent, Page, PageId, Tool, ToolAnnotations,
};

pub trait ToolBackend {
    fn snapshot(&self) -> DebuggerState;
    fn execute(&self, tool: &str, arguments: &Value) -> Result<Value, String>;
}

pub struct FixtureBackend;

impl ToolBackend for FixtureBackend {
    fn snapshot(&self) -> DebuggerState {
        fixture_state()
    }

    fn execute(&self, tool: &str, arguments: &Value) -> Result<Value, String> {
        execute_fixture_tool(tool, arguments)
    }
}

pub fn execute_fixture_tool(tool: &str, arguments: &Value) -> Result<Value, String> {
    match tool {
        "get_user" => Ok(json!({
            "id": "user_1",
            "name": "Ada Lovelace",
            "email": "ada@localhost"
        })),
        "search_products" => {
            let query = arguments
                .get("query")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "query is required".to_string())?;
            Ok(json!({
                "query": query,
                "results": [
                    {
                        "id": "book-1",
                        "title": "Programming GPUI",
                        "author": "Zed Industries"
                    },
                    {
                        "id": "book-2",
                        "title": "WebMCP in Practice",
                        "author": "Chrome Team"
                    }
                ]
            }))
        }
        "create_note" => {
            let text = arguments
                .get("text")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "text is required".to_string())?;
            Ok(json!({
                "ok": true,
                "text": text
            }))
        }
        other => Err(format!("unknown tool: {other}")),
    }
}

fn timestamp(hour: u32, min: u32, sec: u32) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 30, hour, min, sec)
        .unwrap()
}

fn fixture_state() -> DebuggerState {
    let page_id = PageId::from("page_demo");
    let page = Page {
        id: page_id.clone(),
        url: "http://localhost:5173/".to_string(),
        title: "WebMCP demo",
        origin: "http://localhost:5173".to_string(),
    };

    let tools = vec![
        Tool {
            name: "get_user".to_string(),
            title: Some("Get user".to_string()),
            description: "Return the current demo user profile".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            annotations: ToolAnnotations {
                read_only_hint: Some(true),
                untrusted_content_hint: None,
            },
        },
        Tool {
            name: "search_products".to_string(),
            title: Some("Search products".to_string()),
            description: "Search products by query".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string"
                    }
                },
                "required": ["query"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: Some(true),
                untrusted_content_hint: None,
            },
        },
        Tool {
            name: "create_note".to_string(),
            title: Some("Create note".to_string()),
            description: "Create a note from text".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string"
                    }
                },
                "required": ["text"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: Some(false),
                untrusted_content_hint: Some(true),
            },
        },
    ];

    DebuggerState {
        pages: vec![page],
        selected_page: Some(page_id),
        selected_tool: Some("search_products".to_string()),
        tools,
        executions: Vec::new(),
        events: vec![
            LogEvent {
                timestamp: timestamp(17, 54, 1),
                kind: EventKind::Hello,
                message: "fixture backend ready".to_string(),
            },
            LogEvent {
                timestamp: timestamp(17, 54, 2),
                kind: EventKind::PageChanged,
                message: "http://localhost:5173/".to_string(),
            },
            LogEvent {
                timestamp: timestamp(17, 54, 2),
                kind: EventKind::ToolsChanged,
                message: "discovered 3 tools".to_string(),
            },
        ],
        connection: ConnectionStatus::Fixture,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_exposes_the_three_demo_tools() {
        let state = FixtureBackend.snapshot();
        let names: Vec<&str> = state.tools.iter().map(|tool| tool.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["get_user", "search_products", "create_note"]
        );
        assert_eq!(state.connection, ConnectionStatus::Fixture);
        assert_eq!(
            state.selected_tool().unwrap().name,
            "search_products"
        );
        assert_eq!(
            state.selected_tool().unwrap().input_schema["properties"]["query"]["type"],
            "string"
        );
    }

    #[test]
    fn search_products_with_gpui_returns_two_books() {
        let result = FixtureBackend
            .execute("search_products", &json!({ "query": "gpui" }))
            .unwrap();
        assert_eq!(result["query"], "gpui");
        let results = result["results"].as_array().unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["title"], "Programming GPUI");
        assert_eq!(results[1]["title"], "WebMCP in Practice");
    }

    #[test]
    fn get_user_returns_a_profile() {
        let result = FixtureBackend.execute("get_user", &json!({})).unwrap();
        assert_eq!(result["name"], "Ada Lovelace");
        assert_eq!(result["email"], "ada@localhost");
    }

    #[test]
    fn create_note_echoes_text() {
        let result = FixtureBackend
            .execute("create_note", &json!({ "text": "hello" }))
            .unwrap();
        assert_eq!(result, json!({ "ok": true, "text": "hello" }));
    }

    #[test]
    fn unknown_tool_fails() {
        let error = FixtureBackend
            .execute("not_a_tool", &json!({}))
            .unwrap_err();
        assert!(error.contains("unknown tool"));
    }

    #[test]
    fn search_products_requires_query() {
        let error = FixtureBackend
            .execute("search_products", &json!({}))
            .unwrap_err();
        assert_eq!(error, "query is required");
    }
}
