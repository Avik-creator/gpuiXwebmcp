//! The playground: learn what this app does without a browser at all.
//!
//! Sample data used to be a hidden toggle sitting where the connection status
//! lives — two jobs on one control, and the wrong place for it, since "sample
//! data" is not a connection state. It is a place you choose to go.
//!
//! Every step below completes on something the app can actually observe, so the
//! walkthrough can never claim you did something you did not.

use gpui::{div, prelude::*, px, rgb, Context, SharedString};

use crate::theme::{space, text, Palette};
use crate::Debugger;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    PickTool,
    RunIt,
    SeeHistory,
    Done,
}

pub const TOTAL: usize = 3;

impl Step {
    /// Which numbered step this is, or `None` once the tour is finished.
    pub fn number(self) -> Option<usize> {
        match self {
            Self::PickTool => Some(1),
            Self::RunIt => Some(2),
            Self::SeeHistory => Some(3),
            Self::Done => None,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::PickTool => "Pick a tool",
            Self::RunIt => "Fill it in and press Run",
            Self::SeeHistory => "Open History",
            Self::Done => "That is the whole loop",
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            Self::PickTool => {
                "These three are built into the app. Click one to see what it takes."
            }
            Self::RunIt => {
                "Required inputs are marked with a star. Run is disabled until they are filled."
            }
            Self::SeeHistory => "Everything you run is recorded. History keeps the whole session.",
            Self::Done => {
                "Connect Chrome and the same three screens work against a real site instead."
            }
        }
    }
}

/// Derived from what actually happened, never stored separately.
pub fn step(picked_tool: bool, has_result: bool, seen_history: bool) -> Step {
    if !picked_tool {
        return Step::PickTool;
    }
    if !has_result {
        return Step::RunIt;
    }
    if !seen_history {
        return Step::SeeHistory;
    }
    Step::Done
}

pub fn strip(debugger: &Debugger, palette: Palette, cx: &mut Context<Debugger>) -> gpui::Div {
    let current = debugger.walkthrough_step();
    let counter = match current.number() {
        Some(number) => format!("STEP {number} OF {TOTAL}"),
        None => "DONE".to_string(),
    };

    div()
        .flex()
        .flex_col()
        .flex_shrink_0()
        .mb(px(space::LG))
        .pb(px(space::XS))
        .border_b_1()
        .border_color(rgb(palette.hair))
        .child(
            div()
                .flex()
                .flex_row()
                .items_baseline()
                .justify_between()
                .gap(px(space::SM))
                .text_size(px(text::LABEL))
                .line_height(px(text::LABEL_LINE))
                .text_color(rgb(palette.mute))
                .child(SharedString::from(counter))
                .child(
                    div()
                        .id("walkthrough-hide")
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.walkthrough_hidden = true;
                            cx.notify();
                        }))
                        .child(SharedString::from("HIDE")),
                ),
        )
        .child(
            div()
                .mt(px(6.))
                .text_size(px(text::BODY))
                .line_height(px(text::BODY_LINE))
                .text_color(rgb(palette.ink))
                .child(SharedString::from(current.title())),
        )
        .child(
            div()
                .text_size(px(text::BODY))
                .line_height(px(text::BODY_LINE))
                .text_color(rgb(palette.mute))
                .child(SharedString::from(current.detail())),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tour_follows_what_you_actually_did() {
        assert_eq!(step(false, false, false), Step::PickTool);
        assert_eq!(step(true, false, false), Step::RunIt);
        assert_eq!(step(true, true, false), Step::SeeHistory);
        assert_eq!(step(true, true, true), Step::Done);
    }

    #[test]
    fn a_step_cannot_be_skipped_by_doing_a_later_thing_first() {
        // Opening History before running anything must not mark the run done.
        assert_eq!(step(false, false, true), Step::PickTool);
        assert_eq!(step(true, false, true), Step::RunIt);
    }

    #[test]
    fn every_step_is_numbered_and_worded() {
        for candidate in [Step::PickTool, Step::RunIt, Step::SeeHistory] {
            assert!(candidate.number().is_some());
            assert!(!candidate.title().is_empty());
            assert!(!candidate.detail().is_empty());
        }
        assert!(Step::Done.number().is_none(), "done is not a step to do");
        assert!(!Step::Done.title().is_empty());
    }

    #[test]
    fn the_counter_never_runs_past_the_total() {
        for candidate in [Step::PickTool, Step::RunIt, Step::SeeHistory] {
            let number = candidate.number().unwrap();
            assert!(number >= 1 && number <= TOTAL, "step {number} is outside 1..={TOTAL}");
        }
    }
}
