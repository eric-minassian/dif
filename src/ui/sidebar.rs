use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, FocusSection, PaneFocus};
use crate::git::UnstagedKind;

use super::palette::{Palette, border_style, rgb, selected_style};

pub(crate) fn render_unstaged(frame: &mut Frame, app: &App, area: Rect, palette: &Palette) {
    let focused = app.pane_focus == PaneFocus::Sidebar && app.focus == FocusSection::Unstaged;
    let title = format!(" Unstaged ({}) ", app.unstaged.len());
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(border_style(focused, palette));

    let visible_rows = app.layout.unstaged_inner.height as usize;
    let start = app
        .unstaged_scroll
        .min(app.unstaged.len().saturating_sub(visible_rows));
    let end = (start + visible_rows).min(app.unstaged.len());

    let mut lines = Vec::new();
    if app.unstaged.is_empty() {
        lines.push(Line::styled(
            "(none)",
            Style::default().fg(rgb(palette.dim)),
        ));
    } else {
        for idx in start..end {
            let entry = &app.unstaged[idx];
            let prefix = if app.unstaged_selected == Some(idx) {
                ">"
            } else {
                " "
            };

            let mut label = entry.path.clone();
            if entry.kind == UnstagedKind::Untracked {
                label.push_str(" [new]");
            }

            let style = if app.unstaged_selected == Some(idx) {
                selected_style(focused, palette)
            } else if entry.kind == UnstagedKind::Untracked {
                Style::default().fg(rgb(palette.untracked))
            } else {
                Style::default().fg(rgb(palette.text))
            };

            lines.push(Line::styled(format!("{prefix} {label}"), style));
        }
    }

    let paragraph = Paragraph::new(Text::from(lines))
        .style(Style::default().bg(rgb(palette.pane_bg)))
        .block(block);
    frame.render_widget(paragraph, area);
}

pub(crate) fn render_staged(frame: &mut Frame, app: &App, area: Rect, palette: &Palette) {
    let focused = app.pane_focus == PaneFocus::Sidebar && app.focus == FocusSection::Staged;
    let title = format!(" Staged ({}) ", app.staged.len());
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(border_style(focused, palette));

    let visible_rows = app.layout.staged_inner.height as usize;
    let start = app
        .staged_scroll
        .min(app.staged.len().saturating_sub(visible_rows));
    let end = (start + visible_rows).min(app.staged.len());

    let mut lines = Vec::new();
    if app.staged.is_empty() {
        lines.push(Line::styled(
            "(none)",
            Style::default().fg(rgb(palette.dim)),
        ));
    } else {
        for idx in start..end {
            let path = &app.staged[idx];
            let prefix = if app.staged_selected == Some(idx) {
                ">"
            } else {
                " "
            };
            let style = if app.staged_selected == Some(idx) {
                selected_style(focused, palette)
            } else {
                Style::default().fg(rgb(palette.text))
            };

            lines.push(Line::styled(format!("{prefix} {path}"), style));
        }
    }

    let paragraph = Paragraph::new(Text::from(lines))
        .style(Style::default().bg(rgb(palette.pane_bg)))
        .block(block);
    frame.render_widget(paragraph, area);
}
