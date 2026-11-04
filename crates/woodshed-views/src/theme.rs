//! Theme CSS for the genet host, derived through `tinct`.
//!
//! A theme is a few seed colours; `tinct::derive_palette` produces the full
//! semantic palette (surface ladder, text tiers, tonal triad, flags), and
//! this module renders it as the CSS sheet the views class against. Same
//! seeds as `audio-widgets::theme`'s Slate, so the genet host and the
//! xilem app agree until the parity cut.

use tinct::{Palette, Seeds, color_from_hex, color_to_hex, derive_palette};

fn hex(s: &str) -> tinct::Srgb {
    color_from_hex(s).expect("valid seed hex")
}

/// The built-in themes (seed sets match audio-widgets' engine, so the
/// genet host and the xilem app agree until the parity cut).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ThemeMode {
    #[default]
    Slate,
    Ember,
    Light,
    Dusk,
    Meadow,
    Parchment,
}

impl ThemeMode {
    pub const ALL: [ThemeMode; 6] = [
        ThemeMode::Slate,
        ThemeMode::Ember,
        ThemeMode::Light,
        ThemeMode::Dusk,
        ThemeMode::Meadow,
        ThemeMode::Parchment,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::Slate => "Slate",
            ThemeMode::Ember => "Ember",
            ThemeMode::Light => "Light",
            ThemeMode::Dusk => "Dusk",
            ThemeMode::Meadow => "Meadow",
            ThemeMode::Parchment => "Parchment",
        }
    }

    /// Inverse of [`label`](Self::label), for the persisted theme name.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|m| m.label() == name)
    }

    /// Stable identity for the shared Tabard appearance choice.
    pub fn theme_id(self) -> &'static str {
        match self {
            Self::Slate => "woodshed:slate",
            Self::Ember => "woodshed:ember",
            Self::Light => "woodshed:light",
            Self::Dusk => "woodshed:dusk",
            Self::Meadow => "woodshed:meadow",
            Self::Parchment => "woodshed:parchment",
        }
    }

    pub fn from_theme_id(id: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|theme| theme.theme_id() == id.trim().to_ascii_lowercase())
    }

    /// Preserve the authored Woodshed seeds in Tabard's portable definition.
    /// Editing this built-in goes through the workshop's user-copy boundary.
    pub fn definition(self) -> tabard::Theme {
        let mut theme = tabard::Theme::new(self.theme_id(), self.label(), self.seeds());
        theme.source = tabard::theme::registry::ThemeSource::BuiltIn;
        theme
    }

    pub fn seeds(self) -> Seeds {
        match self {
            // Slate — faithful cool-dark: blue / teal / amber.
            ThemeMode::Slate => Seeds {
                primary: hex("#3366c8"),
                secondary: hex("#2e9da6"),
                tertiary: hex("#e0a846"),
                neutral: hex("#101422"),
                text_header: None,
                text_body: None,
                success: hex("#4fb36e"),
                danger: hex("#d54e4e"),
                dark: true,
            },
            // Ember — warm fire: ember orange-red, gold roots, warm
            // charcoal-brown surfaces.
            ThemeMode::Ember => Seeds {
                primary: hex("#da5e3a"),
                secondary: hex("#c27a3c"),
                tertiary: hex("#ebb046"),
                neutral: hex("#24170f"),
                text_header: None,
                text_body: None,
                success: hex("#6fb36e"),
                danger: hex("#d5554e"),
                dark: true,
            },
            ThemeMode::Light => Seeds {
                primary: hex("#2a55b4"),
                secondary: hex("#1f777f"),
                tertiary: hex("#a86c14"),
                neutral: hex("#dfe3ee"),
                text_header: None,
                text_body: None,
                success: hex("#2f8a4f"),
                danger: hex("#b83333"),
                dark: false,
            },
            // Dusk — cool twilight: mauve / periwinkle / violet.
            ThemeMode::Dusk => Seeds {
                primary: hex("#cc6f8c"),
                secondary: hex("#7c71c4"),
                tertiary: hex("#b28adc"),
                neutral: hex("#191628"),
                text_header: None,
                text_body: None,
                success: hex("#6fb36e"),
                danger: hex("#d5554e"),
                dark: true,
            },
            // Meadow — moss / teal / wheat over green-dark panels.
            ThemeMode::Meadow => Seeds {
                primary: hex("#5ba86b"),
                secondary: hex("#3fa89e"),
                tertiary: hex("#e0b84a"),
                neutral: hex("#101e14"),
                text_header: None,
                text_body: None,
                success: hex("#6fc370"),
                danger: hex("#cf5151"),
                dark: true,
            },
            // Parchment — sepia / sage / ochre on warm cream paper.
            ThemeMode::Parchment => Seeds {
                primary: hex("#8a5a2b"),
                secondary: hex("#4e7c6a"),
                tertiary: hex("#a8731a"),
                neutral: hex("#f0e6ce"),
                text_header: None,
                text_body: None,
                success: hex("#2f8a4f"),
                danger: hex("#b83333"),
                dark: false,
            },
        }
    }

    /// The full Stage sheet for this theme.
    pub fn css(self) -> String {
        stage_css(&derive_palette(&self.seeds()))
    }
}

/// The Slate seeds (kept for callers that want the default explicitly).
pub fn slate_seeds() -> Seeds {
    ThemeMode::Slate.seeds()
}

/// The Stage sheet rendered from a derived palette.
pub fn stage_css(p: &Palette) -> String {
    stage_css_with_roles(p, false)
}

/// The same product mapping with shared role-variable intake. Fallback colors
/// stay explicit; arbitrary authored CSS is never converted into a palette.
pub fn stage_css_from_roles(fallback: &Palette) -> String {
    stage_css_with_roles(fallback, true)
}

/// The default sheet: Slate seeds through the derivation.
pub fn slate_stage_css() -> String {
    stage_css(&derive_palette(&slate_seeds()))
}

/// The UI text-scale factor for a named setting ("Large" / "Larger", else 1.0).
pub fn text_scale_factor(name: &str) -> f32 {
    match name {
        "Large" => 1.15,
        "Larger" => 1.3,
        _ => 1.0,
    }
}

/// Apply the accessibility preferences to a generated sheet, as a post-process
/// so the base theme stays untouched. Reduced motion drops the non-essential
/// hover/active fades; a text scale multiplies every `font-size`.
pub fn apply_accessibility(mut css: String, reduce_motion: bool, text_scale: f32) -> String {
    if (text_scale - 1.0).abs() > 0.001 {
        css = scale_font_sizes(&css, text_scale);
    }
    if reduce_motion {
        // Later rules win at equal specificity, so override the four transition
        // rules (theme.rs) to none.
        css.push_str(
            "\n.search-item, .prog-card, .pill, .lens, .film-card, .recipe-tile, \
             .bar-chip, .search-wrap [role=textbox] { transition: none; }\n",
        );
    }
    css
}

/// Scale every `font-size: Npx` in a sheet by `scale`, leaving all else intact.
fn scale_font_sizes(css: &str, scale: f32) -> String {
    let mut css = scale_px_declarations(css, "font-size: ", scale);
    for role in tabard::typography::FontRole::ALL {
        css = scale_px_declarations(&css, &format!("--tabard-font-{}-size: ", role.key()), scale);
    }
    css
}

fn scale_px_declarations(css: &str, needle: &str, scale: f32) -> String {
    let mut out = String::with_capacity(css.len() + 32);
    let mut rest = css;
    while let Some(pos) = rest.find(needle) {
        out.push_str(&rest[..pos + needle.len()]);
        rest = &rest[pos + needle.len()..];
        match rest.find("px") {
            Some(px) => match rest[..px].trim().parse::<f32>() {
                Ok(n) => {
                    out.push_str(&((n * scale).round() as i32).to_string());
                    out.push_str("px");
                    rest = &rest[px + 2..];
                },
                // Not a plain "Npx" — leave the declaration as written.
                Err(_) => {},
            },
            None => {},
        }
    }
    out.push_str(rest);
    out
}

#[path = "theme/stylesheet.rs"]
mod stylesheet;

use stylesheet::stage_css_with_roles;
