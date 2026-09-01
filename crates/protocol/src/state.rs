use std::collections::BTreeMap;

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

/// Session-only history caps. The log is never persisted, but it must not grow
/// without bound for the life of the process either. Oldest entries are evicted
/// first; an execution that has not finished is never evicted.
pub const MAX_EVENTS: usize = 2_000;
pub const MAX_EXECUTIONS: usize = 500;

/// Longest raw frame we keep when reporting an unparsable message. Enough to see
/// the shape of what arrived without letting a hostile page pin memory.
pub const MAX_RAW_FRAME: usize = 4_096;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DebuggerState {
    pub pages: Vec<Page>,
    pub selected_page: Option<PageId>,
    pub tools: Vec<Tool>,
    /// Every page's tools, not just the selected one's.
    ///
    /// Without this, `tools_changed` for a page you are not looking at is
    /// dropped on the floor, so switching pages shows nothing until a fresh
    /// round-trip to the extension comes back.
    #[serde(default)]
    pub tools_by_page: BTreeMap<String, Vec<Tool>>,
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

    /// The run of `tool_name` before `id` that produced a result.
    ///
    /// This is what makes a mutation visible from a native window: read, mutate,
    /// read again, and compare the two reads.
    pub fn previous_result_for(
        &self,
        tool_name: &str,
        id: &ExecutionId,
    ) -> Option<&ToolExecution> {
        let position = self.executions.iter().position(|run| &run.id == id)?;
        self.executions[..position]
            .iter()
            .rev()
            .find(|run| run.tool_name == tool_name && run.result.is_some())
    }

    pub fn waiting_for_extension() -> Self {
        Self {
            pages: Vec::new(),
            selected_page: None,
            tools: Vec::new(),
            tools_by_page: BTreeMap::new(),
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

    /// Point at another page. Tools come from the cache, so this is instant and
    /// never shows an empty list while a round-trip is in flight.
    /// Returns true when the form must be rebuilt.
    pub fn select_page(&mut self, id: PageId) -> bool {
        if self.selected_page.as_ref() == Some(&id) {
            return false;
        }
        self.tools = self
            .tools_by_page
            .get(id.as_str())
            .cloned()
            .unwrap_or_default();
        self.selected_page = Some(id);
        self.reselect_tool();
        true
    }

    /// A tab went away. Returns true when the form must be rebuilt.
    pub fn close_page(&mut self, id: &PageId, at: DateTime<Utc>) -> bool {
        let known = self.pages.iter().any(|page| &page.id == id);
        if !known {
            return false;
        }
        self.pages.retain(|page| &page.id != id);
        self.tools_by_page.remove(id.as_str());
        self.push_event(LogEvent {
            timestamp: at,
            kind: EventKind::PageChanged,
            message: format!("closed {}", id.as_str()),
        });
        if self.selected_page.as_ref() != Some(id) {
            return false;
        }
        match self.pages.first().map(|page| page.id.clone()) {
            Some(next) => {
                self.selected_page = None;
                self.select_page(next);
            }
            None => {
                self.selected_page = None;
                self.tools.clear();
                self.selected_tool = None;
            }
        }
        true
    }

    fn reselect_tool(&mut self) {
        let still_there = self
            .selected_tool
            .as_ref()
            .is_some_and(|name| self.tools.iter().any(|tool| &tool.name == name));
        if !still_there {
            self.selected_tool = self.tools.first().map(|tool| tool.name.clone());
        }
    }

    pub fn upsert_page(&mut self, page: Page) {
        if let Some(existing) = self.pages.iter_mut().find(|item| item.id == page.id) {
            *existing = page;
        } else {
            self.pages.push(page);
        }
    }

    /// Append a log line, evicting the oldest once the cap is reached.
    pub fn push_event(&mut self, event: LogEvent) {
        if self.events.len() >= MAX_EVENTS {
            let overflow = self.events.len() + 1 - MAX_EVENTS;
            self.events.drain(..overflow);
        }
        self.events.push(event);
    }

    /// Append an execution, evicting the oldest *finished* ones once the cap is
    /// reached. An in-flight execution is never evicted: dropping it would strand
    /// the pending id and lose the result when it lands.
    fn push_execution(&mut self, execution: ToolExecution) {
        while self.executions.len() >= MAX_EXECUTIONS {
            let Some(index) = self
                .executions
                .iter()
                .position(|item| item.finished_at.is_some())
            else {
                break;
            };
            self.executions.remove(index);
        }
        self.executions.push(execution);
    }

    /// A frame arrived that we could not turn into an event. Never silently
    /// dropped: a debugger that hides what it received has failed at its one job.
    pub fn record_protocol_error(&mut self, reason: &str, raw: &str, at: DateTime<Utc>) {
        let mut shown: String = raw.chars().take(MAX_RAW_FRAME).collect();
        if shown.len() < raw.len() {
            shown.push('…');
        }
        self.push_event(LogEvent {
            timestamp: at,
            kind: EventKind::ProtocolError,
            message: format!("{reason}: {shown}"),
        });
    }

    /// A result we received but cannot apply — it arrived after the execution was
    /// closed, or names an execution we never started. Recorded, not applied:
    /// rewriting a settled outcome would rewrite history.
    pub fn record_late_result(&mut self, message: String, at: DateTime<Utc>) {
        self.push_event(LogEvent {
            timestamp: at,
            kind: EventKind::LateResult,
            message,
        });
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
                self.push_event(LogEvent {
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
                self.push_event(LogEvent {
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
                // Cache first, always: a page you are not looking at still has
                // tools, and they must be there the moment you switch to it.
                self.tools_by_page
                    .insert(page_id.as_str().to_string(), tools.clone());
                let apply_tools = match &self.selected_page {
                    None => {
                        self.selected_page = Some(page_id);
                        true
                    }
                    Some(selected) if selected == &page_id => true,
                    Some(_) => false,
                };
                self.push_event(LogEvent {
                    timestamp,
                    kind: EventKind::ToolsChanged,
                    message: format!("discovered {count} tools"),
                });
                if !apply_tools {
                    return false;
                }
                self.tools = tools;
                self.reselect_tool();
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
                // No existence guard: an id we do not know is recorded as a late
                // result rather than dropped on the floor.
                self.record_execution_finished(&execution_id, result, timestamp);
                false
            }
            BrowserEvent::ToolExecutionFailed {
                execution_id,
                error,
                duration_ms: _,
                timestamp,
            } => {
                self.record_execution_failed(&execution_id, error, timestamp);
                false
            }
            BrowserEvent::PageClosed {
                page_id,
                timestamp,
            } => self.close_page(&page_id, timestamp),
            BrowserEvent::Disconnected { timestamp } => {
                if self.connection != ConnectionStatus::Fixture {
                    self.connection = ConnectionStatus::Disconnected;
                }
                self.push_event(LogEvent {
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
        self.push_event(LogEvent {
            timestamp: execution.started_at,
            kind: EventKind::ToolExecutionStarted,
            message: format!("{} {}", execution.tool_name, args),
        });
        self.push_execution(execution);
    }

    pub fn record_execution_finished(
        &mut self,
        id: &ExecutionId,
        result: Value,
        finished_at: DateTime<Utc>,
    ) {
        let Some(index) = self.executions.iter().position(|item| &item.id == id) else {
            let message = format!(
                "result for unknown execution {} — recorded, not applied",
                id.as_str()
            );
            self.record_late_result(message, finished_at);
            return;
        };
        if let Some(prior) = self.executions[index].finished_at {
            let delta = (finished_at - prior).num_milliseconds().max(0);
            let message = format!(
                "{} result arrived {delta}ms after it was closed — recorded, not applied",
                self.executions[index].tool_name
            );
            self.record_late_result(message, finished_at);
            return;
        }
        let message = {
            let execution = &mut self.executions[index];
            execution.result = Some(result);
            execution.error = None;
            execution.finished_at = Some(finished_at);
            format!(
                "{} {}ms",
                execution.tool_name,
                execution.duration_ms().unwrap_or(0)
            )
        };
        self.push_event(LogEvent {
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
        let Some(index) = self.executions.iter().position(|item| &item.id == id) else {
            let message = format!(
                "failure for unknown execution {} ({error}) — recorded, not applied",
                id.as_str()
            );
            self.record_late_result(message, finished_at);
            return;
        };
        if let Some(prior) = self.executions[index].finished_at {
            let delta = (finished_at - prior).num_milliseconds().max(0);
            let message = format!(
                "{} failure arrived {delta}ms after it was closed ({error}) — recorded, not applied",
                self.executions[index].tool_name
            );
            self.record_late_result(message, finished_at);
            return;
        }
        let message = {
            let execution = &mut self.executions[index];
            execution.error = Some(error.clone());
            execution.result = None;
            execution.finished_at = Some(finished_at);
            format!("{} {error}", execution.tool_name)
        };
        self.push_event(LogEvent {
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
            EventKind::LateResult,
            EventKind::ProtocolError,
        ];
        for kind in kinds {
            assert!(!kind.as_label().is_empty());
            let short = kind.short();
            assert!(!short.is_empty());
            assert_eq!(short, short.to_lowercase(), "{short} must be lowercase");
            assert!(short.len() <= 8, "{short} is too wide for the kind column");
        }
        // Distinct, or two different things read the same in the log.
        let mut shorts: Vec<&str> = kinds.iter().map(|k| k.short()).collect();
        shorts.sort_unstable();
        shorts.dedup();
        assert_eq!(shorts.len(), kinds.len());
    }

    fn empty_state() -> DebuggerState {
        DebuggerState {
            pages: Vec::new(),
            selected_page: None,
            tools: Vec::new(),
            tools_by_page: Default::default(),
            selected_tool: None,
            executions: Vec::new(),
            events: Vec::new(),
            connection: ConnectionStatus::Fixture,
        }
    }

    fn start(state: &mut DebuggerState, id: &str, at: DateTime<Utc>) -> ExecutionId {
        let id = ExecutionId::from(id);
        state.record_execution_started(ToolExecution {
            id: id.clone(),
            tool_name: "create_order".into(),
            arguments: serde_json::json!({}),
            result: None,
            error: None,
            started_at: at,
            finished_at: None,
        });
        id
    }

    #[test]
    fn a_result_arriving_after_the_run_settled_is_recorded_not_dropped() {
        let mut state = empty_state();
        let t0 = Utc.with_ymd_and_hms(2026, 8, 31, 14, 23, 0).unwrap();
        let id = start(&mut state, "exec_1", t0);

        state.record_execution_failed(&id, "timed out waiting for Chrome".into(), t0 + chrono::Duration::seconds(15));
        // The real result turns up three seconds after we gave up on it.
        state.record_execution_finished(
            &id,
            serde_json::json!({"order_id": "ord_8812"}),
            t0 + chrono::Duration::milliseconds(18_200),
        );

        let late: Vec<_> = state
            .events
            .iter()
            .filter(|event| event.kind == EventKind::LateResult)
            .collect();
        assert_eq!(late.len(), 1, "the late result must leave a trace");
        assert!(late[0].message.contains("3200ms after it was closed"));

        // ...but it must not rewrite the outcome that was already settled.
        let execution = state.last_execution_for("create_order").unwrap();
        assert_eq!(execution.error.as_deref(), Some("timed out waiting for Chrome"));
        assert!(execution.result.is_none());
    }

    #[test]
    fn a_result_for_an_unknown_execution_is_recorded() {
        let mut state = empty_state();
        let at = Utc.with_ymd_and_hms(2026, 8, 31, 14, 25, 0).unwrap();
        state.record_execution_finished(&ExecutionId::from("exec_ghost"), serde_json::json!(1), at);

        let event = state.events.last().unwrap();
        assert_eq!(event.kind, EventKind::LateResult);
        assert!(event.message.contains("exec_ghost"));
    }

    #[test]
    fn an_unparsable_frame_is_recorded_with_its_payload_truncated() {
        let mut state = empty_state();
        let at = Utc.with_ymd_and_hms(2026, 8, 31, 14, 26, 0).unwrap();
        let huge = "x".repeat(MAX_RAW_FRAME * 2);
        state.record_protocol_error("expected a number", &huge, at);

        let event = state.events.last().unwrap();
        assert_eq!(event.kind, EventKind::ProtocolError);
        assert!(event.message.starts_with("expected a number: "));
        assert!(event.message.ends_with('…'));
        assert!(event.message.len() < huge.len());
    }

    #[test]
    fn the_event_log_evicts_the_oldest_once_it_is_full() {
        let mut state = empty_state();
        let at = Utc.with_ymd_and_hms(2026, 8, 31, 14, 27, 0).unwrap();
        for n in 0..MAX_EVENTS + 25 {
            state.record_late_result(format!("event {n}"), at);
        }
        assert_eq!(state.events.len(), MAX_EVENTS);
        assert_eq!(state.events.first().unwrap().message, "event 25");
        assert_eq!(
            state.events.last().unwrap().message,
            format!("event {}", MAX_EVENTS + 24)
        );
    }

    #[test]
    fn an_unfinished_execution_is_never_evicted() {
        let mut state = empty_state();
        let at = Utc.with_ymd_and_hms(2026, 8, 31, 14, 28, 0).unwrap();

        // One run left in flight, then the cap's worth of completed runs on top.
        let pending = start(&mut state, "exec_pending", at);
        for n in 0..MAX_EXECUTIONS + 10 {
            let id = start(&mut state, &format!("exec_{n}"), at);
            state.record_execution_finished(&id, serde_json::json!(n), at);
        }

        assert!(state.executions.len() <= MAX_EXECUTIONS);
        assert!(
            state.executions.iter().any(|item| item.id == pending),
            "evicting an in-flight execution would strand its pending id"
        );
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
            tools_by_page: Default::default(),
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
            tools_by_page: Default::default(),
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

    fn tool_named(name: &str) -> Tool {
        Tool {
            name: name.into(),
            title: None,
            description: String::new(),
            input_schema: serde_json::json!({"type": "object"}),
            annotations: crate::types::ToolAnnotations::default(),
        }
    }

    fn tools_changed(page: &str, names: &[&str]) -> BrowserEvent {
        BrowserEvent::ToolsChanged {
            page_id: PageId::from(page),
            origin: format!("https://{page}.example"),
            url: format!("https://{page}.example/"),
            tools: names.iter().map(|name| tool_named(name)).collect(),
            timestamp: Utc.with_ymd_and_hms(2026, 8, 31, 14, 0, 0).unwrap(),
        }
    }

    #[test]
    fn switching_pages_shows_their_tools_immediately() {
        // The old behaviour dropped tools for any page that was not selected, so
        // switching showed an empty list until a fresh round-trip came back.
        let mut state = DebuggerState::waiting_for_extension();
        state.apply_browser_event(tools_changed("tab:1", &["get_user"]));
        state.apply_browser_event(tools_changed("tab:2", &["create_order", "cancel_order"]));

        assert_eq!(state.tools.len(), 1, "still on tab:1");
        assert!(state.select_page(PageId::from("tab:2")));
        assert_eq!(state.tools.len(), 2, "tab:2 tools were cached, not discarded");
        assert_eq!(state.selected_tool.as_deref(), Some("create_order"));

        assert!(state.select_page(PageId::from("tab:1")));
        assert_eq!(state.tools.len(), 1);
        assert_eq!(state.selected_tool.as_deref(), Some("get_user"));
    }

    #[test]
    fn selecting_the_page_you_are_already_on_changes_nothing() {
        let mut state = DebuggerState::waiting_for_extension();
        state.apply_browser_event(tools_changed("tab:1", &["get_user"]));
        assert!(!state.select_page(PageId::from("tab:1")));
    }

    #[test]
    fn a_closed_tab_leaves_the_page_list_and_takes_its_tools_with_it() {
        let mut state = DebuggerState::waiting_for_extension();
        state.apply_browser_event(tools_changed("tab:1", &["get_user"]));
        state.apply_browser_event(tools_changed("tab:2", &["create_order"]));
        let at = Utc.with_ymd_and_hms(2026, 8, 31, 14, 5, 0).unwrap();

        state.apply_browser_event(BrowserEvent::PageClosed {
            page_id: PageId::from("tab:2"),
            timestamp: at,
        });
        assert_eq!(state.pages.len(), 1);
        assert!(!state.tools_by_page.contains_key("tab:2"));
        assert_eq!(state.selected_page, Some(PageId::from("tab:1")), "selection untouched");
    }

    #[test]
    fn closing_the_page_you_are_on_falls_back_to_another() {
        let mut state = DebuggerState::waiting_for_extension();
        state.apply_browser_event(tools_changed("tab:1", &["get_user"]));
        state.apply_browser_event(tools_changed("tab:2", &["create_order"]));
        state.select_page(PageId::from("tab:2"));
        let at = Utc.with_ymd_and_hms(2026, 8, 31, 14, 6, 0).unwrap();

        assert!(state.apply_browser_event(BrowserEvent::PageClosed {
            page_id: PageId::from("tab:2"),
            timestamp: at,
        }));
        assert_eq!(state.selected_page, Some(PageId::from("tab:1")));
        assert_eq!(state.selected_tool.as_deref(), Some("get_user"));
    }

    #[test]
    fn closing_the_last_page_leaves_nothing_selected_rather_than_something_stale() {
        let mut state = DebuggerState::waiting_for_extension();
        state.apply_browser_event(tools_changed("tab:1", &["get_user"]));
        let at = Utc.with_ymd_and_hms(2026, 8, 31, 14, 7, 0).unwrap();
        state.apply_browser_event(BrowserEvent::PageClosed {
            page_id: PageId::from("tab:1"),
            timestamp: at,
        });
        assert!(state.pages.is_empty());
        assert!(state.selected_page.is_none());
        assert!(state.tools.is_empty());
        assert!(state.selected_tool.is_none());
    }

    #[test]
    fn the_previous_run_of_the_same_tool_is_findable() {
        let mut state = empty_state();
        let at = Utc.with_ymd_and_hms(2026, 8, 31, 14, 0, 0).unwrap();

        let first = start(&mut state, "exec_1", at);
        state.record_execution_finished(&first, serde_json::json!({"notes": 0}), at);
        // A different tool in between must not be picked up.
        let other = ExecutionId::from("exec_other");
        state.record_execution_started(ToolExecution {
            id: other.clone(),
            tool_name: "create_note".into(),
            arguments: serde_json::json!({}),
            result: None,
            error: None,
            started_at: at,
            finished_at: None,
        });
        state.record_execution_finished(&other, serde_json::json!({"ok": true}), at);
        let third = start(&mut state, "exec_3", at);
        state.record_execution_finished(&third, serde_json::json!({"notes": 1}), at);

        let previous = state
            .previous_result_for("create_order", &third)
            .expect("the earlier run of the same tool");
        assert_eq!(previous.id, first);
        assert_eq!(previous.result, Some(serde_json::json!({"notes": 0})));
    }

    #[test]
    fn the_first_run_of_a_tool_has_nothing_to_compare_against() {
        let mut state = empty_state();
        let at = Utc.with_ymd_and_hms(2026, 8, 31, 14, 0, 0).unwrap();
        let only = start(&mut state, "exec_1", at);
        state.record_execution_finished(&only, serde_json::json!({}), at);
        assert!(state.previous_result_for("create_order", &only).is_none());
    }

    #[test]
    fn a_run_that_failed_is_not_offered_as_a_comparison() {
        let mut state = empty_state();
        let at = Utc.with_ymd_and_hms(2026, 8, 31, 14, 0, 0).unwrap();
        let failed = start(&mut state, "exec_1", at);
        state.record_execution_failed(&failed, "boom".into(), at);
        let second = start(&mut state, "exec_2", at);
        state.record_execution_finished(&second, serde_json::json!({}), at);
        assert!(
            state.previous_result_for("create_order", &second).is_none(),
            "a failure has no result to compare with"
        );
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
