//! ⌘K — recall, for the hundreds of things behind the three places.
//!
//! The bar gives recognition: you can always see where you can go. This gives
//! speed once you know where you are going. Grouped rather than ranked, because
//! an operator already knows whether they want a tool, a page or a command.
//!
//! The table below is the single source of truth for every command *and* its
//! shortcut, so the palette and the keymap cannot drift apart the way the old
//! hardcoded keymap bar drifted from the bindings it described.

use gpui::{div, prelude::*, px, rgb, rgba, Context, SharedString};

use crate::shell::Place;
use crate::theme::{space, text, Palette};
use crate::Debugger;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Go(Place),
    Back,
    Forward,
    FocusSite,
    OpenDemo,
    ToggleBackend,
    ToggleTheme,
    ToggleAltitude,
    Execute,
    Cancel,
    Copy,
}

pub struct Entry {
    pub label: &'static str,
    /// How the shortcut is written for a human.
    pub key: &'static str,
    /// How gpui is told to bind it. Both live here so the palette cannot claim
    /// a shortcut the app does not actually listen for.
    pub binding: &'static str,
    /// Other words people reach for. Plain labels read better but search worse —
    /// "Switch between dark and light" is clearer than "Theme" and yet nobody
    /// types the whole sentence.
    pub keywords: &'static str,
    pub command: Command,
}

pub const COMMANDS: &[Entry] = &[
    Entry { label: "Go back", key: "⌘[", binding: "cmd-[", keywords: "back previous return undo screen", command: Command::Back },
    Entry { label: "Go forward", key: "⌘]", binding: "cmd-]", keywords: "forward next", command: Command::Forward },
    Entry { label: "Go to Tools", key: "⌘1", binding: "cmd-1", keywords: "survey list what can this site do", command: Command::Go(Place::Survey) },
    Entry { label: "Go to Run", key: "⌘2", binding: "cmd-2", keywords: "compose execute form arguments", command: Command::Go(Place::Compose) },
    Entry { label: "Go to History", key: "⌘3", binding: "cmd-3", keywords: "record log events activity", command: Command::Go(Place::Record) },
    Entry { label: "Open the built-in demo site", key: "⌘D", binding: "cmd-d", keywords: "demo sample localhost 5173 example try", command: Command::OpenDemo },
    Entry { label: "Open a site…", key: "⌘O", binding: "cmd-o", keywords: "url address navigate go visit", command: Command::FocusSite },
    Entry { label: "Switch between dark and light", key: "⌘⇧L", binding: "cmd-shift-l", keywords: "theme dark light appearance colours colors", command: Command::ToggleTheme },
    Entry { label: "Open or leave the playground", key: "⌃T", binding: "ctrl-t", keywords: "sample data demo offline walkthrough tutorial learn try fixture fake test", command: Command::ToggleBackend },
    Entry { label: "History: show runs, or all activity", key: "⌘E", binding: "cmd-e", keywords: "altitude log protocol events wire", command: Command::ToggleAltitude },
    Entry { label: "Run the selected tool", key: "⌘↵", binding: "cmd-enter", keywords: "execute go call invoke", command: Command::Execute },
    Entry { label: "Stop the current run", key: "⌘.", binding: "cmd-.", keywords: "cancel abort halt", command: Command::Cancel },
    Entry { label: "Copy the result", key: "⌘⇧C", binding: "cmd-shift-c", keywords: "clipboard yank", command: Command::Copy },
];

/// One row of results. Kept plain so matching can be tested without a window.
#[derive(Clone, Debug, PartialEq)]
pub enum Hit {
    Command(usize),
    Tool { name: String, mutates: bool },
    Page { id: String, origin: String, tools: usize },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolHit {
    pub name: String,
    pub mutates: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PageHit {
    pub id: String,
    pub origin: String,
    pub tools: usize,
}

fn hit(haystack: &str, needle: &str) -> bool {
    needle.is_empty() || haystack.to_lowercase().contains(&needle.to_lowercase())
}

/// Everything the query reaches, grouped: commands, then tools, then pages.
pub fn matches(query: &str, tools: &[ToolHit], pages: &[PageHit]) -> Vec<Hit> {
    let query = query.trim();
    let mut out = Vec::new();
    for (index, entry) in COMMANDS.iter().enumerate() {
        if hit(entry.label, query) || hit(entry.keywords, query) {
            out.push(Hit::Command(index));
        }
    }
    for tool in tools {
        if hit(&tool.name, query) {
            out.push(Hit::Tool { name: tool.name.clone(), mutates: tool.mutates });
        }
    }
    for page in pages {
        if hit(&page.origin, query) {
            out.push(Hit::Page {
                id: page.id.clone(),
                origin: page.origin.clone(),
                tools: page.tools,
            });
        }
    }
    out
}

/// Keep the highlighted row inside the results as they narrow under typing.
pub fn clamp(index: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        index.min(len - 1)
    }
}

pub fn step(index: usize, len: usize, forward: bool) -> usize {
    if len == 0 {
        return 0;
    }
    if forward {
        (index + 1) % len
    } else {
        (index + len - 1) % len
    }
}

fn group_of(hit: &Hit) -> &'static str {
    match hit {
        Hit::Command(_) => "COMMANDS",
        Hit::Tool { .. } => "TOOLS",
        Hit::Page { .. } => "SITES",
    }
}

pub fn overlay(debugger: &Debugger, palette: Palette, cx: &mut Context<Debugger>) -> gpui::Div {
    let hits = debugger.palette_hits(cx);
    let selected = clamp(debugger.palette_index, hits.len());

    let mut rows: Vec<gpui::AnyElement> = Vec::new();
    let mut last_group = "";
    for (index, item) in hits.iter().enumerate() {
        let group = group_of(item);
        if group != last_group {
            last_group = group;
            rows.push(
                div()
                    .mt(px(space::MD))
                    .mb(px(space::XS))
                    .text_size(px(text::LABEL))
                    .line_height(px(text::LABEL_LINE))
                    .text_color(rgb(palette.mute))
                    .child(SharedString::from(group))
                    .into_any_element(),
            );
        }
        let lit = index == selected;
        let (left, right, accent) = match item {
            Hit::Command(slot) => (
                COMMANDS[*slot].label.to_string(),
                COMMANDS[*slot].key.to_string(),
                false,
            ),
            Hit::Tool { name, mutates } => (
                name.clone(),
                if *mutates { "can change things".into() } else { String::new() },
                *mutates,
            ),
            Hit::Page { origin, tools, .. } => (origin.clone(), format!("{tools} tools"), false),
        };
        rows.push(
            div()
                .id(SharedString::from(format!("hit-{index}")))
                .cursor_pointer()
                .flex()
                .flex_row()
                .items_baseline()
                .justify_between()
                .gap(px(space::SM))
                .mb(px(space::XS))
                .text_size(px(text::BODY))
                .line_height(px(text::BODY_LINE))
                .text_color(rgb(if lit { palette.ink } else { palette.mute }))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.palette_index = index;
                    this.run_palette(window, cx);
                }))
                .child(div().min_w_0().truncate().child(SharedString::from(left)))
                .child(
                    div()
                        .flex_shrink_0()
                        .text_size(px(text::LABEL))
                        .line_height(px(text::LABEL_LINE))
                        .text_color(rgb(if accent { palette.accent } else { palette.mute }))
                        .child(SharedString::from(right)),
                )
                .into_any_element(),
        );
    }

    div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .bottom_0()
        .bg(rgba((palette.paper << 8) | 0xE0))
        .flex()
        .flex_row()
        .justify_center()
        .child(
            div()
                .w(px(560.))
                .mt(px(120.))
                .flex()
                .flex_col()
                .min_h_0()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(space::XS))
                        .pb(px(7.))
                        .border_b_1()
                        .border_color(rgb(palette.hair))
                        .child(
                            div()
                                .text_color(rgb(palette.mute))
                                .child(SharedString::from("/")),
                        )
                        .child(div().flex_1().min_w_0().child(debugger.palette_query.clone())),
                )
                .child(
                    div()
                        .id("palette-hits")
                        .flex()
                        .flex_col()
                        .min_h_0()
                        .max_h(px(420.))
                        .overflow_scroll()
                        .children(rows),
                )
                .child(
                    div()
                        .mt(px(space::XL))
                        .text_size(px(text::LABEL))
                        .line_height(px(text::LABEL_LINE))
                        .text_color(rgb(palette.mute))
                        .child(SharedString::from("↑↓ to move · ↵ to choose · esc to close")),
                ),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tools() -> Vec<ToolHit> {
        vec![
            ToolHit { name: "get_user".into(), mutates: false },
            ToolHit { name: "create_order".into(), mutates: true },
            ToolHit { name: "cancel_order".into(), mutates: true },
        ]
    }

    fn pages() -> Vec<PageHit> {
        vec![PageHit { id: "tab:7".into(), origin: "http://localhost:5173".into(), tools: 6 }]
    }

    #[test]
    fn every_command_carries_a_shortcut_a_label_and_search_words() {
        for entry in COMMANDS {
            assert!(!entry.label.is_empty());
            assert!(!entry.key.is_empty(), "{} has no shortcut", entry.label);
            assert!(!entry.keywords.is_empty(), "{} is unsearchable", entry.label);
        }
    }

    #[test]
    fn the_words_people_actually_type_still_find_things() {
        // Plain labels read better and search worse. Nobody types "Switch
        // between dark and light" — they type "theme", or "dark".
        for query in ["theme", "dark", "light", "cancel", "url", "fixture", "log", "execute"] {
            assert!(
                matches(query, &[], &[])
                    .iter()
                    .any(|hit| matches!(hit, Hit::Command(_))),
                "nothing found for {query:?}"
            );
        }
    }

    #[test]
    fn the_written_shortcut_and_the_real_binding_agree() {
        // The palette shows "⌘1"; the app binds "cmd-1". If those drift, the
        // palette lies — which is exactly what the old hardcoded keymap bar did.
        for entry in COMMANDS {
            assert!(!entry.binding.is_empty(), "{} has no binding", entry.label);
            let shown = entry.key.to_lowercase();
            let bound = entry.binding.to_lowercase();
            let has_cmd = shown.contains('⌘');
            assert_eq!(has_cmd, bound.contains("cmd-"), "{}: ⌘ vs {}", entry.label, entry.binding);
            assert_eq!(shown.contains('⌃'), bound.contains("ctrl-"), "{}", entry.label);
            assert_eq!(shown.contains('⇧'), bound.contains("shift-"), "{}", entry.label);
        }
    }

    #[test]
    fn no_two_commands_claim_the_same_real_binding() {
        let mut bindings: Vec<&str> = COMMANDS.iter().map(|entry| entry.binding).collect();
        let total = bindings.len();
        bindings.sort_unstable();
        bindings.dedup();
        assert_eq!(bindings.len(), total, "two commands bound to one key");
    }

    #[test]
    fn the_command_table_has_no_duplicate_shortcuts() {
        // Two commands on one key is how a palette starts lying about itself.
        let mut keys: Vec<&str> = COMMANDS.iter().map(|entry| entry.key).collect();
        let total = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), total);
    }

    #[test]
    fn every_place_is_reachable_from_the_palette() {
        for place in Place::ALL {
            assert!(
                COMMANDS.iter().any(|e| e.command == Command::Go(place)),
                "{} has no palette command",
                place.label()
            );
        }
    }

    #[test]
    fn an_empty_query_offers_everything_grouped() {
        let hits = matches("", &tools(), &pages());
        assert_eq!(hits.len(), COMMANDS.len() + 3 + 1);
        assert!(matches!(hits[0], Hit::Command(_)));
        assert!(matches!(hits.last().unwrap(), Hit::Page { .. }));
    }

    #[test]
    fn matching_is_case_insensitive_and_spans_all_three_kinds() {
        let hits = matches("ORDER", &tools(), &pages());
        let names: Vec<String> = hits
            .iter()
            .filter_map(|hit| match hit {
                Hit::Tool { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(names, vec!["create_order", "cancel_order"]);

        assert!(matches("localhost", &tools(), &pages())
            .iter()
            .any(|hit| matches!(hit, Hit::Page { .. })));
        assert!(matches("theme", &tools(), &pages())
            .iter()
            .any(|hit| matches!(hit, Hit::Command(_))));
    }

    #[test]
    fn a_query_that_hits_nothing_returns_nothing_rather_than_everything() {
        assert!(matches("zzzz", &tools(), &pages()).is_empty());
    }

    #[test]
    fn the_selection_stays_inside_the_results_as_they_narrow() {
        assert_eq!(clamp(9, 3), 2);
        assert_eq!(clamp(0, 0), 0, "an empty result set must not panic");
        assert_eq!(clamp(1, 3), 1);
    }

    #[test]
    fn moving_wraps_at_both_ends() {
        assert_eq!(step(0, 3, true), 1);
        assert_eq!(step(2, 3, true), 0);
        assert_eq!(step(0, 3, false), 2);
        assert_eq!(step(0, 0, true), 0);
    }
}
