use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};

use crate::app::App;

pub fn handle_event(app: &mut App, event: Event) -> bool {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => {
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
                KeyCode::Up | KeyCode::Char('k') => {
                    let result = app.move_selection(-1);
                    run_action(app, result);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let result = app.move_selection(1);
                    run_action(app, result);
                }
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
                KeyCode::Char('r') => {
                    let result = app.refresh_with_message();
                    run_action(app, result);
                }
                _ => {}
            }
        }
        Event::Mouse(mouse) if !app.settings_open => match mouse.kind {
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

fn run_action(app: &mut App, result: Result<()>) {
    if let Err(error) = result {
        app.set_error(error);
    }
}
