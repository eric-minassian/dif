use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, PaneFocus};

use super::palette::{Palette, border_style, rgb, selected_style};

pub(crate) fn render_tree(frame: &mut Frame, app: &App, area: Rect, palette: &Palette) {
    let focused = app.pane_focus == PaneFocus::Sidebar;
    let title = format!(" Changes ({}) ", app.tree_files.len());
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(border_style(focused, palette));

    let visible_rows = app.layout.tree_inner.height as usize;
    let file_rows = visible_rows.saturating_sub(1);
    let start = app
        .tree_scroll
        .min(app.tree_files.len().saturating_sub(file_rows));
    let end = (start + file_rows).min(app.tree_files.len());

    let mut lines = vec![Line::from(vec![
        Span::styled("S", Style::default().fg(rgb(palette.marker_add))),
        Span::raw(" "),
        Span::styled("U", Style::default().fg(rgb(palette.marker_remove))),
        Span::styled("  Path", Style::default().fg(rgb(palette.dim))),
    ])];

    if app.tree_files.is_empty() {
        lines.push(Line::styled(
            "(clean) no staged or unstaged files",
            Style::default().fg(rgb(palette.dim)),
        ));
    } else {
        for idx in start..end {
            let entry = &app.tree_files[idx];
            let selected = app.tree_selected == Some(idx);
            let line_style = if selected {
                selected_style(focused, palette)
            } else {
                Style::default().fg(rgb(palette.text))
            };

            let mut spans = vec![
                Span::raw(if selected { "> " } else { "  " }),
                Span::styled(
                    if entry.staged { "M" } else { " " },
                    Style::default().fg(rgb(palette.marker_add)),
                ),
                Span::raw(" "),
                Span::styled(
                    if entry.unstaged { "M" } else { " " },
                    Style::default().fg(rgb(palette.marker_remove)),
                ),
                Span::raw(" "),
            ];

            spans.extend(path_spans(&entry.path, entry.untracked, palette));

            if entry.untracked {
                spans.push(Span::styled(
                    " [new]",
                    Style::default().fg(rgb(palette.untracked)),
                ));
            }

            lines.push(Line::from(spans).style(line_style));
        }
    }

    let paragraph = Paragraph::new(Text::from(lines))
        .style(Style::default().bg(rgb(palette.pane_bg)))
        .block(block);
    frame.render_widget(paragraph, area);
}

fn path_spans(path: &str, untracked: bool, palette: &Palette) -> Vec<Span<'static>> {
    let segments = path.split('/').collect::<Vec<_>>();
    if segments.is_empty() {
        return vec![Span::styled(
            path.to_owned(),
            Style::default().fg(rgb(palette.text)),
        )];
    }

    let mut spans = Vec::new();
    for segment in &segments[..segments.len().saturating_sub(1)] {
        spans.push(Span::styled(
            format!("{segment}/"),
            Style::default().fg(rgb(palette.dim)),
        ));
    }

    let file_name = segments.last().copied().unwrap_or(path);
    spans.push(Span::styled(
        file_name.to_owned(),
        if untracked {
            Style::default().fg(rgb(palette.untracked))
        } else {
            Style::default().fg(rgb(palette.text))
        },
    ));

    spans
}
