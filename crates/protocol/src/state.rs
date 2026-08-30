use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::events::{BrowserEvent, EventKind, LogEvent};
use crate::ids::{ExecutionId, PageId};
use crate::types::{Page, Tool, ToolExecution};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStatus {
    Fixture,
    Disconnected,
    Connected,
}

impl ConnectionStatus {
    pub fn as_label(self) -> &'static str {
        match self {
            Self::Fixture => "Fixture",
            Self::Disconnected => "Disconnected",
            Self::Connected => "Connected",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DebuggerState {
    pub pages: Vec<Page>,
    pub selected_page: Option<PageId>,
    pub tools: Vec<Tool>,
    pub selected_tool: Option<String>,
    pub executions: Vec<ToolExecution>,
    pub events: Vec<LogEvent>,
    pub connection: ConnectionStatus,
}

impl DebuggerState {
    pub fn selected_page(&self) -> Option<&Page> {
        let id = self.selected_page.as_ref()?;
        self.pages.iter().find(|page| &page.id == id)
    }

    pub fn selected_tool(&self) -> Option<&Tool> {
        let name = self.selected_tool.as_ref()?;
        self.tools.iter().find(|tool| &tool.name == name)
    }

    pub fn last_execution_for(&self, tool_name: &str) -> Option<&ToolExecution> {
        self.executions
            .iter()
            .rev()
            .find(|execution| execution.tool_name == tool_name)
    }

    pub fn waiting_for_extension() -> Self {
        Self {
            pages: Vec::new(),
            selected_page: None,
            tools: Vec::new(),
            selected_tool: None,
            executions: Vec::new(),
            events: vec![LogEvent {
                timestamp: Utc::now(),
                kind: EventKind::Disconnected,
                message: "waiting for chrome-extension on ws://127.0.0.1:17321".to_string(),
            }],
            connection: ConnectionStatus::Disconnected,
        }
    }

    pub fn upsert_page(&mut self, page: Page) {
        if let Some(existing) = self.pages.iter_mut().find(|item| item.id == page.id) {
            *existing = page;
        } else {
            self.pages.push(page);
        }
    }

    /// Apply a browser event. Returns true when the tool list or selected tool
    /// changed and the inspector form should be rebuilt.
    pub fn apply_browser_event(&mut self, event: BrowserEvent) -> bool {
        match event {
            BrowserEvent::Hello {
                protocol_version,
                timestamp,
            } => {
                self.connection = ConnectionStatus::Connected;
                self.events.push(LogEvent {
                    timestamp,
                    kind: EventKind::Hello,
                    message: format!("extension protocol v{protocol_version}"),
                });
                false
            }
            BrowserEvent::PageChanged { page, timestamp } => {
                let id = page.id.clone();
                let message = page.url.clone();
                self.upsert_page(page);
                if self.selected_page.is_none() {
                    self.selected_page = Some(id);
                }
                self.events.push(LogEvent {
                    timestamp,
                    kind: EventKind::PageChanged,
                    message,
                });
                false
            }
            BrowserEvent::ToolsChanged {
                page_id,
                origin,
                url,
                tools,
                timestamp,
            } => {
                let count = tools.len();
                let title = self
                    .pages
                    .iter()
                    .find(|page| page.id == page_id)
                    .map(|page| page.title.clone())
                    .unwrap_or_else(|| origin.clone());
                self.upsert_page(Page {
                    id: page_id.clone(),
                    url,
                    title,
                    origin,
                });
                let apply_tools = match &self.selected_page {
                    None => {
                        self.selected_page = Some(page_id);
                        true
                    }
                    Some(selected) if selected == &page_id => true,
                    Some(_) => false,
                };
                self.events.push(LogEvent {
                    timestamp,
                    kind: EventKind::ToolsChanged,
                    message: format!("discovered {count} tools"),
                });
                if !apply_tools {
                    return false;
                }
                self.tools = tools;
                if !self
                    .selected_tool
                    .as_ref()
                    .is_some_and(|name| self.tools.iter().any(|tool| &tool.name == name))
                {
                    self.selected_tool = self.tools.first().map(|tool| tool.name.clone());
                }
                true
            }
            BrowserEvent::ToolExecutionStarted {
                execution_id,
                tool,
                arguments,
                timestamp,
            } => {
                if self
                    .executions
                    .iter()
                    .any(|execution| execution.id == execution_id)
                {
                    return false;
                }
                self.record_execution_started(crate::types::ToolExecution {
                    id: execution_id,
                    tool_name: tool,
                    arguments,
                    result: None,
                    error: None,
                    started_at: timestamp,
                    finished_at: None,
                });
                false
            }
            BrowserEvent::ToolExecutionFinished {
                execution_id,
                result,
                duration_ms: _,
                timestamp,
            } => {
                if self
                    .executions
                    .iter()
                    .any(|execution| execution.id == execution_id)
                {
                    self.record_execution_finished(&execution_id, result, timestamp);
                }
                false
            }
            BrowserEvent::ToolExecutionFailed {
                execution_id,
                error,
                duration_ms: _,
                timestamp,
            } => {
                if self
                    .executions
                    .iter()
                    .any(|execution| execution.id == execution_id)
                {
                    self.record_execution_failed(&execution_id, error, timestamp);
                }
                false
            }
            BrowserEvent::Disconnected { timestamp } => {
                if self.connection != ConnectionStatus::Fixture {
                    self.connection = ConnectionStatus::Disconnected;
                }
                self.events.push(LogEvent {
                    timestamp,
                    kind: EventKind::Disconnected,
                    message: "extension disconnected".to_string(),
                });
                false
            }
        }
    }

    pub fn record_execution_started(&mut self, execution: ToolExecution) {
        let args = serde_json::to_string(&execution.arguments).unwrap_or_else(|_| "{}".into());
        self.events.push(LogEvent {
            timestamp: execution.started_at,
            kind: EventKind::ToolExecutionStarted,
            message: format!("{} {}", execution.tool_name, args),
        });
        self.executions.push(execution);
    }

    pub fn record_execution_finished(
        &mut self,
        id: &ExecutionId,
        result: Value,
        finished_at: DateTime<Utc>,
    ) {
        let message = {
            let Some(execution) = self
                .executions
                .iter_mut()
                .find(|execution| &execution.id == id)
            else {
                return;
            };
            if execution.finished_at.is_some() {
                return;
            }
            execution.result = Some(result);
            execution.error = None;
            execution.finished_at = Some(finished_at);
            format!(
                "{} {}ms",
                execution.tool_name,
                execution.duration_ms().unwrap_or(0)
            )
        };
        self.events.push(LogEvent {
            timestamp: finished_at,
            kind: EventKind::ToolExecutionFinished,
            message,
        });
    }

    pub fn record_execution_failed(
        &mut self,
        id: &ExecutionId,
        error: String,
        finished_at: DateTime<Utc>,
    ) {
        let message = {
            let Some(execution) = self
                .executions
                .iter_mut()
                .find(|execution| &execution.id == id)
            else {
                return;
            };
            if execution.finished_at.is_some() {
                return;
            }
            execution.error = Some(error.clone());
            execution.result = None;
            execution.finished_at = Some(finished_at);
            format!("{} {error}", execution.tool_name)
        };
        self.events.push(LogEvent {
            timestamp: finished_at,
            kind: EventKind::ToolExecutionFailed,
            message,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{BrowserEvent, EventKind};
    use crate::types::Tool;
    use chrono::{TimeZone, Utc};

    #[test]
    fn browser_event_uses_snake_case_type_tag() {
        let timestamp = Utc.with_ymd_and_hms(2026, 8, 30, 17, 54, 1).unwrap();
        let json = serde_json::to_value(BrowserEvent::Hello {
            protocol_version: 1,
            timestamp,
        })
        .unwrap();
        assert_eq!(json["type"], "hello");
        assert_eq!(json["protocol_version"], crate::PROTOCOL_VERSION);
    }

    #[test]
    fn tools_changed_event_uses_snake_case_type_tag() {
        let timestamp = Utc.with_ymd_and_hms(2026, 8, 30, 18, 0, 0).unwrap();
        let json = serde_json::to_value(BrowserEvent::ToolsChanged {
            page_id: crate::ids::PageId::from("tab:1"),
            origin: "http://localhost:5173".into(),
            url: "http://localhost:5173/".into(),
            tools: Vec::new(),
            timestamp,
        })
        .unwrap();
        assert_eq!(json["type"], "tools_changed");
        assert_eq!(json["page_id"], "tab:1");
    }

    #[test]
    fn event_kind_labels_are_exhaustive() {
        let kinds = [
            EventKind::Hello,
            EventKind::PageChanged,
            EventKind::ToolsChanged,
            EventKind::ToolExecutionStarted,
            EventKind::ToolExecutionFinished,
            EventKind::ToolExecutionFailed,
            EventKind::Disconnected,
        ];
        for kind in kinds {
            assert!(!kind.as_label().is_empty());
        }
    }

    #[test]
    fn selected_tool_looks_up_by_name() {
        let state = DebuggerState {
            pages: Vec::new(),
            selected_page: None,
            tools: vec![Tool {
                name: "search_products".into(),
                title: Some("Search products".into()),
                description: "Search".into(),
                input_schema: serde_json::json!({"type": "object"}),
                annotations: crate::types::ToolAnnotations::default(),
            }],
            selected_tool: Some("search_products".into()),
            executions: Vec::new(),
            events: Vec::new(),
            connection: ConnectionStatus::Fixture,
        };
        assert_eq!(state.selected_tool().unwrap().name, "search_products");
    }

    #[test]
    fn execution_events_round_trip_through_state() {
        let mut state = DebuggerState {
            pages: Vec::new(),
            selected_page: None,
            tools: Vec::new(),
            selected_tool: None,
            executions: Vec::new(),
            events: Vec::new(),
            connection: ConnectionStatus::Fixture,
        };
        let started = Utc.with_ymd_and_hms(2026, 8, 30, 18, 0, 0).unwrap();
        let id = crate::ids::ExecutionId::from("exec_1");
        state.record_execution_started(crate::types::ToolExecution {
            id: id.clone(),
            tool_name: "search_products".into(),
            arguments: serde_json::json!({"query": "gpui"}),
            result: None,
            error: None,
            started_at: started,
            finished_at: None,
        });
        state.record_execution_finished(
            &id,
            serde_json::json!({"results": [1, 2]}),
            started + chrono::Duration::milliseconds(120),
        );
        assert_eq!(state.events[0].kind, EventKind::ToolExecutionStarted);
        assert_eq!(state.events[1].kind, EventKind::ToolExecutionFinished);
        assert_eq!(
            state
                .last_execution_for("search_products")
                .unwrap()
                .duration_ms(),
            Some(120)
        );
    }

    #[test]
    fn apply_hello_marks_connected() {
        let mut state = DebuggerState::waiting_for_extension();
        let timestamp = Utc.with_ymd_and_hms(2026, 8, 30, 18, 30, 0).unwrap();
        assert!(!state.apply_browser_event(BrowserEvent::Hello {
            protocol_version: 1,
            timestamp,
        }));
        assert_eq!(state.connection, ConnectionStatus::Connected);
        assert_eq!(state.events.last().unwrap().kind, EventKind::Hello);
    }

    #[test]
    fn apply_tools_changed_selects_page_and_first_tool() {
        let mut state = DebuggerState::waiting_for_extension();
        let timestamp = Utc.with_ymd_and_hms(2026, 8, 30, 18, 31, 0).unwrap();
        let page_id = crate::ids::PageId::from("tab:7");
        let rebuild = state.apply_browser_event(BrowserEvent::ToolsChanged {
            page_id: page_id.clone(),
            origin: "http://localhost:5173".into(),
            url: "http://localhost:5173/".into(),
            tools: vec![
                Tool {
                    name: "get_user".into(),
                    title: None,
                    description: "profile".into(),
                    input_schema: serde_json::json!({"type": "object"}),
                    annotations: crate::types::ToolAnnotations {
                        read_only_hint: Some(true),
                        untrusted_content_hint: None,
                    },
                },
                Tool {
                    name: "search_products".into(),
                    title: None,
                    description: "search".into(),
                    input_schema: serde_json::json!({"type": "object"}),
                    annotations: crate::types::ToolAnnotations::default(),
                },
            ],
            timestamp,
        });
        assert!(rebuild);
        assert_eq!(state.selected_page, Some(page_id));
        assert_eq!(state.selected_tool.as_deref(), Some("get_user"));
        assert_eq!(state.tools.len(), 2);
        assert_eq!(state.pages[0].origin, "http://localhost:5173");
    }

    #[test]
    fn apply_tools_changed_for_other_page_does_not_replace_tools() {
        let mut state = DebuggerState::waiting_for_extension();
        let timestamp = Utc.with_ymd_and_hms(2026, 8, 30, 18, 32, 0).unwrap();
        state.apply_browser_event(BrowserEvent::ToolsChanged {
            page_id: crate::ids::PageId::from("tab:1"),
            origin: "http://localhost:5173".into(),
            url: "http://localhost:5173/".into(),
            tools: vec![Tool {
                name: "get_user".into(),
                title: None,
                description: "profile".into(),
                input_schema: serde_json::json!({"type": "object"}),
                annotations: crate::types::ToolAnnotations::default(),
            }],
            timestamp,
        });
        let rebuild = state.apply_browser_event(BrowserEvent::ToolsChanged {
            page_id: crate::ids::PageId::from("tab:2"),
            origin: "https://example.com".into(),
            url: "https://example.com/".into(),
            tools: vec![Tool {
                name: "other".into(),
                title: None,
                description: "other".into(),
                input_schema: serde_json::json!({"type": "object"}),
                annotations: crate::types::ToolAnnotations::default(),
            }],
            timestamp,
        });
        assert!(!rebuild);
        assert_eq!(state.selected_tool.as_deref(), Some("get_user"));
        assert_eq!(state.tools[0].name, "get_user");
        assert_eq!(state.pages.len(), 2);
    }

    #[test]
    fn apply_execution_finished_is_idempotent() {
        let mut state = DebuggerState::waiting_for_extension();
        let started = Utc.with_ymd_and_hms(2026, 8, 30, 18, 33, 0).unwrap();
        let id = crate::ids::ExecutionId::from("exec_9");
        state.apply_browser_event(BrowserEvent::ToolExecutionStarted {
            execution_id: id.clone(),
            tool: "search_products".into(),
            arguments: serde_json::json!({"query": "gpui"}),
            timestamp: started,
        });
        let result = serde_json::json!({"results": [1, 2]});
        state.apply_browser_event(BrowserEvent::ToolExecutionFinished {
            execution_id: id.clone(),
            result: result.clone(),
            duration_ms: 50,
            timestamp: started + chrono::Duration::milliseconds(50),
        });
        state.apply_browser_event(BrowserEvent::ToolExecutionFinished {
            execution_id: id.clone(),
            result: serde_json::json!({"results": []}),
            duration_ms: 50,
            timestamp: started + chrono::Duration::milliseconds(80),
        });
        let finished = state
            .events
            .iter()
            .filter(|event| event.kind == EventKind::ToolExecutionFinished)
            .count();
        assert_eq!(finished, 1);
        assert_eq!(
            state.last_execution_for("search_products").unwrap().result,
            Some(result)
        );
    }
}
