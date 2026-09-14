//! Seeds, tokens, and the one Redshank stylesheet.
//!
//! Lane S1 owns this file. Both seeds derive through `tinct`; the sheet
//! consumes only `--t-*`, `--font-*`, `--space-*`, and `--text-*` tokens.
//!
//! The brand shell seed derives from `tinct` exactly — its canvas values are
//! themselves a `tinct` dark ladder off the shell neutral `#1B1523`. The
//! wetland seed is not: the canvas author placed the greens off `tinct`'s dark
//! lightness ladder (canvas OKLCH L 0.197 / 0.234 / 0.332 / 0.362 against
//! `tinct`'s 0.16 / 0.21 / 0.26 / 0.32), and `tinct` steps one chroma through
//! the whole ladder while the canvas saturates as it lightens. The best fit
//! still misses by up to 24/255 per channel, so the wetland **dark** neutral
//! and text roles are overridden with the endorsed canvas values; structure,
//! accents, `on_*` picks, and every other wetland mode stay derived.

use crate::{Mode, Seed};
use tinct::{ModeProfile, Palette, Seeds, Srgb, color_to_hex, derive_palette_with};

const EMBER: Srgb = Srgb::rgb(0xC7, 0x80, 0x2F);
const SUCCESS: Srgb = Srgb::rgb(0x4F, 0xB3, 0x6E);

/// Wetland: the launch seed. Neutral is the canvas surface, so hue and chroma
/// of the derived modes stay the canvas green.
fn wetland_seeds() -> Seeds {
    Seeds {
        primary: Srgb::rgb(0xD3, 0xE8, 0xDC),
        secondary: Srgb::rgb(0x49, 0x64, 0x5C),
        tertiary: EMBER,
        neutral: Srgb::rgb(0x17, 0x20, 0x1F),
        text_header: None,
        text_body: None,
        success: SUCCESS,
        danger: Srgb::rgb(0x8D, 0x34, 0x34),
        dark: true,
    }
}

/// Brand shell: oxblood, lake, ember over the shell neutral.
fn brand_seeds() -> Seeds {
    Seeds {
        primary: Srgb::rgb(0x6E, 0x17, 0x12),
        secondary: Srgb::rgb(0x29, 0x48, 0x6B),
        tertiary: EMBER,
        neutral: Srgb::rgb(0x1B, 0x15, 0x23),
        text_header: None,
        text_body: None,
        success: SUCCESS,
        danger: Srgb::rgb(0xD5, 0x4E, 0x4E),
        dark: true,
    }
}

/// The canvas-authored wetland dark neutrals, which `tinct`'s ladder cannot
/// reach (see the module note). Order: bg, surface, surface-2, surface-hover,
/// text, text-dim, text-disabled.
const WETLAND_DARK_NEUTRALS: [Srgb; 7] = [
    Srgb::rgb(0x10, 0x17, 0x16),
    Srgb::rgb(0x17, 0x20, 0x1F),
    Srgb::rgb(0x29, 0x3A, 0x36),
    Srgb::rgb(0x30, 0x42, 0x3D),
    Srgb::rgb(0xED, 0xF2, 0xEF),
    Srgb::rgb(0xB5, 0xC4, 0xBD),
    Srgb::rgb(0x6F, 0x80, 0x79),
];

fn profile(mode: Mode) -> ModeProfile {
    match mode {
        Mode::Dark => ModeProfile::DARK,
        Mode::Light => ModeProfile::LIGHT,
        Mode::HcDark => ModeProfile::HC_DARK,
        Mode::HcLight => ModeProfile::HC_LIGHT,
    }
}

fn seeds(seed: Seed) -> Seeds {
    match seed {
        Seed::Wetland => wetland_seeds(),
        Seed::BrandShell => brand_seeds(),
    }
}

/// The `tinct` derivation with no Redshank correction applied. Public so a test
/// can measure the drift the overrides stand in for.
pub fn derived_palette(seed: Seed, mode: Mode) -> Palette {
    derive_palette_with(&seeds(seed), profile(mode))
}

/// The palette a scope class ships: derived, then corrected where the endorsed
/// canvas is off the ladder.
pub fn palette(seed: Seed, mode: Mode) -> Palette {
    let mut palette = derived_palette(seed, mode);
    if let (Seed::Wetland, Mode::Dark) = (seed, mode) {
        let [bg, surface, surface_2, surface_hover, text, dim, disabled] = WETLAND_DARK_NEUTRALS;
        palette.bg = bg;
        palette.surface = surface;
        palette.surface_2 = surface_2;
        palette.surface_hover = surface_hover;
        palette.text_header = text;
        palette.text = text;
        palette.text_dim = dim;
        palette.text_disabled = disabled;
    }
    palette
}

/// Every seed and mode, in scope-class order.
pub const SCOPES: [(Seed, Mode); 8] = [
    (Seed::Wetland, Mode::Dark),
    (Seed::Wetland, Mode::Light),
    (Seed::Wetland, Mode::HcDark),
    (Seed::Wetland, Mode::HcLight),
    (Seed::BrandShell, Mode::Dark),
    (Seed::BrandShell, Mode::Light),
    (Seed::BrandShell, Mode::HcDark),
    (Seed::BrandShell, Mode::HcLight),
];

/// The scope class a seed and mode pair carries, mirroring
/// [`RedshankSurfaceState::scope_class`](crate::RedshankSurfaceState::scope_class).
pub fn scope_class(seed: Seed, mode: Mode) -> &'static str {
    match (seed, mode) {
        (Seed::Wetland, Mode::Dark) => "t-redshank",
        (Seed::Wetland, Mode::Light) => "t-redshank-light",
        (Seed::Wetland, Mode::HcDark) => "t-redshank-hc-dark",
        (Seed::Wetland, Mode::HcLight) => "t-redshank-hc-light",
        (Seed::BrandShell, Mode::Dark) => "t-dark",
        (Seed::BrandShell, Mode::Light) => "t-light",
        (Seed::BrandShell, Mode::HcDark) => "t-hc-dark",
        (Seed::BrandShell, Mode::HcLight) => "t-hc-light",
    }
}

/// The `--t-*` role block for one palette, without the braces.
fn roles(palette: &Palette) -> String {
    let hex = color_to_hex;
    [
        ("bg", palette.bg),
        ("surface", palette.surface),
        ("surface-2", palette.surface_2),
        ("surface-hover", palette.surface_hover),
        ("text-header", palette.text_header),
        ("text", palette.text),
        ("text-dim", palette.text_dim),
        ("text-disabled", palette.text_disabled),
        ("primary", palette.primary),
        ("on-primary", palette.on_primary),
        ("secondary", palette.secondary),
        ("on-secondary", palette.on_secondary),
        ("tertiary", palette.tertiary),
        ("on-tertiary", palette.on_tertiary),
        ("success", palette.success),
        ("danger", palette.danger),
    ]
    .into_iter()
    .map(|(role, colour)| format!("--t-{role}: {};", hex(colour)))
    .collect::<Vec<_>>()
    .join(" ")
}

/// The design-system tokens the sheet spends: families, type scale, tracking,
/// and the spacing step. Emitted on the root and on the app element, because
/// the scope class rides the app element itself.
fn tokens() -> &'static str {
    concat!(
        ":root, .rs-app { ",
        "--font-ui: 'IBM Plex Sans', sans-serif; ",
        "--font-mono: 'IBM Plex Mono', monospace; ",
        "--text-caption: 9px; --text-microlabel: 10px; --text-label: 11px; ",
        "--text-ui-12: 12px; --text-ui-13: 13px; --text-ui-14: 14px; --text-ui-16: 16px; ",
        "--track-microlabel: 0.14em; --track-badge: 0.1em; ",
        "--space-2: 2px; --space-4: 4px; --space-6: 6px; --space-8: 8px; ",
        "--space-10: 10px; --space-12: 12px; --space-14: 14px; --space-16: 16px; ",
        "--space-20: 20px; --space-24: 24px; --space-32: 32px; --space-40: 40px; ",
        "--space-56: 56px; --radius: 2px; ",
        "}\n"
    )
}

/// The full stylesheet: scope classes for every seed and mode, tokens, and
/// the `rs-*` component rules from `redshank.css`.
pub fn sheet() -> String {
    let mut sheet = String::new();
    for (seed, mode) in SCOPES {
        let palette = palette(seed, mode);
        sheet.push('.');
        sheet.push_str(scope_class(seed, mode));
        sheet.push_str(" { ");
        sheet.push_str(&roles(&palette));
        sheet.push_str(" }\n");
    }
    sheet.push_str(tokens());
    sheet.push_str(include_str!("redshank.css"));
    sheet.push_str(crate::tabs::TABS_CSS);
    sheet.push_str(crate::scene::SCENE_CSS);
    sheet
}

#[cfg(test)]
mod tests {
    use super::*;

    fn delta(a: Srgb, b: Srgb) -> i32 {
        let channel = |x: u8, y: u8| (x as i32 - y as i32).abs();
        channel(a.r, b.r)
            .max(channel(a.g, b.g))
            .max(channel(a.b, b.b))
    }

    fn canvas(hex: &str) -> Srgb {
        tinct::color_from_hex(hex).expect("canvas hex")
    }

    /// The shipped wetland dark palette is the endorsed canvas, within a few
    /// points on every role.
    #[test]
    fn wetland_dark_matches_the_canvas() {
        let palette = palette(Seed::Wetland, Mode::Dark);
        let pairs = [
            (palette.bg, "#101716"),
            (palette.surface, "#17201f"),
            (palette.surface_2, "#293a36"),
            (palette.surface_hover, "#30423d"),
            (palette.text, "#edf2ef"),
            (palette.text_dim, "#b5c4bd"),
            (palette.text_disabled, "#6f8079"),
            (palette.primary, "#d3e8dc"),
            (palette.secondary, "#49645c"),
            (palette.tertiary, "#c7802f"),
            (palette.danger, "#8d3434"),
        ];
        for (got, want) in pairs {
            let want = canvas(want);
            assert!(
                delta(got, want) <= 4,
                "{} drifted from {} by {}",
                color_to_hex(got),
                color_to_hex(want),
                delta(got, want)
            );
        }
    }

    /// The brand shell needs no correction: its canvas values are a `tinct`
    /// ladder off the shell neutral, so the raw derivation is exact.
    #[test]
    fn brand_dark_derives_exactly_from_tinct() {
        let palette = derived_palette(Seed::BrandShell, Mode::Dark);
        let pairs = [
            (palette.bg, "#100A17"),
            (palette.surface, "#1B1523"),
            (palette.surface_2, "#27212F"),
            (palette.surface_hover, "#36303E"),
            (palette.text, "#F2EAFE"),
            (palette.text_dim, "#ADA6B8"),
            (palette.text_disabled, "#716A7B"),
        ];
        // `tinct`'s OKLCH round trip lands a channel off by one on the hover
        // step; every other role is exact.
        for (got, want) in pairs {
            let want = canvas(want);
            assert!(
                delta(got, want) <= 1,
                "{} is not {}",
                color_to_hex(got),
                color_to_hex(want)
            );
        }
    }

    /// Records the drift the wetland override stands in for, so the day the
    /// seeds or `tinct`'s ladder change the number is visible.
    #[test]
    fn wetland_derivation_drift_is_recorded() {
        let derived = derived_palette(Seed::Wetland, Mode::Dark);
        let pairs = [
            ("bg", derived.bg, "#101716"),
            ("surface", derived.surface, "#17201f"),
            ("surface-2", derived.surface_2, "#293a36"),
            ("surface-hover", derived.surface_hover, "#30423d"),
            ("text", derived.text, "#edf2ef"),
            ("text-dim", derived.text_dim, "#b5c4bd"),
            ("text-disabled", derived.text_disabled, "#6f8079"),
        ];
        let mut worst = 0;
        for (role, got, want) in pairs {
            let drift = delta(got, canvas(want));
            println!(
                "wetland {role}: tinct {} canvas {want} drift {drift}",
                color_to_hex(got)
            );
            worst = worst.max(drift);
        }
        assert!(worst > 4, "tinct now reaches the canvas; drop the override");
    }

    #[test]
    fn sheet_carries_every_scope_and_the_component_rules() {
        let sheet = sheet();
        for (seed, mode) in SCOPES {
            assert!(sheet.contains(&format!(".{} {{", scope_class(seed, mode))));
        }
        assert!(sheet.contains("--t-tertiary: #C7802F;"));
        assert!(sheet.contains(".rs-dock"));
        assert!(sheet.contains("--font-ui"));
    }

    /// Livery has no `outline`; active rings are inset box-shadows.
    #[test]
    fn sheet_uses_no_outline_and_no_ellipsis() {
        let sheet = sheet();
        // The banner comment names both; the rules must not use either.
        let rules = sheet
            .lines()
            .filter(|line| !line.trim_start().starts_with('*') && !line.contains("/*"))
            .collect::<String>();
        assert!(!rules.contains("outline"));
        assert!(!rules.contains("text-overflow"));
    }
}
