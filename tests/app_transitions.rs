use std::fs;
use std::path::Path;
use std::process::Command;

use dif::app::{App, FocusSection, GitPanelMode};
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

#[test]
fn creates_switches_and_deletes_branches() {
    let repo = setup_repo().expect("repo setup should succeed");
    let mut app = App::new(repo.path().to_path_buf()).expect("app should initialize");

    let starting_branch = app
        .current_branch_name()
        .expect("starting branch should exist")
        .to_owned();

    app.toggle_git_panel().expect("git panel should open");
    app.open_branch_create_prompt();
    for ch in "feature/ui".chars() {
        app.git_branch_input_append(ch);
    }
    app.submit_new_branch()
        .expect("creating and switching branch should succeed");

    assert_eq!(app.current_branch_name(), Some("feature/ui"));
    assert!(
        app.branches
            .iter()
            .any(|branch| branch.name == "feature/ui")
    );

    let start_idx = app
        .branches
        .iter()
        .position(|branch| branch.name == starting_branch)
        .expect("starting branch should still exist");
    app.branch_selected = Some(start_idx);
    app.switch_to_selected_branch()
        .expect("switching back should succeed");
    assert_eq!(app.current_branch_name(), Some(starting_branch.as_str()));

    let feature_idx = app
        .branches
        .iter()
        .position(|branch| branch.name == "feature/ui")
        .expect("feature branch should still exist");
    app.branch_selected = Some(feature_idx);
    app.request_delete_selected_branch();
    assert_eq!(app.git_panel_mode, GitPanelMode::ConfirmDeleteBranch);
    app.confirm_delete_selected_branch()
        .expect("deleting branch should succeed");

    assert!(
        !app.branches
            .iter()
            .any(|branch| branch.name == "feature/ui")
    );
}

#[test]
fn creates_multiline_commit_with_template() {
    let repo = setup_repo().expect("repo setup should succeed");

    let template_path = repo.path().join(".gitmessage.txt");
    fs::write(&template_path, "# Summary line\n\n# Body\n# - detail\n")
        .expect("template write should succeed");
    git(
        repo.path(),
        &[
            "config",
            "commit.template",
            template_path
                .to_str()
                .expect("template path should be utf-8"),
        ],
    )
    .expect("setting commit template should succeed");

    let mut app = App::new(repo.path().to_path_buf()).expect("app should initialize");

    app.open_commit_prompt()
        .expect("commit prompt should open successfully");
    assert!(
        app.git_commit_input.contains("# Summary line"),
        "template should prefill commit input"
    );

    app.git_commit_input_append_text("add staged file\n\nBody line 1\nBody line 2\n");
    app.submit_commit().expect("commit should succeed");

    assert!(
        app.staged.is_empty(),
        "staged list should be empty after commit"
    );

    let output = git_output(repo.path(), &["log", "-1", "--pretty=%s"])
        .expect("git log should return latest commit subject");
    assert_eq!(output.trim(), "add staged file");

    let full_message = git_output(repo.path(), &["log", "-1", "--pretty=%B"])
        .expect("git log should return full commit message");
    assert_eq!(
        full_message.trim_end(),
        "add staged file\n\nBody line 1\nBody line 2"
    );
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

fn git_output(repo: &Path, args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new("git").current_dir(repo).args(args).output()?;
    if !output.status.success() {
        anyhow::bail!(
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
