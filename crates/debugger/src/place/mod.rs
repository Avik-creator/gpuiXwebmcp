//! The places you can be. One job per screen, most of each screen empty.
//!
//! Every place renders into the same centred column, so the eye never has to
//! re-find the content when you move between them.

pub mod compose;
pub mod record;
pub mod survey;

use gpui::{div, prelude::*, px, rgb, Div, SharedString};

use crate::theme::{self, text, Palette};

/// Width of the reading column. Everything a place shows lives inside it.
pub const COLUMN: f32 = 640.0;

/// The centred stage a place renders into.
pub fn stage() -> Div {
    div()
        .flex()
        .flex_row()
        .justify_center()
        .flex_1()
        .min_h_0()
        .overflow_hidden()
}

/// The reading column, with the space above it that makes a screen feel composed.
pub fn column(top: f32) -> Div {
    div().w(px(COLUMN)).pt(px(top)).flex().flex_col().min_h_0()
}

/// The chrome voice: small, uppercase, quiet.
///
/// The design tracks these out by 0.12em. gpui 0.2.2 has no letter-spacing, so
/// the voice comes from size, case and colour instead — see `theme::text`.
pub fn label(palette: Palette, content: impl Into<SharedString>) -> Div {
    div()
        .flex_shrink_0()
        .text_size(px(text::LABEL))
        .line_height(px(text::LABEL_LINE))
        .text_color(rgb(palette.mute))
        .child(content.into())
}

/// The one thing in focus on a screen.
pub fn focus(palette: Palette, content: impl Into<SharedString>) -> Div {
    div()
        .flex_shrink_0()
        .text_size(px(text::FOCUS))
        .line_height(px(text::FOCUS_LINE))
        .text_color(rgb(palette.ink))
        .child(content.into())
}

pub fn body(palette: Palette, content: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(text::BODY))
        .line_height(px(text::BODY_LINE))
        .text_color(rgb(palette.ink))
        .child(content.into())
}

pub fn muted(palette: Palette, content: impl Into<SharedString>) -> Div {
    body(palette, content).text_color(rgb(palette.mute))
}

/// A hairline under a row. Felt, not read.
#[allow(dead_code)] // Phase 7: the field rows in Compose
pub fn rule(palette: Palette) -> Div {
    div()
        .h(px(1.))
        .flex_shrink_0()
        .bg(rgb(palette.hair))
}

#[allow(dead_code)] // Phase 7
/// A one-line hover explanation, so detail is available without being on screen.
pub struct Tip {
    text: SharedString,
}

impl Tip {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self { text: text.into() }
    }
}

impl gpui::Render for Tip {
    fn render(&mut self, _: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let palette = theme::theme(cx);
        div()
            .max_w(px(340.))
            .p(px(10.))
            .bg(rgb(palette.paper))
            .border_1()
            .border_color(rgb(palette.hair))
            .font(theme::mono())
            .text_size(px(text::BODY))
            .line_height(px(text::BODY_LINE))
            .text_color(rgb(palette.ink))
            .child(self.text.clone())
    }
}

#[allow(dead_code)] // Phase 7
pub fn palette_of(cx: &gpui::App) -> Palette {
    theme::theme(cx)
}
