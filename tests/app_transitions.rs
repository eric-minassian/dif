use std::fs;
use std::path::Path;
use std::process::Command;

use dif::app::{App, FocusSection};
use tempfile::TempDir;

#[test]
fn switches_focus_between_unstaged_and_staged_lists() {
    let repo = setup_repo().expect("repo setup should succeed");
    let mut app = App::new(repo.path().to_path_buf()).expect("app should initialize");

    assert!(!app.unstaged.is_empty(), "expected unstaged entries");
    assert!(!app.staged.is_empty(), "expected staged entries");
    assert_eq!(app.focus, FocusSection::Unstaged);

    app.switch_focus().expect("switch to staged should work");
    assert_eq!(app.focus, FocusSection::Staged);

    app.switch_focus().expect("switch to unstaged should work");
    assert_eq!(app.focus, FocusSection::Unstaged);
}

#[test]
fn supports_undo_confirmation_state_transitions() {
    let repo = setup_repo().expect("repo setup should succeed");
    let mut app = App::new(repo.path().to_path_buf()).expect("app should initialize");

    app.undo_selected_to_mainline()
        .expect("starting undo flow should succeed");
    assert!(app.has_pending_undo_confirmation());

    app.cancel_pending_undo_to_mainline();
    assert!(!app.has_pending_undo_confirmation());
}

fn setup_repo() -> anyhow::Result<TempDir> {
    let temp = TempDir::new()?;
    let repo = temp.path();

    git(repo, &["init"])?;
    git(repo, &["config", "user.email", "dif-tests@example.com"])?;
    git(repo, &["config", "user.name", "dif-tests"])?;

    fs::write(repo.join("tracked.txt"), "line_a\n")?;
    git(repo, &["add", "tracked.txt"])?;
    git(repo, &["commit", "-m", "init"])?;

    fs::write(repo.join("tracked.txt"), "line_a\nline_b\n")?;
    fs::write(repo.join("staged.txt"), "staged\n")?;
    git(repo, &["add", "staged.txt"])?;
    fs::write(repo.join("untracked.txt"), "untracked\n")?;

    Ok(temp)
}

fn git(repo: &Path, args: &[&str]) -> anyhow::Result<()> {
    let output = Command::new("git").current_dir(repo).args(args).output()?;
    if !output.status.success() {
        anyhow::bail!(
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}
