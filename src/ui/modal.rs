use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use vt100::Color as VtColor;

use crate::app::{App, GitPanelMode};
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

    let copy_cursor = app.terminal_cursor();
    let shell_cursor = app.terminal_shell_cursor();
    let cursor_text = if app.terminal_copy_mode {
        format!("{}:{}", copy_cursor.0 + 1, copy_cursor.1 + 1)
    } else if let Some((row, col)) = shell_cursor {
        format!("{}:{}", row + 1, col + 1)
    } else {
        String::from("hidden")
    };

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
        "repo: {}  |  mode: {}  |  scrollback: {}  |  cursor: {}{}",
        app.repo_root_display(),
        mode,
        app.terminal_scrollback,
        cursor_text,
        selection
    ))
    .style(Style::default().fg(rgb(palette.dim)));
    frame.render_widget(header, sections[0]);

    let terminal_rows = app.terminal_rows();
    let render_cursor = if app.terminal_copy_mode {
        Some(copy_cursor)
    } else {
        shell_cursor
    };

    let output_lines: Vec<Line<'static>> = if terminal_rows.is_empty() {
        vec![Line::styled(
            "(waiting for terminal output)",
            Style::default().fg(rgb(palette.dim)),
        )]
    } else {
        terminal_rows
            .into_iter()
            .enumerate()
            .map(|(row_idx, row)| terminal_row_to_line(row, row_idx, app, palette, render_cursor))
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

pub(crate) fn render_git_modal(frame: &mut Frame, app: &App, area: Rect, palette: &Palette) {
    let popup = layout::git_popup(area);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Git ")
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
            Constraint::Min(6),
            Constraint::Length(3),
        ])
        .split(inner);

    let current_branch = app.current_branch_name().unwrap_or("<detached>");
    let selected_branch = app.selected_branch_name().unwrap_or("-");
    let header = Paragraph::new(format!(
        "current: {}  |  selected: {}  |  local branches: {}  |  staged: {}",
        current_branch,
        selected_branch,
        app.branches.len(),
        app.staged.len(),
    ))
    .style(Style::default().fg(rgb(palette.dim)));
    frame.render_widget(header, sections[0]);

    if app.git_panel_mode == GitPanelMode::CommitMessage {
        let editor = Paragraph::new(Text::from(commit_editor_lines(
            &app.git_commit_input,
            app.git_commit_cursor(),
            palette,
        )))
        .block(
            Block::default()
                .title(" Commit Message ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(rgb(palette.modal_border))),
        )
        .style(
            Style::default()
                .bg(rgb(palette.modal_bg))
                .fg(rgb(palette.text)),
        );
        frame.render_widget(editor, sections[1]);
    } else {
        let mut branch_lines = Vec::new();
        if app.branches.is_empty() {
            branch_lines.push(Line::styled(
                "(no local branches)",
                Style::default().fg(rgb(palette.dim)),
            ));
        } else {
            for (idx, branch) in app.branches.iter().enumerate() {
                let prefix = if app.branch_selected == Some(idx) {
                    ">"
                } else {
                    " "
                };
                let marker = if branch.current { "*" } else { " " };
                let style = if app.branch_selected == Some(idx) {
                    Style::default()
                        .fg(rgb(palette.text))
                        .bg(rgb(palette.modal_selected_bg))
                        .add_modifier(Modifier::BOLD)
                } else if branch.current {
                    Style::default().fg(rgb(palette.border_focus))
                } else {
                    Style::default().fg(rgb(palette.text))
                };

                branch_lines.push(Line::styled(
                    format!("{prefix} {marker} {}", branch.name),
                    style,
                ));
            }
        }

        let branches = Paragraph::new(Text::from(branch_lines))
            .block(
                Block::default()
                    .title(" Branches ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(rgb(palette.modal_border))),
            )
            .style(Style::default().bg(rgb(palette.modal_bg)));
        frame.render_widget(branches, sections[1]);
    }

    let footer_lines = match app.git_panel_mode {
        GitPanelMode::Browse => vec![
            Line::styled(
                format!(
                    "{} new branch  Enter/{} switch  {} delete  {} commit",
                    keymap::KEY_GIT_CREATE_BRANCH,
                    keymap::KEY_GIT_SWITCH_BRANCH,
                    keymap::KEY_GIT_DELETE_BRANCH,
                    keymap::KEY_GIT_COMMIT,
                ),
                Style::default().fg(rgb(palette.dim)),
            ),
            Line::styled(
                format!("Esc/q/{} closes this panel", keymap::KEY_OPEN_GIT_PANEL),
                Style::default().fg(rgb(palette.dim)),
            ),
        ],
        GitPanelMode::CreateBranch => vec![
            Line::styled(
                format!("new branch: {}_", app.git_branch_input),
                Style::default().fg(rgb(palette.text)),
            ),
            Line::styled(
                "Enter creates + switches, Esc cancels",
                Style::default().fg(rgb(palette.dim)),
            ),
        ],
        GitPanelMode::CommitMessage => vec![
            Line::styled(
                "Arrows move cursor, Enter newline, Ctrl+S commits, Esc cancels",
                Style::default().fg(rgb(palette.dim)),
            ),
            Line::styled(
                "Template loads automatically from git commit.template when configured",
                Style::default().fg(rgb(palette.dim)),
            ),
        ],
        GitPanelMode::ConfirmDeleteBranch => vec![
            Line::styled(
                format!(
                    "delete branch `{}`?",
                    app.pending_branch_delete_name().unwrap_or("<unknown>")
                ),
                Style::default().fg(rgb(palette.status_warn)),
            ),
            Line::styled(
                "Press Enter/y to delete, n/Esc to cancel",
                Style::default().fg(rgb(palette.dim)),
            ),
        ],
    };

    let footer = Paragraph::new(Text::from(footer_lines)).style(
        Style::default()
            .bg(rgb(palette.modal_bg))
            .fg(rgb(palette.text)),
    );
    frame.render_widget(footer, sections[2]);
}

pub(crate) fn render_help_modal(frame: &mut Frame, app: &App, area: Rect, palette: &Palette) {
    let popup = layout::help_popup(area);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(rgb(palette.modal_border)))
        .style(
            Style::default()
                .bg(rgb(palette.modal_bg))
                .fg(rgb(palette.text)),
        );
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let branch = app.current_branch_name().unwrap_or("<detached>");
    let lines = vec![
        Line::styled(
            format!(
                "repo: {}  |  branch: {}  |  focus: {}",
                app.repo_name_display(),
                branch,
                app.focus_ring_label()
            ),
            Style::default().fg(rgb(palette.dim)),
        ),
        Line::from(""),
        Line::styled(
            "MAIN",
            Style::default()
                .fg(rgb(palette.border_focus))
                .add_modifier(Modifier::BOLD),
        ),
        Line::styled(
            "Tab/Shift+Tab toggles between sidebar and diff",
            Style::default().fg(rgb(palette.text)),
        ),
        Line::styled(
            "j/k or Up/Down move selection or scroll diff",
            Style::default().fg(rgb(palette.text)),
        ),
        Line::styled(
            "h/l or Left/Right switch pane focus",
            Style::default().fg(rgb(palette.text)),
        ),
        Line::styled(
            "Enter/Space toggle stage state; s stage; u unstage; x undo",
            Style::default().fg(rgb(palette.text)),
        ),
        Line::styled(
            "Tree mode markers: left M staged (green), right M unstaged (red)",
            Style::default().fg(rgb(palette.text)),
        ),
        Line::styled(
            "Home/End jump to edge; PageUp/PageDown move by page",
            Style::default().fg(rgb(palette.text)),
        ),
        Line::styled(
            "g branches; c commit; : or ! terminal; o settings; r refresh",
            Style::default().fg(rgb(palette.text)),
        ),
        Line::from(""),
        Line::styled(
            "GIT PANEL",
            Style::default()
                .fg(rgb(palette.border_focus))
                .add_modifier(Modifier::BOLD),
        ),
        Line::styled(
            "Enter switch branch; n (or a) create; d delete; c commit prompt",
            Style::default().fg(rgb(palette.text)),
        ),
        Line::from(""),
        Line::styled(
            "TERMINAL",
            Style::default()
                .fg(rgb(palette.border_focus))
                .add_modifier(Modifier::BOLD),
        ),
        Line::styled(
            "Alt+c copy mode; Shift+Up/Down fast scroll; Esc/Ctrl+]/g/q/w close",
            Style::default().fg(rgb(palette.text)),
        ),
        Line::from(""),
        Line::styled(
            "Close: Esc, q, ?, or F1",
            Style::default().fg(rgb(palette.dim)),
        ),
    ];

    let paragraph = Paragraph::new(Text::from(lines)).style(
        Style::default()
            .bg(rgb(palette.modal_bg))
            .fg(rgb(palette.text)),
    );
    frame.render_widget(paragraph, inner);
}

fn commit_editor_lines(message: &str, cursor: usize, palette: &Palette) -> Vec<Line<'static>> {
    let cursor = cursor.min(message.len());
    let mut lines = Vec::new();
    let mut line_start = 0usize;

    for line in message.split('\n') {
        let line_end = line_start + line.len();
        let cursor_col = if cursor >= line_start && cursor <= line_end {
            Some(message[line_start..cursor].chars().count())
        } else {
            None
        };

        let base_style = if line.trim_start().starts_with('#') {
            Style::default().fg(rgb(palette.dim))
        } else {
            Style::default().fg(rgb(palette.text))
        };

        lines.push(line_with_editor_cursor(
            line, base_style, cursor_col, palette,
        ));
        line_start = line_end.saturating_add(1);
    }

    lines
}

fn line_with_editor_cursor(
    line: &str,
    base_style: Style,
    cursor_col: Option<usize>,
    palette: &Palette,
) -> Line<'static> {
    let Some(cursor_col) = cursor_col else {
        return Line::styled(line.to_owned(), base_style);
    };

    let cursor_style = editor_cursor_style(palette);
    let chars = line.chars().collect::<Vec<_>>();

    if cursor_col >= chars.len() {
        let mut spans = Vec::new();
        if !line.is_empty() {
            spans.push(Span::styled(line.to_owned(), base_style));
        }
        spans.push(Span::styled(" ", cursor_style));
        return Line::from(spans);
    }

    let prefix = chars[..cursor_col].iter().collect::<String>();
    let current = chars[cursor_col].to_string();
    let suffix = chars[cursor_col + 1..].iter().collect::<String>();

    let mut spans = Vec::new();
    if !prefix.is_empty() {
        spans.push(Span::styled(prefix, base_style));
    }
    spans.push(Span::styled(current, cursor_style));
    if !suffix.is_empty() {
        spans.push(Span::styled(suffix, base_style));
    }

    Line::from(spans)
}

fn editor_cursor_style(palette: &Palette) -> Style {
    Style::default()
        .fg(rgb(palette.modal_bg))
        .bg(rgb(palette.border_focus))
        .add_modifier(Modifier::BOLD)
}

fn terminal_row_to_line(
    row: TerminalStyledRow,
    row_idx: usize,
    app: &App,
    palette: &Palette,
    terminal_cursor: Option<(usize, usize)>,
) -> Line<'static> {
    let interactive_cursor_col = if app.terminal_copy_mode {
        None
    } else {
        terminal_cursor.and_then(|(cursor_row, cursor_col)| {
            if row_idx == cursor_row {
                Some(cursor_col)
            } else {
                None
            }
        })
    };

    if row.is_empty() {
        let mut empty = Line::from(Span::raw(String::new()));
        if app.terminal_copy_mode {
            let (cursor_row, _) = app.terminal_cursor();
            if row_idx == cursor_row {
                empty = empty.style(Style::default().bg(rgb(palette.selected_bg_focused)));
            }
        } else if interactive_cursor_col.is_some() {
            empty = Line::from(Span::styled(" ", terminal_cursor_style(palette)));
        }
        return empty;
    }

    let spans = if let Some(cursor_col) = interactive_cursor_col {
        build_row_spans_with_cursor(row, cursor_col, palette)
    } else {
        row.into_iter()
            .map(|span| {
                let style = style_from_terminal_cell(span.style, palette);
                Span::styled(span.text, style)
            })
            .collect::<Vec<_>>()
    };

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

fn build_row_spans_with_cursor(
    row: TerminalStyledRow,
    cursor_col: usize,
    palette: &Palette,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut cursor_drawn = false;
    let mut col = 0usize;

    for terminal_span in row {
        let style = style_from_terminal_cell(terminal_span.style, palette);
        let chars = terminal_span.text.chars().collect::<Vec<_>>();
        let len = chars.len();

        if !cursor_drawn && cursor_col >= col && cursor_col < col.saturating_add(len) {
            let local_col = cursor_col - col;

            if local_col > 0 {
                spans.push(Span::styled(
                    chars[..local_col].iter().collect::<String>(),
                    style,
                ));
            }

            spans.push(Span::styled(
                chars[local_col].to_string(),
                terminal_cursor_style(palette),
            ));

            if local_col + 1 < len {
                spans.push(Span::styled(
                    chars[local_col + 1..].iter().collect::<String>(),
                    style,
                ));
            }

            cursor_drawn = true;
        } else if !terminal_span.text.is_empty() {
            spans.push(Span::styled(terminal_span.text, style));
        }

        col = col.saturating_add(len);
    }

    if !cursor_drawn {
        spans.push(Span::styled(" ", terminal_cursor_style(palette)));
    }

    spans
}

fn terminal_cursor_style(palette: &Palette) -> Style {
    Style::default()
        .fg(rgb(palette.modal_bg))
        .bg(rgb(palette.border_focus))
        .add_modifier(Modifier::BOLD)
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
