use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, Borders};

use crate::settings::{self, AppSettings, SidebarPosition};

pub const MIN_DIFF_WIDTH_WITH_SIDEBAR: u16 = 48;
pub const TERMINAL_MODAL_WIDTH_PERCENT: u16 = 92;
pub const TERMINAL_MODAL_HEIGHT_PERCENT: u16 = 82;
pub const GIT_MODAL_WIDTH_PERCENT: u16 = 72;
pub const GIT_MODAL_HEIGHT_PERCENT: u16 = 72;
pub const SETTINGS_MODAL_WIDTH_PERCENT: u16 = 70;
pub const SETTINGS_MODAL_HEIGHT_PERCENT: u16 = 60;
pub const HELP_MODAL_WIDTH_PERCENT: u16 = 76;
pub const HELP_MODAL_HEIGHT_PERCENT: u16 = 74;

pub fn split_root(root: Rect) -> (Rect, Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(root);
    (rows[0], rows[1])
}

pub fn split_main_area(area: Rect, settings: &AppSettings) -> (Option<Rect>, Rect) {
    if !settings.sidebar_visible {
        return (None, area);
    }

    if area.width <= MIN_DIFF_WIDTH_WITH_SIDEBAR {
        return (None, area);
    }

    let max_sidebar = area.width.saturating_sub(MIN_DIFF_WIDTH_WITH_SIDEBAR);
    let requested = settings
        .sidebar_width
        .clamp(settings::SIDEBAR_WIDTH_MIN, settings::SIDEBAR_WIDTH_MAX);
    let sidebar_width = requested.min(max_sidebar);
    if sidebar_width < settings::SIDEBAR_WIDTH_MIN {
        return (None, area);
    }

    let chunks = match settings.sidebar_position {
        SidebarPosition::Left => Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(sidebar_width), Constraint::Min(1)])
            .split(area),
        SidebarPosition::Right => Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(1), Constraint::Length(sidebar_width)])
            .split(area),
    };

    match settings.sidebar_position {
        SidebarPosition::Left => (Some(chunks[0]), chunks[1]),
        SidebarPosition::Right => (Some(chunks[1]), chunks[0]),
    }
}

pub fn split_sidebar(sidebar_area: Rect) -> (Rect, Rect) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(sidebar_area);
    (sections[0], sections[1])
}

pub fn split_diff(diff_area: Rect) -> (Rect, Rect) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(diff_area);
    (sections[0], sections[1])
}

pub fn split_split_diff(diff_body_area: Rect) -> (Rect, Rect) {
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(diff_body_area);
    (panes[0], panes[1])
}

pub fn centered_rect(horizontal_percent: u16, vertical_percent: u16, area: Rect) -> Rect {
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

pub fn terminal_popup(area: Rect) -> Rect {
    centered_rect(
        TERMINAL_MODAL_WIDTH_PERCENT,
        TERMINAL_MODAL_HEIGHT_PERCENT,
        area,
    )
}

pub fn settings_popup(area: Rect) -> Rect {
    centered_rect(
        SETTINGS_MODAL_WIDTH_PERCENT,
        SETTINGS_MODAL_HEIGHT_PERCENT,
        area,
    )
}

pub fn git_popup(area: Rect) -> Rect {
    centered_rect(GIT_MODAL_WIDTH_PERCENT, GIT_MODAL_HEIGHT_PERCENT, area)
}

pub fn help_popup(area: Rect) -> Rect {
    centered_rect(HELP_MODAL_WIDTH_PERCENT, HELP_MODAL_HEIGHT_PERCENT, area)
}

pub fn terminal_output_area(area: Rect) -> Rect {
    let popup = terminal_popup(area);
    let inner = bordered_inner(popup);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner);
    sections[1]
}

pub fn bordered_inner(area: Rect) -> Rect {
    Block::default().borders(Borders::ALL).inner(area)
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;

    use crate::settings::{AppSettings, SidebarPosition};

    use super::{MIN_DIFF_WIDTH_WITH_SIDEBAR, centered_rect, split_main_area};

    #[test]
    fn hides_sidebar_when_main_area_too_narrow() {
        let settings = AppSettings {
            sidebar_visible: true,
            sidebar_width: 40,
            ..AppSettings::default()
        };
        let area = Rect::new(0, 0, MIN_DIFF_WIDTH_WITH_SIDEBAR, 30);

        let (sidebar, diff) = split_main_area(area, &settings);
        assert!(sidebar.is_none());
        assert_eq!(diff, area);
    }

    #[test]
    fn places_sidebar_on_requested_side() {
        let left_settings = AppSettings {
            sidebar_visible: true,
            sidebar_width: 30,
            sidebar_position: SidebarPosition::Left,
            ..AppSettings::default()
        };
        let right_settings = AppSettings {
            sidebar_position: SidebarPosition::Right,
            ..left_settings.clone()
        };
        let area = Rect::new(0, 0, 140, 30);

        let (left_sidebar, left_diff) = split_main_area(area, &left_settings);
        let (right_sidebar, right_diff) = split_main_area(area, &right_settings);

        let left_sidebar = left_sidebar.expect("left sidebar should be present");
        let right_sidebar = right_sidebar.expect("right sidebar should be present");

        assert_eq!(left_sidebar.x, 0);
        assert_eq!(left_diff.x, left_sidebar.width);
        assert_eq!(right_diff.x, 0);
        assert_eq!(right_sidebar.x, right_diff.width);
    }

    #[test]
    fn centers_rect_with_requested_percentages() {
        let area = Rect::new(0, 0, 100, 50);
        let centered = centered_rect(50, 40, area);

        assert_eq!(centered.width, 50);
        assert_eq!(centered.height, 20);
        assert_eq!(centered.x, 25);
        assert_eq!(centered.y, 15);
    }
}
