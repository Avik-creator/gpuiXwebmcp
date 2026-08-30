use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::Utc;
use gpui::{
    App, Application, Bounds, ClipboardItem, Context, Entity, FocusHandle, Focusable, KeyBinding,
    SharedString, TitlebarOptions, Window, WindowBounds, WindowOptions, actions, div, prelude::*,
    px, rgb, size,
};
use serde_json::{Map, Value, json};
use webmcp_protocol::{
    BrowserEvent, ConnectionStatus, DebuggerCommand, DebuggerState, EventKind, ExecutionId,
    LogEvent, Page, PageId, Tool, ToolExecution,
};

mod fixture;
mod input;
mod schema;
mod theme;

use debugger::ws::{BridgeEvent, ChromeBridge};
use fixture::{FixtureBackend, ToolBackend};
use input::{TextInput, bind_text_input_keys};
use schema::{
    FieldKind, FormField, FormSpec, arguments_from_primitive, form_spec_from_schema,
    required_fields_filled,
};
use theme::{GUTTER, INK, MUTE, PAPER, ROW, RUST, field_name, frame, hard_shadow, kind_cell, mono};

const EXECUTE_TIMEOUT: Duration = Duration::from_secs(15);
const BRIDGE_POLL: Duration = Duration::from_millis(50);

actions!(debugger, [ExecuteTool, ToggleBackend, CopyResult]);

fn bind_debugger_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-enter", ExecuteTool, Some("Debugger")),
        KeyBinding::new("ctrl-enter", ExecuteTool, Some("Debugger")),
        KeyBinding::new("ctrl-t", ToggleBackend, Some("Debugger")),
        KeyBinding::new("cmd-shift-c", CopyResult, Some("Debugger")),
    ]);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BackendKind {
    Fixture,
    Live,
}

struct FormWidget {
    field: FormField,
    input: Option<Entity<TextInput>>,
    bool_value: bool,
}

struct ToolForm {
    spec: FormSpec,
    widgets: Vec<FormWidget>,
    json_input: Option<Entity<TextInput>>,
}

impl ToolForm {
    fn empty() -> Self {
        Self {
            spec: FormSpec::JsonFallback,
            widgets: Vec::new(),
            json_input: None,
        }
    }
}

struct PendingExecution {
    id: ExecutionId,
}

struct Debugger {
    state: DebuggerState,
    form: ToolForm,
    pending: Option<PendingExecution>,
    execution_seq: u64,
    backend: BackendKind,
    bridge: Option<ChromeBridge>,
    extension_clients: usize,
    focus: FocusHandle,
}

impl Debugger {
    fn new(cx: &mut Context<Self>) -> Self {
        let (bridge, bind_error) = match ChromeBridge::bind() {
            Ok(bridge) => (Some(bridge), None),
            Err(error) => (None, Some(error.to_string())),
        };
        let mut this = Self {
            state: DebuggerState::waiting_for_extension(),
            form: ToolForm::empty(),
            pending: None,
            execution_seq: 0,
            backend: BackendKind::Live,
            bridge,
            extension_clients: 0,
            focus: cx.focus_handle(),
        };
        if let Some(error) = bind_error {
            this.state.events.push(LogEvent {
                timestamp: Utc::now(),
                kind: EventKind::Disconnected,
                message: format!("ws bind failed: {error}"),
            });
        }
        this.rebuild_form(cx);
        this.start_bridge_pump(cx);
        this
    }

    fn start_bridge_pump(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, async_cx| {
            loop {
                async_cx.background_executor().timer(BRIDGE_POLL).await;
                if this
                    .update(async_cx, |this, cx| this.drain_bridge(cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }

    fn drain_bridge(&mut self, cx: &mut Context<Self>) {
        let Some(bridge) = &self.bridge else {
            return;
        };
        let events = bridge.poll();
        if events.is_empty() {
            return;
        }
        let mut rebuild = false;
        let mut clear_pending = false;
        for event in events {
            match event {
                BridgeEvent::ClientsChanged { connected } => {
                    self.extension_clients = connected;
                    if self.backend == BackendKind::Live
                        && connected == 0
                        && self.state.connection != ConnectionStatus::Disconnected
                    {
                        let _ = self.state.apply_browser_event(BrowserEvent::Disconnected {
                            timestamp: Utc::now(),
                        });
                    }
                }
                BridgeEvent::Browser(browser) => {
                    if self.backend != BackendKind::Live {
                        continue;
                    }
                    let finishes_pending = match &browser {
                        BrowserEvent::ToolExecutionFinished { execution_id, .. }
                        | BrowserEvent::ToolExecutionFailed { execution_id, .. } => self
                            .pending
                            .as_ref()
                            .is_some_and(|pending| pending.id == *execution_id),
                        BrowserEvent::Hello { .. }
                        | BrowserEvent::PageChanged { .. }
                        | BrowserEvent::ToolsChanged { .. }
                        | BrowserEvent::ToolExecutionStarted { .. }
                        | BrowserEvent::Disconnected { .. } => false,
                    };
                    if self.state.apply_browser_event(browser) {
                        rebuild = true;
                    }
                    if finishes_pending {
                        clear_pending = true;
                    }
                }
            }
        }
        if clear_pending {
            self.pending = None;
        }
        if rebuild {
            self.rebuild_form(cx);
        }
        cx.notify();
    }

    fn toggle_backend(&mut self, cx: &mut Context<Self>) {
        match self.backend {
            BackendKind::Fixture => {
                self.backend = BackendKind::Live;
                self.pending = None;
                self.state = DebuggerState::waiting_for_extension();
                if self.extension_clients > 0 {
                    self.state.connection = ConnectionStatus::Connected;
                }
                self.rebuild_form(cx);
            }
            BackendKind::Live => {
                self.backend = BackendKind::Fixture;
                self.pending = None;
                self.state = FixtureBackend.snapshot();
                self.rebuild_form(cx);
            }
        }
        cx.notify();
    }

    fn select_page(&mut self, id: PageId, cx: &mut Context<Self>) {
        if self.state.selected_page.as_ref() == Some(&id) {
            return;
        }
        self.state.selected_page = Some(id.clone());
        if self.backend == BackendKind::Live {
            self.state.tools.clear();
            self.state.selected_tool = None;
            self.rebuild_form(cx);
            if let Some(bridge) = &self.bridge {
                let _ = bridge.send(&DebuggerCommand::SubscribePage { page_id: id });
            }
        }
        cx.notify();
    }

    fn select_tool(&mut self, name: String, cx: &mut Context<Self>) {
        if self.state.selected_tool.as_deref() == Some(name.as_str()) {
            return;
        }
        self.state.selected_tool = Some(name);
        self.rebuild_form(cx);
        cx.notify();
    }

    fn rebuild_form(&mut self, cx: &mut Context<Self>) {
        let Some(tool) = self.state.selected_tool().cloned() else {
            self.form = ToolForm::empty();
            return;
        };
        let spec = form_spec_from_schema(&tool.input_schema);
        let mut widgets = Vec::new();
        let mut json_input = None;
        match &spec {
            FormSpec::Primitive { fields } => {
                for field in fields {
                    let input = match field.kind {
                        FieldKind::Boolean => None,
                        FieldKind::String | FieldKind::Number | FieldKind::Integer => {
                            let entity = cx.new(|cx| TextInput::new(cx, field.kind.placeholder()));
                            cx.observe(&entity, |_, _, cx| cx.notify()).detach();
                            Some(entity)
                        }
                    };
                    widgets.push(FormWidget {
                        field: field.clone(),
                        input,
                        bool_value: false,
                    });
                }
            }
            FormSpec::JsonFallback => {
                let entity = cx.new(|cx| TextInput::new(cx, "JSON arguments"));
                cx.observe(&entity, |_, _, cx| cx.notify()).detach();
                json_input = Some(entity);
            }
        }
        self.form = ToolForm {
            spec,
            widgets,
            json_input,
        };
    }

    fn form_string_values(&self, cx: &App) -> Map<String, Value> {
        let mut map = Map::new();
        for widget in &self.form.widgets {
            if let Some(input) = &widget.input {
                map.insert(widget.field.name.clone(), Value::String(input.read(cx).text()));
            }
        }
        map
    }

    fn form_bool_values(&self) -> Map<String, Value> {
        let mut map = Map::new();
        for widget in &self.form.widgets {
            if widget.field.kind == FieldKind::Boolean {
                map.insert(widget.field.name.clone(), Value::Bool(widget.bool_value));
            }
        }
        map
    }

    fn json_text(&self, cx: &App) -> String {
        self.form
            .json_input
            .as_ref()
            .map(|input| input.read(cx).text())
            .unwrap_or_default()
    }

    fn numeric_fields_valid(&self, cx: &App) -> bool {
        self.form.widgets.iter().all(|widget| {
            let Some(input) = &widget.input else {
                return true;
            };
            let trimmed = input.read(cx).text();
            let trimmed = trimmed.trim();
            if trimmed.is_empty() {
                return !widget.field.required;
            }
            match widget.field.kind {
                FieldKind::Integer => trimmed.parse::<i64>().is_ok(),
                FieldKind::Number => trimmed.parse::<f64>().is_ok(),
                FieldKind::String | FieldKind::Boolean => true,
            }
        })
    }

    fn can_execute(&self, cx: &App) -> bool {
        if self.pending.is_some() || self.state.selected_tool.is_none() {
            return false;
        }
        if self.backend == BackendKind::Live
            && self.state.connection != ConnectionStatus::Connected
        {
            return false;
        }
        match &self.form.spec {
            FormSpec::Primitive { .. } => {
                required_fields_filled(&self.form.spec, &self.form_string_values(cx), "")
                    && self.numeric_fields_valid(cx)
            }
            FormSpec::JsonFallback => {
                required_fields_filled(&self.form.spec, &Map::new(), &self.json_text(cx))
            }
        }
    }

    fn collect_arguments(&self, cx: &App) -> Result<Value, String> {
        match &self.form.spec {
            FormSpec::Primitive { fields } => arguments_from_primitive(
                fields,
                &self.form_string_values(cx),
                &self.form_bool_values(),
            ),
            FormSpec::JsonFallback => {
                let text = self.json_text(cx);
                if text.trim().is_empty() {
                    Ok(json!({}))
                } else {
                    serde_json::from_str(&text).map_err(|err| err.to_string())
                }
            }
        }
    }

    fn execute_selected(&mut self, cx: &mut Context<Self>) {
        if !self.can_execute(cx) {
            return;
        }
        let Some(tool) = self.state.selected_tool.clone() else {
            return;
        };
        let Ok(arguments) = self.collect_arguments(cx) else {
            return;
        };
        self.execution_seq += 1;
        let id = ExecutionId::from(format!("exec_{}", self.execution_seq));
        self.state.record_execution_started(ToolExecution {
            id: id.clone(),
            tool_name: tool.clone(),
            arguments: arguments.clone(),
            result: None,
            error: None,
            started_at: Utc::now(),
            finished_at: None,
        });
        self.pending = Some(PendingExecution { id: id.clone() });
        cx.notify();
        match self.backend {
            BackendKind::Fixture => {
                let delay = fixture_delay();
                cx.spawn(async move |this, async_cx| {
                    async_cx.background_executor().timer(delay).await;
                    let result = FixtureBackend.execute(&tool, &arguments);
                    this.update(async_cx, |this, cx| {
                        this.complete_execution(id, result, cx);
                    })
                    .ok();
                })
                .detach();
            }
            BackendKind::Live => {
                let Some(page_id) = self.state.selected_page.clone() else {
                    self.complete_execution(id, Err("no page selected".to_string()), cx);
                    return;
                };
                let sent = self.bridge.as_ref().is_some_and(|bridge| {
                    bridge.send(&DebuggerCommand::ExecuteTool {
                        page_id,
                        tool,
                        arguments,
                        execution_id: id.clone(),
                    })
                });
                if !sent {
                    self.complete_execution(
                        id,
                        Err("extension not connected".to_string()),
                        cx,
                    );
                    return;
                }
                cx.spawn(async move |this, async_cx| {
                    async_cx.background_executor().timer(EXECUTE_TIMEOUT).await;
                    this.update(async_cx, |this, cx| {
                        if this.pending.as_ref().is_some_and(|pending| pending.id == id) {
                            this.complete_execution(
                                id,
                                Err("timed out waiting for Chrome".to_string()),
                                cx,
                            );
                        }
                    })
                    .ok();
                })
                .detach();
            }
        }
    }

    fn complete_execution(
        &mut self,
        id: ExecutionId,
        result: Result<Value, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(pending) = self.pending.as_ref() else {
            return;
        };
        if pending.id != id {
            return;
        }
        let finished_at = Utc::now();
        match result {
            Ok(value) => self
                .state
                .record_execution_finished(&id, value, finished_at),
            Err(error) => self.state.record_execution_failed(&id, error, finished_at),
        }
        self.pending = None;
        cx.notify();
    }

    fn copy_last_result(&mut self, cx: &mut Context<Self>) {
        let Some(name) = self.state.selected_tool.as_deref() else {
            return;
        };
        let Some(execution) = self.state.last_execution_for(name) else {
            return;
        };
        let text = if let Some(error) = &execution.error {
            error.clone()
        } else if let Some(result) = &execution.result {
            pretty_json(result)
        } else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }
}

impl Focusable for Debugger {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

fn fixture_delay() -> Duration {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos())
        .unwrap_or(0);
    Duration::from_millis(50 + u64::from(nanos % 151))
}

impl Render for Debugger {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context("Debugger")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &ExecuteTool, _, cx| this.execute_selected(cx)))
            .on_action(cx.listener(|this, _: &ToggleBackend, _, cx| this.toggle_backend(cx)))
            .on_action(cx.listener(|this, _: &CopyResult, _, cx| this.copy_last_result(cx)))
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(PAPER))
            .text_color(rgb(INK))
            .font(mono())
            .text_size(px(12.))
            .line_height(px(18.))
            .p(px(GUTTER))
            .gap(px(GUTTER))
            .child(header(self, cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .gap(px(GUTTER))
                    .child(page_list(&self.state, cx))
                    .child(tool_list(&self.state, cx))
                    .child(inspector(self, cx)),
            )
            .child(event_log(&self.state))
            .child(keymap_bar())
    }
}

fn header(debugger: &Debugger, cx: &mut Context<Debugger>) -> impl IntoElement {
    let origin = debugger
        .state
        .selected_page()
        .map(|page| page.origin.clone())
        .unwrap_or_else(|| "NO PAGE".to_string());
    let (tag, fault) = status_tag(debugger);

    frame()
        .flex_row()
        .items_center()
        .justify_between()
        .flex_shrink_0()
        .h(px(ROW + 8.))
        .px_3()
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .min_w_0()
                .child(SharedString::from("WEBMCP"))
                .child(
                    div()
                        .min_w_0()
                        .text_color(rgb(MUTE))
                        .truncate()
                        .child(SharedString::from(origin)),
                ),
        )
        .child(
            div()
                .id("connection-status")
                .flex()
                .items_center()
                .px_2()
                .h(px(ROW))
                .cursor_pointer()
                .text_color(rgb(if fault { RUST } else { INK }))
                .on_click(cx.listener(|this, _, _, cx| this.toggle_backend(cx)))
                .child(SharedString::from(tag)),
        )
}

fn status_tag(debugger: &Debugger) -> (String, bool) {
    match debugger.backend {
        BackendKind::Fixture => ("[ FIXTURE ]".into(), false),
        BackendKind::Live => match debugger.state.connection {
            ConnectionStatus::Connected => {
                (format!("[ LIVE · {} ]", debugger.extension_clients), false)
            }
            ConnectionStatus::Disconnected | ConnectionStatus::Fixture => {
                ("[ WAIT ]".into(), true)
            }
        },
    }
}

fn page_list(state: &DebuggerState, cx: &mut Context<Debugger>) -> gpui::Div {
    if state.pages.is_empty() {
        return column("[ PAGES ]", px(228.), std::iter::once(empty_hint("NO PAGES")));
    }
    let selected = state.selected_page.clone();
    column(
        "[ PAGES ]",
        px(228.),
        state.pages.iter().map(|page| {
            let id = page.id.clone();
            let is_selected = selected.as_ref() == Some(&id);
            let row = page_row(page, is_selected);
            row.id(SharedString::from(format!("page-{}", id.as_str())))
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.select_page(id.clone(), cx);
                }))
        }),
    )
}

fn page_row(page: &Page, selected: bool) -> gpui::Div {
    let fg = if selected { PAPER } else { INK };
    let dim = if selected { PAPER } else { MUTE };
    let mark = if selected { "*" } else { " " };
    div()
        .flex()
        .flex_col()
        .px_2()
        .py_1()
        .when(selected, |el| el.bg(rgb(INK)).text_color(rgb(PAPER)))
        .when(!selected, |el| el.hover(|style| style.bg(rgb(theme::HOVER))))
        .child(
            div()
                .flex()
                .gap_2()
                .min_w_0()
                .child(SharedString::from(mark))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(rgb(fg))
                        .child(SharedString::from(page.origin.clone())),
                ),
        )
        .child(
            div()
                .pl_4()
                .truncate()
                .text_color(rgb(dim))
                .child(SharedString::from(page.title.clone())),
        )
}

fn tool_list(state: &DebuggerState, cx: &mut Context<Debugger>) -> gpui::Div {
    if state.tools.is_empty() {
        return column("[ TOOLS ]", px(268.), std::iter::once(empty_hint("NO TOOLS")));
    }
    let selected = state.selected_tool.clone();
    column(
        "[ TOOLS ]",
        px(268.),
        state.tools.iter().map(|tool| {
            let name = tool.name.clone();
            let is_selected = selected.as_deref() == Some(name.as_str());
            let row = tool_row(tool, is_selected);
            row.id(SharedString::from(format!("tool-{name}")))
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.select_tool(name.clone(), cx);
                }))
        }),
    )
}

fn tool_row(tool: &Tool, selected: bool) -> gpui::Div {
    let fg = if selected { PAPER } else { INK };
    let dim = if selected { PAPER } else { MUTE };
    let mark = if selected { "*" } else { " " };
    let title = tool.title.clone().unwrap_or_else(|| tool.name.clone());
    div()
        .flex()
        .flex_col()
        .px_2()
        .py_1()
        .when(selected, |el| el.bg(rgb(INK)).text_color(rgb(PAPER)))
        .when(!selected, |el| el.hover(|style| style.bg(rgb(theme::HOVER))))
        .child(
            div()
                .flex()
                .gap_2()
                .min_w_0()
                .child(SharedString::from(mark))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(rgb(fg))
                        .child(SharedString::from(tool.name.clone())),
                )
                .children(annotation_marks(tool).into_iter().map(|mark| {
                    div()
                        .flex_shrink_0()
                        .text_color(rgb(dim))
                        .child(SharedString::from(mark))
                })),
        )
        .child(
            div()
                .pl_4()
                .truncate()
                .text_color(rgb(dim))
                .child(SharedString::from(title)),
        )
}

fn annotation_marks(tool: &Tool) -> Vec<String> {
    match (
        tool.annotations.read_only_hint,
        tool.annotations.untrusted_content_hint,
    ) {
        (Some(true), Some(true)) => vec!["[RO]".into(), "[UNTRUSTED]".into()],
        (Some(true), _) => vec!["[RO]".into()],
        (_, Some(true)) => vec!["[UNTRUSTED]".into()],
        _ => Vec::new(),
    }
}

fn empty_hint(text: &'static str) -> gpui::Div {
    div()
        .px_2()
        .py_1()
        .text_color(rgb(MUTE))
        .child(SharedString::from(text))
}

fn inspector(debugger: &Debugger, cx: &mut Context<Debugger>) -> impl IntoElement {
    let enabled = debugger.can_execute(cx);
    let running = debugger.pending.is_some();
    let body = match debugger.state.selected_tool() {
        Some(tool) => inspector_body(debugger, tool, enabled, running, cx).into_any_element(),
        None => div()
            .p_3()
            .text_color(rgb(MUTE))
            .child(SharedString::from("SELECT A TOOL"))
            .into_any_element(),
    };

    frame()
        .flex_1()
        .min_w_0()
        .shadow(hard_shadow())
        .child(theme::bracket("[ INSPECT ]"))
        .child(body)
}

fn inspector_body(
    debugger: &Debugger,
    tool: &Tool,
    enabled: bool,
    running: bool,
    cx: &mut Context<Debugger>,
) -> impl IntoElement {
    let schema = pretty_json(&tool.input_schema);
    let hint = match (
        tool.annotations.read_only_hint,
        tool.annotations.untrusted_content_hint,
    ) {
        (Some(true), Some(true)) => "RO · UNTRUSTED OUTPUT",
        (Some(true), _) => "RO",
        (_, Some(true)) => "UNTRUSTED OUTPUT",
        _ => "",
    };

    div()
        .id("inspector-scroll")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .p_3()
        .gap_3()
        .overflow_scroll()
        .child(SharedString::from(tool.name.clone()))
        .when(!hint.is_empty(), |el| {
            el.child(
                div()
                    .text_color(rgb(MUTE))
                    .child(SharedString::from(hint)),
            )
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(muted_label("DESCRIPTION"))
                .child(SharedString::from(tool.description.clone())),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .h(px(132.))
                .gap_1()
                .child(muted_label("SCHEMA"))
                .child(code_view("schema-scroll", schema)),
        )
        .child(arguments_form(debugger, cx))
        .child(execute_button(enabled, running, cx))
        .child(result_panel(debugger, &tool.name, cx))
}

fn arguments_form(debugger: &Debugger, cx: &mut Context<Debugger>) -> gpui::Div {
    let mut body = div()
        .flex()
        .flex_col()
        .gap_1()
        .child(muted_label("ARGUMENTS"));
    match &debugger.form.spec {
        FormSpec::Primitive { fields } if fields.is_empty() => {
            body = body.child(
                div()
                    .text_color(rgb(MUTE))
                    .child(SharedString::from("NONE")),
            );
        }
        FormSpec::Primitive { .. } => {
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(debugger.form.widgets.iter().map(|widget| field_row(widget, cx))),
            );
        }
        FormSpec::JsonFallback => {
            if let Some(input) = debugger.form.json_input.clone() {
                body = body.child(input);
            }
        }
    }
    body
}

fn field_row(widget: &FormWidget, cx: &mut Context<Debugger>) -> gpui::Div {
    let title = field_name(&widget.field.name, widget.field.required);
    let mut row = div()
        .flex()
        .flex_col()
        .gap_1()
        .child(muted_label(title));
    match widget.field.kind {
        FieldKind::Boolean => {
            row = row.child(bool_toggle(
                widget.field.name.clone(),
                widget.bool_value,
                cx,
            ));
        }
        FieldKind::String | FieldKind::Number | FieldKind::Integer => {
            if let Some(input) = widget.input.clone() {
                row = row.child(input);
            }
        }
    }
    row
}

fn bool_toggle(name: String, value: bool, cx: &mut Context<Debugger>) -> impl IntoElement {
    let id = SharedString::from(format!("bool-{name}"));
    let label = if value { "[ TRUE ]" } else { "[ FALSE ]" };
    div()
        .id(id)
        .flex()
        .items_center()
        .h(px(ROW))
        .px_2()
        .border_1()
        .border_dashed()
        .border_color(rgb(theme::RULE))
        .when(value, |el| el.bg(rgb(INK)).text_color(rgb(PAPER)))
        .when(!value, |el| el.bg(rgb(PAPER)).text_color(rgb(MUTE)))
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, _, cx| {
            if let Some(widget) = this
                .form
                .widgets
                .iter_mut()
                .find(|widget| widget.field.name == name)
            {
                widget.bool_value = !widget.bool_value;
            }
            cx.notify();
        }))
        .child(SharedString::from(label))
}

fn execute_button(enabled: bool, running: bool, cx: &mut Context<Debugger>) -> impl IntoElement {
    let label = if running { "WORKING" } else { "↵ EXECUTE" };
    div()
        .id("execute")
        .flex()
        .items_center()
        .justify_center()
        .h(px(ROW))
        .px_3()
        .border_1()
        .border_color(rgb(INK))
        .when(enabled, |el| {
            el.bg(rgb(INK))
                .text_color(rgb(PAPER))
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| this.execute_selected(cx)))
        })
        .when(!enabled, |el| {
            el.border_dashed()
                .border_color(rgb(theme::RULE))
                .text_color(rgb(MUTE))
        })
        .child(SharedString::from(label))
}

fn result_panel(
    debugger: &Debugger,
    tool_name: &str,
    cx: &mut Context<Debugger>,
) -> gpui::Div {
    let execution = debugger.state.last_execution_for(tool_name);
    let is_running = debugger.pending.as_ref().is_some_and(|pending| {
        execution
            .map(|item| item.id == pending.id)
            .unwrap_or(false)
    });
    let can_copy = execution.is_some_and(|item| item.result.is_some() || item.error.is_some());

    let panel = div().flex().flex_col().gap_1().child(
        div()
            .flex()
            .items_center()
            .justify_between()
            .child(muted_label("RESULT"))
            .when(can_copy, |el| {
                el.child(
                    div()
                        .id("copy-result")
                        .cursor_pointer()
                        .text_color(rgb(MUTE))
                        .on_click(cx.listener(|this, _, _, cx| this.copy_last_result(cx)))
                        .child(SharedString::from("[ COPY ]")),
                )
            }),
    );

    if is_running {
        return panel.child(
            div()
                .text_color(rgb(MUTE))
                .child(SharedString::from("WORKING")),
        );
    }

    match execution {
        Some(execution) if execution.error.is_some() => panel.child(
            div()
                .text_color(rgb(RUST))
                .child(SharedString::from(execution.error.clone().unwrap())),
        ),
        Some(execution) if execution.result.is_some() => {
            let duration = execution
                .duration_ms()
                .map(|ms| format!("{ms}ms"))
                .unwrap_or_else(|| "—".into());
            panel
                .child(
                    div()
                        .text_color(rgb(MUTE))
                        .child(SharedString::from(duration)),
                )
                .child(code_view(
                    "result-scroll",
                    pretty_json(execution.result.as_ref().unwrap()),
                ))
        }
        _ => panel.child(
            div()
                .text_color(rgb(MUTE))
                .child(SharedString::from("NONE")),
        ),
    }
}

fn pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string())
}

fn code_view(id: &'static str, body: String) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .p_2()
        .border_1()
        .border_dashed()
        .border_color(rgb(theme::RULE))
        .bg(rgb(PAPER))
        .overflow_scroll()
        .text_color(rgb(INK))
        .children(
            body.lines()
                .map(|line| div().child(SharedString::from(line.to_string()))),
        )
}

fn event_log(state: &DebuggerState) -> impl IntoElement {
    frame()
        .h(px(168.))
        .flex_shrink_0()
        .child(theme::bracket("[ LOG ]"))
        .child(
            div()
                .id("event-log-scroll")
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .px_2()
                .py_1()
                .overflow_scroll()
                .children({
                    let start = state.events.len().saturating_sub(20);
                    state.events[start..].iter().map(|event| {
                        let line = format!(
                            "{}  {}  {}",
                            event.timestamp.format("%H:%M:%S"),
                            kind_cell(event.kind.as_label()),
                            event.message
                        );
                        let color = match event.kind {
                            EventKind::ToolExecutionFailed | EventKind::Disconnected => RUST,
                            EventKind::Hello
                            | EventKind::PageChanged
                            | EventKind::ToolsChanged
                            | EventKind::ToolExecutionStarted
                            | EventKind::ToolExecutionFinished => MUTE,
                        };
                        div()
                            .text_color(rgb(color))
                            .child(SharedString::from(line))
                    })
                }),
        )
}

fn keymap_bar() -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_4()
        .px_1()
        .flex_shrink_0()
        .text_color(rgb(MUTE))
        .child(SharedString::from("⌘↵ EXECUTE"))
        .child(SharedString::from("⌃T MODE"))
        .child(SharedString::from("⌘⇧C COPY"))
}

fn column(
    title: &'static str,
    width: gpui::Pixels,
    children: impl IntoIterator<Item = impl IntoElement>,
) -> gpui::Div {
    frame()
        .w(width)
        .flex_shrink_0()
        .child(theme::bracket(title))
        .child(
            div()
                .id(SharedString::from(title))
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .py_1()
                .overflow_scroll()
                .children(children),
        )
}

fn muted_label(label: impl Into<SharedString>) -> impl IntoElement {
    div().text_color(rgb(MUTE)).child(label.into())
}

fn main() {
    Application::new().run(|cx: &mut App| {
        bind_text_input_keys(cx);
        bind_debugger_keys(cx);
        let bounds = Bounds::centered(None, size(px(1200.), px(800.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("WEBMCP".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                let debugger = cx.new(Debugger::new);
                window.focus(&debugger.read(cx).focus);
                debugger
            },
        )
        .unwrap();
        cx.activate(true);
    });
}

