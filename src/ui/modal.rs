use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};
use tui_term::widget::{Cursor, PseudoTerminal};

use crate::app::{App, GitPanelMode};
use crate::keymap;
use crate::layout;

use super::palette::{Palette, rgb};

pub(crate) fn render_terminal_modal(frame: &mut Frame, app: &App, area: Rect, palette: &Palette) {
    let popup = layout::terminal_popup(area);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(rgb(palette.modal_border)))
        .style(
            Style::default()
                .bg(rgb(palette.modal_bg))
                .fg(rgb(palette.text)),
        );
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let terminal_screen = app.terminal_screen();
    let copy_cursor = app.terminal_cursor();

    if let Some(screen) = terminal_screen {
        let terminal_widget = if app.terminal_copy_mode {
            PseudoTerminal::new(screen).cursor(Cursor::default().visibility(false))
        } else {
            PseudoTerminal::new(screen)
        };

        frame.render_widget(terminal_widget, inner);
        frame.render_widget(
            TerminalPaletteDefaults {
                default_fg: rgb(palette.text),
                default_bg: rgb(palette.modal_bg),
            },
            inner,
        );

        if app.terminal_copy_mode {
            let overlay = CopyModeOverlay {
                selection_rows: app.terminal_selection_rows(),
                cursor: copy_cursor,
                selection_bg: rgb(palette.selected_bg_unfocused),
                cursor_row_bg: rgb(palette.selected_bg_focused),
                cursor_style: terminal_cursor_style(palette),
            };
            frame.render_widget(overlay, inner);
        }
    } else {
        let output = Paragraph::new(Line::styled(
            "(waiting for terminal output)",
            Style::default().fg(rgb(palette.dim)),
        ))
        .style(Style::default().bg(rgb(palette.modal_bg)));
        frame.render_widget(output, inner);
    }
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

    let cursor_style = terminal_cursor_style(palette);
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

struct CopyModeOverlay {
    selection_rows: Option<(usize, usize)>,
    cursor: (usize, usize),
    selection_bg: ratatui::style::Color,
    cursor_row_bg: ratatui::style::Color,
    cursor_style: Style,
}

struct TerminalPaletteDefaults {
    default_fg: Color,
    default_bg: Color,
}

impl Widget for TerminalPaletteDefaults {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for y in area.y..area.y.saturating_add(area.height) {
            for x in area.x..area.x.saturating_add(area.width) {
                let cell = &mut buf[(x, y)];
                if cell.fg == Color::Reset {
                    cell.set_fg(self.default_fg);
                }
                if cell.bg == Color::Reset {
                    cell.set_bg(self.default_bg);
                }
            }
        }
    }
}

impl Widget for CopyModeOverlay {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let width = area.width as usize;
        let max_row = area.height as usize;
        let mut fill_row_bg = |row: usize, color: Color| {
            let y = area.y + row as u16;
            for col in 0..width {
                let x = area.x + col as u16;
                buf[(x, y)].set_bg(color);
            }
        };

        if let Some((start, end)) = self.selection_rows {
            let start = start.min(max_row.saturating_sub(1));
            let end = end.min(max_row.saturating_sub(1));

            for row in start..=end {
                fill_row_bg(row, self.selection_bg);
            }
        }

        let cursor_row = self.cursor.0.min(max_row.saturating_sub(1));
        let cursor_col = self.cursor.1.min(width.saturating_sub(1));
        let y = area.y + cursor_row as u16;

        fill_row_bg(cursor_row, self.cursor_row_bg);

        let x = area.x + cursor_col as u16;

        let cell = &mut buf[(x, y)];
        if cell.symbol().is_empty() {
            cell.set_symbol(" ");
        }
        cell.set_style(self.cursor_style);
    }
}

fn terminal_cursor_style(palette: &Palette) -> Style {
    Style::default()
        .fg(rgb(palette.modal_bg))
        .bg(rgb(palette.border_focus))
        .add_modifier(Modifier::BOLD)
}
