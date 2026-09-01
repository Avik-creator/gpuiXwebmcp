//! Compose — fill in and run one tool.
//!
//! One surface, four states. The tool name never moves; the body below it
//! becomes the run, then the result, then the failure. That is why no single
//! screen here is crowded even though this is where all the work happens.

use gpui::{div, prelude::*, px, rgb, Context, SharedString};
use serde_json::Value;
use webmcp_protocol::{Tool, ToolExecution};

use super::survey::access;
use super::{body, column, focus, label, muted, stage};
use crate::diff;
use crate::form::child_path;
use crate::schema::{Field, FieldError, Kind};
use crate::theme::{self, space, text, Palette};
use crate::Debugger;

/// A very large result would otherwise build one element per line and stall the
/// window. Truncation is stated, never silent.
pub const RESULT_LINES: usize = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Editing,
    Running,
    Done,
    Failed,
}

/// Derived, never stored twice: the only extra input is whether the operator has
/// explicitly asked to go back to the form.
pub fn stage_of(pending: bool, execution: Option<&ToolExecution>, editing: bool) -> Stage {
    if pending {
        return Stage::Running;
    }
    if editing {
        return Stage::Editing;
    }
    match execution {
        Some(run) if run.error.is_some() => Stage::Failed,
        Some(run) if run.result.is_some() => Stage::Done,
        _ => Stage::Editing,
    }
}

/// A one-line recap of what was sent, so a result never floats free of its input.
pub fn recap(arguments: &Value) -> String {
    let Some(object) = arguments.as_object() else {
        return String::new();
    };
    object
        .iter()
        .map(|(key, value)| match value {
            Value::String(text) => format!("{key} {text}"),
            Value::Array(items) => format!("{key} {} item(s)", items.len()),
            Value::Object(_) => format!("{key} {{…}}"),
            other => format!("{key} {other}"),
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Cap the rendered result, and say so when we do.
pub fn result_lines(pretty: &str) -> (Vec<String>, Option<String>) {
    let all: Vec<String> = pretty.lines().map(str::to_string).collect();
    if all.len() <= RESULT_LINES {
        return (all, None);
    }
    let note = format!("Showing the first {RESULT_LINES} of {} lines", all.len());
    (all.into_iter().take(RESULT_LINES).collect(), Some(note))
}

pub fn render(debugger: &mut Debugger, cx: &mut Context<Debugger>) -> gpui::Div {
    let palette = theme::theme(cx);
    let Some(tool) = debugger.state.selected_tool().cloned() else {
        return stage_empty(palette);
    };

    let snapshot = debugger.snapshot_raw(cx);
    let (arguments, mut errors) = crate::form::assemble(debugger.form.fields(), &snapshot, "");
    errors.extend(crate::schema::validate(&debugger.form, &arguments));

    let execution = debugger.state.last_execution_for(&tool.name).cloned();
    let pending = debugger
        .pending
        .as_ref()
        .is_some_and(|p| execution.as_ref().is_some_and(|e| e.id == p.id));
    let current = stage_of(pending, execution.as_ref(), debugger.compose_editing);

    let mut body_rows: Vec<gpui::AnyElement> = Vec::new();
    match current {
        Stage::Editing => {
            let fields = debugger.form.fields().to_vec();
            for field in &fields {
                body_rows.push(
                    field_row(debugger, field, "", &errors, palette, cx).into_any_element(),
                );
            }
            let ready = errors.is_empty() && debugger.can_execute(cx);
            body_rows.push(action(palette, "RUN", "⌘↵", ready, cx, |this, cx| {
                this.execute_selected(cx)
            }));
        }
        Stage::Running => {
            let run = execution.as_ref();
            body_rows.push(
                muted(palette, SharedString::from(run.map(|r| recap(&r.arguments)).unwrap_or_default()))
                    .mt(px(space::MD))
                    .into_any_element(),
            );
            body_rows.push(
                muted(
                    palette,
                    "Waiting for the browser. Nothing else can run until this finishes.",
                )
                .mt(px(space::XL))
                .into_any_element(),
            );
            body_rows.push(action(palette, "STOP", "⌘.", true, cx, |this, cx| {
                this.cancel_execution(cx)
            }));
        }
        Stage::Done | Stage::Failed => {
            let Some(run) = execution.as_ref() else {
                return stage_empty(palette);
            };
            body_rows.push(
                muted(palette, SharedString::from(recap(&run.arguments)))
                    .mt(px(space::MD))
                    .into_any_element(),
            );
            let mut comparable = false;
            if current == Stage::Failed {
                body_rows.push(
                    body(palette, SharedString::from(run.error.clone().unwrap_or_default()))
                        .text_color(rgb(palette.accent))
                        .mt(px(space::XL))
                        .into_any_element(),
                );
                body_rows.push(
                    label(palette, "This message came from the page, shown as plain text")
                        .mt(px(space::XS))
                        .into_any_element(),
                );
            } else {
                let pretty = serde_json::to_string_pretty(
                    run.result.as_ref().unwrap_or(&Value::Null),
                )
                .unwrap_or_else(|_| "{}".into());
                // Read, mutate, read again — the change is in data we already
                // keep, which is how a native window shows a mutation landing.
                let earlier = debugger
                    .state
                    .previous_result_for(&tool.name, &run.id)
                    .and_then(|previous| previous.result.as_ref().map(|value| {
                        (
                            previous.started_at.format("%H:%M:%S").to_string(),
                            serde_json::to_string_pretty(value).unwrap_or_default(),
                        )
                    }));

                if debugger.comparing {
                    if let Some((when, before)) = &earlier {
                        let lines = diff::compare(before, &pretty);
                        let counted = diff::tally(&lines);
                        body_rows.push(
                            label(
                                palette,
                                SharedString::from(format!(
                                    "COMPARED WITH THE RUN AT {when} · {}",
                                    counted.summary()
                                )),
                            )
                            .mt(px(space::XL))
                            .into_any_element(),
                        );
                        body_rows.push(
                            div()
                                .id("result-diff")
                                .flex()
                                .flex_col()
                                .mt(px(space::XS))
                                .text_size(px(text::BODY))
                                .line_height(px(text::BODY_LINE))
                                .children(lines.into_iter().map(|line| {
                                    let colour = match line.change {
                                        diff::Change::Added => palette.ink,
                                        diff::Change::Removed => palette.accent,
                                        diff::Change::Same => palette.mute,
                                    };
                                    div().text_color(rgb(colour)).child(SharedString::from(
                                        format!("{} {}", line.change.marker(), line.text),
                                    ))
                                }))
                                .into_any_element(),
                        );
                    }
                }

                let (lines, note) = result_lines(&pretty);
                if !debugger.comparing {
                body_rows.push(
                    div()
                        .id("result-lines")
                        .flex()
                        .flex_col()
                        .mt(px(space::XL))
                        .text_size(px(text::BODY))
                        .line_height(px(text::BODY_LINE))
                        .text_color(rgb(palette.ink))
                        .children(lines.into_iter().map(SharedString::from))
                        .into_any_element(),
                );
                if let Some(note) = note {
                    body_rows.push(
                        label(palette, SharedString::from(note))
                            .mt(px(space::XS))
                            .into_any_element(),
                    );
                }
                }
                comparable = earlier.is_some();
            }
            body_rows.push(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(space::XL))
                    .child(action_inline(palette, "RUN AGAIN", "⌘↵", true, cx, |this, cx| {
                        this.execute_selected(cx)
                    }))
                    .child(action_inline(palette, "CHANGE THE INPUTS", "", false, cx, |this, cx| {
                        this.edit_arguments(cx)
                    }))
                    .when(current == Stage::Done, |el| {
                        el.child(action_inline(palette, "COPY RESULT", "⌘⇧C", false, cx, |this, cx| {
                            this.copy_last_result(cx)
                        }))
                    })
                    .when(current == Stage::Done && comparable, |el| {
                        el.child(action_inline(
                            palette,
                            if debugger.comparing { "SHOW THE RESULT" } else { "COMPARE WITH LAST RUN" },
                            "",
                            false,
                            cx,
                            |this, cx| this.toggle_compare(cx),
                        ))
                    })
                    .mt(px(space::HUGE))
                    .into_any_element(),
            );
        }
    }

    let (access_label, mutates) = access(&tool);
    let untrusted = tool.annotations.untrusted_content_hint == Some(true);
    let banner = match (mutates, untrusted) {
        (true, true) => format!("{access_label} · runs as you · output is not trusted"),
        (true, false) => format!("{access_label} · runs as you in the browser"),
        (false, true) => format!("{access_label} · output is not trusted"),
        (false, false) => access_label.to_string(),
    };

    let heading = match current {
        Stage::Running => Some(format!("Running · {}", elapsed(execution.as_ref()))),
        Stage::Done => execution
            .as_ref()
            .and_then(|r| r.duration_ms())
            .map(|ms| format!("Finished in {ms}ms")),
        Stage::Failed => Some("Failed".to_string()),
        Stage::Editing => None,
    };

    stage().child(
        column(space::HUGE)
            .child(focus(palette, SharedString::from(tool.name.clone())))
            .child(
                label(palette, SharedString::from(banner))
                    .when(mutates, |el| el.text_color(rgb(palette.accent)))
                    .mt(px(10.)),
            )
            .when_some(heading, |el, heading| {
                el.child(label(palette, SharedString::from(heading)).mt(px(space::XS)))
            })
            .when(current == Stage::Editing, |el| {
                el.child(description(debugger, &tool, palette, cx))
            })
            .child(
                div()
                    .id("compose-scroll")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .mt(px(space::XXL))
                    .overflow_scroll()
                    .children(body_rows),
            )
            .child(div().h(px(space::MD)).flex_shrink_0()),
    )
}

fn elapsed(execution: Option<&ToolExecution>) -> String {
    let Some(run) = execution else {
        return "…".into();
    };
    let ms = (chrono::Utc::now() - run.started_at).num_milliseconds().max(0);
    format!("{:.1}S", ms as f64 / 1000.0)
}

fn stage_empty(palette: Palette) -> gpui::Div {
    stage().child(column(200.0).child(muted(palette, "Pick a tool first.")))
}

/// Description plus the schema, disclosed in place — nothing navigates away.
fn description(
    debugger: &Debugger,
    tool: &Tool,
    palette: Palette,
    cx: &mut Context<Debugger>,
) -> gpui::Div {
    let open = debugger.raw.is_open("");
    let pretty = serde_json::to_string_pretty(&tool.input_schema).unwrap_or_default();
    div()
        .flex()
        .flex_col()
        .flex_shrink_0()
        .child(muted(palette, SharedString::from(tool.description.clone())).mt(px(space::MD)))
        .child(
            label(palette, if open { "HIDE THE SCHEMA" } else { "SHOW THE SCHEMA" })
                .id("schema-toggle")
                .cursor_pointer()
                .mt(px(space::SM))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.raw.toggle("");
                    cx.notify();
                })),
        )
        .when(open, |el| {
            el.child(
                div()
                    .mt(px(space::XS))
                    .pl(px(space::SM))
                    .border_l_1()
                    .border_color(rgb(palette.hair))
                    .text_color(rgb(palette.mute))
                    .children(pretty.lines().map(|line| SharedString::from(line.to_string()))),
            )
        })
}

#[allow(clippy::too_many_arguments)]
fn field_row(
    debugger: &mut Debugger,
    field: &Field,
    prefix: &str,
    errors: &[FieldError],
    palette: Palette,
    cx: &mut Context<Debugger>,
) -> gpui::Div {
    let path = child_path(prefix, &field.name);
    let title = field
        .title
        .clone()
        .unwrap_or_else(|| field.name.to_ascii_uppercase());
    let heading = if field.required {
        format!("{} *", title.to_ascii_uppercase())
    } else {
        title.to_ascii_uppercase()
    };
    let mine: Vec<&FieldError> = errors.iter().filter(|e| e.path == path).collect();
    let faulty = !mine.is_empty();

    div()
        .flex()
        .flex_col()
        .mb(px(space::LG))
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .gap(px(space::SM))
                .child(label(palette, SharedString::from(heading)))
                .child(
                    label(palette, SharedString::from(field.kind.summary()))
                        .when(field.kind.is_raw(), |el| el.text_color(rgb(palette.accent))),
                ),
        )
        .child(control(debugger, field, &path, palette, cx).mt(px(space::XS)))
        .child(
            div()
                .h(px(1.))
                .mt(px(7.))
                .bg(rgb(if faulty { palette.accent } else { palette.hair })),
        )
        .when_some(field.description.clone(), |el, text| {
            el.child(muted(palette, SharedString::from(text)).mt(px(6.)))
        })
        .children(mine.into_iter().map(|error| {
            body(palette, SharedString::from(error.message.clone()))
                .text_color(rgb(palette.accent))
                .mt(px(6.))
        }))
}

fn control(
    debugger: &mut Debugger,
    field: &Field,
    path: &str,
    palette: Palette,
    cx: &mut Context<Debugger>,
) -> gpui::Div {
    match &field.kind {
        // A choice is its values with one lit — not a segmented box. Boxes were
        // doing work that weight and space do better.
        Kind::Choice { options } => {
            let picked = debugger.raw.choices.get(path).copied();
            let mut words = Vec::with_capacity(options.len());
            for (index, option) in options.iter().enumerate() {
                let shown = option
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| option.to_string());
                let lit = picked == Some(index);
                let owned = path.to_string();
                words.push(
                    body(palette, SharedString::from(shown))
                        .id(SharedString::from(format!("{path}#{index}")))
                        .cursor_pointer()
                        .text_color(rgb(if lit { palette.ink } else { palette.mute }))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.raw.choices.insert(owned.clone(), index);
                            this.compose_editing = true;
                            cx.notify();
                        })),
                );
            }
            div().flex().flex_row().gap(px(space::MD)).children(words)
        }
        Kind::Boolean => {
            let value = debugger.raw.bools.get(path).copied().unwrap_or(false);
            let mut words = Vec::with_capacity(2);
            for option in [false, true] {
                let owned = path.to_string();
                words.push(
                    body(palette, if option { "true" } else { "false" })
                        .id(SharedString::from(format!("{path}#{option}")))
                        .cursor_pointer()
                        .text_color(rgb(if option == value { palette.ink } else { palette.mute }))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.raw.bools.insert(owned.clone(), option);
                            this.compose_editing = true;
                            cx.notify();
                        })),
                );
            }
            div().flex().flex_row().gap(px(space::MD)).children(words)
        }
        Kind::List { item, .. } => {
            let count = debugger.raw.list_len(path);
            let mut rows = Vec::with_capacity(count);
            for index in 0..count {
                let row_path = format!("{path}[{index}]");
                let row_field = Field {
                    name: field.name.clone(),
                    title: None,
                    description: None,
                    required: false,
                    default: None,
                    kind: (**item).clone(),
                };
                let owned = path.to_string();
                rows.push(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(space::SM))
                        .mb(px(6.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(control(debugger, &row_field, &row_path, palette, cx)),
                        )
                        .child(
                            label(palette, "−")
                                .id(SharedString::from(format!("{row_path}#drop")))
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.drop_list_row(&owned, index, cx);
                                })),
                        ),
                );
            }
            let owned = path.to_string();
            div()
                .flex()
                .flex_col()
                .children(rows)
                .child(
                    label(palette, "+ ADD")
                        .id(SharedString::from(format!("{path}#add")))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let next = this.raw.list_len(&owned) + 1;
                            this.raw.lists.insert(owned.clone(), next);
                            this.compose_editing = true;
                            cx.notify();
                        })),
                )
        }
        // A nested object is its summary and a ›, until you want the rest.
        Kind::Group { fields } => {
            let open = debugger.raw.is_open(path);
            let owned = path.to_string();
            let mut inner = Vec::new();
            if open {
                let nested = fields.clone();
                for child in &nested {
                    inner.push(field_row(debugger, child, path, &[], palette, cx));
                }
            }
            div()
                .flex()
                .flex_col()
                .child(
                    body(palette, if open { "hide" } else { "show" })
                        .id(SharedString::from(format!("{path}#open")))
                        .cursor_pointer()
                        .text_color(rgb(palette.mute))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.raw.toggle(&owned);
                            cx.notify();
                        })),
                )
                .when(open, |el| {
                    el.child(
                        div()
                            .mt(px(space::XS))
                            .pl(px(space::SM))
                            .border_l_1()
                            .border_color(rgb(palette.hair))
                            .children(inner),
                    )
                })
        }
        // Text, numbers and anything with no widget all type into a field.
        _ => {
            let placeholder = match &field.kind {
                Kind::Raw { reason } => reason.clone(),
                other => other.summary(),
            };
            let entity = debugger.ensure_input(path, &placeholder, cx);
            div().child(entity)
        }
    }
}

fn action(
    palette: Palette,
    text_label: &'static str,
    key: &'static str,
    enabled: bool,
    cx: &mut Context<Debugger>,
    handler: impl Fn(&mut Debugger, &mut Context<Debugger>) + 'static,
) -> gpui::AnyElement {
    div()
        .flex()
        .flex_row()
        .mt(px(space::HUGE))
        .child(action_inline(palette, text_label, key, enabled, cx, handler))
        .into_any_element()
}

fn action_inline(
    palette: Palette,
    text_label: &'static str,
    key: &'static str,
    primary: bool,
    cx: &mut Context<Debugger>,
    handler: impl Fn(&mut Debugger, &mut Context<Debugger>) + 'static,
) -> gpui::Stateful<gpui::Div> {
    let colour = if primary { palette.ink } else { palette.mute };
    div()
        .id(SharedString::from(text_label))
        .cursor_pointer()
        .flex()
        .flex_row()
        .items_baseline()
        .gap(px(space::SM))
        .pb(px(7.))
        .border_b_1()
        .border_color(rgb(colour))
        .text_size(px(text::BODY))
        .line_height(px(text::BODY_LINE))
        .text_color(rgb(colour))
        .on_click(cx.listener(move |this, _, _, cx| handler(this, cx)))
        .child(SharedString::from(text_label))
        .when(!key.is_empty(), |el| el.child(label(palette, key)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use webmcp_protocol::ExecutionId;

    fn run(error: Option<&str>, result: Option<Value>) -> ToolExecution {
        let started = Utc.with_ymd_and_hms(2026, 8, 31, 14, 23, 0).unwrap();
        ToolExecution {
            id: ExecutionId::from("exec_1"),
            tool_name: "create_order".into(),
            arguments: serde_json::json!({"priority": "urgent", "items": ["a", "b"]}),
            result,
            error: error.map(str::to_string),
            started_at: started,
            finished_at: Some(started + chrono::Duration::milliseconds(412)),
        }
    }

    #[test]
    fn a_run_in_flight_beats_everything_else() {
        assert_eq!(stage_of(true, None, false), Stage::Running);
        assert_eq!(stage_of(true, Some(&run(None, Some(Value::Null))), true), Stage::Running);
    }

    #[test]
    fn asking_to_edit_returns_you_to_the_form() {
        let finished = run(None, Some(serde_json::json!({"ok": true})));
        assert_eq!(stage_of(false, Some(&finished), false), Stage::Done);
        assert_eq!(stage_of(false, Some(&finished), true), Stage::Editing);
    }

    #[test]
    fn a_failure_is_its_own_state_not_a_result() {
        assert_eq!(stage_of(false, Some(&run(Some("text is required"), None)), false), Stage::Failed);
    }

    #[test]
    fn a_tool_never_run_starts_in_the_form() {
        assert_eq!(stage_of(false, None, false), Stage::Editing);
    }

    #[test]
    fn the_recap_keeps_a_result_attached_to_what_produced_it() {
        let text = recap(&serde_json::json!({
            "priority": "urgent", "items": ["a", "b"], "customer": {"name": "Ada"}, "n": 3
        }));
        assert!(text.contains("priority urgent"));
        assert!(text.contains("items 2 item(s)"));
        assert!(text.contains("customer {…}"));
        assert!(text.contains("n 3"));
    }

    #[test]
    fn a_short_result_is_shown_whole_and_says_nothing() {
        let (lines, note) = result_lines("a\nb\nc");
        assert_eq!(lines.len(), 3);
        assert!(note.is_none());
    }

    #[test]
    fn a_huge_result_is_capped_and_says_so() {
        let pretty = (0..5_000).map(|n| n.to_string()).collect::<Vec<_>>().join("\n");
        let (lines, note) = result_lines(&pretty);
        assert_eq!(lines.len(), RESULT_LINES);
        assert_eq!(note.as_deref(), Some("Showing the first 200 of 5000 lines"));
    }
}
