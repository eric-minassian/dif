mod diff;
mod modal;
mod palette;
mod sidebar;

use ratatui::Frame;
use ratatui::style::Style;
use ratatui::widgets::Paragraph;

use crate::app::{App, ResolvedDiffLayout, StatusKind};
use crate::highlight::Highlighter;
use crate::keymap;
use crate::layout;

use self::palette::{palette_for, rgb};

pub fn render(frame: &mut Frame, app: &App, highlighter: &Highlighter) {
    let palette = palette_for(app.settings.theme);
    let root = frame.area();

    let (main_area, footer_area) = layout::split_root(root);
    let (sidebar_area, diff_area) = layout::split_main_area(main_area, &app.settings);

    let (unstaged_area, staged_area) = if let Some(area) = sidebar_area {
        let (unstaged, staged) = layout::split_sidebar(area);
        (Some(unstaged), Some(staged))
    } else {
        (None, None)
    };

    let (diff_header_area, diff_body_area) = layout::split_diff(diff_area);
    let resolved_layout = app.resolved_diff_layout(diff_body_area.width);

    if let Some(area) = unstaged_area {
        sidebar::render_unstaged(frame, app, area, &palette);
    }
    if let Some(area) = staged_area {
        sidebar::render_staged(frame, app, area, &palette);
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
    } else if app.settings_open {
        modal::render_settings_modal(frame, app, root, &palette);
    }
}

fn render_footer(
    frame: &mut Frame,
    app: &App,
    area: ratatui::layout::Rect,
    palette: &palette::Palette,
) {
    let hint = if app.has_pending_undo_confirmation() {
        keymap::footer_hint_pending_undo().to_owned()
    } else if app.terminal_open && app.terminal_search_open {
        keymap::footer_hint_terminal_search().to_owned()
    } else if app.terminal_open && app.terminal_copy_mode {
        keymap::footer_hint_terminal_copy().to_owned()
    } else if app.terminal_open {
        keymap::footer_hint_terminal()
    } else if app.settings_open {
        keymap::footer_hint_settings().to_owned()
    } else {
        keymap::footer_hint_main()
    };

    let status_color = match app.status_kind() {
        StatusKind::Info => palette.footer,
        StatusKind::Warn => palette.status_warn,
        StatusKind::Error => palette.status_error,
    };

    let text = format!("{} | {hint}", app.status_text());
    let footer = Paragraph::new(text).style(Style::default().fg(rgb(status_color)));
    frame.render_widget(footer, area);
}
