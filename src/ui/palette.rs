use ratatui::style::{Color, Modifier, Style};

use crate::settings::AppTheme;

#[derive(Clone, Copy)]
pub(crate) struct Palette {
    pub pane_bg: (u8, u8, u8),
    pub meta_bg: (u8, u8, u8),
    pub added_bg: (u8, u8, u8),
    pub removed_bg: (u8, u8, u8),
    pub text: (u8, u8, u8),
    pub dim: (u8, u8, u8),
    pub line_no: (u8, u8, u8),
    pub marker_add: (u8, u8, u8),
    pub marker_remove: (u8, u8, u8),
    pub marker_context: (u8, u8, u8),
    pub border: (u8, u8, u8),
    pub border_focus: (u8, u8, u8),
    pub selected_bg_focused: (u8, u8, u8),
    pub selected_bg_unfocused: (u8, u8, u8),
    pub untracked: (u8, u8, u8),
    pub footer: (u8, u8, u8),
    pub modal_bg: (u8, u8, u8),
    pub modal_border: (u8, u8, u8),
    pub modal_selected_bg: (u8, u8, u8),
    pub status_warn: (u8, u8, u8),
    pub status_error: (u8, u8, u8),
}

pub(crate) fn palette_for(theme: AppTheme) -> Palette {
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
            status_warn: (230, 185, 90),
            status_error: (232, 130, 140),
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
            status_warn: (236, 194, 108),
            status_error: (236, 146, 162),
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
            status_warn: (203, 165, 80),
            status_error: (220, 144, 138),
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
            status_warn: (253, 200, 97),
            status_error: (249, 122, 132),
        },
    }
}

pub(crate) fn border_style(focused: bool, palette: &Palette) -> Style {
    if focused {
        Style::default().fg(rgb(palette.border_focus))
    } else {
        Style::default().fg(rgb(palette.border))
    }
}

pub(crate) fn selected_style(focused: bool, palette: &Palette) -> Style {
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

pub(crate) fn rgb(value: (u8, u8, u8)) -> Color {
    Color::Rgb(value.0, value.1, value.2)
}
