use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::Utc;
use gpui::{
    App, Application, Bounds, Context, Entity, SharedString, TitlebarOptions, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, rgb, size,
};
use serde_json::{Map, Value, json};
use webmcp_protocol::{DebuggerState, ExecutionId, Page, PageId, Tool, ToolExecution};

mod fixture;
mod input;
mod schema;

use fixture::{FixtureBackend, ToolBackend};
use input::{TextInput, bind_text_input_keys};
use schema::{
    FieldKind, FormField, FormSpec, arguments_from_primitive, form_spec_from_schema,
    required_fields_filled,
};

const BG: u32 = 0x0F_17_2A;
const CARD: u32 = 0x1B_23_36;
const MUTED: u32 = 0x27_2F_42;
const BORDER: u32 = 0x47_55_69;
const FG: u32 = 0xF8_FA_FC;
const MUTED_FG: u32 = 0x94_A3_B8;
const ACCENT: u32 = 0x22_C5_5E;
const SELECTED: u32 = 0x33_41_55;
const ERROR: u32 = 0xF8_71_71;

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
}

impl Debugger {
    fn new(cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            state: FixtureBackend.snapshot(),
            form: ToolForm::empty(),
            pending: None,
            execution_seq: 0,
        };
        this.rebuild_form(cx);
        this
    }

    fn select_page(&mut self, id: PageId, cx: &mut Context<Self>) {
        self.state.selected_page = Some(id);
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
        let delay = fixture_delay();
        cx.notify();
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
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(BG))
            .text_color(rgb(FG))
            .text_sm()
            .child(header(&self.state))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(page_list(&self.state, cx))
                    .child(tool_list(&self.state, cx))
                    .child(inspector(self, cx)),
            )
            .child(event_log(&self.state))
    }
}

fn header(state: &DebuggerState) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .px_4()
        .py_3()
        .border_b_1()
        .border_color(rgb(BORDER))
        .bg(rgb(CARD))
        .child(
            div()
                .text_color(rgb(FG))
                .child(SharedString::from("WebMCP Debugger")),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(div().size(px(8.)).rounded_full().bg(rgb(ACCENT)))
                .child(
                    div()
                        .text_color(rgb(MUTED_FG))
                        .child(SharedString::from(state.connection.as_label())),
                ),
        )
}

fn page_list(state: &DebuggerState, cx: &mut Context<Debugger>) -> impl IntoElement {
    let selected = state.selected_page.clone();
    column(
        "Pages",
        px(220.),
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
    let bg = if selected { SELECTED } else { CARD };
    div()
        .flex()
        .flex_col()
        .gap_1()
        .px_3()
        .py_2()
        .rounded_md()
        .bg(rgb(bg))
        .hover(|style| style.bg(rgb(MUTED)))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .size(px(6.))
                        .rounded_full()
                        .bg(rgb(if selected { ACCENT } else { MUTED_FG })),
                )
                .child(SharedString::from(page.origin.clone())),
        )
        .child(
            div()
                .pl_4()
                .text_color(rgb(MUTED_FG))
                .child(SharedString::from(page.title.clone())),
        )
}

fn tool_list(state: &DebuggerState, cx: &mut Context<Debugger>) -> impl IntoElement {
    let selected = state.selected_tool.clone();
    column(
        "Tools",
        px(260.),
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
    let bg = if selected { SELECTED } else { CARD };
    let label = tool.title.clone().unwrap_or_else(|| tool.name.clone());
    div()
        .flex()
        .flex_col()
        .gap_1()
        .px_3()
        .py_2()
        .rounded_md()
        .bg(rgb(bg))
        .hover(|style| style.bg(rgb(MUTED)))
        .child(SharedString::from(tool.name.clone()))
        .child(
            div()
                .text_color(rgb(MUTED_FG))
                .child(SharedString::from(label)),
        )
}

fn inspector(debugger: &Debugger, cx: &mut Context<Debugger>) -> impl IntoElement {
    let enabled = debugger.can_execute(cx);
    let running = debugger.pending.is_some();
    let body = match debugger.state.selected_tool() {
        Some(tool) => inspector_body(debugger, tool, enabled, running, cx),
        None => div()
            .p_4()
            .text_color(rgb(MUTED_FG))
            .child(SharedString::from("Select a tool")),
    };

    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .border_l_1()
        .border_color(rgb(BORDER))
        .child(section_title("Inspector"))
        .child(body)
}

fn inspector_body(
    debugger: &Debugger,
    tool: &Tool,
    enabled: bool,
    running: bool,
    cx: &mut Context<Debugger>,
) -> gpui::Div {
    let schema = pretty_json(&tool.input_schema);
    let hint = match (
        tool.annotations.read_only_hint,
        tool.annotations.untrusted_content_hint,
    ) {
        (Some(true), Some(true)) => "read-only, untrusted output",
        (Some(true), _) => "read-only",
        (_, Some(true)) => "untrusted output",
        _ => "",
    };

    div()
        .id("inspector-scroll")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .p_4()
        .gap_3()
        .overflow_scroll()
        .child(
            div()
                .text_color(rgb(FG))
                .child(SharedString::from(tool.name.clone())),
        )
        .when(!hint.is_empty(), |el| {
            el.child(
                div()
                    .text_color(rgb(MUTED_FG))
                    .child(SharedString::from(hint)),
            )
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(muted_label("Description"))
                .child(SharedString::from(tool.description.clone())),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .h(px(132.))
                .gap_1()
                .child(muted_label("Schema"))
                .child(code_view("schema-scroll", schema)),
        )
        .child(arguments_form(debugger, cx))
        .child(execute_button(enabled, running, cx))
        .child(result_panel(debugger, &tool.name))
}

fn arguments_form(debugger: &Debugger, cx: &mut Context<Debugger>) -> gpui::Div {
    let mut body = div()
        .flex()
        .flex_col()
        .gap_1()
        .child(muted_label("Arguments"));
    match &debugger.form.spec {
        FormSpec::Primitive { fields } if fields.is_empty() => {
            body = body.child(
                div()
                    .text_color(rgb(MUTED_FG))
                    .child(SharedString::from("No arguments")),
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
    let title = if widget.field.required {
        format!("{} *", widget.field.name)
    } else {
        widget.field.name.clone()
    };
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

fn bool_toggle(name: String, value: bool, cx: &mut Context<Debugger>) -> gpui::Div {
    let id = SharedString::from(format!("bool-{name}"));
    div()
        .id(id)
        .flex()
        .items_center()
        .h(px(32.))
        .px_2()
        .rounded_md()
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(MUTED))
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
        .child(SharedString::from(if value { "true" } else { "false" }))
}

fn execute_button(enabled: bool, running: bool, cx: &mut Context<Debugger>) -> gpui::Div {
    let label = if running { "Executing…" } else { "Execute" };
    let bg = if enabled { ACCENT } else { MUTED };
    div()
        .id("execute")
        .flex()
        .items_center()
        .justify_center()
        .h(px(32.))
        .px_4()
        .rounded_md()
        .bg(rgb(bg))
        .text_color(rgb(FG))
        .when(enabled, |el| {
            el.cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| this.execute_selected(cx)))
        })
        .child(SharedString::from(label))
}

fn result_panel(debugger: &Debugger, tool_name: &str) -> gpui::Div {
    let execution = debugger.state.last_execution_for(tool_name);
    let is_running = debugger.pending.as_ref().is_some_and(|pending| {
        execution
            .map(|item| item.id == pending.id)
            .unwrap_or(false)
    });

    let mut panel = div()
        .flex()
        .flex_col()
        .gap_1()
        .child(muted_label("Result"));

    if is_running {
        return panel.child(
            div()
                .text_color(rgb(MUTED_FG))
                .child(SharedString::from("Executing…")),
        );
    }

    match execution {
        Some(execution) if execution.error.is_some() => panel.child(
            div()
                .text_color(rgb(ERROR))
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
                        .text_color(rgb(MUTED_FG))
                        .child(SharedString::from(duration)),
                )
                .child(code_view(
                    "result-scroll",
                    pretty_json(execution.result.as_ref().unwrap()),
                ))
        }
        _ => panel.child(
            div()
                .text_color(rgb(MUTED_FG))
                .child(SharedString::from("No result yet")),
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
        .p_3()
        .rounded_md()
        .bg(rgb(MUTED))
        .overflow_scroll()
        .text_color(rgb(FG))
        .children(
            body.lines()
                .map(|line| div().child(SharedString::from(line.to_string()))),
        )
}

fn event_log(state: &DebuggerState) -> impl IntoElement {
    div()
        .h(px(168.))
        .flex()
        .flex_col()
        .border_t_1()
        .border_color(rgb(BORDER))
        .bg(rgb(CARD))
        .child(section_title("Event Log"))
        .child(
            div()
                .id("event-log-scroll")
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .px_4()
                .pb_3()
                .gap_1()
                .overflow_scroll()
                .children(state.events.iter().map(|event| {
                    let line = format!(
                        "{}  {}  {}",
                        event.timestamp.format("%H:%M:%S"),
                        event.kind.as_label(),
                        event.message
                    );
                    div()
                        .text_color(rgb(MUTED_FG))
                        .child(SharedString::from(line))
                })),
        )
}

fn column(
    title: &'static str,
    width: gpui::Pixels,
    children: impl IntoIterator<Item = impl IntoElement>,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .w(width)
        .flex_shrink_0()
        .border_r_1()
        .border_color(rgb(BORDER))
        .child(section_title(title))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .p_2()
                .gap_1()
                .children(children),
        )
}

fn section_title(title: &'static str) -> impl IntoElement {
    div()
        .px_4()
        .py_2()
        .text_color(rgb(MUTED_FG))
        .border_b_1()
        .border_color(rgb(BORDER))
        .child(SharedString::from(title))
}

fn muted_label(label: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_color(rgb(MUTED_FG))
        .child(label.into())
}

fn main() {
    Application::new().run(|cx: &mut App| {
        bind_text_input_keys(cx);
        let bounds = Bounds::centered(None, size(px(1200.), px(800.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("WebMCP Debugger".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(Debugger::new),
        )
        .unwrap();
        cx.activate(true);
    });
}
