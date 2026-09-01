//! Survey — what this site offers.
//!
//! This screen exists because ⌘K is *recall*: you have to know a name to type
//! it. The first question a debugger has to answer is "what can this page even
//! do?", and that needs recognition. So the tools are simply on screen, with
//! what each one does and whether it acts as you, the moment you connect.

use gpui::{div, prelude::*, px, rgb, Context, SharedString};
use webmcp_protocol::Tool;

use super::{body, column, focus, label, muted, stage};
use crate::theme::{self, space, text};
use crate::{walkthrough, Debugger, SiteStatus};

/// What running this tool costs you.
///
/// The threat model's rule is "assume mutation unless annotated otherwise", so
/// anything without an explicit `readOnlyHint` is reported as mutating. This is
/// the first thing shown about a tool, not a badge to decode, because a tool
/// that acts as the logged-in user is the one fact that must never be quiet.
pub fn access(tool: &Tool) -> (&'static str, bool) {
    match tool.annotations.read_only_hint {
        Some(true) => ("Only reads", false),
        _ => ("Can change things", true),
    }
}

/// Tool description, falling back to the title, then to nothing. Page-supplied
/// text, rendered as plain text only.
pub fn blurb(tool: &Tool) -> String {
    let text = tool.description.trim();
    if !text.is_empty() {
        return text.to_string();
    }
    tool.title.clone().unwrap_or_default()
}

pub fn render(debugger: &Debugger, cx: &mut Context<Debugger>) -> gpui::Div {
    let palette = theme::theme(cx);
    let empty = debugger.state.pages.is_empty();
    stage().child(
        column(if empty { 128.0 } else { space::XL })
            .when(debugger.in_playground(), |el| {
                el.child(if debugger.walkthrough_hidden {
                    div()
                } else {
                    walkthrough::strip(debugger, palette, cx)
                })
                .child(demo_line(debugger, palette, cx))
            })
            .child(if debugger.in_playground() { div() } else { site_field(debugger, palette, cx) })
            .child(if empty {
                nothing_yet(debugger, palette)
            } else {
                tools(debugger, cx)
            })
            .when(!debugger.in_playground(), |el| {
                el.child(playground_offer(palette, cx))
            }),
    )
}

/// The site field lives on every Tools screen, not only the empty one — it is
/// how you point the app somewhere, and ⌘O focuses it.
fn site_field(
    debugger: &Debugger,
    palette: crate::theme::Palette,
    cx: &mut Context<Debugger>,
) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .flex_shrink_0()
        .gap(px(space::XS))
        .mb(px(space::XL))
        .child(label(palette, "OPEN A SITE"))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(space::SM))
                .pb(px(7.))
                .border_b_1()
                .border_color(rgb(palette.hair))
                .child(div().flex_1().min_w_0().child(debugger.site_input.clone()))
                .child(
                    div()
                        .id("open-site")
                        .cursor_pointer()
                        .flex_shrink_0()
                        .text_color(rgb(palette.ink))
                        .on_click(cx.listener(|this, _, _, cx| this.open_site(cx)))
                        .child(SharedString::from("OPEN")),
                ),
        )
        .child(match &debugger.site_status {
            SiteStatus::Idle => div(),
            SiteStatus::Opening(url) => muted(
                palette,
                SharedString::from(format!("Opening {url} — waiting for the page to report its tools…")),
            )
            .mt(px(6.)),
            SiteStatus::Problem(message) => body(palette, SharedString::from(message.clone()))
                .text_color(rgb(palette.accent))
                .mt(px(6.)),
        })
}

/// Heading and guidance for the empty screen, derived from what the site field
/// is actually doing. Previously the heading was hardcoded, so it could claim
/// "No site open yet" directly beneath a message saying the site had opened.
pub fn empty_state(status: &SiteStatus) -> (&'static str, &'static str) {
    match status {
        SiteStatus::Idle => (
            "No site open yet",
            "Type an address above and press Open, or switch to a tab you already have in Chrome.",
        ),
        SiteStatus::Opening(_) => (
            "Opening…",
            "Waiting for the page to report what it can do.",
        ),
        SiteStatus::Problem(_) => (
            "Nothing to show yet",
            "The note above says why. Try another address, or take the playground for a spin.",
        ),
    }
}

fn nothing_yet(debugger: &Debugger, palette: crate::theme::Palette) -> gpui::Div {
    let (heading, guidance) = empty_state(&debugger.site_status);
    div()
        .flex()
        .flex_col()
        .child(focus(palette, heading))
        .child(muted(palette, guidance).mt(px(space::MD)))
}

/// The demo site, which the playground serves and stops with itself.
///
/// An optional step up from the sample tools, not a prerequisite — so it is
/// only offered when Chrome is actually there to open it in.
fn demo_line(
    debugger: &Debugger,
    palette: crate::theme::Palette,
    cx: &mut Context<Debugger>,
) -> gpui::Div {
    let row = div()
        .flex()
        .flex_row()
        .items_baseline()
        .gap(px(space::SM))
        .flex_shrink_0()
        .mb(px(space::LG));

    if let Some(problem) = debugger.demo_problem() {
        return row.child(muted(palette, SharedString::from(problem.to_string())));
    }
    let Some(url) = debugger.demo_url() else {
        return div();
    };
    if !debugger.chrome_connected() {
        return row
            .child(muted(palette, "Demo site ready").flex_shrink_0())
            .child(muted(
                palette,
                "Connect Chrome to try these against a real page instead of sample data.",
            ));
    }
    row.child(muted(palette, "Try it for real").flex_shrink_0()).child(
        body(palette, SharedString::from(url.to_string()))
            .id("open-demo")
            .cursor_pointer()
            .pb(px(3.))
            .border_b_1()
            .border_color(rgb(palette.ink))
            .hover(|style| style.text_color(rgb(palette.accent)))
            .tooltip(|_, cx| {
                cx.new(|_| {
                    super::Tip::new(
                        "Opens the bundled demo page in Chrome and points the debugger at it, \
                         so the same walkthrough runs against a real page. Served only while \
                         the playground is open.",
                    )
                })
                .into()
            })
            .on_click(cx.listener(|this, _, _, cx| this.open_demo_site(cx))),
    )
}

/// One quiet line. The explanation lives on hover so it is available without
/// competing with the tools, which are what this screen is for.
fn playground_offer(
    palette: crate::theme::Palette,
    cx: &mut Context<Debugger>,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id("open-playground")
        .flex_shrink_0()
        .cursor_pointer()
        .mt(px(space::MD))
        .text_size(px(text::LABEL))
        .line_height(px(text::LABEL_LINE))
        .text_color(rgb(palette.mute))
        .hover(|style| style.text_color(rgb(palette.ink)))
        .tooltip(|_, cx| {
            cx.new(|_| {
                super::Tip::new(
                    "A three-step walkthrough on built-in sample data: pick a tool, run it, \
                     look at History. Nothing touches Chrome, and you can leave any time.",
                )
            })
            .into()
        })
        .on_click(cx.listener(|this, _, _, cx| this.enter_playground(cx)))
        .child(SharedString::from("TRY THE PLAYGROUND  ⌃T"))
}

fn tools(debugger: &Debugger, cx: &mut Context<Debugger>) -> gpui::Div {
    let palette = theme::theme(cx);
    let state = &debugger.state;
    let selected = state.selected_tool.clone();

    let mut rows = Vec::with_capacity(state.tools.len());
    for tool in &state.tools {
        let name = tool.name.clone();
        let (access_label, mutates) = access(tool);
        let is_selected = selected.as_deref() == Some(name.as_str());
        let description = blurb(tool);
        rows.push(
            div()
                .id(SharedString::from(format!("tool-{name}")))
                .cursor_pointer()
                .flex()
                .flex_col()
                .mb(px(space::MD))
                .on_click(cx.listener({
                    let name = name.clone();
                    move |this, _, _, cx| this.select_tool(name.clone(), cx)
                }))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .items_baseline()
                        .gap(px(space::SM))
                        .child(
                            body(palette, SharedString::from(name))
                                .min_w_0()
                                .truncate()
                                .when(is_selected, |el| el.text_color(rgb(palette.ink))),
                        )
                        .child(
                            label(palette, access_label)
                                .when(mutates, |el| el.text_color(rgb(palette.accent))),
                        ),
                )
                .when(!description.is_empty(), |el| {
                    el.child(muted(palette, SharedString::from(description)).truncate())
                }),
        );
    }

    let mut others = Vec::new();
    for page in &state.pages {
        if Some(&page.id) == state.selected_page.as_ref() {
            continue;
        }
        let id = page.id.clone();
        others.push(
            div()
                .id(SharedString::from(format!("page-{}", id.as_str())))
                .cursor_pointer()
                .mb(px(space::XS))
                .child(muted(palette, SharedString::from(page.origin.clone())).truncate())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.select_page(id.clone(), cx);
                })),
        );
    }

    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .child(label(palette, SharedString::from(match state.tools.len() {
            0 => "THIS PAGE REPORTED NO TOOLS".to_string(),
            1 => "1 TOOL ON THIS PAGE".to_string(),
            count => format!("{count} TOOLS ON THIS PAGE"),
        })))
        .when(state.tools.is_empty(), |el| {
            // "No tools" is a symptom with three common causes, and the operator
            // cannot act on the symptom alone.
            el.child(
                muted(
                    palette,
                    "The page may not use WebMCP. If you expected tools here, check that Chrome is \
                     150 or newer, that chrome://flags/#enable-webmcp-testing is on, and reload the tab.",
                )
                .mt(px(space::XS)),
            )
        })
        .child(
            div()
                .id("survey-scroll")
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .mt(px(space::MD))
                .overflow_scroll()
                .children(rows)
                .when(!others.is_empty(), |el| {
                    el.child(label(palette, "OTHER SITES OPEN IN CHROME").mt(px(space::XL)))
                        .child(div().mt(px(space::XS)).children(others))
                }),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use webmcp_protocol::ToolAnnotations;

    fn tool(read_only: Option<bool>, description: &str, title: Option<&str>) -> Tool {
        Tool {
            name: "create_order".into(),
            title: title.map(str::to_string),
            description: description.into(),
            input_schema: serde_json::json!({"type": "object"}),
            annotations: ToolAnnotations {
                read_only_hint: read_only,
                untrusted_content_hint: None,
            },
        }
    }

    #[test]
    fn the_empty_screen_never_contradicts_the_site_field() {
        // It used to read "No site open yet" directly beneath a message saying
        // the site had been opened but reported no tools.
        let (heading, _) = empty_state(&SiteStatus::Idle);
        assert_eq!(heading, "No site open yet");

        let (heading, _) = empty_state(&SiteStatus::Opening("https://example.com".into()));
        assert_ne!(heading, "No site open yet", "a site is being opened right now");

        let (heading, guidance) =
            empty_state(&SiteStatus::Problem("Opened it, but no tools came back".into()));
        assert_ne!(heading, "No site open yet", "the site did open");
        assert!(
            !guidance.contains("Opened it"),
            "the problem is already shown above; do not repeat it"
        );
    }

    #[test]
    fn every_site_state_has_a_heading_and_guidance() {
        for status in [
            SiteStatus::Idle,
            SiteStatus::Opening("x".into()),
            SiteStatus::Problem("y".into()),
        ] {
            let (heading, guidance) = empty_state(&status);
            assert!(!heading.is_empty());
            assert!(!guidance.is_empty());
        }
    }

    #[test]
    fn a_tool_without_an_explicit_read_only_hint_is_treated_as_mutating() {
        // The threat model says assume mutation unless annotated otherwise.
        // Silence must never read as safe.
        assert_eq!(access(&tool(None, "", None)), ("Can change things", true));
        assert_eq!(access(&tool(Some(false), "", None)), ("Can change things", true));
        assert_eq!(access(&tool(Some(true), "", None)), ("Only reads", false));
    }

    #[test]
    fn the_blurb_prefers_description_then_title_then_nothing() {
        assert_eq!(blurb(&tool(None, "Places an order.", Some("Order"))), "Places an order.");
        assert_eq!(blurb(&tool(None, "   ", Some("Order"))), "Order");
        assert_eq!(blurb(&tool(None, "", None)), "");
    }
}
