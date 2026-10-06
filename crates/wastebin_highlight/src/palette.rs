//! UI chrome colors derived from a syntax highlighting theme.
//!
//! Derives the colors of the UI surrounding the editor and paste views (header, panels, gutters,
//! accents, Markdown admonitions) from the theme's background, foreground and scope colors.

use std::fmt::Write as _;
use std::str::FromStr;

use syntect::highlighting::{Color, Highlighter, Theme};
use syntect::parsing::ScopeStack;

/// Color scheme a theme variant is rendered in.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Scheme {
    /// Light variant.
    Light,
    /// Dark variant.
    Dark,
}

impl Scheme {
    /// Value for the `color-scheme` property.
    fn name(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    /// Raised surface, lighter than `bg` in both schemes.
    fn raised_surface(self, bg: Color, fg: Color) -> Color {
        match self {
            Self::Light => mix(bg, WHITE, 0.5),
            Self::Dark => mix(bg, fg, 0.03),
        }
    }

    /// Inset surface for code blocks, quotes, inputs and gutters.
    fn inset_surface(self, bg: Color, fg: Color) -> Color {
        match self {
            Self::Light => mix(bg, fg, 0.03),
            Self::Dark => mix(bg, fg, 0.05),
        }
    }

    /// Alpha of accent tints over panel backgrounds.
    fn tint(self) -> f32 {
        match self {
            Self::Light => 0.10,
            Self::Dark => 0.14,
        }
    }

    /// Alpha of accent tints over gutter backgrounds.
    fn tint_gutter(self) -> f32 {
        match self {
            Self::Light => 0.15,
            Self::Dark => 0.20,
        }
    }

    /// Alpha of admonition backgrounds over the page background.
    fn alert_tint(self) -> f32 {
        match self {
            Self::Light => 0.07,
            Self::Dark => 0.10,
        }
    }
}

/// Minimum sRGB channel spread for a theme color to count as colorful.
const MIN_CHROMA: u8 = 24;
/// Minimum hue separation between the accent and the danger color.
const MIN_HUE_SEPARATION: f32 = 25.0;
/// Saturation cap for the danger color.
const MAX_DANGER_SATURATION: f32 = 0.70;
/// Hue tolerance when matching a theme scope color to a semantic color.
const HUE_TOLERANCE: f32 = 55.0;
/// Contrast ratio required for regular text.
const TEXT_CONTRAST: f32 = 4.5;
/// Contrast ratio required for primary text in light schemes.
const LIGHT_TEXT_CONTRAST: f32 = 12.0;
/// Contrast ratio required for secondary text and borders.
const MUTED_CONTRAST: f32 = 3.0;

const WHITE: Color = Color {
    r: 0xff,
    g: 0xff,
    b: 0xff,
    a: 0xff,
};
const BLACK: Color = Color {
    r: 0,
    g: 0,
    b: 0,
    a: 0xff,
};

/// Hues used when a theme lacks a suitable scope color.
const NOTE_HUE: f32 = 205.0;
const TIP_HUE: f32 = 150.0;
const IMPORTANT_HUE: f32 = 285.0;
const WARNING_HUE: f32 = 60.0;
const DANGER_HUE: f32 = 5.0;

/// Scope colors preferred for the accent, in order.
const ACCENT_SCOPES: [&str; 5] = [
    "accent",
    "entity.name.tag",
    "entity.name.function",
    "keyword.control",
    "keyword",
];

/// Semantic color with its tinted background and text color.
struct Alert {
    base: Color,
    bg: Color,
    fg: Color,
}

/// Chrome colors for a single theme variant.
pub(crate) struct Palette {
    scheme: Scheme,
    page_bg: Color,
    fg: Color,
    /// Foreground of the theme itself, kept for code so syntax colors stay untouched.
    code_fg: Color,
    panel_bg: Color,
    panel_bg2: Color,
    fg_dim: Color,
    fg_muted: Color,
    gutter_bg: Color,
    gutter_fg: Color,
    border: Color,
    border_soft: Color,
    accent: Color,
    accent_bg: Color,
    accent_bg_gutter: Color,
    accent_fg: Color,
    danger: Color,
    danger_bg: Color,
    selection: Color,
    note: Alert,
    tip: Alert,
    important: Alert,
    warning: Alert,
}

impl Palette {
    /// Derive the chrome palette of `theme` for `scheme`.
    pub(crate) fn new(theme: &Theme, scheme: Scheme) -> Self {
        let scopes = ScopeColors::new(theme);

        let page_bg = opaque(theme.settings.background.expect("theme background color"));
        let theme_fg = opaque(theme.settings.foreground.expect("theme foreground color"));
        let panel_bg = scheme.raised_surface(page_bg, theme_fg);
        let panel_bg2 = scheme.inset_surface(page_bg, theme_fg);
        let surfaces = [page_bg, panel_bg, panel_bg2];

        // Light themes provide a foreground tuned for code contrast, which reads too light for UI
        // text, so darken it a bit.
        let fg = match scheme {
            Scheme::Light => readable(
                theme_fg,
                worst_surface(theme_fg, surfaces),
                LIGHT_TEXT_CONTRAST,
            ),
            Scheme::Dark => theme_fg,
        };

        let gutter_base = scheme.inset_surface(page_bg, theme_fg);
        let gutter_text = readable(
            theme
                .settings
                .gutter_foreground
                .map_or_else(|| mix(fg, page_bg, 0.45), |color| blend(color, gutter_base)),
            gutter_base,
            MUTED_CONTRAST,
        );

        let dim = mix(fg, page_bg, 0.28);
        let fg_dim = readable(dim, worst_surface(dim, surfaces), TEXT_CONTRAST);
        let muted = mix(fg, page_bg, 0.45);
        let fg_muted = readable(muted, worst_surface(muted, surfaces), TEXT_CONTRAST);

        // Tone for synthesized semantic colors.
        let tone = ACCENT_SCOPES
            .iter()
            .filter_map(|name| scopes.get(name))
            .find(|color| scopes.usable(*color))
            .unwrap_or(fg);

        let semantic = |candidates: &[Option<Color>], hue: f32| {
            let base = candidates
                .iter()
                .flatten()
                .find(|color| scopes.usable(**color) && hue_within(**color, hue))
                .copied()
                .unwrap_or_else(|| with_hue(tone, hue));
            readable(base, worst_surface(base, surfaces), MUTED_CONTRAST)
        };

        // Error and diff scopes are fully saturated. Cap the saturation so the color passes as a
        // UI accent next to the theme's syntax colors.
        let danger_seed = {
            let base = semantic(
                &[
                    theme.settings.misspelling,
                    scopes.get("markup.deleted"),
                    scopes.get("invalid"),
                    scopes.get("keyword"),
                    scopes.get("variable"),
                ],
                DANGER_HUE,
            );
            let base = with_saturation(base, saturation(base).min(MAX_DANGER_SATURATION));
            readable(base, worst_surface(base, surfaces), TEXT_CONTRAST)
        };

        // Keep the accent distinct from the danger hue.
        let accent_base = ACCENT_SCOPES
            .iter()
            .filter_map(|name| scopes.get(name))
            .find(|color| {
                scopes.usable(*color) && far_from(*color, danger_seed, MIN_HUE_SEPARATION)
            })
            .unwrap_or(tone);
        let accent = readable(
            accent_base,
            worst_surface(accent_base, surfaces),
            TEXT_CONTRAST,
        );

        let accent_tint = blend(alpha(accent, scheme.tint()), panel_bg);
        let accent_tint_gutter = blend(alpha(accent, scheme.tint_gutter()), gutter_base);
        let accent_text = readable(accent, accent_tint, TEXT_CONTRAST);
        let danger_bg = blend(alpha(danger_seed, scheme.alert_tint()), page_bg);
        // The caution title sits on the danger tint.
        let danger = readable(danger_seed, danger_bg, TEXT_CONTRAST);

        let alert = |candidates: &[Option<Color>], hue: f32| {
            let base = semantic(candidates, hue);
            let bg = blend(alpha(base, scheme.alert_tint()), page_bg);
            Alert {
                base,
                bg,
                fg: readable(base, bg, TEXT_CONTRAST),
            }
        };

        Self {
            scheme,
            page_bg,
            fg,
            code_fg: theme_fg,
            panel_bg,
            panel_bg2,
            fg_dim,
            fg_muted,
            gutter_bg: gutter_base,
            gutter_fg: gutter_text,
            border: mix(page_bg, theme_fg, 0.16),
            border_soft: mix(page_bg, theme_fg, 0.08),
            accent,
            accent_bg: accent_tint,
            accent_bg_gutter: accent_tint_gutter,
            accent_fg: accent_text,
            danger,
            danger_bg,
            // The theme selection is an overlay, so it keeps its alpha.
            selection: theme
                .settings
                .selection
                .unwrap_or_else(|| alpha(accent, 0.25)),
            note: alert(
                &[
                    scopes.get("entity.name.tag"),
                    scopes.get("entity.name.function"),
                ],
                NOTE_HUE,
            ),
            tip: alert(
                &[
                    scopes.get("string"),
                    scopes.get("markup.inserted"),
                    scopes.get("string.regexp"),
                ],
                TIP_HUE,
            ),
            important: alert(
                &[
                    scopes.get("keyword.control"),
                    scopes.get("keyword"),
                    scopes.get("support.type"),
                ],
                IMPORTANT_HUE,
            ),
            warning: alert(
                &[
                    scopes.get("entity.name.type"),
                    scopes.get("constant.numeric"),
                    scopes.get("constant.language"),
                ],
                WARNING_HUE,
            ),
        }
    }

    /// Render the palette as CSS custom properties on `:root`.
    pub(crate) fn css(&self) -> String {
        let mut css = String::new();
        let _ = write!(css, ":root {{\n  color-scheme: {};\n", self.scheme.name());

        let mut token = |name: &str, value: String| {
            let _ = writeln!(css, "  {name}: {value};");
        };
        token("--page-bg", hex(self.page_bg));
        token("--fg", hex(self.fg));
        token("--code-fg", hex(self.code_fg));
        token("--panel-bg", hex(self.panel_bg));
        token("--panel-bg2", hex(self.panel_bg2));
        token("--fg-dim", hex(self.fg_dim));
        token("--fg-muted", hex(self.fg_muted));
        token("--gutter-bg", hex(self.gutter_bg));
        token("--gutter-fg", hex(self.gutter_fg));
        token("--border", hex(self.border));
        token("--border-soft", hex(self.border_soft));
        token("--accent", hex(self.accent));
        token("--accent-bg", hex(self.accent_bg));
        token("--accent-bg-gutter", hex(self.accent_bg_gutter));
        token("--accent-fg", hex(self.accent_fg));
        token("--danger", hex(self.danger));
        token("--danger-bg", hex(self.danger_bg));
        token("--selection-bg", rgb_with_alpha(self.selection));
        for (name, alert) in [
            ("note", &self.note),
            ("tip", &self.tip),
            ("important", &self.important),
            ("warning", &self.warning),
        ] {
            token(&format!("--{name}"), hex(alert.base));
            token(&format!("--{name}-bg"), hex(alert.bg));
            token(&format!("--{name}-fg"), hex(alert.fg));
        }

        css.push_str("}\n");
        css
    }
}

/// Foreground colors of named scopes.
struct ScopeColors<'a> {
    highlighter: Highlighter<'a>,
    fg: Color,
    bg: Color,
}

impl<'a> ScopeColors<'a> {
    /// Create the lookup for `theme`.
    fn new(theme: &'a Theme) -> Self {
        Self {
            highlighter: Highlighter::new(theme),
            fg: opaque(theme.settings.foreground.expect("theme foreground color")),
            bg: opaque(theme.settings.background.expect("theme background color")),
        }
    }

    /// Foreground color of the scope `name`.
    fn get(&self, name: &str) -> Option<Color> {
        let stack = ScopeStack::from_str(name).ok()?;
        Some(
            self.highlighter
                .style_for_stack(stack.as_slice())
                .foreground,
        )
    }

    /// Whether `color` is saturated and differs from the theme fore- and background.
    fn usable(&self, color: Color) -> bool {
        chroma(color) >= MIN_CHROMA && !similar(color, self.fg) && !similar(color, self.bg)
    }
}

/// Return `color` with an opaque alpha channel.
fn opaque(color: Color) -> Color {
    Color { a: 0xff, ..color }
}

/// Return `color` with the given alpha value.
fn alpha(color: Color, value: f32) -> Color {
    Color {
        a: to_u8(value * 255.0),
        ..color
    }
}

/// Linearly interpolate between `from` and `to` in sRGB space, `t` in `0.0..=1.0`.
fn mix(from: Color, to: Color, t: f32) -> Color {
    Color {
        r: lerp(from.r, to.r, t),
        g: lerp(from.g, to.g, t),
        b: lerp(from.b, to.b, t),
        a: 0xff,
    }
}

/// Composite `fg` (using its alpha) over `bg`.
fn blend(fg: Color, bg: Color) -> Color {
    mix(bg, fg, f32::from(fg.a) / 255.0)
}

/// Render `color` as `#rrggbb`.
fn hex(color: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}

/// Render `color` as `rgb(r g b / a%)`.
fn rgb_with_alpha(color: Color) -> String {
    let percent = f32::from(color.a) / 255.0 * 100.0;
    format!("rgb({} {} {} / {percent:.1}%)", color.r, color.g, color.b)
}

/// WCAG relative luminance of `color`.
fn luminance(color: Color) -> f32 {
    let channel = |value: u8| {
        let value = f32::from(value) / 255.0;
        if value <= 0.039_28 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };

    0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
}

/// WCAG contrast ratio between `a` and `b`.
fn contrast(a: Color, b: Color) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// sRGB channel spread of `color`.
fn chroma(color: Color) -> u8 {
    color.r.max(color.g).max(color.b) - color.r.min(color.g).min(color.b)
}

/// Whether `a` and `b` are within a few sRGB steps of each other.
fn similar(a: Color, b: Color) -> bool {
    a.r.abs_diff(b.r) <= 12 && a.g.abs_diff(b.g) <= 12 && a.b.abs_diff(b.b) <= 12
}

/// Hue of `color` in degrees, or `None` for near-gray colors.
fn hue_of(color: Color) -> Option<f32> {
    (chroma(color) >= MIN_CHROMA).then(|| hsl(color).0)
}

/// Whether the hue of `color` is within [`HUE_TOLERANCE`] of `hue`.
fn hue_within(color: Color, hue: f32) -> bool {
    hue_of(color).is_some_and(|actual| hue_distance(actual, hue) <= HUE_TOLERANCE)
}

/// Shortest distance between two hues in degrees.
fn hue_distance(a: f32, b: f32) -> f32 {
    let distance = (a - b).abs() % 360.0;
    distance.min(360.0 - distance)
}

/// Whether `a` and `b` are far enough apart in hue to be distinguishable.
fn far_from(a: Color, b: Color, min: f32) -> bool {
    match (hue_of(a), hue_of(b)) {
        (Some(a), Some(b)) => hue_distance(a, b) >= min,
        _ => true,
    }
}

/// The surface from `surfaces` with the least contrast to `color`.
fn worst_surface(color: Color, surfaces: [Color; 3]) -> Color {
    surfaces
        .into_iter()
        .min_by(|a, b| contrast(color, *a).total_cmp(&contrast(color, *b)))
        .expect("surfaces are not empty")
}

/// Adjust `color` until it reaches `min` contrast on `bg`, keeping its hue and saturation.
fn readable(color: Color, bg: Color, min: f32) -> Color {
    let (hue, saturation, lightness) = hsl(color);
    let mut best = color;
    let mut best_ratio = contrast(color, bg);
    let mut best_distance = 0.0_f32;
    let mut best_meets = best_ratio >= min;

    for step in 0..=29_u8 {
        let candidate_lightness = 0.08 + 0.03 * f32::from(step);
        let candidate = from_hsl(hue, saturation, candidate_lightness);
        let ratio = contrast(candidate, bg);
        let distance = (candidate_lightness - lightness).abs();
        let meets = ratio >= min;
        let better = if meets == best_meets {
            if meets {
                distance < best_distance
            } else {
                ratio > best_ratio
            }
        } else {
            meets
        };

        if better {
            best = candidate;
            best_ratio = ratio;
            best_distance = distance;
            best_meets = meets;
        }
    }

    if best_meets {
        return best;
    }

    // No lightness reaches `min`. Use the closest extreme.
    if contrast(WHITE, bg) >= contrast(BLACK, bg) {
        WHITE
    } else {
        BLACK
    }
}

/// HSL representation of `color` as `(hue in degrees, saturation, lightness)`.
fn hsl(color: Color) -> (f32, f32, f32) {
    let r = f32::from(color.r) / 255.0;
    let g = f32::from(color.g) / 255.0;
    let b = f32::from(color.b) / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let lightness = (max + min) / 2.0;
    let delta = max - min;

    if delta == 0.0 {
        return (0.0, 0.0, lightness);
    }

    let saturation = (delta / (1.0 - (2.0 * lightness - 1.0).abs())).clamp(0.0, 1.0);
    let hue = if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };

    (hue.rem_euclid(360.0), saturation, lightness)
}

/// sRGB color for the HSL components `hue` (degrees), `saturation` and `lightness`.
fn from_hsl(hue: f32, saturation: f32, lightness: f32) -> Color {
    let a = saturation * lightness.min(1.0 - lightness);
    let channel = |n: f32| {
        let k = (n + hue / 30.0) % 12.0;
        lightness - a * (k - 3.0).min(9.0 - k).clamp(-1.0, 1.0)
    };

    Color {
        r: to_u8(channel(0.0) * 255.0),
        g: to_u8(channel(8.0) * 255.0),
        b: to_u8(channel(4.0) * 255.0),
        a: 0xff,
    }
}

/// Return `color` with its HSL hue set to `hue`.
fn with_hue(color: Color, hue: f32) -> Color {
    let (_, saturation, lightness) = hsl(color);
    from_hsl(hue, saturation, lightness)
}

/// Return `color` with its HSL saturation set to `saturation`.
fn with_saturation(color: Color, saturation: f32) -> Color {
    let (hue, _, lightness) = hsl(color);
    from_hsl(hue, saturation, lightness)
}

/// HSL saturation of `color`.
fn saturation(color: Color) -> f32 {
    hsl(color).1
}

/// Interpolate between the channel values `from` and `to`.
fn lerp(from: u8, to: u8, t: f32) -> u8 {
    to_u8(f32::from(from) + (f32::from(to) - f32::from(from)) * t)
}

/// Round a float channel value to a `u8`, clamping out-of-range values.
#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_u8(value: f32) -> u8 {
    u8::try_from(value.clamp(0.0, 255.0).round() as u32).expect("clamped to u8 range")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Theme;
    use std::collections::HashMap;

    const THEMES: [Theme; 9] = [
        Theme::Ayu,
        Theme::Base16Ocean,
        Theme::Catppuccin,
        Theme::Coldark,
        Theme::Gruvbox,
        Theme::Monokai,
        Theme::Onehalf,
        Theme::RosePine,
        Theme::Solarized,
    ];

    const TOKENS: [&str; 18] = [
        "--page-bg",
        "--fg",
        "--code-fg",
        "--panel-bg",
        "--panel-bg2",
        "--fg-dim",
        "--fg-muted",
        "--gutter-bg",
        "--gutter-fg",
        "--border",
        "--border-soft",
        "--accent",
        "--accent-bg",
        "--accent-bg-gutter",
        "--accent-fg",
        "--danger",
        "--danger-bg",
        "--selection-bg",
    ];

    fn palettes() -> Vec<(Scheme, Palette)> {
        let mut palettes = Vec::new();
        for theme in THEMES {
            for scheme in [Scheme::Light, Scheme::Dark] {
                let highlighting = match scheme {
                    Scheme::Light => theme.light_theme(),
                    Scheme::Dark => theme.dark_theme(),
                };
                palettes.push((scheme, Palette::new(&highlighting, scheme)));
            }
        }
        palettes
    }

    #[test]
    fn color_math_mixes_and_composites() {
        assert_eq!(hex(mix(WHITE, BLACK, 0.0)), "#ffffff");
        assert_eq!(hex(mix(WHITE, BLACK, 1.0)), "#000000");
        assert_eq!(hex(mix(WHITE, BLACK, 0.5)), "#808080");
        // Half-opaque black over white is mid gray.
        assert_eq!(
            hex(blend(
                Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 128
                },
                WHITE
            )),
            "#7f7f7f"
        );
        assert!((contrast(WHITE, BLACK) - 21.0).abs() < 0.01);
        assert_eq!(hue_of(WHITE), None);
        assert!(
            (hue_of(Color {
                r: 0xff,
                g: 0,
                b: 0,
                a: 0xff
            })
            .unwrap()
                - 0.0)
                .abs()
                < 0.5
        );
    }

    #[test]
    fn hsl_round_trips() {
        let color = Color {
            r: 0x33,
            g: 0x99,
            b: 0xcc,
            a: 0xff,
        };
        let (hue, saturation, lightness) = hsl(color);
        assert_eq!(from_hsl(hue, saturation, lightness), color);
    }

    #[test]
    fn readable_reaches_required_contrast() {
        let bg = Color {
            r: 0xf0,
            g: 0xf0,
            b: 0xf0,
            a: 0xff,
        };
        let pale = Color {
            r: 0xd0,
            g: 0xd8,
            b: 0xe0,
            a: 0xff,
        };
        assert!(contrast(pale, bg) < TEXT_CONTRAST);
        let adjusted = readable(pale, bg, TEXT_CONTRAST);
        assert!(contrast(adjusted, bg) >= TEXT_CONTRAST);
        // A color that already meets the requirement passes through unchanged.
        let dark = Color {
            r: 0x20,
            g: 0x20,
            b: 0x20,
            a: 0xff,
        };
        assert_eq!(readable(dark, bg, TEXT_CONTRAST), dark);
    }

    #[test]
    fn every_theme_emits_every_token() {
        for palette in palettes() {
            let css = palette.1.css();
            let mut names: Vec<&str> = TOKENS.to_vec();
            let alerts: Vec<String> = ["note", "tip", "important", "warning"]
                .into_iter()
                .flat_map(|name| {
                    [
                        format!("--{name}"),
                        format!("--{name}-bg"),
                        format!("--{name}-fg"),
                    ]
                })
                .collect();
            names.extend(alerts.iter().map(String::as_str));

            let values = parse(&css);
            for name in names {
                assert!(
                    values.contains_key(name),
                    "theme {:?} is missing {name}",
                    palette.0
                );
            }
            assert!(css.contains(&format!(
                "color-scheme: {};",
                match palette.0 {
                    Scheme::Light => "light",
                    Scheme::Dark => "dark",
                }
            )));
        }
    }

    #[test]
    fn chrome_text_meets_contrast_requirements() {
        for (scheme, palette) in palettes() {
            let values = parse(&palette.css());
            let color = |name: &str| parse_hex(&values[name]);
            let surfaces = [
                color("--page-bg"),
                color("--panel-bg"),
                color("--panel-bg2"),
            ];

            // Light schemes darken the theme foreground to a fixed floor. Dark schemes keep the
            // contrast the theme chose.
            if scheme == Scheme::Light {
                let text = color("--fg");
                assert!(
                    contrast(text, worst_surface(text, surfaces)) >= LIGHT_TEXT_CONTRAST,
                    "--fg is too light ({scheme:?})"
                );
            }

            for name in ["--fg-dim", "--fg-muted"] {
                let text = color(name);
                let worst = worst_surface(text, surfaces);
                assert!(
                    contrast(text, worst) >= TEXT_CONTRAST,
                    "{name} has too little contrast ({scheme:?})"
                );
            }

            let gutter = contrast(color("--gutter-fg"), color("--gutter-bg"));
            assert!(
                gutter >= MUTED_CONTRAST,
                "gutter text has too little contrast ({scheme:?})"
            );

            let accent = color("--accent");
            assert!(
                contrast(accent, worst_surface(accent, surfaces)) >= TEXT_CONTRAST,
                "accent has too little contrast ({scheme:?})"
            );

            assert!(
                contrast(color("--danger"), color("--danger-bg")) >= TEXT_CONTRAST,
                "danger has too little contrast on its tint ({scheme:?})"
            );

            for name in ["--note", "--tip", "--important", "--warning"] {
                let alert = color(name);
                assert!(
                    contrast(alert, worst_surface(alert, surfaces)) >= MUTED_CONTRAST,
                    "{name} has too little contrast ({scheme:?})"
                );
                assert!(
                    contrast(color(&format!("{name}-fg")), color(&format!("{name}-bg")))
                        >= TEXT_CONTRAST,
                    "{name} title has too little contrast ({scheme:?})"
                );
            }
        }
    }

    #[test]
    fn accent_and_danger_are_distinguishable() {
        for (scheme, palette) in palettes() {
            let values = parse(&palette.css());
            let accent = parse_hex(&values["--accent"]);
            let danger = parse_hex(&values["--danger"]);
            assert!(
                far_from(accent, danger, MIN_HUE_SEPARATION),
                "accent and danger look alike for {scheme:?}"
            );
        }
    }

    #[test]
    fn selection_keeps_theme_alpha() {
        let dark = Theme::Catppuccin.dark_theme();
        let palette = Palette::new(&dark, Scheme::Dark);
        let css = palette.css();
        assert!(css.contains("--selection-bg: rgb("), "{css}");
        assert!(css.contains("/ 2"), "theme selection alpha is lost: {css}");
    }

    #[test]
    fn code_text_keeps_the_theme_foreground() {
        for theme in THEMES {
            for scheme in [Scheme::Light, Scheme::Dark] {
                let highlighting = match scheme {
                    Scheme::Light => theme.light_theme(),
                    Scheme::Dark => theme.dark_theme(),
                };
                let expected = hex(opaque(
                    highlighting.settings.foreground.expect("theme foreground"),
                ));
                let values = parse(&Palette::new(&highlighting, scheme).css());
                assert_eq!(
                    values["--code-fg"],
                    expected,
                    "{} {scheme:?} must render code in the theme's own foreground",
                    theme.name()
                );
            }
        }
    }

    #[test]
    fn ui_text_is_darkened_without_touching_code_text() {
        let highlighting = Theme::Ayu.light_theme();
        let theme_fg = hex(opaque(
            highlighting.settings.foreground.expect("theme foreground"),
        ));
        let values = parse(&Palette::new(&highlighting, Scheme::Light).css());
        let contrast_on_page =
            |name: &str| contrast(parse_hex(&values[name]), parse_hex(&values["--page-bg"]));

        assert_eq!(values["--code-fg"], theme_fg);
        assert_ne!(
            values["--fg"], theme_fg,
            "pale light themes darken their UI text"
        );
        assert!(
            contrast_on_page("--fg") > contrast_on_page("--code-fg"),
            "the darkened UI text must not leak into code"
        );
    }

    /// Parse the custom properties of generated CSS.
    fn parse(css: &str) -> HashMap<String, String> {
        css.lines()
            .filter_map(|line| {
                let line = line.trim().trim_end_matches(';');
                let (name, value) = line.split_once(": ")?;
                name.starts_with("--")
                    .then(|| (name.to_owned(), value.to_owned()))
            })
            .collect()
    }

    /// Parse `#rrggbb`.
    fn parse_hex(value: &str) -> Color {
        let value = u32::from_str_radix(value.trim_start_matches('#'), 16).expect("valid hex");
        Color {
            r: u8::try_from((value >> 16) & 0xff).expect("in range"),
            g: u8::try_from((value >> 8) & 0xff).expect("in range"),
            b: u8::try_from(value & 0xff).expect("in range"),
            a: 0xff,
        }
    }
}
