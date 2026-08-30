use gpui::{
    App, Application, Bounds, Context, SharedString, TitlebarOptions, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, rgb, size,
};
use webmcp_protocol::{DebuggerState, Page, PageId, Tool};

mod fixture;

use fixture::{FixtureBackend, ToolBackend};

const BG: u32 = 0x0F_17_2A;
const CARD: u32 = 0x1B_23_36;
const MUTED: u32 = 0x27_2F_42;
const BORDER: u32 = 0x47_55_69;
const FG: u32 = 0xF8_FA_FC;
const MUTED_FG: u32 = 0x94_A3_B8;
const ACCENT: u32 = 0x22_C5_5E;
const SELECTED: u32 = 0x33_41_55;

struct Debugger {
    state: DebuggerState,
}

impl Debugger {
    fn new() -> Self {
        Self {
            state: FixtureBackend.snapshot(),
        }
    }

    fn select_page(&mut self, id: PageId, cx: &mut Context<Self>) {
        self.state.selected_page = Some(id);
        cx.notify();
    }

    fn select_tool(&mut self, name: String, cx: &mut Context<Self>) {
        self.state.selected_tool = Some(name);
        cx.notify();
    }
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
                    .child(inspector(&self.state)),
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
                .child(
                    div()
                        .size(px(8.))
                        .rounded_full()
                        .bg(rgb(ACCENT)),
                )
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

fn inspector(state: &DebuggerState) -> impl IntoElement {
    let body = match state.selected_tool() {
        Some(tool) => inspector_body(tool),
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

fn inspector_body(tool: &Tool) -> gpui::Div {
    let schema = serde_json::to_string_pretty(&tool.input_schema)
        .unwrap_or_else(|_| "{}".to_string());
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
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .p_4()
        .gap_3()
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
            div().flex().flex_col().gap_1().child(muted_label("Description")).child(
                SharedString::from(tool.description.clone()),
            ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .gap_1()
                .child(muted_label("Schema"))
                .child(schema_view(schema)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(muted_label("Result"))
                .child(
                    div()
                        .text_color(rgb(MUTED_FG))
                        .child(SharedString::from("Execute is Phase 2")),
                ),
        )
}

fn schema_view(schema: String) -> impl IntoElement {
    div()
        .id("schema-scroll")
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
            schema
                .lines()
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

fn muted_label(label: &'static str) -> impl IntoElement {
    div()
        .text_color(rgb(MUTED_FG))
        .child(SharedString::from(label))
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1120.), px(720.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("WebMCP Debugger".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Debugger::new()),
        )
        .unwrap();
        cx.activate(true);
    });
}
