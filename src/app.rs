use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use crossterm::event::KeyEvent;
use ratatui::layout::Rect;

use crate::diff::{DiffRow, parse_unified_diff, unified_line_count};
use crate::git::{self, DiffMode, FileEntry, UnstagedKind};
use crate::layout;
use crate::settings::{
    self, AUTO_SPLIT_MIN_WIDTH_MAX, AUTO_SPLIT_MIN_WIDTH_MIN, AppSettings, DiffViewMode,
    SIDEBAR_WIDTH_MAX, SIDEBAR_WIDTH_MIN,
};
use crate::terminal::{TerminalSession, TerminalStyledRow};

mod status;
mod util;

pub use status::{StatusKind, StatusMessage};
use util::{contains, ensure_visible, find_query_in_line, order_positions, shift_and_clamp_u16};

const SETTINGS_FIELD_COUNT: usize = 7;
const AUTO_REFRESH_INTERVAL: Duration = Duration::from_millis(750);
const SETTINGS_WRITE_DEBOUNCE: Duration = Duration::from_millis(400);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusSection {
    Unstaged,
    Staged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneFocus {
    Sidebar,
    Diff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedDiffLayout {
    Split,
    Unified,
}

impl ResolvedDiffLayout {
    pub fn label(self) -> &'static str {
        match self {
            Self::Split => "Split",
            Self::Unified => "Unified",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UiLayout {
    pub unstaged_inner: Rect,
    pub staged_inner: Rect,
    pub diff_area: Rect,
    pub diff_viewport_height: usize,
}

impl Default for UiLayout {
    fn default() -> Self {
        Self {
            unstaged_inner: Rect::new(0, 0, 0, 0),
            staged_inner: Rect::new(0, 0, 0, 0),
            diff_area: Rect::new(0, 0, 0, 0),
            diff_viewport_height: 0,
        }
    }
}

#[derive(Debug, Clone)]
struct PendingUndoConfirmation {
    path: String,
    was_untracked: bool,
}

pub struct App {
    repo_root: PathBuf,
    last_auto_refresh: Instant,
    settings_dirty: bool,
    last_settings_change: Option<Instant>,
    pub settings: AppSettings,
    pub settings_open: bool,
    pub settings_selected: usize,
    pub terminal_open: bool,
    pub terminal_scrollback: usize,
    pub terminal_copy_mode: bool,
    pub terminal_search_open: bool,
    pub terminal_search_query: String,
    terminal_last_search: String,
    terminal_cursor_row: usize,
    terminal_cursor_col: usize,
    terminal_selection_anchor: Option<(usize, usize)>,
    terminal_view_rows: usize,
    terminal_view_cols: usize,
    terminal_session: Option<TerminalSession>,
    pub unstaged: Vec<FileEntry>,
    pub staged: Vec<String>,
    pub focus: FocusSection,
    pub pane_focus: PaneFocus,
    pub unstaged_selected: Option<usize>,
    pub staged_selected: Option<usize>,
    pub unstaged_scroll: usize,
    pub staged_scroll: usize,
    pub diff_rows: Vec<DiffRow>,
    pub diff_scroll: usize,
    pub diff_content_height: usize,
    pub status: StatusMessage,
    pub layout: UiLayout,
    pending_undo_confirmation: Option<PendingUndoConfirmation>,
}

impl App {
    pub fn new(repo_root: PathBuf) -> Result<Self> {
        let (settings, status) = match settings::load() {
            Ok(settings) => (settings, StatusMessage::info("Ready")),
            Err(error) => (
                AppSettings::default(),
                StatusMessage::warn(format!("Settings parse error, using defaults ({error})")),
            ),
        };

        let mut app = Self {
            repo_root,
            last_auto_refresh: Instant::now(),
            settings_dirty: false,
            last_settings_change: None,
            settings,
            settings_open: false,
            settings_selected: 0,
            terminal_open: false,
            terminal_scrollback: 0,
            terminal_copy_mode: false,
            terminal_search_open: false,
            terminal_search_query: String::new(),
            terminal_last_search: String::new(),
            terminal_cursor_row: 0,
            terminal_cursor_col: 0,
            terminal_selection_anchor: None,
            terminal_view_rows: 0,
            terminal_view_cols: 0,
            terminal_session: None,
            unstaged: Vec::new(),
            staged: Vec::new(),
            focus: FocusSection::Unstaged,
            pane_focus: PaneFocus::Sidebar,
            unstaged_selected: None,
            staged_selected: None,
            unstaged_scroll: 0,
            staged_scroll: 0,
            diff_rows: Vec::new(),
            diff_scroll: 0,
            diff_content_height: 0,
            status,
            layout: UiLayout::default(),
            pending_undo_confirmation: None,
        };

        app.refresh()?;
        app.last_auto_refresh = Instant::now();
        Ok(app)
    }

    pub fn refresh_with_message(&mut self) -> Result<()> {
        self.refresh()?;
        self.set_status_info("Refreshed");
        Ok(())
    }

    pub fn refresh(&mut self) -> Result<()> {
        let previous_unstaged = self.selected_unstaged_path().map(ToOwned::to_owned);
        let previous_staged = self.selected_staged_path().map(ToOwned::to_owned);
        let previous_active = self.active_selection();
        let previous_diff_scroll = self.diff_scroll;

        let status = git::status(&self.repo_root)?;
        self.unstaged = status.unstaged;
        self.staged = status.staged;

        self.restore_unstaged_selection(previous_unstaged);
        self.restore_staged_selection(previous_staged);
        self.normalize_focus();

        let preserve_diff_scroll = self.active_selection() == previous_active;
        self.load_current_diff_with_scroll(preserve_diff_scroll, previous_diff_scroll)?;
        self.last_auto_refresh = Instant::now();

        Ok(())
    }

    pub fn tick(&mut self) -> bool {
        let mut changed = false;
        let mut exit_message = None;
        let mut terminal_error = None;

        if let Some(session) = self.terminal_session.as_mut() {
            if session.pump_output() {
                changed = true;
            }

            match session.poll_exit_message() {
                Ok(message) => exit_message = message,
                Err(error) => terminal_error = Some(error.to_string()),
            }

            if self.terminal_scrollback > 0 {
                session.set_scrollback(self.terminal_scrollback);
                self.terminal_scrollback = session.scrollback();
                changed = true;
            }
        }

        if let Some(error) = terminal_error {
            self.set_status_error(error);
            changed = true;
        }

        if let Some(message) = exit_message {
            self.set_status_info(format!("Terminal session ended: {message}"));
            changed = true;
        }

        if !self.terminal_open
            && !self.settings_open
            && let Err(error) = self.auto_refresh_if_due()
        {
            self.set_status_error(error);
            changed = true;
        }

        match self.flush_settings_if_due() {
            Ok(flushed) => {
                changed |= flushed;
            }
            Err(error) => {
                self.set_status_error(error);
                changed = true;
            }
        }

        changed
    }

    pub fn switch_focus(&mut self) -> Result<()> {
        self.focus = match self.focus {
            FocusSection::Unstaged => FocusSection::Staged,
            FocusSection::Staged => FocusSection::Unstaged,
        };
        self.normalize_focus();
        self.load_current_diff()
    }

    pub fn move_selection(&mut self, delta: isize) -> Result<()> {
        let list_len = self.focus_len();
        if list_len == 0 {
            return Ok(());
        }

        let current = self.focus_selected().unwrap_or(0);
        let next = if delta < 0 {
            current.saturating_sub(delta.unsigned_abs())
        } else {
            (current + delta as usize).min(list_len - 1)
        };

        self.set_focus_selected(Some(next));
        self.load_current_diff()
    }

    pub fn stage_selected(&mut self) -> Result<()> {
        if self.focus != FocusSection::Unstaged {
            self.set_status_warn("Focus Unstaged to stage files");
            return Ok(());
        }

        let Some(entry) = self.selected_unstaged().cloned() else {
            self.set_status_warn("No unstaged file selected");
            return Ok(());
        };

        git::stage_file(&self.repo_root, &entry.path)?;
        self.refresh()?;
        self.set_status_info(format!("Staged {}", entry.path));
        Ok(())
    }

    pub fn unstage_selected(&mut self) -> Result<()> {
        if self.focus != FocusSection::Staged {
            self.set_status_warn("Focus Staged to unstage files");
            return Ok(());
        }

        let Some(path) = self.selected_staged_path().map(ToOwned::to_owned) else {
            self.set_status_warn("No staged file selected");
            return Ok(());
        };

        git::unstage_file(&self.repo_root, &path)?;
        self.refresh()?;
        self.set_status_info(format!("Unstaged {path}"));
        Ok(())
    }

    pub fn undo_selected_to_mainline(&mut self) -> Result<()> {
        let Some(target) = self.selected_undo_target() else {
            return Ok(());
        };

        if self.settings.confirm_undo_to_mainline {
            self.pending_undo_confirmation = Some(target.clone());
            self.set_status_warn(format!(
                "Undo {} to mainline? Press y to confirm, n/Esc to cancel",
                target.path
            ));
            return Ok(());
        }

        self.apply_undo_to_mainline(target)
    }

    pub fn has_pending_undo_confirmation(&self) -> bool {
        self.pending_undo_confirmation.is_some()
    }

    pub fn confirm_pending_undo_to_mainline(&mut self) -> Result<()> {
        let Some(target) = self.pending_undo_confirmation.take() else {
            return Ok(());
        };

        self.apply_undo_to_mainline(target)
    }

    pub fn cancel_pending_undo_to_mainline(&mut self) {
        if self.pending_undo_confirmation.take().is_some() {
            self.set_status_info("Undo cancelled");
        }
    }

    pub fn load_current_diff(&mut self) -> Result<()> {
        self.load_current_diff_with_scroll(false, 0)
    }

    fn load_current_diff_with_scroll(
        &mut self,
        preserve_scroll: bool,
        preserved_scroll: usize,
    ) -> Result<()> {
        if !preserve_scroll {
            self.diff_scroll = 0;
        }

        let Some((path, mode)) = self.active_selection() else {
            self.diff_rows.clear();
            self.diff_content_height = 0;
            self.diff_scroll = 0;
            return Ok(());
        };

        let raw_diff = git::diff_for_file(&self.repo_root, &path, mode)?;
        self.diff_rows = parse_unified_diff(&raw_diff);
        self.diff_content_height = self.diff_rows.len();

        if preserve_scroll {
            self.diff_scroll = preserved_scroll;
            self.sync_scrolls();
        }

        Ok(())
    }

    pub fn cycle_diff_view_mode(&mut self, delta: isize) -> Result<()> {
        self.settings.diff_view_mode = self.settings.diff_view_mode.cycle(delta);
        self.mark_settings_dirty();
        self.set_status_info(format!(
            "Diff view: {}",
            self.settings.diff_view_mode.label()
        ));
        Ok(())
    }

    pub fn toggle_sidebar_visibility(&mut self) -> Result<()> {
        self.settings.sidebar_visible = !self.settings.sidebar_visible;
        if !self.settings.sidebar_visible {
            self.pane_focus = PaneFocus::Diff;
        }
        self.mark_settings_dirty();
        self.set_status_info(if self.settings.sidebar_visible {
            String::from("Sidebar shown")
        } else {
            String::from("Sidebar hidden")
        });
        Ok(())
    }

    pub fn resize_sidebar(&mut self, delta: isize) -> Result<()> {
        let next = shift_and_clamp_u16(
            self.settings.sidebar_width,
            delta,
            2,
            SIDEBAR_WIDTH_MIN,
            SIDEBAR_WIDTH_MAX,
        );

        if next == self.settings.sidebar_width {
            return Ok(());
        }

        self.settings.sidebar_width = next;
        self.mark_settings_dirty();
        self.set_status_info(format!("Sidebar width: {}", self.settings.sidebar_width));
        Ok(())
    }

    pub fn open_terminal(&mut self) -> Result<()> {
        self.settings_open = false;
        self.terminal_open = true;
        self.terminal_copy_mode = false;
        self.terminal_search_open = false;
        self.terminal_search_query.clear();
        self.terminal_selection_anchor = None;
        self.terminal_cursor_row = 0;
        self.terminal_cursor_col = 0;
        self.terminal_scrollback = 0;

        self.ensure_live_terminal_session()?;
        if let Some(session) = self.terminal_session.as_mut() {
            session.set_scrollback(0);
            session.pump_output();
            self.terminal_scrollback = session.scrollback();
        }

        self.set_status_info("Terminal open (interactive)");
        Ok(())
    }

    pub fn close_terminal(&mut self) {
        if self.terminal_open {
            self.terminal_open = false;
            self.terminal_copy_mode = false;
            self.terminal_search_open = false;
            self.terminal_search_query.clear();
            self.terminal_selection_anchor = None;
            self.terminal_scrollback = 0;
            self.terminal_cursor_row = 0;
            self.terminal_cursor_col = 0;
            self.terminal_view_rows = 0;
            self.terminal_view_cols = 0;
            self.terminal_session = None;
            self.set_status_info("Terminal closed");
            if let Err(error) = self.refresh() {
                self.set_status_error(error);
            }
        }
    }

    pub fn terminal_send_key(&mut self, key: KeyEvent) -> Result<()> {
        let session = self.ensure_live_terminal_session()?;
        session.send_key(key)?;
        session.pump_output();
        self.terminal_scrollback = session.scrollback();
        Ok(())
    }

    pub fn terminal_send_text(&mut self, text: &str) -> Result<()> {
        let session = self.ensure_live_terminal_session()?;
        session.send_text(text)?;
        session.pump_output();
        self.terminal_scrollback = session.scrollback();
        Ok(())
    }

    pub fn terminal_resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        if let Some(session) = self.terminal_session.as_mut() {
            session.resize(rows, cols)?;
        }
        Ok(())
    }

    pub fn terminal_rows(&self) -> Vec<TerminalStyledRow> {
        self.terminal_session
            .as_ref()
            .map(TerminalSession::styled_rows)
            .unwrap_or_default()
    }

    pub fn terminal_plain_rows(&self) -> Vec<String> {
        self.terminal_session
            .as_ref()
            .map(TerminalSession::plain_rows)
            .unwrap_or_default()
    }

    pub fn scroll_terminal(&mut self, delta: isize) {
        let Some(session) = self.terminal_session.as_mut() else {
            return;
        };

        let target = if delta < 0 {
            self.terminal_scrollback
                .saturating_sub(delta.unsigned_abs())
        } else {
            self.terminal_scrollback.saturating_add(delta as usize)
        };

        session.set_scrollback(target);
        self.terminal_scrollback = session.scrollback();

        let max_row = self.terminal_view_rows.saturating_sub(1);
        self.terminal_cursor_row = self.terminal_cursor_row.min(max_row);
    }

    pub fn set_terminal_viewport(&mut self, rows: usize, cols: usize) -> Result<()> {
        self.terminal_view_rows = rows;
        self.terminal_view_cols = cols;
        self.terminal_cursor_row = self.terminal_cursor_row.min(rows.saturating_sub(1));
        self.terminal_cursor_col = self.terminal_cursor_col.min(cols.saturating_sub(1));
        self.terminal_resize(rows as u16, cols as u16)
    }

    pub fn terminal_enter_copy_mode(&mut self) {
        self.terminal_copy_mode = true;
        self.terminal_search_open = false;
        self.terminal_search_query.clear();
        self.terminal_selection_anchor = None;
        self.terminal_cursor_row = self.terminal_view_rows.saturating_sub(1);
        self.terminal_cursor_col = 0;
        self.set_status_info("Copy mode");
    }

    pub fn terminal_exit_copy_mode(&mut self) {
        if self.terminal_copy_mode {
            self.terminal_copy_mode = false;
            self.terminal_search_open = false;
            self.terminal_search_query.clear();
            self.terminal_selection_anchor = None;
            self.set_status_info("Terminal interactive mode");
        }
    }

    pub fn terminal_move_cursor(&mut self, row_delta: isize, col_delta: isize) {
        if row_delta != 0 {
            if row_delta < 0 {
                for _ in 0..row_delta.unsigned_abs() {
                    if self.terminal_cursor_row > 0 {
                        self.terminal_cursor_row -= 1;
                    } else {
                        self.scroll_terminal(1);
                    }
                }
            } else {
                for _ in 0..row_delta as usize {
                    let max_row = self.terminal_view_rows.saturating_sub(1);
                    if self.terminal_cursor_row < max_row {
                        self.terminal_cursor_row += 1;
                    } else if self.terminal_scrollback > 0 {
                        self.scroll_terminal(-1);
                    }
                }
            }
        }

        if col_delta < 0 {
            self.terminal_cursor_col = self
                .terminal_cursor_col
                .saturating_sub(col_delta.unsigned_abs());
        } else if col_delta > 0 {
            self.terminal_cursor_col = self
                .terminal_cursor_col
                .saturating_add(col_delta as usize)
                .min(self.terminal_view_cols.saturating_sub(1));
        }
    }

    pub fn terminal_toggle_selection_anchor(&mut self) {
        if self.terminal_selection_anchor.is_some() {
            self.terminal_selection_anchor = None;
            self.set_status_info("Selection cleared");
        } else {
            self.terminal_selection_anchor =
                Some((self.terminal_cursor_row, self.terminal_cursor_col));
            self.set_status_info("Selection anchor set");
        }
    }

    pub fn terminal_yank_selection(&mut self) -> Result<()> {
        let Some(anchor) = self.terminal_selection_anchor else {
            self.set_status_warn("No selection anchor; press v first");
            return Ok(());
        };

        let rows = self.terminal_plain_rows();
        if rows.is_empty() {
            self.set_status_warn("Nothing to copy");
            return Ok(());
        }

        let cursor = (self.terminal_cursor_row, self.terminal_cursor_col);
        let ((start_row, start_col), (end_row, end_col)) = order_positions(anchor, cursor);

        let mut out = String::new();
        for row_idx in start_row..=end_row {
            let line = rows.get(row_idx).map(String::as_str).unwrap_or("");
            let chars = line.chars().collect::<Vec<_>>();

            let from = if row_idx == start_row {
                start_col.min(chars.len())
            } else {
                0
            };
            let to_exclusive = if row_idx == end_row {
                end_col.saturating_add(1).min(chars.len())
            } else {
                chars.len()
            };

            if from < to_exclusive {
                out.extend(chars[from..to_exclusive].iter());
            }

            if row_idx < end_row {
                out.push('\n');
            }
        }

        if out.is_empty() {
            self.set_status_warn("Selection is empty");
            return Ok(());
        }

        let mut clipboard =
            arboard::Clipboard::new().context("failed to access system clipboard")?;
        clipboard
            .set_text(out.clone())
            .context("failed to copy selection to clipboard")?;

        self.set_status_info(format!("Copied {} chars", out.chars().count()));
        self.terminal_selection_anchor = None;
        Ok(())
    }

    pub fn terminal_open_search(&mut self) {
        self.terminal_search_open = true;
        self.terminal_search_query = self.terminal_last_search.clone();
    }

    pub fn terminal_cancel_search(&mut self) {
        self.terminal_search_open = false;
        self.terminal_search_query.clear();
        self.set_status_info("Search cancelled");
    }

    pub fn terminal_search_append(&mut self, ch: char) {
        self.terminal_search_query.push(ch);
    }

    pub fn terminal_search_backspace(&mut self) {
        self.terminal_search_query.pop();
    }

    pub fn terminal_search_next(&mut self) {
        let query = if self.terminal_search_open {
            self.terminal_search_query.trim().to_owned()
        } else {
            self.terminal_last_search.trim().to_owned()
        };

        if query.is_empty() {
            self.set_status_warn("Search query is empty");
            self.terminal_search_open = false;
            return;
        }

        self.terminal_last_search = query.clone();
        self.terminal_search_open = false;

        let rows = self.terminal_plain_rows();
        if rows.is_empty() {
            self.set_status_warn("No terminal output to search");
            return;
        }

        let start_row = self.terminal_cursor_row.min(rows.len().saturating_sub(1));
        let start_col = self.terminal_cursor_col.saturating_add(1);

        let mut found = None;

        for pass in 0..2 {
            let row_iter: Box<dyn Iterator<Item = usize>> = if pass == 0 {
                Box::new(start_row..rows.len())
            } else {
                Box::new(0..start_row)
            };

            for row_idx in row_iter {
                let col_start = if pass == 0 && row_idx == start_row {
                    start_col
                } else {
                    0
                };
                if let Some(col_idx) = find_query_in_line(&rows[row_idx], &query, col_start) {
                    found = Some((row_idx, col_idx));
                    break;
                }
            }

            if found.is_some() {
                break;
            }
        }

        if let Some((row_idx, col_idx)) = found {
            self.terminal_cursor_row = row_idx.min(self.terminal_view_rows.saturating_sub(1));
            self.terminal_cursor_col = col_idx.min(self.terminal_view_cols.saturating_sub(1));
            self.set_status_info(format!("Found `{}`", query));
        } else {
            self.set_status_warn(format!("No match for `{}`", query));
        }
    }

    pub fn terminal_cursor(&self) -> (usize, usize) {
        (self.terminal_cursor_row, self.terminal_cursor_col)
    }

    pub fn terminal_selection_rows(&self) -> Option<(usize, usize)> {
        self.terminal_selection_anchor.map(|(anchor_row, _)| {
            let start = anchor_row.min(self.terminal_cursor_row);
            let end = anchor_row.max(self.terminal_cursor_row);
            (start, end)
        })
    }

    pub fn toggle_settings_panel(&mut self) {
        self.terminal_open = false;
        self.terminal_copy_mode = false;
        self.terminal_search_open = false;
        self.terminal_search_query.clear();
        self.terminal_selection_anchor = None;
        self.settings_open = !self.settings_open;
        if self.settings_open {
            self.set_status_info("Settings open");
        } else {
            if let Err(error) = self.flush_settings_if_dirty() {
                self.set_status_error(error);
                return;
            }
            self.set_status_info("Settings closed");
        }
    }

    pub fn close_settings_panel(&mut self) {
        if self.settings_open {
            self.settings_open = false;
            if let Err(error) = self.flush_settings_if_dirty() {
                self.set_status_error(error);
                return;
            }
            self.set_status_info("Settings closed");
        }
    }

    pub fn move_settings_selection(&mut self, delta: isize) {
        if SETTINGS_FIELD_COUNT == 0 {
            self.settings_selected = 0;
            return;
        }

        let current = self.settings_selected.min(SETTINGS_FIELD_COUNT - 1);
        self.settings_selected = if delta < 0 {
            current.saturating_sub(delta.unsigned_abs())
        } else {
            (current + delta as usize).min(SETTINGS_FIELD_COUNT - 1)
        };
    }

    pub fn adjust_selected_setting(&mut self, delta: isize) -> Result<()> {
        match self.settings_selected {
            0 => {
                self.settings.diff_view_mode = self.settings.diff_view_mode.cycle(delta);
                self.mark_settings_dirty();
                self.set_status_info(format!(
                    "Diff view: {}",
                    self.settings.diff_view_mode.label()
                ));
            }
            1 => {
                self.settings.sidebar_visible = !self.settings.sidebar_visible;
                self.mark_settings_dirty();
                self.set_status_info(if self.settings.sidebar_visible {
                    String::from("Sidebar shown")
                } else {
                    String::from("Sidebar hidden")
                });
            }
            2 => {
                self.settings.sidebar_position = self.settings.sidebar_position.cycle(delta);
                self.mark_settings_dirty();
                self.set_status_info(format!(
                    "Sidebar side: {}",
                    self.settings.sidebar_position.label()
                ));
            }
            3 => {
                self.settings.sidebar_width = shift_and_clamp_u16(
                    self.settings.sidebar_width,
                    delta,
                    2,
                    SIDEBAR_WIDTH_MIN,
                    SIDEBAR_WIDTH_MAX,
                );
                self.mark_settings_dirty();
                self.set_status_info(format!("Sidebar width: {}", self.settings.sidebar_width));
            }
            4 => {
                self.settings.auto_split_min_width = shift_and_clamp_u16(
                    self.settings.auto_split_min_width,
                    delta,
                    4,
                    AUTO_SPLIT_MIN_WIDTH_MIN,
                    AUTO_SPLIT_MIN_WIDTH_MAX,
                );
                self.mark_settings_dirty();
                self.set_status_info(format!(
                    "Auto split min width: {}",
                    self.settings.auto_split_min_width
                ));
            }
            5 => {
                self.settings.theme = self.settings.theme.cycle(delta);
                self.mark_settings_dirty();
                self.set_status_info(format!("Theme: {}", self.settings.theme.label()));
            }
            6 => {
                self.settings.confirm_undo_to_mainline = !self.settings.confirm_undo_to_mainline;
                self.mark_settings_dirty();
                self.set_status_info(if self.settings.confirm_undo_to_mainline {
                    String::from("Undo confirmation: enabled")
                } else {
                    String::from("Undo confirmation: disabled")
                });
            }
            _ => {}
        }

        Ok(())
    }

    pub fn settings_rows(&self) -> Vec<(&'static str, String)> {
        vec![
            (
                "Diff Layout",
                self.settings.diff_view_mode.label().to_owned(),
            ),
            (
                "Sidebar Visible",
                if self.settings.sidebar_visible {
                    String::from("Yes")
                } else {
                    String::from("No")
                },
            ),
            (
                "Sidebar Side",
                self.settings.sidebar_position.label().to_owned(),
            ),
            ("Sidebar Width", self.settings.sidebar_width.to_string()),
            (
                "Auto Split Min Width",
                format!("{}", self.settings.auto_split_min_width),
            ),
            ("Theme", self.settings.theme.label().to_owned()),
            (
                "Confirm Undo",
                if self.settings.confirm_undo_to_mainline {
                    String::from("Yes")
                } else {
                    String::from("No")
                },
            ),
        ]
    }

    pub fn resolved_diff_layout(&self, diff_width: u16) -> ResolvedDiffLayout {
        match self.settings.diff_view_mode {
            DiffViewMode::Split => ResolvedDiffLayout::Split,
            DiffViewMode::Unified => ResolvedDiffLayout::Unified,
            DiffViewMode::Auto => {
                if diff_width >= self.settings.auto_split_min_width {
                    ResolvedDiffLayout::Split
                } else {
                    ResolvedDiffLayout::Unified
                }
            }
        }
    }

    pub fn diff_mode_hint(&self, diff_width: u16) -> String {
        match self.settings.diff_view_mode {
            DiffViewMode::Auto => {
                format!("Auto->{}", self.resolved_diff_layout(diff_width).label())
            }
            mode => mode.label().to_owned(),
        }
    }

    pub fn config_path_display(&self) -> String {
        match settings::config_file_path() {
            Some(path) => path.display().to_string(),
            None => String::from("<HOME or XDG_CONFIG_HOME not set>"),
        }
    }

    pub fn repo_root_display(&self) -> String {
        self.repo_root.display().to_string()
    }

    pub fn status_text(&self) -> &str {
        self.status.text.as_str()
    }

    pub fn status_kind(&self) -> StatusKind {
        self.status.kind
    }

    pub fn update_layout(&mut self, root: Rect) -> Result<()> {
        let (main_area, _) = layout::split_root(root);
        let (sidebar_area, diff_area) = layout::split_main_area(main_area, &self.settings);

        let (unstaged_inner, staged_inner) = if let Some(area) = sidebar_area {
            let (unstaged_area, staged_area) = layout::split_sidebar(area);
            (
                layout::bordered_inner(unstaged_area),
                layout::bordered_inner(staged_area),
            )
        } else {
            (Rect::new(0, 0, 0, 0), Rect::new(0, 0, 0, 0))
        };

        let (_, diff_body_area) = layout::split_diff(diff_area);
        let resolved_layout = self.resolved_diff_layout(diff_body_area.width);

        let (diff_viewport_height, diff_content_height) = match resolved_layout {
            ResolvedDiffLayout::Split => {
                let (old_pane, new_pane) = layout::split_split_diff(diff_body_area);
                let old_inner = layout::bordered_inner(old_pane);
                let new_inner = layout::bordered_inner(new_pane);
                (
                    old_inner.height.min(new_inner.height) as usize,
                    self.diff_rows.len(),
                )
            }
            ResolvedDiffLayout::Unified => (
                layout::bordered_inner(diff_body_area).height as usize,
                unified_line_count(&self.diff_rows),
            ),
        };

        self.layout = UiLayout {
            unstaged_inner,
            staged_inner,
            diff_area: diff_body_area,
            diff_viewport_height,
        };
        self.diff_content_height = diff_content_height;
        self.sync_scrolls();

        if !self.has_sidebar() {
            self.pane_focus = PaneFocus::Diff;
        }

        if self.terminal_open {
            let output_area = layout::terminal_output_area(root);
            self.set_terminal_viewport(output_area.height as usize, output_area.width as usize)?;
        }

        Ok(())
    }

    fn set_status_info(&mut self, text: impl Into<String>) {
        self.status = StatusMessage::info(text);
    }

    fn set_status_warn(&mut self, text: impl Into<String>) {
        self.status = StatusMessage::warn(text);
    }

    fn set_status_error(&mut self, error: impl ToString) {
        self.status = StatusMessage::error(format!("Error: {}", error.to_string()));
    }

    pub fn sync_scrolls(&mut self) {
        let unstaged_visible = self.layout.unstaged_inner.height as usize;
        let staged_visible = self.layout.staged_inner.height as usize;

        ensure_visible(
            self.unstaged_selected,
            self.unstaged.len(),
            unstaged_visible,
            &mut self.unstaged_scroll,
        );
        ensure_visible(
            self.staged_selected,
            self.staged.len(),
            staged_visible,
            &mut self.staged_scroll,
        );

        let diff_visible = self.layout.diff_viewport_height;
        if diff_visible == 0 {
            self.diff_scroll = 0;
        } else {
            let max_scroll = self.diff_content_height.saturating_sub(diff_visible);
            self.diff_scroll = self.diff_scroll.min(max_scroll);
        }
    }

    pub fn scroll_diff(&mut self, delta: isize) {
        if delta < 0 {
            self.diff_scroll = self.diff_scroll.saturating_sub(delta.unsigned_abs());
        } else {
            self.diff_scroll = self.diff_scroll.saturating_add(delta as usize);
        }
        self.sync_scrolls();
    }

    pub fn is_diff_focused(&self) -> bool {
        self.pane_focus == PaneFocus::Diff
    }

    pub fn toggle_pane_focus(&mut self) {
        if !self.has_sidebar() {
            self.pane_focus = PaneFocus::Diff;
            return;
        }

        self.pane_focus = match self.pane_focus {
            PaneFocus::Sidebar => PaneFocus::Diff,
            PaneFocus::Diff => PaneFocus::Sidebar,
        };
    }

    pub fn click(&mut self, column: u16, row: u16) -> Result<()> {
        if contains(self.layout.unstaged_inner, column, row) {
            self.pane_focus = PaneFocus::Sidebar;
            self.focus = FocusSection::Unstaged;
            let offset = (row - self.layout.unstaged_inner.y) as usize;
            let idx = self.unstaged_scroll + offset;
            if idx < self.unstaged.len() {
                self.unstaged_selected = Some(idx);
            }
            self.load_current_diff()?;
            return Ok(());
        }

        if contains(self.layout.staged_inner, column, row) {
            self.pane_focus = PaneFocus::Sidebar;
            self.focus = FocusSection::Staged;
            let offset = (row - self.layout.staged_inner.y) as usize;
            let idx = self.staged_scroll + offset;
            if idx < self.staged.len() {
                self.staged_selected = Some(idx);
            }
            self.load_current_diff()?;
            return Ok(());
        }

        if contains(self.layout.diff_area, column, row) {
            self.pane_focus = PaneFocus::Diff;
        }

        Ok(())
    }

    pub fn is_in_diff(&self, column: u16, row: u16) -> bool {
        contains(self.layout.diff_area, column, row)
    }

    pub fn set_error(&mut self, error: impl ToString) {
        self.set_status_error(error.to_string());
    }

    pub fn active_path(&self) -> Option<&str> {
        match self.focus {
            FocusSection::Unstaged => self.selected_unstaged().map(|entry| entry.path.as_str()),
            FocusSection::Staged => self.selected_staged_path(),
        }
    }

    pub fn active_label(&self) -> &'static str {
        match self.focus {
            FocusSection::Unstaged => match self.selected_unstaged().map(|entry| entry.kind) {
                Some(UnstagedKind::Untracked) => "Untracked",
                _ => "Unstaged",
            },
            FocusSection::Staged => "Staged",
        }
    }

    fn selected_undo_target(&mut self) -> Option<PendingUndoConfirmation> {
        match self.focus {
            FocusSection::Unstaged => {
                let Some(entry) = self.selected_unstaged().cloned() else {
                    self.set_status_warn("No unstaged file selected");
                    return None;
                };

                Some(PendingUndoConfirmation {
                    path: entry.path,
                    was_untracked: entry.kind == UnstagedKind::Untracked,
                })
            }
            FocusSection::Staged => {
                let Some(path) = self.selected_staged_path().map(ToOwned::to_owned) else {
                    self.set_status_warn("No staged file selected");
                    return None;
                };

                Some(PendingUndoConfirmation {
                    path,
                    was_untracked: false,
                })
            }
        }
    }

    fn apply_undo_to_mainline(&mut self, target: PendingUndoConfirmation) -> Result<()> {
        let mainline =
            git::undo_file_to_mainline(&self.repo_root, &target.path, target.was_untracked)?;
        self.refresh()?;
        self.set_status_info(format!("Reverted {} to {mainline}", target.path));
        Ok(())
    }

    pub fn flush_pending_settings(&mut self) -> Result<()> {
        self.flush_settings_if_dirty()
    }

    fn mark_settings_dirty(&mut self) {
        self.settings.normalize();
        self.settings_dirty = true;
        self.last_settings_change = Some(Instant::now());
    }

    fn flush_settings_if_due(&mut self) -> Result<bool> {
        if !self.settings_dirty {
            return Ok(false);
        }

        let Some(changed_at) = self.last_settings_change else {
            return Ok(false);
        };

        if changed_at.elapsed() < SETTINGS_WRITE_DEBOUNCE {
            return Ok(false);
        }

        settings::save(&self.settings)?;
        self.settings_dirty = false;
        self.last_settings_change = None;
        Ok(true)
    }

    fn flush_settings_if_dirty(&mut self) -> Result<()> {
        if !self.settings_dirty {
            return Ok(());
        }

        settings::save(&self.settings)?;
        self.settings_dirty = false;
        self.last_settings_change = None;
        Ok(())
    }

    fn ensure_live_terminal_session(&mut self) -> Result<&mut TerminalSession> {
        let must_start = self
            .terminal_session
            .as_ref()
            .map(|session| session.is_exited())
            .unwrap_or(true);

        if must_start {
            self.terminal_session = Some(TerminalSession::start(&self.repo_root, 24, 80)?);
            self.terminal_scrollback = 0;
        }

        self.terminal_session
            .as_mut()
            .ok_or_else(|| anyhow!("terminal session should exist after initialization"))
    }

    fn auto_refresh_if_due(&mut self) -> Result<()> {
        if self.last_auto_refresh.elapsed() < AUTO_REFRESH_INTERVAL {
            return Ok(());
        }

        self.last_auto_refresh = Instant::now();
        self.refresh()
    }

    fn has_sidebar(&self) -> bool {
        self.layout.unstaged_inner.width > 0 || self.layout.staged_inner.width > 0
    }

    fn selected_unstaged(&self) -> Option<&FileEntry> {
        self.unstaged_selected
            .and_then(|idx| self.unstaged.get(idx))
    }

    fn selected_unstaged_path(&self) -> Option<&str> {
        self.selected_unstaged().map(|entry| entry.path.as_str())
    }

    fn selected_staged_path(&self) -> Option<&str> {
        self.staged_selected
            .and_then(|idx| self.staged.get(idx).map(String::as_str))
    }

    fn restore_unstaged_selection(&mut self, preferred_path: Option<String>) {
        if self.unstaged.is_empty() {
            self.unstaged_selected = None;
            self.unstaged_scroll = 0;
            return;
        }

        if let Some(path) = preferred_path
            && let Some(idx) = self.unstaged.iter().position(|entry| entry.path == path)
        {
            self.unstaged_selected = Some(idx);
            return;
        }

        if let Some(idx) = self.unstaged_selected
            && idx < self.unstaged.len()
        {
            return;
        }

        self.unstaged_selected = Some(0);
    }

    fn restore_staged_selection(&mut self, preferred_path: Option<String>) {
        if self.staged.is_empty() {
            self.staged_selected = None;
            self.staged_scroll = 0;
            return;
        }

        if let Some(path) = preferred_path
            && let Some(idx) = self.staged.iter().position(|entry| entry == &path)
        {
            self.staged_selected = Some(idx);
            return;
        }

        if let Some(idx) = self.staged_selected
            && idx < self.staged.len()
        {
            return;
        }

        self.staged_selected = Some(0);
    }

    fn normalize_focus(&mut self) {
        match self.focus {
            FocusSection::Unstaged if self.unstaged.is_empty() && !self.staged.is_empty() => {
                self.focus = FocusSection::Staged;
            }
            FocusSection::Staged if self.staged.is_empty() && !self.unstaged.is_empty() => {
                self.focus = FocusSection::Unstaged;
            }
            _ => {}
        }

        if !self.unstaged.is_empty() && self.unstaged_selected.is_none() {
            self.unstaged_selected = Some(0);
        }
        if !self.staged.is_empty() && self.staged_selected.is_none() {
            self.staged_selected = Some(0);
        }
    }

    fn active_selection(&self) -> Option<(String, DiffMode)> {
        match self.focus {
            FocusSection::Unstaged => self.selected_unstaged().map(|entry| {
                let mode = match entry.kind {
                    UnstagedKind::Tracked => DiffMode::UnstagedTracked,
                    UnstagedKind::Untracked => DiffMode::Untracked,
                };
                (entry.path.clone(), mode)
            }),
            FocusSection::Staged => self
                .selected_staged_path()
                .map(|path| (path.to_owned(), DiffMode::Staged)),
        }
    }

    fn focus_len(&self) -> usize {
        match self.focus {
            FocusSection::Unstaged => self.unstaged.len(),
            FocusSection::Staged => self.staged.len(),
        }
    }

    fn focus_selected(&self) -> Option<usize> {
        match self.focus {
            FocusSection::Unstaged => self.unstaged_selected,
            FocusSection::Staged => self.staged_selected,
        }
    }

    fn set_focus_selected(&mut self, value: Option<usize>) {
        match self.focus {
            FocusSection::Unstaged => self.unstaged_selected = value,
            FocusSection::Staged => self.staged_selected = value,
        }
    }
}
