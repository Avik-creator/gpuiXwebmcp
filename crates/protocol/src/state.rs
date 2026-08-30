use serde::{Deserialize, Serialize};

use crate::events::LogEvent;
use crate::ids::PageId;
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
}
