use std::io;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;

use dif::app::App;
use dif::git;
use dif::highlight::Highlighter;
use dif::input;
use dif::ui;

fn main() -> Result<()> {
    let repo_root = git::repo_root()?;
    let mut app = App::new(repo_root)?;
    let highlighter = Highlighter::new()?;
    let mut terminal_guard = TerminalGuard::new()?;

    let run_result = run_app(terminal_guard.terminal_mut(), &mut app, &highlighter);
    let settings_result = app.flush_pending_settings();

    drop(terminal_guard);

    run_result.and(settings_result)
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    highlighter: &Highlighter,
) -> Result<()> {
    let poll_timeout_idle = Duration::from_millis(80);
    let poll_timeout_terminal = Duration::from_millis(8);
    let drain_timeout = Duration::from_millis(0);
    let mut needs_draw = true;

    loop {
        if app.tick() {
            needs_draw = true;
        }

        let timeout = if app.terminal_open {
            poll_timeout_terminal
        } else {
            poll_timeout_idle
        };

        if event::poll(timeout)? {
            loop {
                let next_event = event::read()?;
                if !input::handle_event(app, next_event) {
                    return Ok(());
                }
                needs_draw = true;

                if !event::poll(drain_timeout)? {
                    break;
                }
            }
        }

        if needs_draw {
            let size = terminal.size()?;
            let root = Rect::new(0, 0, size.width, size.height);
            if let Err(error) = app.update_layout(root) {
                app.set_error(error);
            }

            terminal.draw(|frame| ui::render(frame, app, highlighter))?;
            needs_draw = false;
        }
    }
}

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl TerminalGuard {
    fn new() -> Result<Self> {
        enable_raw_mode()?;

        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen, EnableMouseCapture) {
            let _ = disable_raw_mode();
            return Err(error.into());
        }

        match Terminal::new(CrosstermBackend::new(stdout)) {
            Ok(terminal) => Ok(Self { terminal }),
            Err(error) => {
                let _ = disable_raw_mode();
                Err(error.into())
            }
        }
    }

    fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<io::Stdout>> {
        &mut self.terminal
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
        let _ = self.terminal.show_cursor();
    }
}
