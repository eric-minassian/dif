use std::env;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub const SIDEBAR_WIDTH_MIN: u16 = 24;
pub const SIDEBAR_WIDTH_MAX: u16 = 60;
pub const AUTO_SPLIT_MIN_WIDTH_MIN: u16 = 90;
pub const AUTO_SPLIT_MIN_WIDTH_MAX: u16 = 220;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffViewMode {
    Auto,
    Split,
    Unified,
}

impl DiffViewMode {
    pub fn cycle(self, delta: isize) -> Self {
        let items = [Self::Auto, Self::Split, Self::Unified];
        cycle(items, self, delta)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Split => "Split",
            Self::Unified => "Unified",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidebarPosition {
    Left,
    Right,
}

impl SidebarPosition {
    pub fn cycle(self, delta: isize) -> Self {
        let items = [Self::Left, Self::Right];
        cycle(items, self, delta)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Right => "Right",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppTheme {
    Ocean,
    Eighties,
    Solarized,
    Monokai,
}

impl AppTheme {
    pub fn cycle(self, delta: isize) -> Self {
        let items = [Self::Ocean, Self::Eighties, Self::Solarized, Self::Monokai];
        cycle(items, self, delta)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Ocean => "Ocean",
            Self::Eighties => "Eighties",
            Self::Solarized => "Solarized",
            Self::Monokai => "Monokai",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub diff_view_mode: DiffViewMode,
    pub sidebar_position: SidebarPosition,
    pub sidebar_visible: bool,
    pub sidebar_width: u16,
    pub auto_split_min_width: u16,
    pub theme: AppTheme,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            diff_view_mode: DiffViewMode::Auto,
            sidebar_position: SidebarPosition::Left,
            sidebar_visible: true,
            sidebar_width: 34,
            auto_split_min_width: 140,
            theme: AppTheme::Ocean,
        }
    }
}

impl AppSettings {
    pub fn normalize(&mut self) {
        self.sidebar_width = self
            .sidebar_width
            .clamp(SIDEBAR_WIDTH_MIN, SIDEBAR_WIDTH_MAX);
        self.auto_split_min_width = self
            .auto_split_min_width
            .clamp(AUTO_SPLIT_MIN_WIDTH_MIN, AUTO_SPLIT_MIN_WIDTH_MAX);
    }
}

pub fn load() -> Result<AppSettings> {
    let Some(config_path) = config_file_path() else {
        return Ok(AppSettings::default());
    };

    if !config_path.exists() {
        return Ok(AppSettings::default());
    }

    let raw = fs::read_to_string(&config_path)
        .with_context(|| format!("failed to read settings at `{}`", config_path.display()))?;
    let mut settings: AppSettings = toml::from_str(&raw)
        .with_context(|| format!("failed to parse settings at `{}`", config_path.display()))?;
    settings.normalize();

    Ok(settings)
}

pub fn save(settings: &AppSettings) -> Result<PathBuf> {
    let Some(path) = config_file_path() else {
        bail!("unable to determine config path; set HOME or XDG_CONFIG_HOME");
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create config directory `{}`",
                parent.to_string_lossy()
            )
        })?;
    }

    let mut normalized = settings.clone();
    normalized.normalize();
    let toml = toml::to_string_pretty(&normalized).context("failed to serialize settings")?;

    fs::write(&path, toml)
        .with_context(|| format!("failed to write settings to `{}`", path.display()))?;

    Ok(path)
}

pub fn config_file_path() -> Option<PathBuf> {
    if let Some(xdg) = env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg).join("dif").join("config.toml"));
    }

    env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join(".config")
            .join("dif")
            .join("config.toml")
    })
}

fn cycle<T: Copy + Eq, const N: usize>(items: [T; N], current: T, delta: isize) -> T {
    cycle_slice(&items, current, delta)
}

fn cycle_slice<T: Copy + Eq>(items: &[T], current: T, delta: isize) -> T {
    let len = items.len();
    let idx = items.iter().position(|item| *item == current).unwrap_or(0);

    let shift = if delta >= 0 {
        delta as usize % len
    } else {
        let abs = delta.unsigned_abs() % len;
        (len - abs) % len
    };

    items[(idx + shift) % len]
}
