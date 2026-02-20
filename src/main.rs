mod app;
mod diff;
mod git;
mod highlight;
mod input;
mod settings;
mod terminal;
mod ui;

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

use crate::app::App;
use crate::highlight::Highlighter;

fn main() -> Result<()> {
    let repo_root = git::repo_root()?;
    let mut app = App::new(repo_root)?;
    let highlighter = Highlighter::new();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let run_result = run_app(&mut terminal, &mut app, &highlighter);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    run_result
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    highlighter: &Highlighter,
) -> Result<()> {
    let poll_timeout_idle = Duration::from_millis(80);
    let poll_timeout_terminal = Duration::from_millis(8);
    let drain_timeout = Duration::from_millis(0);

    loop {
        app.tick();
        terminal.draw(|frame| ui::render(frame, app, highlighter))?;

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

                if !event::poll(drain_timeout)? {
                    break;
                }
            }
        }
    }
}
