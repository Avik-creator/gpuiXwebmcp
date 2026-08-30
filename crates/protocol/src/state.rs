use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::events::{EventKind, LogEvent};
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
            let Some(execution) = self.executions.iter_mut().find(|execution| &execution.id == id)
            else {
                return;
            };
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
            let Some(execution) = self.executions.iter_mut().find(|execution| &execution.id == id)
            else {
                return;
            };
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
        assert_eq!(json["protocol_version"], 1);
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
            state.last_execution_for("search_products").unwrap().duration_ms(),
            Some(120)
        );
    }
}
