use gpui::{
    div, font, point, prelude::*, px, rgb, BoxShadow, Div, Font, FontFallbacks, SharedString,
};

/// Near-black ground. Not slate-900.
pub const PAPER: u32 = 0x0A_0A_0A;
/// Slightly lifted panel fill, like SRCL window body.
pub const LIFT: u32 = 0x12_12_10;
pub const HOVER: u32 = 0x1C_1C_18;
/// Warm bone ink. Not slate-50.
pub const INK: u32 = 0xE6_E1_D3;
pub const MUTE: u32 = 0x7A_75_68;
/// Rule used for dashed frames.
pub const RULE: u32 = 0x4A_46_3C;
/// Fault color. Not Tailwind rose.
pub const RUST: u32 = 0xC4_5C_3A;

pub const ROW: f32 = 24.0;
pub const GUTTER: f32 = 10.0;

pub fn mono() -> Font {
    let mut face = font("Menlo");
    face.fallbacks = Some(FontFallbacks::from_fonts(vec![
        "SF Mono".into(),
        "JetBrains Mono".into(),
        "Cascadia Mono".into(),
        "Consolas".into(),
        "DejaVu Sans Mono".into(),
        "monospace".into(),
    ]));
    face
}

pub fn hard_shadow() -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: rgb(0x00_00_00).into(),
        offset: point(px(6.), px(6.)),
        blur_radius: px(0.),
        spread_radius: px(0.),
    }]
}

pub fn frame() -> Div {
    div()
        .flex()
        .flex_col()
        .min_h_0()
        .border_1()
        .border_dashed()
        .border_color(rgb(RULE))
        .bg(rgb(LIFT))
}

pub fn bracket(title: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .h(px(ROW))
        .px_2()
        .border_b_1()
        .border_dashed()
        .border_color(rgb(RULE))
        .text_color(rgb(MUTE))
        .child(title.into())
}

pub fn field_name(name: &str, required: bool) -> String {
    let upper = name.to_ascii_uppercase();
    if required {
        format!("{upper} *")
    } else {
        upper
    }
}

pub fn kind_cell(label: &str) -> String {
    format!("{label:<22}")
}
