//! The window shell: a bar that is also the router.
//!
//! Of everything the debugger shows, only three things are *places* you go to.
//! Running, result and failure are states Compose passes through; the palette is
//! an overlay; the origin is context. Three destinations do not earn a permanent
//! rail — they earn three words in a bar the window already had.

use gpui::{div, prelude::*, px, rgb, Context, IntoElement, SharedString};

use crate::theme::{self, space, text};
use crate::Debugger;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// What this site offers.
    Survey,
    /// Fill in and run one tool.
    Compose,
    /// What has happened.
    Record,
}

impl Place {
    pub const ALL: [Place; 3] = [Place::Survey, Place::Compose, Place::Record];

    pub fn label(self) -> &'static str {
        match self {
            Self::Survey => "TOOLS",
            Self::Compose => "RUN",
            Self::Record => "HISTORY",
        }
    }

    /// Compose needs an object before it means anything; everywhere else is
    /// always reachable. The bar and the keyboard shortcut both ask this, so
    /// they cannot disagree about what is navigable.
    pub fn reachable(self, has_selected_tool: bool) -> bool {
        match self {
            Self::Compose => has_selected_tool,
            Self::Survey | Self::Record => true,
        }
    }
}

/// Record shows one stream at two altitudes: the runs you made, or every event
/// that crossed the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Altitude {
    Runs,
    Everything,
}

impl Altitude {
    pub fn label(self) -> &'static str {
        match self {
            Self::Runs => "RUNS",
            Self::Everything => "ALL ACTIVITY",
        }
    }

    pub fn flipped(self) -> Self {
        match self {
            Self::Runs => Self::Everything,
            Self::Everything => Self::Runs,
        }
    }
}

/// Where you have been, so there is always a way back.
///
/// The bar could jump you anywhere but never return you, which leaves a dead end
/// after any move you did not mean to make.
#[derive(Clone, Debug, PartialEq)]
pub struct Nav {
    current: Place,
    back: Vec<Place>,
    forward: Vec<Place>,
}

/// Deep enough for any real session, shallow enough that it cannot grow forever.
pub const NAV_DEPTH: usize = 32;

impl Nav {
    pub fn new(start: Place) -> Self {
        Self { current: start, back: Vec::new(), forward: Vec::new() }
    }

    pub fn current(&self) -> Place {
        self.current
    }

    /// Move somewhere new. Going where you already are is not a step.
    pub fn go(&mut self, place: Place) -> bool {
        if place == self.current {
            return false;
        }
        self.back.push(self.current);
        if self.back.len() > NAV_DEPTH {
            self.back.remove(0);
        }
        // A new move abandons whatever you had gone back from.
        self.forward.clear();
        self.current = place;
        true
    }

    pub fn back(&mut self) -> bool {
        match self.back.pop() {
            Some(previous) => {
                self.forward.push(self.current);
                self.current = previous;
                true
            }
            None => false,
        }
    }

    pub fn forward(&mut self) -> bool {
        match self.forward.pop() {
            Some(next) => {
                self.back.push(self.current);
                self.current = next;
                true
            }
            None => false,
        }
    }

    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    /// Forget a place that no longer exists — Run stops being reachable when the
    /// tool it was showing goes away, and a back button must not strand you there.
    pub fn forget(&mut self, place: Place) {
        self.back.retain(|item| *item != place);
        self.forward.retain(|item| *item != place);
    }
}

/// A 10px tracked-out label, the chrome voice used everywhere.
#[allow(dead_code)] // consumed by the place views in Phase 6
pub fn label(content: impl Into<SharedString>) -> gpui::Div {
    div()
        .text_size(px(text::LABEL))
        .line_height(px(text::LABEL_LINE))
        .child(content.into())
}

/// The bar. Places on the left with the current one lit; origin and connection
/// on the right. No app name — the OS titlebar already carries it.
pub fn bar(debugger: &Debugger, cx: &mut Context<Debugger>) -> impl IntoElement {
    let palette = theme::theme(cx);
    let active = debugger.nav.current();
    // Compose needs an object before it means anything. Dimming it is more
    // honest than hiding it and making it appear later.
    let compose_ready = debugger.state.selected_tool.is_some();
    let (status, fault) = debugger.status_tag();
    let origin = debugger
        .state
        .selected_page()
        .map(|page| page.origin.clone())
        .unwrap_or_else(|| "No site".to_string());

    // Built eagerly: each entry needs its own listener, so `cx` is borrowed once
    // per place rather than captured into a closure that outlives the loop.
    let mut places = Vec::with_capacity(Place::ALL.len());
    for place in Place::ALL {
        let reachable = place.reachable(compose_ready);
        let color = if place == active {
            palette.ink
        } else if reachable {
            palette.mute
        } else {
            palette.hair
        };
        places.push(
            div()
                .id(SharedString::from(place.label()))
                .text_color(rgb(color))
                .when(reachable, |el| {
                    el.cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| this.go_to(place, cx)))
                })
                .child(SharedString::from(place.label())),
        );
    }

    // Right side is a report, not a control. It used to double as a hidden
    // toggle into sample data, which meant the one place showing connection
    // state also silently changed what you were looking at.
    let right = if debugger.in_playground() {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(space::SM))
            .child(
                div()
                    .text_color(rgb(palette.accent))
                    .child(SharedString::from("Playground · sample data")),
            )
            .child(
                div()
                    .id("leave-playground")
                    .cursor_pointer()
                    .text_color(rgb(palette.ink))
                    .on_click(cx.listener(|this, _, _, cx| this.leave_playground(cx)))
                    .child(SharedString::from("LEAVE")),
            )
    } else {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(space::XS))
            .min_w_0()
            .child(div().min_w_0().truncate().child(SharedString::from(origin)))
            .child(SharedString::from("·"))
            .child(
                div()
                    .text_color(rgb(if fault { palette.accent } else { palette.mute }))
                    .child(SharedString::from(status)),
            )
    };

    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .flex_shrink_0()
        .h(px(56.))
        .px(px(space::XL))
        .text_size(px(text::LABEL))
        .line_height(px(text::LABEL_LINE))
        .text_color(rgb(palette.mute))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(space::MD))
                .child(
                    div()
                        .id("go-back")
                        .cursor_pointer()
                        .text_color(rgb(if debugger.nav.can_go_back() {
                            palette.mute
                        } else {
                            palette.hair
                        }))
                        .on_click(cx.listener(|this, _, _, cx| this.go_back(cx)))
                        .child(SharedString::from("‹ BACK")),
                )
                .child(
                    div()
                        .id("go-forward")
                        .cursor_pointer()
                        .text_color(rgb(if debugger.nav.can_go_forward() {
                            palette.mute
                        } else {
                            palette.hair
                        }))
                        .on_click(cx.listener(|this, _, _, cx| this.go_forward(cx)))
                        .child(SharedString::from("›")),
                )
                .children(places),
        )
        .child(right)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_place_is_named_and_reachable_from_the_bar() {
        assert_eq!(Place::ALL.len(), 3);
        for place in Place::ALL {
            assert!(!place.label().is_empty());
        }
    }

    #[test]
    fn place_labels_are_distinct() {
        let labels: Vec<_> = Place::ALL.into_iter().map(Place::label).collect();
        let mut unique = labels.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(labels.len(), unique.len());
    }

    #[test]
    fn compose_is_the_only_place_that_needs_a_tool_first() {
        assert!(!Place::Compose.reachable(false));
        assert!(Place::Compose.reachable(true));
        for place in [Place::Survey, Place::Record] {
            assert!(place.reachable(false), "{} must always be reachable", place.label());
            assert!(place.reachable(true));
        }
    }

    #[test]
    fn every_place_is_reachable_once_a_tool_is_selected() {
        // "any route from any screen" — with a tool picked, nothing is a dead end.
        for place in Place::ALL {
            assert!(place.reachable(true));
        }
    }

    #[test]
    fn back_returns_you_to_where_you_were() {
        let mut nav = Nav::new(Place::Survey);
        nav.go(Place::Compose);
        nav.go(Place::Record);
        assert!(nav.back());
        assert_eq!(nav.current(), Place::Compose);
        assert!(nav.back());
        assert_eq!(nav.current(), Place::Survey);
        assert!(!nav.back(), "nothing before the start");
        assert_eq!(nav.current(), Place::Survey);
    }

    #[test]
    fn forward_undoes_a_back() {
        let mut nav = Nav::new(Place::Survey);
        nav.go(Place::Record);
        nav.back();
        assert!(nav.can_go_forward());
        assert!(nav.forward());
        assert_eq!(nav.current(), Place::Record);
        assert!(!nav.forward());
    }

    #[test]
    fn a_new_move_abandons_the_forward_trail() {
        let mut nav = Nav::new(Place::Survey);
        nav.go(Place::Record);
        nav.back();
        nav.go(Place::Compose);
        assert!(!nav.can_go_forward(), "the branch you left is gone");
        assert!(nav.back());
        assert_eq!(nav.current(), Place::Survey);
    }

    #[test]
    fn going_where_you_already_are_is_not_a_step() {
        let mut nav = Nav::new(Place::Survey);
        assert!(!nav.go(Place::Survey));
        assert!(!nav.can_go_back(), "clicking the current place must not stack up");
    }

    #[test]
    fn the_trail_cannot_grow_without_bound() {
        let mut nav = Nav::new(Place::Survey);
        for index in 0..NAV_DEPTH * 3 {
            nav.go(if index % 2 == 0 { Place::Record } else { Place::Survey });
        }
        assert!(nav.back.len() <= NAV_DEPTH);
    }

    #[test]
    fn a_place_that_stops_existing_is_forgotten() {
        // Run is unreachable once its tool is gone; back must not strand you there.
        let mut nav = Nav::new(Place::Survey);
        nav.go(Place::Compose);
        nav.go(Place::Record);
        nav.forget(Place::Compose);
        assert!(nav.back());
        assert_eq!(nav.current(), Place::Survey);
    }

    #[test]
    fn record_altitude_round_trips() {
        assert_eq!(Altitude::Runs.flipped(), Altitude::Everything);
        assert_eq!(Altitude::Runs.flipped().flipped(), Altitude::Runs);
        assert_ne!(Altitude::Runs.label(), Altitude::Everything.label());
    }
}
