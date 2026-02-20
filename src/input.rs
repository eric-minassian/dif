use anyhow::Result;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};

use crate::app::App;

pub fn handle_event(app: &mut App, event: Event) -> bool {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            if app.terminal_open {
                handle_terminal_key(app, key);
                return true;
            }

            if key.code == KeyCode::Char('q') {
                return false;
            }

            if app.settings_open {
                handle_settings_key(app, key.code);
                return true;
            }

            match key.code {
                KeyCode::Tab => {
                    let result = app.switch_focus();
                    run_action(app, result);
                }
                KeyCode::BackTab => {
                    let result = app.switch_focus();
                    run_action(app, result);
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if app.is_diff_focused() {
                        app.scroll_diff(-1);
                    } else {
                        let result = app.move_selection(-1);
                        run_action(app, result);
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if app.is_diff_focused() {
                        app.scroll_diff(1);
                    } else {
                        let result = app.move_selection(1);
                        run_action(app, result);
                    }
                }
                KeyCode::Left | KeyCode::Right => app.toggle_pane_focus(),
                KeyCode::PageUp => app.scroll_diff(-10),
                KeyCode::PageDown => app.scroll_diff(10),
                KeyCode::Char('s') => {
                    let result = app.stage_selected();
                    run_action(app, result);
                }
                KeyCode::Char('u') => {
                    let result = app.unstage_selected();
                    run_action(app, result);
                }
                KeyCode::Char('v') => {
                    let result = app.cycle_diff_view_mode(1);
                    run_action(app, result);
                }
                KeyCode::Char('b') => {
                    let result = app.toggle_sidebar_visibility();
                    run_action(app, result);
                }
                KeyCode::Char('[') => {
                    let result = app.resize_sidebar(-1);
                    run_action(app, result);
                }
                KeyCode::Char(']') => {
                    let result = app.resize_sidebar(1);
                    run_action(app, result);
                }
                KeyCode::Char('o') => app.toggle_settings_panel(),
                KeyCode::Char(':') | KeyCode::Char('!') => {
                    let result = app.open_terminal();
                    run_action(app, result);
                }
                KeyCode::Char('r') => {
                    let result = app.refresh_with_message();
                    run_action(app, result);
                }
                _ => {}
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

fn handle_settings_key(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('o') => app.close_settings_panel(),
        KeyCode::Up | KeyCode::Char('k') => app.move_settings_selection(-1),
        KeyCode::Down | KeyCode::Char('j') => app.move_settings_selection(1),
        KeyCode::Left | KeyCode::Char('h') => {
            let result = app.adjust_selected_setting(-1);
            run_action(app, result);
        }
        KeyCode::Right | KeyCode::Char('l') => {
            let result = app.adjust_selected_setting(1);
            run_action(app, result);
        }
        _ => {}
    }
}

fn handle_terminal_key(app: &mut App, key: KeyEvent) {
    if is_terminal_close_chord(key) {
        app.close_terminal();
        return;
    }

    if app.terminal_search_open {
        handle_terminal_search_key(app, key);
        return;
    }

    if app.terminal_copy_mode {
        handle_terminal_copy_key(app, key);
        return;
    }

    if key.modifiers.contains(KeyModifiers::ALT) && key.code == KeyCode::Char('c') {
        app.terminal_enter_copy_mode();
        return;
    }

    let result = app.terminal_send_key(key);
    run_action(app, result);
}

fn handle_terminal_copy_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('i') => app.terminal_exit_copy_mode(),
        KeyCode::Up | KeyCode::Char('k') => app.terminal_move_cursor(-1, 0),
        KeyCode::Down | KeyCode::Char('j') => app.terminal_move_cursor(1, 0),
        KeyCode::Left | KeyCode::Char('h') => app.terminal_move_cursor(0, -1),
        KeyCode::Right | KeyCode::Char('l') => app.terminal_move_cursor(0, 1),
        KeyCode::PageUp => app.scroll_terminal(10),
        KeyCode::PageDown => app.scroll_terminal(-10),
        KeyCode::Home => app.scroll_terminal(10_000),
        KeyCode::End => app.scroll_terminal(-10_000),
        KeyCode::Char('v') => app.terminal_toggle_selection_anchor(),
        KeyCode::Char('y') => {
            let result = app.terminal_yank_selection();
            run_action(app, result);
        }
        KeyCode::Char('/') => app.terminal_open_search(),
        KeyCode::Char('n') => app.terminal_search_next(),
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
