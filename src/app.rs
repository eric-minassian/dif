use std::path::PathBuf;

use anyhow::Result;
use ratatui::layout::Rect;

use crate::diff::{parse_unified_diff, DiffRow};
use crate::git::{self, DiffMode, FileEntry, UnstagedKind};
use crate::settings::{
    self, AppSettings, DiffViewMode, AUTO_SPLIT_MIN_WIDTH_MAX, AUTO_SPLIT_MIN_WIDTH_MIN,
    SIDEBAR_WIDTH_MAX, SIDEBAR_WIDTH_MIN,
};

const SETTINGS_FIELD_COUNT: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusSection {
    Unstaged,
    Staged,
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

pub struct App {
    repo_root: PathBuf,
    pub settings: AppSettings,
    pub settings_open: bool,
    pub settings_selected: usize,
    pub unstaged: Vec<FileEntry>,
    pub staged: Vec<String>,
    pub focus: FocusSection,
    pub unstaged_selected: Option<usize>,
    pub staged_selected: Option<usize>,
    pub unstaged_scroll: usize,
    pub staged_scroll: usize,
    pub diff_rows: Vec<DiffRow>,
    pub diff_scroll: usize,
    pub diff_content_height: usize,
    pub status_line: String,
    pub layout: UiLayout,
}

impl App {
    pub fn new(repo_root: PathBuf) -> Result<Self> {
        let (settings, status_line) = match settings::load() {
            Ok(settings) => (settings, String::from("Ready")),
            Err(error) => (
                AppSettings::default(),
                format!("Settings parse error, using defaults ({error})"),
            ),
        };

        let mut app = Self {
            repo_root,
            settings,
            settings_open: false,
            settings_selected: 0,
            unstaged: Vec::new(),
            staged: Vec::new(),
            focus: FocusSection::Unstaged,
            unstaged_selected: None,
            staged_selected: None,
            unstaged_scroll: 0,
            staged_scroll: 0,
            diff_rows: Vec::new(),
            diff_scroll: 0,
            diff_content_height: 0,
            status_line,
            layout: UiLayout::default(),
        };

        app.refresh()?;
        Ok(app)
    }

    pub fn refresh_with_message(&mut self) -> Result<()> {
        self.refresh()?;
        self.status_line = String::from("Refreshed");
        Ok(())
    }

    pub fn refresh(&mut self) -> Result<()> {
        let previous_unstaged = self.selected_unstaged_path().map(ToOwned::to_owned);
        let previous_staged = self.selected_staged_path().map(ToOwned::to_owned);

        let status = git::status(&self.repo_root)?;
        self.unstaged = status.unstaged;
        self.staged = status.staged;

        self.restore_unstaged_selection(previous_unstaged);
        self.restore_staged_selection(previous_staged);
        self.normalize_focus();
        self.load_current_diff()?;

        Ok(())
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
            self.status_line = String::from("Focus Unstaged to stage files");
            return Ok(());
        }

        let Some(entry) = self.selected_unstaged().cloned() else {
            self.status_line = String::from("No unstaged file selected");
            return Ok(());
        };

        git::stage_file(&self.repo_root, &entry.path)?;
        self.refresh()?;
        self.status_line = format!("Staged {}", entry.path);
        Ok(())
    }

    pub fn unstage_selected(&mut self) -> Result<()> {
        if self.focus != FocusSection::Staged {
            self.status_line = String::from("Focus Staged to unstage files");
            return Ok(());
        }

        let Some(path) = self.selected_staged_path().map(ToOwned::to_owned) else {
            self.status_line = String::from("No staged file selected");
            return Ok(());
        };

        git::unstage_file(&self.repo_root, &path)?;
        self.refresh()?;
        self.status_line = format!("Unstaged {path}");
        Ok(())
    }

    pub fn load_current_diff(&mut self) -> Result<()> {
        self.diff_scroll = 0;

        let Some((path, mode)) = self.active_selection() else {
            self.diff_rows.clear();
            self.diff_content_height = 0;
            return Ok(());
        };

        let raw_diff = git::diff_for_file(&self.repo_root, &path, mode)?;
        self.diff_rows = parse_unified_diff(&raw_diff);
        self.diff_content_height = self.diff_rows.len();
        Ok(())
    }

    pub fn cycle_diff_view_mode(&mut self, delta: isize) -> Result<()> {
        self.settings.diff_view_mode = self.settings.diff_view_mode.cycle(delta);
        self.persist_settings()?;
        self.status_line = format!("Diff view: {}", self.settings.diff_view_mode.label());
        Ok(())
    }

    pub fn toggle_sidebar_visibility(&mut self) -> Result<()> {
        self.settings.sidebar_visible = !self.settings.sidebar_visible;
        self.persist_settings()?;
        self.status_line = if self.settings.sidebar_visible {
            String::from("Sidebar shown")
        } else {
            String::from("Sidebar hidden")
        };
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
        self.persist_settings()?;
        self.status_line = format!("Sidebar width: {}", self.settings.sidebar_width);
        Ok(())
    }

    pub fn toggle_settings_panel(&mut self) {
        self.settings_open = !self.settings_open;
        if self.settings_open {
            self.status_line = String::from("Settings open");
        } else {
            self.status_line = String::from("Settings closed");
        }
    }

    pub fn close_settings_panel(&mut self) {
        if self.settings_open {
            self.settings_open = false;
            self.status_line = String::from("Settings closed");
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
                self.persist_settings()?;
                self.status_line = format!("Diff view: {}", self.settings.diff_view_mode.label());
            }
            1 => {
                self.settings.sidebar_visible = !self.settings.sidebar_visible;
                self.persist_settings()?;
                self.status_line = if self.settings.sidebar_visible {
                    String::from("Sidebar shown")
                } else {
                    String::from("Sidebar hidden")
                };
            }
            2 => {
                self.settings.sidebar_position = self.settings.sidebar_position.cycle(delta);
                self.persist_settings()?;
                self.status_line =
                    format!("Sidebar side: {}", self.settings.sidebar_position.label());
            }
            3 => {
                self.settings.sidebar_width = shift_and_clamp_u16(
                    self.settings.sidebar_width,
                    delta,
                    2,
                    SIDEBAR_WIDTH_MIN,
                    SIDEBAR_WIDTH_MAX,
                );
                self.persist_settings()?;
                self.status_line = format!("Sidebar width: {}", self.settings.sidebar_width);
            }
            4 => {
                self.settings.auto_split_min_width = shift_and_clamp_u16(
                    self.settings.auto_split_min_width,
                    delta,
                    4,
                    AUTO_SPLIT_MIN_WIDTH_MIN,
                    AUTO_SPLIT_MIN_WIDTH_MAX,
                );
                self.persist_settings()?;
                self.status_line = format!(
                    "Auto split min width: {}",
                    self.settings.auto_split_min_width
                );
            }
            5 => {
                self.settings.theme = self.settings.theme.cycle(delta);
                self.persist_settings()?;
                self.status_line = format!("Theme: {}", self.settings.theme.label());
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

    pub fn set_layout(&mut self, layout: UiLayout) {
        self.layout = layout;
    }

    pub fn set_diff_content_height(&mut self, content_height: usize) {
        self.diff_content_height = content_height;
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

    pub fn click(&mut self, column: u16, row: u16) -> Result<()> {
        if contains(self.layout.unstaged_inner, column, row) {
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
            self.focus = FocusSection::Staged;
            let offset = (row - self.layout.staged_inner.y) as usize;
            let idx = self.staged_scroll + offset;
            if idx < self.staged.len() {
                self.staged_selected = Some(idx);
            }
            self.load_current_diff()?;
        }

        Ok(())
    }

    pub fn is_in_diff(&self, column: u16, row: u16) -> bool {
        contains(self.layout.diff_area, column, row)
    }

    pub fn set_error(&mut self, error: impl ToString) {
        self.status_line = format!("Error: {}", error.to_string());
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

    fn persist_settings(&mut self) -> Result<()> {
        self.settings.normalize();
        settings::save(&self.settings)?;
        Ok(())
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

        if let Some(path) = preferred_path {
            if let Some(idx) = self.unstaged.iter().position(|entry| entry.path == path) {
                self.unstaged_selected = Some(idx);
                return;
            }
        }

        if let Some(idx) = self.unstaged_selected {
            if idx < self.unstaged.len() {
                return;
            }
        }

        self.unstaged_selected = Some(0);
    }

    fn restore_staged_selection(&mut self, preferred_path: Option<String>) {
        if self.staged.is_empty() {
            self.staged_selected = None;
            self.staged_scroll = 0;
            return;
        }

        if let Some(path) = preferred_path {
            if let Some(idx) = self.staged.iter().position(|entry| entry == &path) {
                self.staged_selected = Some(idx);
                return;
            }
        }

        if let Some(idx) = self.staged_selected {
            if idx < self.staged.len() {
                return;
            }
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

fn contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x
        && x < rect.x.saturating_add(rect.width)
        && y >= rect.y
        && y < rect.y.saturating_add(rect.height)
}

fn ensure_visible(selected: Option<usize>, len: usize, visible: usize, scroll: &mut usize) {
    if len == 0 || visible == 0 {
        *scroll = 0;
        return;
    }

    *scroll = (*scroll).min(len.saturating_sub(visible));

    let Some(selected) = selected else {
        return;
    };

    if selected < *scroll {
        *scroll = selected;
        return;
    }

    let bottom = *scroll + visible;
    if selected >= bottom {
        *scroll = selected + 1 - visible;
    }
}

fn shift_and_clamp_u16(value: u16, delta: isize, step: u16, min: u16, max: u16) -> u16 {
    let candidate = value as i32 + (delta as i32 * step as i32);
    candidate.clamp(min as i32, max as i32) as u16
}
