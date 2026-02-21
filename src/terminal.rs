use std::env;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use vt100::Parser;

const SCROLLBACK_LEN: usize = 20_000;

pub struct TerminalSession {
    parser: Parser,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    output_rx: Receiver<Vec<u8>>,
    rows: u16,
    cols: u16,
    exited: bool,
}

struct ShellCommand {
    program: String,
    args: Vec<String>,
}

impl TerminalSession {
    pub fn start(cwd: &Path, rows: u16, cols: u16) -> Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let shell = shell_command();
        let mut command = CommandBuilder::new(shell.program);
        for arg in shell.args {
            command.arg(arg);
        }
        command.cwd(cwd);

        let child = pair
            .slave
            .spawn_command(command)
            .context("failed to spawn interactive shell in PTY")?;

        let mut reader = pair
            .master
            .try_clone_reader()
            .context("failed to clone PTY reader")?;
        let writer = pair
            .master
            .take_writer()
            .context("failed to acquire PTY writer")?;

        let (output_tx, output_rx) = mpsc::channel::<Vec<u8>>();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(read) => {
                        if output_tx.send(buf[..read].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            parser: Parser::new(rows, cols, SCROLLBACK_LEN),
            master: pair.master,
            writer,
            child,
            output_rx,
            rows,
            cols,
            exited: false,
        })
    }

    pub fn pump_output(&mut self) -> bool {
        let mut updated = false;

        while let Ok(chunk) = self.output_rx.try_recv() {
            self.parser.process(&chunk);
            updated = true;
        }

        updated
    }

    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        if rows == 0 || cols == 0 {
            return Ok(());
        }
        if rows == self.rows && cols == self.cols {
            return Ok(());
        }

        self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        self.parser.set_size(rows, cols);
        self.rows = rows;
        self.cols = cols;
        Ok(())
    }

    pub fn send_key(&mut self, key: KeyEvent) -> Result<()> {
        let Some(bytes) = encode_key_event(key) else {
            return Ok(());
        };

        self.writer
            .write_all(&bytes)
            .context("failed to write key to PTY")?;
        self.writer.flush().ok();
        Ok(())
    }

    pub fn send_text(&mut self, text: &str) -> Result<()> {
        self.writer
            .write_all(text.as_bytes())
            .context("failed to write text to PTY")?;
        self.writer.flush().ok();
        Ok(())
    }

    pub fn set_scrollback(&mut self, rows: usize) {
        self.parser.set_scrollback(rows);
    }

    pub fn scrollback(&self) -> usize {
        self.parser.screen().scrollback()
    }

    pub fn screen(&self) -> &vt100::Screen {
        self.parser.screen()
    }

    pub fn is_exited(&self) -> bool {
        self.exited
    }

    pub fn poll_exit_message(&mut self) -> Result<Option<String>> {
        if self.exited {
            return Ok(None);
        }

        let maybe_status = self
            .child
            .try_wait()
            .context("failed checking PTY child status")?;
        if let Some(status) = maybe_status {
            self.exited = true;
            return Ok(Some(status.to_string()));
        }

        Ok(None)
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn shell_command() -> ShellCommand {
    let env_shell = env::var("SHELL").ok().filter(|value| !value.is_empty());

    let preferred = env_shell
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .or_else(default_shell_program)
        .unwrap_or_else(fallback_shell_program);

    let program = preferred.to_string_lossy().into_owned();
    let args = if cfg!(windows) {
        shell_args_for_windows(&program)
    } else {
        vec![String::from("-i")]
    };

    ShellCommand { program, args }
}

fn default_shell_program() -> Option<PathBuf> {
    if cfg!(windows) {
        for candidate in [
            PathBuf::from("C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe"),
            PathBuf::from("C:/Windows/System32/cmd.exe"),
        ] {
            if candidate.exists() {
                return Some(candidate);
            }
        }
        None
    } else {
        for candidate in [PathBuf::from("/bin/zsh"), PathBuf::from("/bin/bash")] {
            if candidate.exists() {
                return Some(candidate);
            }
        }
        None
    }
}

fn fallback_shell_program() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from("cmd.exe")
    } else {
        PathBuf::from("/bin/sh")
    }
}

fn shell_args_for_windows(program: &str) -> Vec<String> {
    let lower = program.to_ascii_lowercase();
    if lower.ends_with("powershell.exe") || lower.ends_with("pwsh.exe") {
        vec![String::from("-NoLogo"), String::from("-NoExit")]
    } else if lower.ends_with("cmd.exe") {
        vec![String::from("/K")]
    } else {
        Vec::new()
    }
}

fn encode_key_event(event: KeyEvent) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();

    let has_alt = event.modifiers.contains(KeyModifiers::ALT);
    let has_ctrl = event.modifiers.contains(KeyModifiers::CONTROL);

    match event.code {
        KeyCode::Enter => bytes.push(b'\r'),
        KeyCode::Tab => {
            if event.modifiers.contains(KeyModifiers::SHIFT) {
                bytes.extend_from_slice(b"\x1b[Z");
            } else {
                bytes.push(b'\t');
            }
        }
        KeyCode::BackTab => bytes.extend_from_slice(b"\x1b[Z"),
        KeyCode::Backspace => bytes.push(0x7f),
        KeyCode::Esc => bytes.push(0x1b),
        KeyCode::Left => bytes.extend_from_slice(b"\x1b[D"),
        KeyCode::Right => bytes.extend_from_slice(b"\x1b[C"),
        KeyCode::Up => bytes.extend_from_slice(b"\x1b[A"),
        KeyCode::Down => bytes.extend_from_slice(b"\x1b[B"),
        KeyCode::Home => bytes.extend_from_slice(b"\x1b[H"),
        KeyCode::End => bytes.extend_from_slice(b"\x1b[F"),
        KeyCode::PageUp => bytes.extend_from_slice(b"\x1b[5~"),
        KeyCode::PageDown => bytes.extend_from_slice(b"\x1b[6~"),
        KeyCode::Insert => bytes.extend_from_slice(b"\x1b[2~"),
        KeyCode::Delete => bytes.extend_from_slice(b"\x1b[3~"),
        KeyCode::F(1) => bytes.extend_from_slice(b"\x1bOP"),
        KeyCode::F(2) => bytes.extend_from_slice(b"\x1bOQ"),
        KeyCode::F(3) => bytes.extend_from_slice(b"\x1bOR"),
        KeyCode::F(4) => bytes.extend_from_slice(b"\x1bOS"),
        KeyCode::F(5) => bytes.extend_from_slice(b"\x1b[15~"),
        KeyCode::F(6) => bytes.extend_from_slice(b"\x1b[17~"),
        KeyCode::F(7) => bytes.extend_from_slice(b"\x1b[18~"),
        KeyCode::F(8) => bytes.extend_from_slice(b"\x1b[19~"),
        KeyCode::F(9) => bytes.extend_from_slice(b"\x1b[20~"),
        KeyCode::F(10) => bytes.extend_from_slice(b"\x1b[21~"),
        KeyCode::F(11) => bytes.extend_from_slice(b"\x1b[23~"),
        KeyCode::F(12) => bytes.extend_from_slice(b"\x1b[24~"),
        KeyCode::Char(ch) if has_ctrl => {
            let ctrl = ctrl_code(ch)?;
            bytes.push(ctrl);
        }
        KeyCode::Char(ch) => {
            let mut tmp = [0u8; 4];
            let encoded = ch.encode_utf8(&mut tmp);
            bytes.extend_from_slice(encoded.as_bytes());
        }
        _ => return None,
    }

    if has_alt && !bytes.is_empty() {
        bytes.insert(0, 0x1b);
    }

    Some(bytes)
}

fn ctrl_code(ch: char) -> Option<u8> {
    match ch {
        ' ' | '2' | '@' => Some(0),
        'a'..='z' => Some((ch as u8 - b'a') + 1),
        'A'..='Z' => Some((ch as u8 - b'A') + 1),
        '3' | '[' => Some(27),
        '4' | '\\' => Some(28),
        '5' | ']' => Some(29),
        '6' | '^' => Some(30),
        '7' | '/' | '_' => Some(31),
        '8' | '?' => Some(127),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::{ctrl_code, encode_key_event};

    #[test]
    fn encodes_ctrl_character() {
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(encode_key_event(key), Some(vec![3]));
    }

    #[test]
    fn encodes_alt_character_with_escape_prefix() {
        let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT);
        assert_eq!(encode_key_event(key), Some(vec![0x1b, b'x']));
    }

    #[test]
    fn encodes_shift_tab_sequence() {
        let key = KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT);
        assert_eq!(encode_key_event(key), Some(b"\x1b[Z".to_vec()));
    }

    #[test]
    fn maps_ctrl_code_variants() {
        assert_eq!(ctrl_code('a'), Some(1));
        assert_eq!(ctrl_code('A'), Some(1));
        assert_eq!(ctrl_code(']'), Some(29));
        assert_eq!(ctrl_code('~'), None);
    }
}
