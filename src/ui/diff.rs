use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, PaneFocus, ResolvedDiffLayout};
use crate::diff::{CellKind, DiffCell, DiffRow};
use crate::highlight::{Highlighter, LineHighlighter};

use super::palette::{Palette, rgb};

#[derive(Clone)]
struct UnifiedLine {
    old_no: Option<usize>,
    new_no: Option<usize>,
    text: String,
    kind: CellKind,
    marker: char,
}

pub(crate) fn render_diff_header(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    resolved_layout: ResolvedDiffLayout,
    diff_width: u16,
    palette: &Palette,
) {
    let text = match app.active_path() {
        Some(path) => format!(
            "{} ({}) [{}]",
            path,
            app.active_label(),
            app.diff_mode_hint(diff_width)
        ),
        None => format!(
            "No file selected [{}]",
            match app.settings.diff_view_mode {
                crate::settings::DiffViewMode::Auto => format!("Auto->{}", resolved_layout.label()),
                mode => mode.label().to_owned(),
            }
        ),
    };
    let header = Paragraph::new(text).style(Style::default().fg(rgb(palette.dim)));
    frame.render_widget(header, area);
}

pub(crate) fn render_split_diff_panes(
    frame: &mut Frame,
    app: &App,
    panes: (Rect, Rect),
    highlighter: &Highlighter,
    palette: &Palette,
) {
    let source_path = app.active_path();
    let old_width = line_number_width(&app.diff_rows, true);
    let new_width = line_number_width(&app.diff_rows, false);

    let mut old_lines = Vec::new();
    let mut new_lines = Vec::new();
    let mut old_highlighter = highlighter.begin(source_path, app.settings.theme);
    let mut new_highlighter = highlighter.begin(source_path, app.settings.theme);

    if app.diff_rows.is_empty() {
        old_lines.push(Line::styled(
            "No diff",
            Style::default().fg(rgb(palette.dim)),
        ));
        new_lines.push(Line::styled(
            "No diff",
            Style::default().fg(rgb(palette.dim)),
        ));
    } else {
        for row in &app.diff_rows {
            old_lines.push(build_split_line(
                row.old.as_ref(),
                old_width,
                &mut old_highlighter,
                palette,
            ));
            new_lines.push(build_split_line(
                row.new.as_ref(),
                new_width,
                &mut new_highlighter,
                palette,
            ));
        }
    }

    let scroll = to_u16(app.diff_scroll);
    let pane_style = Style::default()
        .fg(rgb(palette.text))
        .bg(rgb(palette.pane_bg));
    let diff_border = if app.pane_focus == PaneFocus::Diff {
        Style::default().fg(rgb(palette.border_focus))
    } else {
        Style::default().fg(rgb(palette.border))
    };

    let old = Paragraph::new(Text::from(old_lines))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Old ")
                .border_style(diff_border),
        )
        .style(pane_style)
        .scroll((scroll, 0));
    let new = Paragraph::new(Text::from(new_lines))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" New ")
                .border_style(diff_border),
        )
        .style(pane_style)
        .scroll((scroll, 0));

    frame.render_widget(old, panes.0);
    frame.render_widget(new, panes.1);
}

pub(crate) fn render_unified_diff_pane(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    highlighter: &Highlighter,
    palette: &Palette,
) {
    let source_path = app.active_path();
    let unified = collect_unified_lines(&app.diff_rows);

    let old_width = unified
        .iter()
        .filter_map(|line| line.old_no)
        .max()
        .unwrap_or(1)
        .to_string()
        .len()
        .max(1);
    let new_width = unified
        .iter()
        .filter_map(|line| line.new_no)
        .max()
        .unwrap_or(1)
        .to_string()
        .len()
        .max(1);

    let mut lines = Vec::new();
    let mut line_highlighter = highlighter.begin(source_path, app.settings.theme);
    if unified.is_empty() {
        lines.push(Line::styled(
            "No diff",
            Style::default().fg(rgb(palette.dim)),
        ));
    } else {
        for line in &unified {
            lines.push(build_unified_line(
                line,
                old_width,
                new_width,
                &mut line_highlighter,
                palette,
            ));
        }
    }

    let scroll = to_u16(app.diff_scroll);
    let pane_style = Style::default()
        .fg(rgb(palette.text))
        .bg(rgb(palette.pane_bg));
    let diff_border = if app.pane_focus == PaneFocus::Diff {
        Style::default().fg(rgb(palette.border_focus))
    } else {
        Style::default().fg(rgb(palette.border))
    };
    let paragraph = Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Diff ")
                .border_style(diff_border),
        )
        .style(pane_style)
        .scroll((scroll, 0));

    frame.render_widget(paragraph, area);
}

fn collect_unified_lines(rows: &[DiffRow]) -> Vec<UnifiedLine> {
    let mut lines = Vec::new();

    for row in rows {
        match (&row.old, &row.new) {
            (Some(old), Some(new))
                if old.kind == CellKind::Removed && new.kind == CellKind::Added =>
            {
                lines.push(UnifiedLine {
                    old_no: old.line_no,
                    new_no: None,
                    text: old.text.clone(),
                    kind: CellKind::Removed,
                    marker: '-',
                });
                lines.push(UnifiedLine {
                    old_no: None,
                    new_no: new.line_no,
                    text: new.text.clone(),
                    kind: CellKind::Added,
                    marker: '+',
                });
            }
            (Some(old), Some(new)) if old.kind == CellKind::Meta || new.kind == CellKind::Meta => {
                lines.push(UnifiedLine {
                    old_no: None,
                    new_no: None,
                    text: if !old.text.is_empty() {
                        old.text.clone()
                    } else {
                        new.text.clone()
                    },
                    kind: CellKind::Meta,
                    marker: ' ',
                });
            }
            (Some(old), Some(new)) => {
                lines.push(UnifiedLine {
                    old_no: old.line_no,
                    new_no: new.line_no,
                    text: new.text.clone(),
                    kind: new.kind,
                    marker: marker_for_kind(new.kind),
                });
            }
            (Some(old), None) => {
                lines.push(UnifiedLine {
                    old_no: old.line_no,
                    new_no: None,
                    text: old.text.clone(),
                    kind: old.kind,
                    marker: marker_for_kind(old.kind),
                });
            }
            (None, Some(new)) => {
                lines.push(UnifiedLine {
                    old_no: None,
                    new_no: new.line_no,
                    text: new.text.clone(),
                    kind: new.kind,
                    marker: marker_for_kind(new.kind),
                });
            }
            (None, None) => {}
        }
    }

    lines
}

fn marker_for_kind(kind: CellKind) -> char {
    match kind {
        CellKind::Added => '+',
        CellKind::Removed => '-',
        CellKind::Context | CellKind::Meta => ' ',
    }
}

fn build_split_line(
    cell: Option<&DiffCell>,
    line_number_width: usize,
    line_highlighter: &mut LineHighlighter<'_>,
    palette: &Palette,
) -> Line<'static> {
    let bg_rgb = background_for_kind(cell.map(|item| item.kind), palette);

    let Some(cell) = cell else {
        return Line::from(Span::styled(
            " ".repeat(line_number_width + 1),
            Style::default().bg(rgb(bg_rgb)),
        ));
    };

    let number = match cell.line_no {
        Some(value) => format!("{value:>width$} ", width = line_number_width),
        None => " ".repeat(line_number_width + 1),
    };

    let mut spans = vec![Span::styled(
        number,
        Style::default().fg(rgb(palette.line_no)),
    )];

    if cell.kind == CellKind::Meta {
        spans.push(Span::styled(
            cell.text.clone(),
            Style::default().fg(rgb(palette.dim)),
        ));
    } else {
        spans.extend(line_highlighter.highlight(&cell.text, bg_rgb));
    }

    Line::from(spans).style(Style::default().bg(rgb(bg_rgb)))
}

fn build_unified_line(
    line: &UnifiedLine,
    old_width: usize,
    new_width: usize,
    line_highlighter: &mut LineHighlighter<'_>,
    palette: &Palette,
) -> Line<'static> {
    let bg_rgb = background_for_kind(Some(line.kind), palette);
    let old = match line.old_no {
        Some(value) => format!("{value:>width$}", width = old_width),
        None => " ".repeat(old_width),
    };
    let new = match line.new_no {
        Some(value) => format!("{value:>width$}", width = new_width),
        None => " ".repeat(new_width),
    };

    let marker_color = match line.kind {
        CellKind::Added => palette.marker_add,
        CellKind::Removed => palette.marker_remove,
        CellKind::Meta | CellKind::Context => palette.marker_context,
    };

    let mut spans = vec![
        Span::styled(old, Style::default().fg(rgb(palette.line_no))),
        Span::raw(" "),
        Span::styled(new, Style::default().fg(rgb(palette.line_no))),
        Span::raw(" "),
        Span::styled(
            line.marker.to_string(),
            Style::default().fg(rgb(marker_color)),
        ),
        Span::raw(" "),
    ];

    if line.kind == CellKind::Meta {
        spans.push(Span::styled(
            line.text.clone(),
            Style::default().fg(rgb(palette.dim)),
        ));
    } else {
        spans.extend(line_highlighter.highlight(&line.text, bg_rgb));
    }

    Line::from(spans).style(Style::default().bg(rgb(bg_rgb)))
}

fn background_for_kind(kind: Option<CellKind>, palette: &Palette) -> (u8, u8, u8) {
    match kind {
        Some(CellKind::Added) => palette.added_bg,
        Some(CellKind::Removed) => palette.removed_bg,
        Some(CellKind::Meta) => palette.meta_bg,
        Some(CellKind::Context) | None => palette.pane_bg,
    }
}

fn line_number_width(rows: &[DiffRow], old_side: bool) -> usize {
    let max_line = rows
        .iter()
        .filter_map(|row| {
            if old_side {
                row.old.as_ref().and_then(|cell| cell.line_no)
            } else {
                row.new.as_ref().and_then(|cell| cell.line_no)
            }
        })
        .max()
        .unwrap_or(1);

    max_line.to_string().len().max(1)
}

fn to_u16(value: usize) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}

#[cfg(test)]
mod tests {
    use crate::diff::{CellKind, DiffCell, DiffRow};

    use super::collect_unified_lines;

    #[test]
    fn snapshot_collects_replacement_as_remove_then_add() {
        let rows = vec![DiffRow {
            old: Some(DiffCell {
                line_no: Some(2),
                text: String::from("old line"),
                kind: CellKind::Removed,
            }),
            new: Some(DiffCell {
                line_no: Some(2),
                text: String::from("new line"),
                kind: CellKind::Added,
            }),
        }];

        let unified = collect_unified_lines(&rows);
        let snapshot = unified
            .iter()
            .map(|line| {
                format!(
                    "{:?}|{:?}|{}|{}",
                    line.old_no, line.new_no, line.marker, line.text
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            snapshot,
            vec![
                String::from("Some(2)|None|-|old line"),
                String::from("None|Some(2)|+|new line")
            ]
        );
    }

    #[test]
    fn snapshot_collects_context_line_with_both_numbers() {
        let rows = vec![DiffRow {
            old: Some(DiffCell {
                line_no: Some(3),
                text: String::from("same"),
                kind: CellKind::Context,
            }),
            new: Some(DiffCell {
                line_no: Some(3),
                text: String::from("same"),
                kind: CellKind::Context,
            }),
        }];

        let unified = collect_unified_lines(&rows);
        let snapshot = unified
            .iter()
            .map(|line| {
                format!(
                    "{:?}|{:?}|{}|{}",
                    line.old_no, line.new_no, line.marker, line.text
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(snapshot, vec![String::from("Some(3)|Some(3)| |same")]);
    }
}
