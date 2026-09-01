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
    /// A result we received but could not apply: it arrived after the execution
    /// was closed, or names an execution we never started. Recorded, never dropped.
    LateResult,
    /// A frame reached us that we could not turn into a `BrowserEvent`.
    ProtocolError,
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
            Self::LateResult => "LATE_RESULT",
            Self::ProtocolError => "PROTOCOL_ERROR",
        }
    }

    /// Short, lowercase, and never longer than the column that holds it.
    ///
    /// The screaming labels above were space-padded to 22 characters, but
    /// `TOOL_EXECUTION_FINISHED` is 23 — so the message column sat one character
    /// out of true for the whole life of the old log. These are laid out in a
    /// fixed column by the view instead, which removes the class of bug.
    pub fn short(self) -> &'static str {
        match self {
            Self::Hello => "hello",
            Self::PageChanged => "page",
            Self::ToolsChanged => "tools",
            Self::ToolExecutionStarted => "started",
            Self::ToolExecutionFinished => "ok",
            Self::ToolExecutionFailed => "failed",
            Self::Disconnected => "gone",
            Self::LateResult => "late",
            Self::ProtocolError => "dropped",
        }
    }

    /// True for kinds that mean something went wrong on the wire or in a tool.
    pub fn is_fault(self) -> bool {
        matches!(
            self,
            Self::ToolExecutionFailed
                | Self::Disconnected
                | Self::LateResult
                | Self::ProtocolError
        )
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
    /// A tab went away. Without this the page list only ever grows.
    /// The extension did not always stamp this one, so a missing time is now.
    PageClosed {
        page_id: PageId,
        #[serde(default = "Utc::now")]
        timestamp: DateTime<Utc>,
    },
    Disconnected {
        timestamp: DateTime<Utc>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_closed_without_a_timestamp_still_parses() {
        // The extension sent `page_closed` bare, so every tab close was logged
        // as a protocol error and the page list never shrank.
        let event: BrowserEvent =
            serde_json::from_str(r#"{"type":"page_closed","page_id":"tab:4"}"#).unwrap();
        assert!(matches!(event, BrowserEvent::PageClosed { page_id, .. } if page_id.as_str() == "tab:4"));
    }
}
