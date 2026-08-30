use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ids::{ExecutionId, PageId};
use crate::types::{Page, Tool};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Hello,
    PageChanged,
    ToolsChanged,
    ToolExecutionStarted,
    ToolExecutionFinished,
    ToolExecutionFailed,
    Disconnected,
}

impl EventKind {
    pub fn as_label(self) -> &'static str {
        match self {
            Self::Hello => "HELLO",
            Self::PageChanged => "PAGE_CHANGED",
            Self::ToolsChanged => "TOOLS_CHANGED",
            Self::ToolExecutionStarted => "TOOL_EXECUTION_STARTED",
            Self::ToolExecutionFinished => "TOOL_EXECUTION_FINISHED",
            Self::ToolExecutionFailed => "TOOL_EXECUTION_FAILED",
            Self::Disconnected => "DISCONNECTED",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogEvent {
    pub timestamp: DateTime<Utc>,
    pub kind: EventKind,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BrowserEvent {
    Hello {
        protocol_version: u32,
        timestamp: DateTime<Utc>,
    },
    PageChanged {
        page: Page,
        timestamp: DateTime<Utc>,
    },
    ToolsChanged {
        page_id: PageId,
        origin: String,
        url: String,
        tools: Vec<Tool>,
        timestamp: DateTime<Utc>,
    },
    ToolExecutionStarted {
        execution_id: ExecutionId,
        tool: String,
        arguments: Value,
        timestamp: DateTime<Utc>,
    },
    ToolExecutionFinished {
        execution_id: ExecutionId,
        result: Value,
        duration_ms: u64,
        timestamp: DateTime<Utc>,
    },
    ToolExecutionFailed {
        execution_id: ExecutionId,
        error: String,
        duration_ms: u64,
        timestamp: DateTime<Utc>,
    },
    Disconnected {
        timestamp: DateTime<Utc>,
    },
}
