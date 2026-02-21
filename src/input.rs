use anyhow::Result;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};

use crate::app::{App, GitPanelMode};
use crate::keymap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MainKeyAction {
    SwitchFocus,
    MoveUp,
    MoveDown,
    TogglePaneFocus,
    PageUp,
    PageDown,
    Stage,
    Unstage,
    UndoToMainline,
    CycleDiffView,
    ToggleSidebar,
    SidebarNarrow,
    SidebarWide,
    ToggleSettings,
    ToggleGitPanel,
    OpenCommitPrompt,
    OpenTerminal,
    Refresh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsKeyAction {
    Close,
    MoveUp,
    MoveDown,
    AdjustLeft,
    AdjustRight,
}

pub fn handle_event(app: &mut App, event: Event) -> bool {
    if app.has_pending_undo_confirmation() {
        if let Event::Key(key) = event
            && key.kind == KeyEventKind::Press
        {
            handle_pending_undo_key(app, key.code);
        }
        return true;
    }

    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            if app.terminal_open {
                handle_terminal_key(app, key);
                return true;
            }

            if key.code == KeyCode::Char(keymap::KEY_QUIT) {
                return false;
            }

            if app.git_panel_open {
                handle_git_panel_key(app, key);
                return true;
            }

            if app.settings_open {
                handle_settings_key(app, key.code);
                return true;
            }

            if let Some(action) = map_main_key(key.code) {
                run_main_action(app, action);
            }
        }
        Event::Paste(text) if app.terminal_open => {
            if app.terminal_search_open {
                for ch in text.chars() {
                    app.terminal_search_append(ch);
                }
            } else {
                let result = app.terminal_send_text(&text);
                run_action(app, result);
            }
        }
        Event::Paste(text) if app.git_panel_open => handle_git_panel_paste(app, &text),
        Event::Mouse(mouse) if app.terminal_open => match mouse.kind {
            MouseEventKind::ScrollUp => app.scroll_terminal(3),
            MouseEventKind::ScrollDown => app.scroll_terminal(-3),
            _ => {}
        },
        Event::Mouse(mouse) if !app.settings_open && !app.terminal_open => match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let result = app.click(mouse.column, mouse.row);
                run_action(app, result);
            }
            MouseEventKind::ScrollUp if app.is_in_diff(mouse.column, mouse.row) => {
                app.scroll_diff(-3);
            }
            MouseEventKind::ScrollDown if app.is_in_diff(mouse.column, mouse.row) => {
                app.scroll_diff(3);
            }
            _ => {}
        },
        _ => {}
    }

    true
}

fn map_main_key(code: KeyCode) -> Option<MainKeyAction> {
    match code {
        KeyCode::Tab | KeyCode::BackTab => Some(MainKeyAction::SwitchFocus),
        KeyCode::Up | KeyCode::Char('k') => Some(MainKeyAction::MoveUp),
        KeyCode::Down | KeyCode::Char('j') => Some(MainKeyAction::MoveDown),
        KeyCode::Left | KeyCode::Right => Some(MainKeyAction::TogglePaneFocus),
        KeyCode::PageUp => Some(MainKeyAction::PageUp),
        KeyCode::PageDown => Some(MainKeyAction::PageDown),
        KeyCode::Char(keymap::KEY_STAGE) => Some(MainKeyAction::Stage),
        KeyCode::Char(keymap::KEY_UNSTAGE) => Some(MainKeyAction::Unstage),
        KeyCode::Char(keymap::KEY_UNDO_MAINLINE) => Some(MainKeyAction::UndoToMainline),
        KeyCode::Char(keymap::KEY_CYCLE_DIFF_VIEW) => Some(MainKeyAction::CycleDiffView),
        KeyCode::Char(keymap::KEY_TOGGLE_SIDEBAR) => Some(MainKeyAction::ToggleSidebar),
        KeyCode::Char(keymap::KEY_SIDEBAR_NARROW) => Some(MainKeyAction::SidebarNarrow),
        KeyCode::Char(keymap::KEY_SIDEBAR_WIDE) => Some(MainKeyAction::SidebarWide),
        KeyCode::Char(keymap::KEY_OPEN_SETTINGS) => Some(MainKeyAction::ToggleSettings),
        KeyCode::Char(keymap::KEY_OPEN_GIT_PANEL) => Some(MainKeyAction::ToggleGitPanel),
        KeyCode::Char(keymap::KEY_OPEN_COMMIT) => Some(MainKeyAction::OpenCommitPrompt),
        KeyCode::Char(keymap::KEY_OPEN_TERMINAL_PRIMARY)
        | KeyCode::Char(keymap::KEY_OPEN_TERMINAL_ALT) => Some(MainKeyAction::OpenTerminal),
        KeyCode::Char(keymap::KEY_REFRESH) => Some(MainKeyAction::Refresh),
        _ => None,
    }
}

fn run_main_action(app: &mut App, action: MainKeyAction) {
    match action {
        MainKeyAction::SwitchFocus => run_action_with(app, App::switch_focus),
        MainKeyAction::MoveUp => {
            if app.is_diff_focused() {
                app.scroll_diff(-1);
            } else {
                run_action_with(app, |app| app.move_selection(-1));
            }
        }
        MainKeyAction::MoveDown => {
            if app.is_diff_focused() {
                app.scroll_diff(1);
            } else {
                run_action_with(app, |app| app.move_selection(1));
            }
        }
        MainKeyAction::TogglePaneFocus => app.toggle_pane_focus(),
        MainKeyAction::PageUp => app.scroll_diff(-10),
        MainKeyAction::PageDown => app.scroll_diff(10),
        MainKeyAction::Stage => run_action_with(app, App::stage_selected),
        MainKeyAction::Unstage => run_action_with(app, App::unstage_selected),
        MainKeyAction::UndoToMainline => run_action_with(app, App::undo_selected_to_mainline),
        MainKeyAction::CycleDiffView => run_action_with(app, |app| app.cycle_diff_view_mode(1)),
        MainKeyAction::ToggleSidebar => run_action_with(app, App::toggle_sidebar_visibility),
        MainKeyAction::SidebarNarrow => run_action_with(app, |app| app.resize_sidebar(-1)),
        MainKeyAction::SidebarWide => run_action_with(app, |app| app.resize_sidebar(1)),
        MainKeyAction::ToggleSettings => app.toggle_settings_panel(),
        MainKeyAction::ToggleGitPanel => run_action_with(app, App::toggle_git_panel),
        MainKeyAction::OpenCommitPrompt => run_action_with(app, App::open_commit_prompt),
        MainKeyAction::OpenTerminal => run_action_with(app, App::open_terminal),
        MainKeyAction::Refresh => run_action_with(app, App::refresh_with_message),
    }
}

fn handle_git_panel_key(app: &mut App, key: KeyEvent) {
    match app.git_panel_mode {
        GitPanelMode::Browse => handle_git_panel_browse_key(app, key),
        GitPanelMode::CreateBranch => handle_git_panel_create_branch_key(app, key),
        GitPanelMode::CommitMessage => handle_git_panel_commit_key(app, key),
        GitPanelMode::ConfirmDeleteBranch => handle_git_panel_delete_confirm_key(app, key.code),
    }
}

fn handle_git_panel_browse_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char(keymap::KEY_OPEN_GIT_PANEL) => app.close_git_panel(),
        KeyCode::Up | KeyCode::Char('k') => app.move_branch_selection(-1),
        KeyCode::Down | KeyCode::Char('j') => app.move_branch_selection(1),
        KeyCode::Enter | KeyCode::Char(keymap::KEY_GIT_SWITCH_BRANCH) => {
            run_action_with(app, App::switch_to_selected_branch)
        }
        KeyCode::Char(keymap::KEY_GIT_CREATE_BRANCH) => app.open_branch_create_prompt(),
        KeyCode::Char(keymap::KEY_GIT_DELETE_BRANCH) => app.request_delete_selected_branch(),
        KeyCode::Char(keymap::KEY_GIT_COMMIT) => run_action_with(app, App::open_commit_prompt),
        KeyCode::Char(keymap::KEY_REFRESH) => run_action_with(app, App::refresh_with_message),
        _ => {}
    }
}

fn handle_git_panel_create_branch_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.cancel_git_prompt(),
        KeyCode::Enter => run_action_with(app, App::submit_new_branch),
        KeyCode::Backspace => app.git_branch_input_backspace(),
        KeyCode::Char(ch)
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            app.git_branch_input_append(ch);
        }
        _ => {}
    }
}

fn handle_git_panel_commit_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.cancel_git_prompt(),
        KeyCode::Enter => run_action_with(app, App::submit_commit),
        KeyCode::Backspace => app.git_commit_input_backspace(),
        KeyCode::Char(ch)
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            app.git_commit_input_append(ch);
        }
        _ => {}
    }
}

fn handle_git_panel_delete_confirm_key(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            run_action_with(app, App::confirm_delete_selected_branch)
        }
        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => app.cancel_git_prompt(),
        _ => {}
    }
}

fn handle_git_panel_paste(app: &mut App, text: &str) {
    match app.git_panel_mode {
        GitPanelMode::CreateBranch => {
            for ch in text.chars() {
                if ch != '\n' && ch != '\r' {
                    app.git_branch_input_append(ch);
                }
            }
        }
        GitPanelMode::CommitMessage => {
            for ch in text.chars() {
                if ch != '\n' && ch != '\r' {
                    app.git_commit_input_append(ch);
                }
            }
        }
        GitPanelMode::Browse | GitPanelMode::ConfirmDeleteBranch => {}
    }
}

fn handle_pending_undo_key(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            let result = app.confirm_pending_undo_to_mainline();
            run_action(app, result);
        }
        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
            app.cancel_pending_undo_to_mainline()
        }
        _ => {}
    }
}

fn handle_settings_key(app: &mut App, code: KeyCode) {
    if let Some(action) = map_settings_key(code) {
        match action {
            SettingsKeyAction::Close => app.close_settings_panel(),
            SettingsKeyAction::MoveUp => app.move_settings_selection(-1),
            SettingsKeyAction::MoveDown => app.move_settings_selection(1),
            SettingsKeyAction::AdjustLeft => {
                run_action_with(app, |app| app.adjust_selected_setting(-1))
            }
            SettingsKeyAction::AdjustRight => {
                run_action_with(app, |app| app.adjust_selected_setting(1))
            }
        }
    }
}

fn map_settings_key(code: KeyCode) -> Option<SettingsKeyAction> {
    match code {
        KeyCode::Esc | KeyCode::Enter | KeyCode::Char(keymap::KEY_SETTINGS_CLOSE) => {
            Some(SettingsKeyAction::Close)
        }
        KeyCode::Up | KeyCode::Char('k') => Some(SettingsKeyAction::MoveUp),
        KeyCode::Down | KeyCode::Char('j') => Some(SettingsKeyAction::MoveDown),
        KeyCode::Left | KeyCode::Char('h') => Some(SettingsKeyAction::AdjustLeft),
        KeyCode::Right | KeyCode::Char('l') => Some(SettingsKeyAction::AdjustRight),
        _ => None,
    }
}

fn handle_terminal_key(app: &mut App, key: KeyEvent) {
    if app.terminal_search_open {
        if is_terminal_close_chord(key) {
            app.close_terminal();
            return;
        }

        handle_terminal_search_key(app, key);
        return;
    }

    if app.terminal_copy_mode {
        if is_terminal_close_chord(key) {
            app.close_terminal();
            return;
        }

        handle_terminal_copy_key(app, key);
        return;
    }

    if key.code == KeyCode::Esc || is_terminal_close_chord(key) {
        app.close_terminal();
        return;
    }

    if key.modifiers.contains(KeyModifiers::ALT)
        && key.code == KeyCode::Char(keymap::KEY_TERMINAL_COPY_MODE)
    {
        app.terminal_enter_copy_mode();
        return;
    }

    let result = app.terminal_send_key(key);
    run_action(app, result);
}

fn handle_terminal_copy_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char(keymap::KEY_TERMINAL_INTERACTIVE_MODE) => {
            app.terminal_exit_copy_mode()
        }
        KeyCode::Up | KeyCode::Char('k') => app.terminal_move_cursor(-1, 0),
        KeyCode::Down | KeyCode::Char('j') => app.terminal_move_cursor(1, 0),
        KeyCode::Left | KeyCode::Char('h') => app.terminal_move_cursor(0, -1),
        KeyCode::Right | KeyCode::Char('l') => app.terminal_move_cursor(0, 1),
        KeyCode::PageUp => app.scroll_terminal(10),
        KeyCode::PageDown => app.scroll_terminal(-10),
        KeyCode::Home => app.scroll_terminal(10_000),
        KeyCode::End => app.scroll_terminal(-10_000),
        KeyCode::Char(keymap::KEY_TERMINAL_SELECTION_ANCHOR) => {
            app.terminal_toggle_selection_anchor()
        }
        KeyCode::Char(keymap::KEY_TERMINAL_YANK) => {
            let result = app.terminal_yank_selection();
            run_action(app, result);
        }
        KeyCode::Char(keymap::KEY_TERMINAL_SEARCH) => app.terminal_open_search(),
        KeyCode::Char(keymap::KEY_TERMINAL_SEARCH_NEXT) => app.terminal_search_next(),
        _ => {}
    }
}

fn handle_terminal_search_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.terminal_cancel_search(),
        KeyCode::Enter => app.terminal_search_next(),
        KeyCode::Backspace => app.terminal_search_backspace(),
        KeyCode::Char(ch)
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            app.terminal_search_append(ch);
        }
        _ => {}
    }
}

fn is_terminal_close_chord(key: KeyEvent) -> bool {
    if key.code == KeyCode::Char('\u{1d}') {
        return true;
    }

    key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(
            key.code,
            KeyCode::Char(']') | KeyCode::Char('g') | KeyCode::Char('5') | KeyCode::Char('q')
        )
}

fn run_action(app: &mut App, result: Result<()>) {
    if let Err(error) = result {
        app.set_error(error);
    }
}

fn run_action_with<F>(app: &mut App, action: F)
where
    F: FnOnce(&mut App) -> Result<()>,
{
    let result = action(app);
    run_action(app, result);
}

#[cfg(test)]
mod tests {
    use crossterm::event::KeyCode;

    use super::{MainKeyAction, SettingsKeyAction, map_main_key, map_settings_key};
    use crate::keymap;

    #[test]
    fn maps_main_keybindings_to_actions() {
        assert_eq!(
            map_main_key(KeyCode::Char(keymap::KEY_STAGE)),
            Some(MainKeyAction::Stage)
        );
        assert_eq!(
            map_main_key(KeyCode::Char(keymap::KEY_UNDO_MAINLINE)),
            Some(MainKeyAction::UndoToMainline)
        );
        assert_eq!(
            map_main_key(KeyCode::Char(keymap::KEY_OPEN_TERMINAL_PRIMARY)),
            Some(MainKeyAction::OpenTerminal)
        );
        assert_eq!(
            map_main_key(KeyCode::Char(keymap::KEY_OPEN_GIT_PANEL)),
            Some(MainKeyAction::ToggleGitPanel)
        );
        assert_eq!(
            map_main_key(KeyCode::Char(keymap::KEY_OPEN_COMMIT)),
            Some(MainKeyAction::OpenCommitPrompt)
        );
        assert_eq!(map_main_key(KeyCode::F(5)), None);
    }

    #[test]
    fn maps_settings_keybindings_to_actions() {
        assert_eq!(
            map_settings_key(KeyCode::Char(keymap::KEY_SETTINGS_CLOSE)),
            Some(SettingsKeyAction::Close)
        );
        assert_eq!(
            map_settings_key(KeyCode::Char('h')),
            Some(SettingsKeyAction::AdjustLeft)
        );
        assert_eq!(map_settings_key(KeyCode::Char('x')), None);
    }
}
