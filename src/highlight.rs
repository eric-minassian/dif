use std::collections::BTreeMap;
use std::path::Path;

use ratatui::style::{Color, Style};
use ratatui::text::Span;
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::highlighting::{Style as SyntectStyle, Theme};
use syntect::parsing::{SyntaxReference, SyntaxSet};

use crate::settings::AppTheme;

pub struct Highlighter {
    syntax_set: SyntaxSet,
    ocean_theme: Theme,
    eighties_theme: Theme,
    solarized_theme: Theme,
    monokai_theme: Theme,
}

impl Highlighter {
    pub fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let themes = ThemeSet::load_defaults().themes;
        let fallback = themes
            .values()
            .next()
            .cloned()
            .expect("syntect default themes should not be empty");

        let ocean_theme = pick_theme(
            &themes,
            &["base16-ocean.dark", "base16-ocean.light"],
            &fallback,
        );
        let eighties_theme = pick_theme(
            &themes,
            &["base16-eighties.dark", "base16-eighties.light"],
            &fallback,
        );
        let solarized_theme = pick_theme(
            &themes,
            &["Solarized (dark)", "Solarized (light)"],
            &fallback,
        );
        let monokai_theme = pick_theme(
            &themes,
            &["Monokai Extended", "Monokai Extended Bright"],
            &fallback,
        );

        Self {
            syntax_set,
            ocean_theme,
            eighties_theme,
            solarized_theme,
            monokai_theme,
        }
    }

    pub fn highlight_line(
        &self,
        path: Option<&str>,
        line: &str,
        background_rgb: (u8, u8, u8),
        app_theme: AppTheme,
    ) -> Vec<Span<'static>> {
        if line.is_empty() {
            return vec![Span::raw(String::new())];
        }

        let syntax = self.syntax_for_path(path);
        let mut highlighter = HighlightLines::new(syntax, self.theme_for(app_theme));

        match highlighter.highlight_line(line, &self.syntax_set) {
            Ok(ranges) => ranges
                .into_iter()
                .map(|(style, segment)| {
                    let fg_rgb = color_from_syntect(style);
                    let adjusted = ensure_contrast(fg_rgb, background_rgb);
                    Span::styled(
                        segment.to_owned(),
                        Style::default().fg(Color::Rgb(adjusted.0, adjusted.1, adjusted.2)),
                    )
                })
                .collect(),
            Err(_) => vec![Span::styled(
                line.to_owned(),
                Style::default().fg(Color::Rgb(224, 228, 236)),
            )],
        }
    }

    fn theme_for(&self, theme: AppTheme) -> &Theme {
        match theme {
            AppTheme::Ocean => &self.ocean_theme,
            AppTheme::Eighties => &self.eighties_theme,
            AppTheme::Solarized => &self.solarized_theme,
            AppTheme::Monokai => &self.monokai_theme,
        }
    }

    fn syntax_for_path(&self, path: Option<&str>) -> &SyntaxReference {
        let plain = self.syntax_set.find_syntax_plain_text();

        let Some(path) = path else {
            return plain;
        };

        let file_path = Path::new(path);
        if let Some(ext) = file_path.extension().and_then(|value| value.to_str())
            && let Some(syntax) = self.syntax_set.find_syntax_by_extension(ext)
        {
            return syntax;
        }

        plain
    }
}

fn pick_theme(themes: &BTreeMap<String, Theme>, preferred: &[&str], fallback: &Theme) -> Theme {
    preferred
        .iter()
        .find_map(|name| themes.get(*name).cloned())
        .unwrap_or_else(|| fallback.clone())
}

fn color_from_syntect(style: SyntectStyle) -> (u8, u8, u8) {
    (style.foreground.r, style.foreground.g, style.foreground.b)
}

fn ensure_contrast(foreground: (u8, u8, u8), background: (u8, u8, u8)) -> (u8, u8, u8) {
    const MIN_RATIO: f32 = 4.5;

    if contrast_ratio(foreground, background) >= MIN_RATIO {
        return foreground;
    }

    let toward_white =
        contrast_ratio((255, 255, 255), background) >= contrast_ratio((0, 0, 0), background);
    let target = if toward_white {
        (255, 255, 255)
    } else {
        (0, 0, 0)
    };

    for step in 1..=16 {
        let t = step as f32 / 16.0;
        let candidate = lerp_rgb(foreground, target, t);
        if contrast_ratio(candidate, background) >= MIN_RATIO {
            return candidate;
        }
    }

    target
}

fn lerp_rgb(from: (u8, u8, u8), to: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    (
        lerp_channel(from.0, to.0, t),
        lerp_channel(from.1, to.1, t),
        lerp_channel(from.2, to.2, t),
    )
}

fn lerp_channel(from: u8, to: u8, t: f32) -> u8 {
    let from = from as f32;
    let to = to as f32;
    (from + (to - from) * t).round().clamp(0.0, 255.0) as u8
}

fn contrast_ratio(a: (u8, u8, u8), b: (u8, u8, u8)) -> f32 {
    let l1 = relative_luminance(a);
    let l2 = relative_luminance(b);

    let (lighter, darker) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

fn relative_luminance(rgb: (u8, u8, u8)) -> f32 {
    let r = channel_luminance(rgb.0);
    let g = channel_luminance(rgb.1);
    let b = channel_luminance(rgb.2);

    0.2126 * r + 0.7152 * g + 0.0722 * b
}

fn channel_luminance(channel: u8) -> f32 {
    let normalized = channel as f32 / 255.0;
    if normalized <= 0.04045 {
        normalized / 12.92
    } else {
        ((normalized + 0.055) / 1.055).powf(2.4)
    }
}
