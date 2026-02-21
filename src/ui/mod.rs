mod diff;
mod modal;
mod palette;
mod sidebar;

use ratatui::Frame;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, GitPanelMode, ResolvedDiffLayout, StatusKind};
use crate::highlight::Highlighter;
use crate::keymap;
use crate::layout;

use self::palette::{palette_for, rgb};

pub fn render(frame: &mut Frame, app: &App, highlighter: &Highlighter) {
    let palette = palette_for(app.settings.theme);
    let root = frame.area();

    let (main_area, footer_area) = layout::split_root(root);
    let (sidebar_area, diff_area) = layout::split_main_area(main_area, &app.settings);

    let (diff_header_area, diff_body_area) = layout::split_diff(diff_area);
    let resolved_layout = app.resolved_diff_layout(diff_body_area.width);

    if let Some(area) = sidebar_area {
        sidebar::render_tree(frame, app, area, &palette);
    }

    diff::render_diff_header(
        frame,
        app,
        diff_header_area,
        resolved_layout,
        diff_body_area.width,
        &palette,
    );

    match resolved_layout {
        ResolvedDiffLayout::Split => {
            let panes = layout::split_split_diff(diff_body_area);
            diff::render_split_diff_panes(frame, app, panes, highlighter, &palette);
        }
        ResolvedDiffLayout::Unified => {
            diff::render_unified_diff_pane(frame, app, diff_body_area, highlighter, &palette);
        }
    }

    render_footer(frame, app, footer_area, &palette);

    if app.terminal_open {
        modal::render_terminal_modal(frame, app, root, &palette);
    } else if app.git_panel_open {
        modal::render_git_modal(frame, app, root, &palette);
    } else if app.settings_open {
        modal::render_settings_modal(frame, app, root, &palette);
    }

    if app.help_open {
        modal::render_help_modal(frame, app, root, &palette);
    }
}

fn render_footer(
    frame: &mut Frame,
    app: &App,
    area: ratatui::layout::Rect,
    palette: &palette::Palette,
) {
    let status_color = match app.status_kind() {
        StatusKind::Info => palette.footer,
        StatusKind::Warn => palette.status_warn,
        StatusKind::Error => palette.status_error,
    };

    let hints = footer_hint_variants(app);
    let (status_text, hint_text) = fit_footer_parts(app.status_text(), &hints, area.width as usize);

    let mut spans = Vec::new();
    if !status_text.is_empty() {
        spans.push(Span::styled(
            status_text,
            Style::default().fg(rgb(status_color)),
        ));
    }
    if !hint_text.is_empty() {
        spans.push(Span::styled(" | ", Style::default().fg(rgb(palette.dim))));
        spans.push(Span::styled(
            hint_text,
            Style::default().fg(rgb(palette.footer)),
        ));
    }

    let footer = Paragraph::new(Line::from(spans));
    frame.render_widget(footer, area);
}

fn footer_hint_variants(app: &App) -> Vec<String> {
    if app.help_open {
        return vec![
            String::from("help: Esc/q/?/F1 close"),
            String::from("main: Tab pane  Enter toggle  q quit"),
        ];
    }

    if app.has_pending_undo_confirmation() {
        return vec![
            keymap::footer_hint_pending_undo().to_owned(),
            String::from("undo: Enter/y confirm  n/Esc cancel"),
        ];
    }

    if app.terminal_open && app.terminal_search_open {
        return vec![
            keymap::footer_hint_terminal_search().to_owned(),
            String::from("search: type, Enter find, Esc cancel"),
        ];
    }

    if app.terminal_open && app.terminal_copy_mode {
        return vec![
            keymap::footer_hint_terminal_copy().to_owned(),
            String::from("copy: move  v mark  y copy  / search  i shell"),
        ];
    }

    if app.terminal_open {
        return vec![
            keymap::footer_hint_terminal(),
            String::from("terminal: Alt+c copy mode  Esc/Ctrl+]/g/q/w close"),
        ];
    }

    if app.git_panel_open {
        if app.git_panel_mode == GitPanelMode::CommitMessage {
            return vec![
                String::from("commit: arrows move  Enter newline  Ctrl+S commit  Esc cancel"),
                String::from("template loads from git commit.template"),
            ];
        }

        return vec![
            keymap::footer_hint_git_panel(),
            String::from("git: Enter switch  n new  d delete  c commit  Esc close"),
        ];
    }

    if app.settings_open {
        return vec![
            keymap::footer_hint_settings().to_owned(),
            String::from("settings: jk select  hl change  Esc/q close"),
        ];
    }

    vec![
        String::from("Tab/h/l pane  j/k move  Enter toggle  PgUp/PgDn/Home/End navigate"),
        String::from("s stage  u unstage  x undo  g branches  : terminal  ? help  q quit"),
    ]
}

fn fit_footer_parts(status: &str, hint_variants: &[String], width: usize) -> (String, String) {
    const SEPARATOR: &str = " | ";
    const MIN_STATUS: usize = 8;
    const MIN_HINT: usize = 8;

    if width == 0 {
        return (String::new(), String::new());
    }

    let status_len = status.chars().count();
    if status_len >= width {
        return (truncate_with_ellipsis(status, width), String::new());
    }

    for hint in hint_variants {
        if status_len + SEPARATOR.len() + hint.chars().count() <= width {
            return (status.to_owned(), hint.to_owned());
        }
    }

    if hint_variants.is_empty() || width <= SEPARATOR.len() + MIN_STATUS + MIN_HINT {
        return (truncate_with_ellipsis(status, width), String::new());
    }

    let mut status_budget = ((width as f32) * 0.45) as usize;
    status_budget =
        status_budget.clamp(MIN_STATUS, width.saturating_sub(SEPARATOR.len() + MIN_HINT));

    let status_part = truncate_with_ellipsis(status, status_budget);
    let hint_budget = width.saturating_sub(status_part.chars().count() + SEPARATOR.len());

    if hint_budget < MIN_HINT {
        return (truncate_with_ellipsis(status, width), String::new());
    }

    let hint_part = hint_variants
        .iter()
        .find(|hint| hint.chars().count() <= hint_budget)
        .map(|hint| hint.to_owned())
        .unwrap_or_else(|| truncate_with_ellipsis(&hint_variants[0], hint_budget));

    (status_part, hint_part)
}

fn truncate_with_ellipsis(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    let text_len = text.chars().count();
    if text_len <= max_width {
        return text.to_owned();
    }

    if max_width <= 3 {
        return ".".repeat(max_width);
    }

    let mut out = text.chars().take(max_width - 3).collect::<String>();
    out.push_str("...");
    out
}

#[cfg(test)]
mod tests {
    use super::{fit_footer_parts, truncate_with_ellipsis};

    #[test]
    fn truncates_text_with_ellipsis_when_needed() {
        assert_eq!(truncate_with_ellipsis("abcdef", 6), "abcdef");
        assert_eq!(truncate_with_ellipsis("abcdef", 5), "ab...");
        assert_eq!(truncate_with_ellipsis("abcdef", 2), "..");
    }

    #[test]
    fn footer_parts_fit_target_width() {
        let hints = vec![
            String::from("this is a very long hint that does not fit"),
            String::from("short hint"),
        ];

        let (status, hint) = fit_footer_parts("Ready", &hints, 28);
        let width = if hint.is_empty() {
            status.chars().count()
        } else {
            status.chars().count() + 3 + hint.chars().count()
        };

        assert!(width <= 28);
        assert!(!hint.is_empty());
    }

    #[test]
    fn tiny_footer_width_falls_back_to_status_only() {
        let hints = vec![String::from("long hint")];
        let (status, hint) = fit_footer_parts("All good", &hints, 7);

        assert!(hint.is_empty());
        assert_eq!(status.chars().count(), 7);
    }
}
