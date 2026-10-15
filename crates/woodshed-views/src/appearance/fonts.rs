//! Product role mapping over Tabard's portable requests and authored defaults.

use std::collections::BTreeMap;

use tabard::Theme;
use tabard::typography::{FontFamily, FontPreferences, FontRole};

pub const INTERFACE: &str = ".root, .chrome, .pill, .t-btn, .side-item, .seg, .appearance-options button, .appearance-modes button, input, select, button";
pub const READING: &str = ".recipe-desc, .recipe-body, .related-why, .related-subtitle, .related-empty, .caption, .set-empty, .card-setup-description, .settings-line, .appearance-notice, .loop-export-help";
pub const HEADING: &str = ".chrome-title, .settings-heading, .set-heading, .related-heading, .recipe-name, .stage-context-title, .note-card-title";
pub const LABEL: &str = ".set-card-title, .set-card-meta, .set-card-kind, .set-card-source, .bar-label, .bar-chord, .bar-meta, .stage-context-label, .set-graph-heading, .set-graph-layout-label, .set-graph-reading, .neck-label, .neck-value, .prog-numeral, .prog-chord, .tool-reading";
pub const FRET_LABELS: &str =
    ".fret-label, .fret-dot, .open-label, .fret-number, .graph-canvas-swatch-label";
pub const CODE: &str = "code, pre, .machine-text";

/// Current Woodshed text stays sans-serif until a role is explicitly authored
/// or requested. Code has its ordinary semantic monospace default.
pub fn defaults(theme: &Theme, available: &[String]) -> BTreeMap<FontRole, FontFamily> {
    FontRole::ALL
        .into_iter()
        .map(|role| {
            let baseline = if role == FontRole::Code {
                "monospace"
            } else {
                "sans-serif"
            };
            let family = theme
                .typography
                .0
                .get(&role)
                .and_then(|style| style.family.as_ref());
            let family = family.and_then(|name| FontFamily::new(name.trim()).ok());
            let family = family
                .filter(|family| {
                    family.is_generic()
                        || available
                            .iter()
                            .any(|name| name.eq_ignore_ascii_case(family.name()))
                })
                .unwrap_or_else(|| FontFamily::new(baseline).expect("static generic"));
            (role, family)
        })
        .collect()
}

pub fn role_stylesheet(theme: &Theme) -> String {
    let mut css = theme.typography.css_custom_properties();
    for (role, selector) in [
        (FontRole::Interface, INTERFACE),
        (FontRole::Reading, READING),
        (FontRole::Heading, HEADING),
        (FontRole::Label, LABEL),
        (FontRole::Code, CODE),
    ] {
        let fallback = if role == FontRole::Code {
            "monospace"
        } else {
            "sans-serif"
        };
        css.push_str(&format!(
            "{selector} {{ font-family: var(--tabard-font-{}, {fallback}); }}\n",
            role.key()
        ));
        css.push_str(&theme.typography.role_css(role, selector));
    }
    // Positioned instrument markers retain their fixed sizes and hit geometry.
    // Their family follows the label role; authored size/leading is supported
    // on ordinary labels above, not on these positioned overlays.
    css.push_str(&format!(
        "{FRET_LABELS} {{ font-family: var(--tabard-font-label, sans-serif); }}\n"
    ));
    css
}

pub fn request_stylesheet(
    theme: &Theme,
    requests: &FontPreferences,
    available: &[String],
) -> String {
    requests.css_custom_properties(available, &defaults(theme, available))
}

pub fn with_requests(
    base: &str,
    theme: &Theme,
    requests: &FontPreferences,
    available: &[String],
) -> String {
    let requests = request_stylesheet(theme, requests, available);
    if requests.is_empty() {
        base.to_owned()
    } else {
        format!("{base}\n{requests}")
    }
}

pub fn diagnostics(theme: &Theme, requests: &FontPreferences, available: &[String]) -> Vec<String> {
    let mut notices = theme.typography.missing_families(available);
    for role in requests.resolve(available, &defaults(theme, available)) {
        let missing: Vec<_> = role
            .requested
            .iter()
            .filter(|family| {
                !family.is_generic()
                    && !available
                        .iter()
                        .any(|name| name.eq_ignore_ascii_case(family.name()))
            })
            .map(FontFamily::name)
            .collect();
        if !missing.is_empty() {
            notices.push(format!(
                "{}: {} unavailable here; using {}. Saved font requests are retained.",
                role.role.label(),
                missing.join(", "),
                role.family.name()
            ));
        }
    }
    notices
}
