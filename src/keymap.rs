pub const KEY_QUIT: char = 'q';
pub const KEY_STAGE: char = 's';
pub const KEY_UNSTAGE: char = 'u';
pub const KEY_UNDO_MAINLINE: char = 'x';
pub const KEY_CYCLE_DIFF_VIEW: char = 'v';
pub const KEY_TOGGLE_SIDEBAR: char = 'b';
pub const KEY_SIDEBAR_NARROW: char = '[';
pub const KEY_SIDEBAR_WIDE: char = ']';
pub const KEY_OPEN_SETTINGS: char = 'o';
pub const KEY_OPEN_GIT_PANEL: char = 'g';
pub const KEY_OPEN_COMMIT: char = 'c';
pub const KEY_OPEN_TERMINAL_PRIMARY: char = ':';
pub const KEY_OPEN_TERMINAL_ALT: char = '!';
pub const KEY_REFRESH: char = 'r';

pub const KEY_GIT_CREATE_BRANCH: char = 'a';
pub const KEY_GIT_SWITCH_BRANCH: char = 's';
pub const KEY_GIT_DELETE_BRANCH: char = 'd';
pub const KEY_GIT_COMMIT: char = KEY_OPEN_COMMIT;

pub const KEY_TERMINAL_COPY_MODE: char = 'c';
pub const KEY_TERMINAL_INTERACTIVE_MODE: char = 'i';
pub const KEY_TERMINAL_SELECTION_ANCHOR: char = 'v';
pub const KEY_TERMINAL_YANK: char = 'y';
pub const KEY_TERMINAL_SEARCH: char = '/';
pub const KEY_TERMINAL_SEARCH_NEXT: char = 'n';

pub const KEY_SETTINGS_CLOSE: char = KEY_OPEN_SETTINGS;

pub fn terminal_close_hint() -> &'static str {
    "Ctrl+], Ctrl+g, or Ctrl+q"
}

pub fn footer_hint_pending_undo() -> &'static str {
    "confirm undo: y apply to mainline, n/Esc cancel"
}

pub fn footer_hint_terminal_search() -> &'static str {
    "terminal search: type query, Enter find, Esc cancel"
}

pub fn footer_hint_git_panel() -> String {
    format!(
        "git: j/k move  Enter/{} switch  {} new branch  {} delete  {} commit  Esc close",
        KEY_GIT_SWITCH_BRANCH, KEY_GIT_CREATE_BRANCH, KEY_GIT_DELETE_BRANCH, KEY_GIT_COMMIT,
    )
}

pub fn footer_hint_terminal_copy() -> &'static str {
    "copy mode: move(hjkl/arrows)  v mark  y copy  / search  n next  i interactive"
}

pub fn footer_hint_terminal() -> String {
    format!(
        "terminal: all keys -> shell, Esc close, Alt+{} copy mode, {} close",
        KEY_TERMINAL_COPY_MODE,
        terminal_close_hint()
    )
}

pub fn footer_hint_settings() -> &'static str {
    "settings: j/k select, h/l change, Esc close"
}

pub fn footer_hint_main() -> String {
    format!(
        "Tab list  Left/Right pane  Up/Down move-or-scroll  {} stage  {} unstage  {} undo->mainline  {} branches  {} commit  {} terminal  {} settings  {} quit",
        KEY_STAGE,
        KEY_UNSTAGE,
        KEY_UNDO_MAINLINE,
        KEY_OPEN_GIT_PANEL,
        KEY_OPEN_COMMIT,
        KEY_OPEN_TERMINAL_PRIMARY,
        KEY_OPEN_SETTINGS,
        KEY_QUIT,
    )
}

pub fn terminal_modal_interactive_hint() -> String {
    format!(
        "interactive shell. Esc closes. Alt+{} enters copy mode. {} also close.",
        KEY_TERMINAL_COPY_MODE,
        terminal_close_hint()
    )
}

pub fn terminal_modal_copy_hint() -> &'static str {
    "copy: hjkl/arrows move  v mark  y yank  / search  n next  i interactive"
}

#[cfg(test)]
mod tests {
    use super::{
        KEY_OPEN_GIT_PANEL, KEY_OPEN_SETTINGS, KEY_OPEN_TERMINAL_PRIMARY, KEY_QUIT, KEY_STAGE,
        KEY_TERMINAL_COPY_MODE, KEY_UNDO_MAINLINE, footer_hint_main, footer_hint_terminal,
        terminal_modal_interactive_hint,
    };

    #[test]
    fn footer_main_hint_contains_primary_keys() {
        let hint = footer_hint_main();
        assert!(hint.contains(KEY_STAGE));
        assert!(hint.contains(KEY_UNDO_MAINLINE));
        assert!(hint.contains(KEY_OPEN_GIT_PANEL));
        assert!(hint.contains(KEY_OPEN_TERMINAL_PRIMARY));
        assert!(hint.contains(KEY_OPEN_SETTINGS));
        assert!(hint.contains(KEY_QUIT));
    }

    #[test]
    fn terminal_hints_reference_copy_mode_key() {
        let footer = footer_hint_terminal();
        let modal = terminal_modal_interactive_hint();
        assert!(footer.contains(KEY_TERMINAL_COPY_MODE));
        assert!(modal.contains(KEY_TERMINAL_COPY_MODE));
    }
}
