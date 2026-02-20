use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use vt100::Color as VtColor;

use crate::app::App;
use crate::keymap;
use crate::layout;
use crate::terminal::{TerminalCellStyle, TerminalStyledRow};

use super::palette::{Palette, rgb};

pub(crate) fn render_terminal_modal(frame: &mut Frame, app: &App, area: Rect, palette: &Palette) {
    let popup = layout::terminal_popup(area);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Terminal ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(rgb(palette.modal_border)))
        .style(
            Style::default()
                .bg(rgb(palette.modal_bg))
                .fg(rgb(palette.text)),
        );
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner);

    let (cursor_row, cursor_col) = app.terminal_cursor();
    let mode = if app.terminal_copy_mode {
        "COPY"
    } else {
        "SHELL"
    };
    let selection = app
        .terminal_selection_rows()
        .map(|(start, end)| format!("  |  sel rows: {}-{}", start + 1, end + 1))
        .unwrap_or_default();

    let header = Paragraph::new(format!(
        "repo: {}  |  mode: {}  |  scrollback: {}  |  cursor: {}:{}{}",
        app.repo_root_display(),
        mode,
        app.terminal_scrollback,
        cursor_row + 1,
        cursor_col + 1,
        selection
    ))
    .style(Style::default().fg(rgb(palette.dim)));
    frame.render_widget(header, sections[0]);

    let terminal_rows = app.terminal_rows();
    let output_lines: Vec<Line<'static>> = if terminal_rows.is_empty() {
        vec![Line::styled(
            "(waiting for terminal output)",
            Style::default().fg(rgb(palette.dim)),
        )]
    } else {
        terminal_rows
            .into_iter()
            .enumerate()
            .map(|(row_idx, row)| terminal_row_to_line(row, row_idx, app, palette))
            .collect()
    };

    let output =
        Paragraph::new(Text::from(output_lines)).style(Style::default().bg(rgb(palette.modal_bg)));
    frame.render_widget(output, sections[1]);

    let help = if app.terminal_search_open {
        Paragraph::new(format!("/{}_", app.terminal_search_query))
            .style(Style::default().fg(rgb(palette.text)))
    } else if app.terminal_copy_mode {
        Paragraph::new(keymap::terminal_modal_copy_hint())
            .style(Style::default().fg(rgb(palette.dim)))
    } else {
        Paragraph::new(keymap::terminal_modal_interactive_hint())
            .style(Style::default().fg(rgb(palette.dim)))
    };
    frame.render_widget(help, sections[2]);
}

pub(crate) fn render_settings_modal(frame: &mut Frame, app: &App, area: Rect, palette: &Palette) {
    let popup = layout::settings_popup(area);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Settings ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(rgb(palette.modal_border)))
        .style(
            Style::default()
                .bg(rgb(palette.modal_bg))
                .fg(rgb(palette.text)),
        );
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut lines = Vec::new();
    for (idx, (label, value)) in app.settings_rows().iter().enumerate() {
        let prefix = if idx == app.settings_selected {
            ">"
        } else {
            " "
        };
        let style = if idx == app.settings_selected {
            Style::default()
                .fg(rgb(palette.text))
                .bg(rgb(palette.modal_selected_bg))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(rgb(palette.text))
        };

        lines.push(Line::styled(format!("{prefix} {label:<22} {value}"), style));
    }

    lines.push(Line::from(""));
    lines.push(Line::styled(
        "Use Up/Down to choose a setting and Left/Right to change it.",
        Style::default().fg(rgb(palette.dim)),
    ));
    lines.push(Line::styled(
        "Settings save automatically.",
        Style::default().fg(rgb(palette.dim)),
    ));
    lines.push(Line::styled(
        format!("Config path: {}", app.config_path_display()),
        Style::default().fg(rgb(palette.dim)),
    ));

    let paragraph =
        Paragraph::new(Text::from(lines)).style(Style::default().bg(rgb(palette.modal_bg)));
    frame.render_widget(paragraph, inner);
}

fn terminal_row_to_line(
    row: TerminalStyledRow,
    row_idx: usize,
    app: &App,
    palette: &Palette,
) -> Line<'static> {
    if row.is_empty() {
        let mut empty = Line::from(Span::raw(String::new()));
        if app.terminal_copy_mode {
            let (cursor_row, _) = app.terminal_cursor();
            if row_idx == cursor_row {
                empty = empty.style(Style::default().bg(rgb(palette.selected_bg_focused)));
            }
        }
        return empty;
    }

    let spans = row
        .into_iter()
        .map(|span| {
            let style = style_from_terminal_cell(span.style, palette);
            Span::styled(span.text, style)
        })
        .collect::<Vec<_>>();

    let mut line = Line::from(spans);
    if app.terminal_copy_mode {
        if let Some((start, end)) = app.terminal_selection_rows()
            && row_idx >= start
            && row_idx <= end
        {
            line = line.style(Style::default().bg(rgb(palette.selected_bg_unfocused)));
        }

        let (cursor_row, _) = app.terminal_cursor();
        if row_idx == cursor_row {
            line = line.style(Style::default().bg(rgb(palette.selected_bg_focused)));
        }
    }

    line
}

fn style_from_terminal_cell(style: TerminalCellStyle, palette: &Palette) -> Style {
    let default_fg = rgb(palette.text);
    let default_bg = rgb(palette.modal_bg);
    let mut fg = vt_color_to_tui(style.fg, default_fg);
    let mut bg = vt_color_to_tui(style.bg, default_bg);

    if style.inverse {
        std::mem::swap(&mut fg, &mut bg);
    }

    let mut resolved = Style::default().fg(fg).bg(bg);
    if style.bold {
        resolved = resolved.add_modifier(Modifier::BOLD);
    }
    if style.italic {
        resolved = resolved.add_modifier(Modifier::ITALIC);
    }
    if style.underline {
        resolved = resolved.add_modifier(Modifier::UNDERLINED);
    }

    resolved
}

fn vt_color_to_tui(color: VtColor, default: ratatui::style::Color) -> ratatui::style::Color {
    match color {
        VtColor::Default => default,
        VtColor::Rgb(r, g, b) => ratatui::style::Color::Rgb(r, g, b),
        VtColor::Idx(idx) => indexed_color(idx),
    }
}

fn indexed_color(idx: u8) -> ratatui::style::Color {
    match idx {
        0 => ratatui::style::Color::Rgb(0, 0, 0),
        1 => ratatui::style::Color::Rgb(205, 49, 49),
        2 => ratatui::style::Color::Rgb(13, 188, 121),
        3 => ratatui::style::Color::Rgb(229, 229, 16),
        4 => ratatui::style::Color::Rgb(36, 114, 200),
        5 => ratatui::style::Color::Rgb(188, 63, 188),
        6 => ratatui::style::Color::Rgb(17, 168, 205),
        7 => ratatui::style::Color::Rgb(229, 229, 229),
        8 => ratatui::style::Color::Rgb(102, 102, 102),
        9 => ratatui::style::Color::Rgb(241, 76, 76),
        10 => ratatui::style::Color::Rgb(35, 209, 139),
        11 => ratatui::style::Color::Rgb(245, 245, 67),
        12 => ratatui::style::Color::Rgb(59, 142, 234),
        13 => ratatui::style::Color::Rgb(214, 112, 214),
        14 => ratatui::style::Color::Rgb(41, 184, 219),
        15 => ratatui::style::Color::Rgb(255, 255, 255),
        16..=231 => {
            let index = idx - 16;
            let r = index / 36;
            let g = (index % 36) / 6;
            let b = index % 6;

            let channel = |value: u8| {
                if value == 0 { 0 } else { value * 40 + 55 }
            };

            ratatui::style::Color::Rgb(channel(r), channel(g), channel(b))
        }
        232..=255 => {
            let gray = (idx - 232) * 10 + 8;
            ratatui::style::Color::Rgb(gray, gray, gray)
        }
    }
}
