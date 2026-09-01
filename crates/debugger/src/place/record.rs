//! Record — what has happened.
//!
//! One stream at two altitudes. `RUNS` answers "what did I do?"; `EVERYTHING`
//! answers "what actually crossed the wire?". Same column, same rhythm, so the
//! toggle changes the level of detail rather than taking you somewhere else.

use gpui::{div, prelude::*, px, rgb, Context, SharedString};
use webmcp_protocol::{LogEvent, ToolExecution};

use super::{column, label, stage};
use crate::shell::Altitude;
use crate::theme::{self, space, text, Palette};
use crate::Debugger;

/// How a finished run reads on the right of its row.
///
/// A failure shows the page's own message, so you never have to open a run to
/// learn why it failed.
pub fn outcome(execution: &ToolExecution) -> (String, bool) {
    if let Some(error) = &execution.error {
        return (error.clone(), true);
    }
    match execution.duration_ms() {
        Some(ms) => (format!("{ms}ms"), false),
        None => ("running".to_string(), false),
    }
}

pub fn render(debugger: &Debugger, cx: &mut Context<Debugger>) -> gpui::Div {
    let palette = theme::theme(cx);
    let altitude = debugger.altitude;
    let state = &debugger.state;

    let mut toggle = Vec::with_capacity(2);
    for option in [Altitude::Runs, Altitude::Everything] {
        toggle.push(
            label(palette, option.label())
                .id(SharedString::from(option.label()))
                .cursor_pointer()
                .text_color(rgb(if option == altitude {
                    palette.ink
                } else {
                    palette.mute
                }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.altitude = option;
                    cx.notify();
                })),
        );
    }

    let tally = match altitude {
        Altitude::Runs => format!("{} runs this session", state.executions.len()),
        Altitude::Everything => format!(
            "{} events · keeping the last {}",
            state.events.len(),
            webmcp_protocol::MAX_EVENTS
        ),
    };

    let rows = match altitude {
        Altitude::Runs => runs(debugger, palette, cx),
        Altitude::Everything => everything(state.events.iter(), palette),
    };

    stage().child(
        column(space::HUGE)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(space::MD))
                    .flex_shrink_0()
                    .children(toggle)
                    .child(div().flex_1())
                    .child(label(palette, SharedString::from(tally))),
            )
            .child(
                div()
                    .id("record-scroll")
                    .track_scroll(&debugger.record_scroll)
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .mt(px(space::XL))
                    .overflow_scroll()
                    .children(rows),
            )
            .child(match altitude {
                Altitude::Runs => label(palette, "Click a run to open it"),
                Altitude::Everything => label(palette, "Following the newest line"),
            })
            .child(div().h(px(space::MD)).flex_shrink_0()),
    )
}

fn runs(
    debugger: &Debugger,
    palette: Palette,
    cx: &mut Context<Debugger>,
) -> Vec<gpui::Stateful<gpui::Div>> {
    let mut rows = Vec::with_capacity(debugger.state.executions.len());
    for execution in &debugger.state.executions {
        let (right, fault) = outcome(execution);
        let tool = execution.tool_name.clone();
        rows.push(
            div()
                .id(SharedString::from(format!("run-{}", execution.id.as_str())))
                .cursor_pointer()
                .flex()
                .flex_row()
                .items_baseline()
                .gap(px(space::SM))
                .mb(px(space::MD))
                .text_size(px(text::BODY))
                .line_height(px(text::BODY_LINE))
                .on_click(cx.listener({
                    let tool = tool.clone();
                    move |this, _, _, cx| this.select_tool(tool.clone(), cx)
                }))
                .child(
                    div()
                        .w(px(96.))
                        .flex_shrink_0()
                        .text_color(rgb(palette.mute))
                        .child(SharedString::from(
                            execution.started_at.format("%H:%M:%S").to_string(),
                        )),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(rgb(palette.ink))
                        .child(SharedString::from(tool)),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .max_w(px(260.))
                        .truncate()
                        .text_color(rgb(if fault { palette.accent } else { palette.mute }))
                        .child(SharedString::from(right)),
                ),
        );
    }
    rows
}

fn everything<'a>(
    events: impl Iterator<Item = &'a LogEvent>,
    palette: Palette,
) -> Vec<gpui::Stateful<gpui::Div>> {
    events
        .enumerate()
        .map(|(index, event)| {
            // Columns are laid out, not space-padded — which is how the old log
            // ended up one character out of true on its longest label.
            div()
                .id(SharedString::from(format!("event-{index}")))
                .flex()
                .flex_row()
                .items_baseline()
                .gap(px(space::SM))
                .mb(px(6.))
                .text_size(px(text::BODY))
                .line_height(px(text::BODY_LINE))
                .child(
                    div()
                        .w(px(80.))
                        .flex_shrink_0()
                        .text_color(rgb(palette.mute))
                        .child(SharedString::from(
                            event.timestamp.format("%H:%M:%S").to_string(),
                        )),
                )
                .child(
                    div()
                        .w(px(72.))
                        .flex_shrink_0()
                        .text_color(rgb(palette.mute))
                        .child(SharedString::from(event.kind.short())),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(rgb(if event.kind.is_fault() {
                            palette.accent
                        } else {
                            palette.mute
                        }))
                        .child(SharedString::from(event.message.clone())),
                )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use webmcp_protocol::ExecutionId;

    fn execution(error: Option<&str>, ms: Option<i64>) -> ToolExecution {
        let started = Utc.with_ymd_and_hms(2026, 8, 31, 14, 23, 0).unwrap();
        ToolExecution {
            id: ExecutionId::from("exec_1"),
            tool_name: "create_order".into(),
            arguments: serde_json::json!({}),
            result: None,
            error: error.map(str::to_string),
            started_at: started,
            finished_at: ms.map(|ms| started + chrono::Duration::milliseconds(ms)),
        }
    }

    #[test]
    fn a_finished_run_shows_its_duration() {
        assert_eq!(outcome(&execution(None, Some(412))), ("412ms".into(), false));
    }

    #[test]
    fn a_run_still_in_flight_says_so_rather_than_showing_a_wrong_duration() {
        assert_eq!(outcome(&execution(None, None)), ("running".into(), false));
    }

    #[test]
    fn a_failure_shows_the_pages_own_message_and_is_marked_as_a_fault() {
        let (text, fault) = outcome(&execution(Some("text is required"), Some(3)));
        assert_eq!(text, "text is required");
        assert!(fault, "a failure must be visually distinct from a duration");
    }
}
