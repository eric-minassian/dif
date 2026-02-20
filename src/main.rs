mod app;
mod diff;
mod git;
mod highlight;
mod input;
mod settings;
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
    let poll_timeout = Duration::from_millis(120);

    loop {
        terminal.draw(|frame| ui::render(frame, app, highlighter))?;

        if event::poll(poll_timeout)? {
            let next_event = event::read()?;
            if !input::handle_event(app, next_event) {
                break;
            }
        }
    }

    Ok(())
}
