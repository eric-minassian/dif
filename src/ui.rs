use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::{App, FocusSection, PaneFocus, ResolvedDiffLayout, UiLayout};
use crate::diff::{CellKind, DiffCell, DiffRow};
use crate::git::UnstagedKind;
use crate::highlight::Highlighter;
use crate::settings::{self, AppTheme, SidebarPosition};
use crate::terminal::{TerminalCellStyle, TerminalStyledRow};
use vt100::Color as VtColor;

const MIN_DIFF_WIDTH_WITH_SIDEBAR: u16 = 48;

#[derive(Clone, Copy)]
struct Palette {
    pane_bg: (u8, u8, u8),
    meta_bg: (u8, u8, u8),
    added_bg: (u8, u8, u8),
    removed_bg: (u8, u8, u8),
    text: (u8, u8, u8),
    dim: (u8, u8, u8),
    line_no: (u8, u8, u8),
    marker_add: (u8, u8, u8),
    marker_remove: (u8, u8, u8),
    marker_context: (u8, u8, u8),
    border: (u8, u8, u8),
    border_focus: (u8, u8, u8),
    selected_bg_focused: (u8, u8, u8),
    selected_bg_unfocused: (u8, u8, u8),
    untracked: (u8, u8, u8),
    footer: (u8, u8, u8),
    modal_bg: (u8, u8, u8),
    modal_border: (u8, u8, u8),
    modal_selected_bg: (u8, u8, u8),
}

#[derive(Clone)]
struct UnifiedLine {
    old_no: Option<usize>,
    new_no: Option<usize>,
    text: String,
    kind: CellKind,
    marker: char,
}

pub fn render(frame: &mut Frame, app: &mut App, highlighter: &Highlighter) {
    let palette = palette_for(app.settings.theme);
    let root = frame.area();

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(root);

    let main_area = rows[0];
    let footer_area = rows[1];

    let (sidebar_area, diff_area) = split_main_area(main_area, app);

    let (unstaged_area, staged_area, unstaged_inner, staged_inner) =
        if let Some(sidebar_area) = sidebar_area {
            let sidebar_sections = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(sidebar_area);

            (
                Some(sidebar_sections[0]),
                Some(sidebar_sections[1]),
                bordered_inner(sidebar_sections[0]),
                bordered_inner(sidebar_sections[1]),
            )
        } else {
            (None, None, Rect::new(0, 0, 0, 0), Rect::new(0, 0, 0, 0))
        };

    let diff_sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(diff_area);

    let diff_header_area = diff_sections[0];
    let diff_body_area = diff_sections[1];
    let resolved_layout = app.resolved_diff_layout(diff_body_area.width);

    let (diff_viewport_height, diff_content_height) = match resolved_layout {
        ResolvedDiffLayout::Split => {
            let panes = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(diff_body_area);
            let old_inner = bordered_inner(panes[0]);
            let new_inner = bordered_inner(panes[1]);
            (
                old_inner.height.min(new_inner.height) as usize,
                app.diff_rows.len(),
            )
        }
        ResolvedDiffLayout::Unified => (
            bordered_inner(diff_body_area).height as usize,
            unified_line_count(&app.diff_rows),
        ),
    };

    app.set_layout(UiLayout {
        unstaged_inner,
        staged_inner,
        diff_area: diff_body_area,
        diff_viewport_height,
    });
    app.set_diff_content_height(diff_content_height);
    app.sync_scrolls();

    if let Some(area) = unstaged_area {
        render_unstaged(frame, app, area, &palette);
    }
    if let Some(area) = staged_area {
        render_staged(frame, app, area, &palette);
    }

    render_diff_header(
        frame,
        app,
        diff_header_area,
        resolved_layout,
        diff_body_area.width,
        &palette,
    );

    match resolved_layout {
        ResolvedDiffLayout::Split => {
            let panes = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(diff_body_area);
            render_split_diff_panes(frame, app, &panes, highlighter, &palette);
        }
        ResolvedDiffLayout::Unified => {
            render_unified_diff_pane(frame, app, diff_body_area, highlighter, &palette);
        }
    }

    render_footer(frame, app, footer_area, &palette);

    if app.terminal_open {
        render_terminal_modal(frame, app, root, &palette);
    } else if app.settings_open {
        render_settings_modal(frame, app, root, &palette);
    }
}

fn split_main_area(area: Rect, app: &App) -> (Option<Rect>, Rect) {
    if !app.settings.sidebar_visible {
        return (None, area);
    }

    if area.width <= MIN_DIFF_WIDTH_WITH_SIDEBAR {
        return (None, area);
    }

    let max_sidebar = area.width.saturating_sub(MIN_DIFF_WIDTH_WITH_SIDEBAR);
    let requested = app
        .settings
        .sidebar_width
        .clamp(settings::SIDEBAR_WIDTH_MIN, settings::SIDEBAR_WIDTH_MAX);
    let sidebar_width = requested.min(max_sidebar);
    if sidebar_width < settings::SIDEBAR_WIDTH_MIN {
        return (None, area);
    }

    let chunks = match app.settings.sidebar_position {
        SidebarPosition::Left => Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(sidebar_width), Constraint::Min(1)])
            .split(area),
        SidebarPosition::Right => Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(1), Constraint::Length(sidebar_width)])
            .split(area),
    };

    match app.settings.sidebar_position {
        SidebarPosition::Left => (Some(chunks[0]), chunks[1]),
        SidebarPosition::Right => (Some(chunks[1]), chunks[0]),
    }
}

fn render_unstaged(frame: &mut Frame, app: &App, area: Rect, palette: &Palette) {
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

fn render_staged(frame: &mut Frame, app: &App, area: Rect, palette: &Palette) {
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

fn render_diff_header(
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

fn render_split_diff_panes(
    frame: &mut Frame,
    app: &App,
    panes: &[Rect],
    highlighter: &Highlighter,
    palette: &Palette,
) {
    let source_path = app.active_path();
    let old_width = line_number_width(&app.diff_rows, true);
    let new_width = line_number_width(&app.diff_rows, false);

    let mut old_lines = Vec::new();
    let mut new_lines = Vec::new();

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
                source_path,
                highlighter,
                app.settings.theme,
                palette,
            ));
            new_lines.push(build_split_line(
                row.new.as_ref(),
                new_width,
                source_path,
                highlighter,
                app.settings.theme,
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

    frame.render_widget(old, panes[0]);
    frame.render_widget(new, panes[1]);
}

fn render_unified_diff_pane(
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
                source_path,
                highlighter,
                app.settings.theme,
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

fn render_footer(frame: &mut Frame, app: &App, area: Rect, palette: &Palette) {
    let hint = if app.terminal_open && app.terminal_search_open {
        "terminal search: type query, Enter find, Esc cancel"
    } else if app.terminal_open && app.terminal_copy_mode {
        "copy mode: move(hjkl/arrows)  v mark  y copy  / search  n next  i interactive"
    } else if app.terminal_open {
        "terminal: all keys -> shell, Alt+c copy mode, Ctrl+]/Ctrl+g/Ctrl+q close"
    } else if app.settings_open {
        "settings: j/k select, h/l change, Esc close"
    } else {
        "Tab list  Left/Right pane  Up/Down move-or-scroll  s stage  u unstage  : terminal  o settings  q quit"
    };

    let text = format!("{} | {hint}", app.status_line);
    let footer = Paragraph::new(text).style(Style::default().fg(rgb(palette.footer)));
    frame.render_widget(footer, area);
}

fn render_terminal_modal(frame: &mut Frame, app: &mut App, area: Rect, palette: &Palette) {
    let popup = centered_rect(92, 82, area);
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

    if sections[1].width > 0
        && sections[1].height > 0
        && let Err(error) =
            app.set_terminal_viewport(sections[1].height as usize, sections[1].width as usize)
    {
        app.set_error(error);
    }

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
        Paragraph::new("copy: hjkl/arrows move  v mark  y yank  / search  n next  i interactive")
            .style(Style::default().fg(rgb(palette.dim)))
    } else {
        Paragraph::new(
            "interactive shell (zsh aliases supported). Alt+c enters copy mode. Ctrl+], Ctrl+g, or Ctrl+q closes.",
        )
        .style(Style::default().fg(rgb(palette.dim)))
    };
    frame.render_widget(help, sections[2]);
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

fn vt_color_to_tui(color: VtColor, default: Color) -> Color {
    match color {
        VtColor::Default => default,
        VtColor::Rgb(r, g, b) => Color::Rgb(r, g, b),
        VtColor::Idx(idx) => indexed_color(idx),
    }
}

fn indexed_color(idx: u8) -> Color {
    match idx {
        0 => Color::Rgb(0, 0, 0),
        1 => Color::Rgb(205, 49, 49),
        2 => Color::Rgb(13, 188, 121),
        3 => Color::Rgb(229, 229, 16),
        4 => Color::Rgb(36, 114, 200),
        5 => Color::Rgb(188, 63, 188),
        6 => Color::Rgb(17, 168, 205),
        7 => Color::Rgb(229, 229, 229),
        8 => Color::Rgb(102, 102, 102),
        9 => Color::Rgb(241, 76, 76),
        10 => Color::Rgb(35, 209, 139),
        11 => Color::Rgb(245, 245, 67),
        12 => Color::Rgb(59, 142, 234),
        13 => Color::Rgb(214, 112, 214),
        14 => Color::Rgb(41, 184, 219),
        15 => Color::Rgb(255, 255, 255),
        16..=231 => {
            let index = idx - 16;
            let r = index / 36;
            let g = (index % 36) / 6;
            let b = index % 6;

            let channel = |value: u8| {
                if value == 0 { 0 } else { value * 40 + 55 }
            };

            Color::Rgb(channel(r), channel(g), channel(b))
        }
        232..=255 => {
            let gray = (idx - 232) * 10 + 8;
            Color::Rgb(gray, gray, gray)
        }
    }
}

fn render_settings_modal(frame: &mut Frame, app: &App, area: Rect, palette: &Palette) {
    let popup = centered_rect(70, 60, area);
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

fn unified_line_count(rows: &[DiffRow]) -> usize {
    rows.iter()
        .map(|row| match (&row.old, &row.new) {
            (Some(old), Some(new))
                if old.kind == CellKind::Removed && new.kind == CellKind::Added =>
            {
                2
            }
            (None, None) => 0,
            _ => 1,
        })
        .sum()
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
    source_path: Option<&str>,
    highlighter: &Highlighter,
    app_theme: AppTheme,
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
        spans.extend(highlighter.highlight_line(source_path, &cell.text, bg_rgb, app_theme));
    }

    Line::from(spans).style(Style::default().bg(rgb(bg_rgb)))
}

fn build_unified_line(
    line: &UnifiedLine,
    old_width: usize,
    new_width: usize,
    source_path: Option<&str>,
    highlighter: &Highlighter,
    app_theme: AppTheme,
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
        spans.extend(highlighter.highlight_line(source_path, &line.text, bg_rgb, app_theme));
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

fn border_style(focused: bool, palette: &Palette) -> Style {
    if focused {
        Style::default().fg(rgb(palette.border_focus))
    } else {
        Style::default().fg(rgb(palette.border))
    }
}

fn selected_style(focused: bool, palette: &Palette) -> Style {
    if focused {
        Style::default()
            .fg(rgb(palette.text))
            .bg(rgb(palette.selected_bg_focused))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(rgb(palette.text))
            .bg(rgb(palette.selected_bg_unfocused))
    }
}

fn centered_rect(horizontal_percent: u16, vertical_percent: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - vertical_percent) / 2),
            Constraint::Percentage(vertical_percent),
            Constraint::Percentage((100 - vertical_percent) / 2),
        ])
        .split(area);

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - horizontal_percent) / 2),
            Constraint::Percentage(horizontal_percent),
            Constraint::Percentage((100 - horizontal_percent) / 2),
        ])
        .split(vertical[1]);

    horizontal[1]
}

fn palette_for(theme: AppTheme) -> Palette {
    match theme {
        AppTheme::Ocean => Palette {
            pane_bg: (17, 20, 27),
            meta_bg: (35, 39, 47),
            added_bg: (21, 50, 36),
            removed_bg: (68, 30, 36),
            text: (224, 228, 236),
            dim: (136, 144, 160),
            line_no: (124, 132, 150),
            marker_add: (136, 216, 152),
            marker_remove: (232, 130, 140),
            marker_context: (136, 144, 160),
            border: (82, 90, 108),
            border_focus: (230, 185, 90),
            selected_bg_focused: (60, 70, 92),
            selected_bg_unfocused: (46, 55, 72),
            untracked: (122, 204, 194),
            footer: (124, 132, 146),
            modal_bg: (24, 28, 37),
            modal_border: (150, 158, 176),
            modal_selected_bg: (63, 75, 101),
        },
        AppTheme::Eighties => Palette {
            pane_bg: (22, 20, 27),
            meta_bg: (40, 36, 48),
            added_bg: (25, 50, 41),
            removed_bg: (72, 33, 46),
            text: (232, 226, 240),
            dim: (152, 143, 167),
            line_no: (140, 132, 157),
            marker_add: (152, 218, 168),
            marker_remove: (236, 146, 162),
            marker_context: (152, 143, 167),
            border: (96, 87, 114),
            border_focus: (236, 194, 108),
            selected_bg_focused: (74, 65, 95),
            selected_bg_unfocused: (60, 52, 78),
            untracked: (132, 212, 198),
            footer: (140, 132, 154),
            modal_bg: (30, 27, 38),
            modal_border: (167, 156, 188),
            modal_selected_bg: (77, 68, 100),
        },
        AppTheme::Solarized => Palette {
            pane_bg: (0, 43, 54),
            meta_bg: (7, 54, 66),
            added_bg: (20, 71, 51),
            removed_bg: (95, 46, 50),
            text: (238, 232, 213),
            dim: (147, 161, 161),
            line_no: (131, 148, 150),
            marker_add: (133, 199, 167),
            marker_remove: (220, 144, 138),
            marker_context: (147, 161, 161),
            border: (88, 110, 117),
            border_focus: (203, 165, 80),
            selected_bg_focused: (26, 73, 85),
            selected_bg_unfocused: (18, 62, 73),
            untracked: (126, 204, 183),
            footer: (131, 148, 150),
            modal_bg: (3, 48, 60),
            modal_border: (148, 163, 164),
            modal_selected_bg: (28, 77, 91),
        },
        AppTheme::Monokai => Palette {
            pane_bg: (30, 31, 28),
            meta_bg: (43, 44, 40),
            added_bg: (36, 67, 43),
            removed_bg: (79, 40, 45),
            text: (248, 248, 242),
            dim: (152, 152, 141),
            line_no: (132, 134, 126),
            marker_add: (166, 226, 146),
            marker_remove: (249, 122, 132),
            marker_context: (152, 152, 141),
            border: (93, 94, 87),
            border_focus: (253, 200, 97),
            selected_bg_focused: (63, 64, 57),
            selected_bg_unfocused: (52, 53, 47),
            untracked: (120, 220, 190),
            footer: (140, 141, 133),
            modal_bg: (36, 37, 33),
            modal_border: (164, 165, 157),
            modal_selected_bg: (70, 71, 63),
        },
    }
}

fn bordered_inner(area: Rect) -> Rect {
    Block::default().borders(Borders::ALL).inner(area)
}

fn rgb(value: (u8, u8, u8)) -> Color {
    Color::Rgb(value.0, value.1, value.2)
}

fn to_u16(value: usize) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}
