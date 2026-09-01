//! Design tokens.
//!
//! Five colour roles, two palettes. Roles invert between light and dark; hues do
//! not — the identity is warm bone ink on a near-black ground, with one rust
//! accent that means *fault* and nothing else. Every value here is measured
//! against WCAG AA on its own ground; see the tests at the bottom, which fail if
//! a future edit makes text unreadable.

use gpui::{font, rgba, App, Font, FontFallbacks, Global, Rgba};

/// The five roles every surface is built from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// The ground the window sits on.
    pub paper: u32,
    /// Text and anything in focus.
    pub ink: u32,
    /// Labels, secondary values, chrome.
    pub mute: u32,
    /// Hairline rules. Never used for text.
    pub hair: u32,
    /// Fault only: a mutating tool, an invalid field, a failed run.
    pub accent: u32,
}

/// Near-black ground, warm bone ink. Not slate, not Tailwind grey.
pub const DARK: Palette = Palette {
    paper: 0x0A_0A_0A,
    ink: 0xE6_E1_D3,
    mute: 0x8A_85_77,
    hair: 0x2A_27_24,
    accent: 0xC4_5C_3A,
};

/// Roles inverted, hues kept. The accent is *not* a flip of the dark one: rust
/// at `#C45C3A` measures 3.6:1 on this ground, under AA, so it darkens.
pub const LIGHT: Palette = Palette {
    paper: 0xEF_EB_E0,
    ink: 0x1A_17_13,
    mute: 0x6B_66_58,
    hair: 0xDA_D3_C2,
    accent: 0xA8_45_2A,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Dark,
    Light,
}

impl Mode {
    pub fn palette(self) -> Palette {
        match self {
            Self::Dark => DARK,
            Self::Light => LIGHT,
        }
    }

    #[allow(dead_code)] // Phase 8: the ⌘⇧L theme toggle
    pub fn flipped(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }

    #[allow(dead_code)] // Phase 8: shown in the command palette
    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

/// Held as a GPUI global so any view can read the active palette without it
/// being threaded through every constructor.
pub struct Theme {
    pub mode: Mode,
}

impl Global for Theme {}

impl Theme {
    pub fn palette(&self) -> Palette {
        self.mode.palette()
    }
}

/// The active palette. Falls back to dark if the global was never installed, so
/// a view can never fail to render for want of a theme.
pub fn theme(cx: &App) -> Palette {
    cx.try_global::<Theme>()
        .map(Theme::palette)
        .unwrap_or(DARK)
}

#[allow(dead_code)] // Phase 8: the toggle needs to read the current mode
pub fn mode(cx: &App) -> Mode {
    cx.try_global::<Theme>().map(|t| t.mode).unwrap_or(Mode::Dark)
}

/// Selection fill: the ink colour held back far enough that text reads through it.
pub fn selection(palette: Palette) -> Rgba {
    rgba((palette.ink << 8) | 0x48)
}

/// Type scale. One size for everything is why V1 reads flat.
#[allow(dead_code)] // consumed from Phase 4, when views are built against it
pub mod text {
    /// Labels and chrome. Uppercase, tracked out.
    pub const LABEL: f32 = 10.0;
    pub const LABEL_LINE: f32 = 14.0;
    pub const LABEL_TRACKING: f32 = 0.12;
    /// Values, prose, code.
    pub const BODY: f32 = 13.0;
    pub const BODY_LINE: f32 = 22.0;
    /// The one thing in focus on a screen.
    pub const FOCUS: f32 = 22.0;
    pub const FOCUS_LINE: f32 = 30.0;
}

/// Spacing scale, replacing `ROW`/`GUTTER` coexisting with an implicit
/// 4/8/12/16 from the `px_2`/`gap_3` helpers.
#[allow(dead_code)] // consumed from Phase 4, when views are built against it
pub mod space {
    pub const XS: f32 = 8.0;
    pub const SM: f32 = 16.0;
    pub const MD: f32 = 24.0;
    pub const LG: f32 = 32.0;
    pub const XL: f32 = 40.0;
    pub const XXL: f32 = 56.0;
    pub const HUGE: f32 = 64.0;
    /// Top margin of a stage; the emptiness is the design.
    pub const PAGE: f32 = 96.0;
}

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

// --- Contrast, so the palette cannot silently regress ---

#[allow(dead_code)] // guards the palette in tests; no runtime caller by design
fn relative_luminance(color: u32) -> f64 {
    let channel = |raw: u32| {
        let value = f64::from(raw & 0xFF) / 255.0;
        if value <= 0.039_28 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(color >> 16) + 0.7152 * channel(color >> 8) + 0.0722 * channel(color)
}

/// WCAG 2.1 contrast ratio between two opaque colours, 1.0..=21.0.
#[allow(dead_code)] // guards the palette in tests; no runtime caller by design
pub fn contrast(a: u32, b: u32) -> f64 {
    let (first, second) = (relative_luminance(a), relative_luminance(b));
    let (hi, lo) = if first > second {
        (first, second)
    } else {
        (second, first)
    };
    (hi + 0.05) / (lo + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::rgb;

    #[test]
    fn contrast_matches_the_known_extremes() {
        assert!((contrast(0xFF_FF_FF, 0x00_00_00) - 21.0).abs() < 0.01);
        assert!((contrast(0x0A_0A_0A, 0x0A_0A_0A) - 1.0).abs() < 0.001);
    }

    #[test]
    fn every_text_role_meets_wcag_aa_on_its_own_ground() {
        for (name, palette) in [("dark", DARK), ("light", LIGHT)] {
            let ink = contrast(palette.ink, palette.paper);
            let mute = contrast(palette.mute, palette.paper);
            let accent = contrast(palette.accent, palette.paper);
            assert!(ink >= 7.0, "{name} ink is {ink:.2}:1, wanted AAA");
            assert!(mute >= 4.5, "{name} mute is {mute:.2}:1, wanted AA");
            assert!(accent >= 4.5, "{name} accent is {accent:.2}:1, wanted AA");
        }
    }

    #[test]
    fn the_mute_that_shipped_in_v1_would_fail_this_bar() {
        // `#7A7568` is what the app used. It measures under AA on the dark
        // ground and is the colour of every secondary label in the UI, which is
        // why the token moved. This test exists so it cannot move back.
        assert!(contrast(0x7A_75_68, DARK.paper) < 4.5);
        assert!(contrast(DARK.mute, DARK.paper) >= 4.5);
    }

    #[test]
    fn the_dark_accent_would_be_unreadable_on_light_paper() {
        // Error text must never be the least readable thing on screen, so the
        // light palette carries its own, darker rust rather than reusing this one.
        assert!(contrast(DARK.accent, LIGHT.paper) < 4.5);
        assert!(contrast(LIGHT.accent, LIGHT.paper) >= 4.5);
    }

    #[test]
    fn hairlines_stay_quiet_in_both_palettes() {
        // A rule should be felt, not read: visible against the ground but never
        // competing with text.
        for (name, palette) in [("dark", DARK), ("light", LIGHT)] {
            let hair = contrast(palette.hair, palette.paper);
            assert!(hair > 1.05, "{name} hair is invisible at {hair:.2}:1");
            assert!(hair < 2.0, "{name} hair is loud at {hair:.2}:1");
        }
    }

    #[test]
    fn the_two_palettes_are_genuinely_different() {
        assert_ne!(DARK, LIGHT);
        assert_eq!(Mode::Dark.flipped(), Mode::Light);
        assert_eq!(Mode::Light.flipped().palette(), DARK);
    }

    #[test]
    fn a_palette_is_five_distinct_roles() {
        // If two roles collapse to the same value, something on screen becomes
        // invisible — a hairline that reads as text, an accent that reads as ink.
        for (name, palette) in [("dark", DARK), ("light", LIGHT)] {
            let roles = [
                palette.paper,
                palette.ink,
                palette.mute,
                palette.hair,
                palette.accent,
            ];
            let mut unique = roles;
            unique.sort_unstable();
            let before = unique.len();
            let mut deduped = unique.to_vec();
            deduped.dedup();
            assert_eq!(deduped.len(), before, "{name} has duplicate roles");
        }
    }

    #[test]
    fn selection_is_the_ink_colour_with_alpha() {
        let fill = selection(DARK);
        let ink = rgb(DARK.ink);
        assert!((fill.r - ink.r).abs() < f32::EPSILON);
        assert!((fill.g - ink.g).abs() < f32::EPSILON);
        assert!((fill.b - ink.b).abs() < f32::EPSILON);
        assert!(fill.a > 0.0 && fill.a < 1.0);
    }
}
