use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnstagedKind {
    Tracked,
    Untracked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub path: String,
    pub kind: UnstagedKind,
}

#[derive(Debug, Clone, Default)]
pub struct RepoStatus {
    pub unstaged: Vec<FileEntry>,
    pub staged: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffMode {
    UnstagedTracked,
    Untracked,
    Staged,
}

pub fn repo_root() -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("failed to run `git rev-parse --show-toplevel`")?;

    if !output.status.success() {
        bail!(git_error("determine git repo root", &output));
    }

    let root = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if root.is_empty() {
        bail!("git reported an empty repo root");
    }

    Ok(PathBuf::from(root))
}

pub fn status(repo_root: &Path) -> Result<RepoStatus> {
    let mut staged = run_z_list(repo_root, &["diff", "--cached", "--name-only", "-z"])?;
    staged.sort_unstable();
    staged.dedup();

    let mut unstaged_tracked = run_z_list(repo_root, &["diff", "--name-only", "-z"])?;
    unstaged_tracked.sort_unstable();
    unstaged_tracked.dedup();

    let mut untracked = run_z_list(
        repo_root,
        &["ls-files", "--others", "--exclude-standard", "-z"],
    )?;
    untracked.sort_unstable();
    untracked.dedup();

    let tracked_set: HashSet<String> = unstaged_tracked.iter().cloned().collect();
    let mut unstaged = Vec::with_capacity(unstaged_tracked.len() + untracked.len());

    for path in unstaged_tracked {
        unstaged.push(FileEntry {
            path,
            kind: UnstagedKind::Tracked,
        });
    }

    for path in untracked {
        if tracked_set.contains(path.as_str()) {
            continue;
        }
        unstaged.push(FileEntry {
            path,
            kind: UnstagedKind::Untracked,
        });
    }

    unstaged.sort_unstable_by(|a, b| a.path.cmp(&b.path));

    Ok(RepoStatus { unstaged, staged })
}

pub fn diff_for_file(repo_root: &Path, path: &str, mode: DiffMode) -> Result<String> {
    let mut command = Command::new("git");
    command.current_dir(repo_root);

    match mode {
        DiffMode::UnstagedTracked => {
            command.args(["diff", "--"]).arg(path);
        }
        DiffMode::Untracked => {
            command
                .args(["diff", "--no-index", "--", "/dev/null"])
                .arg(path);
        }
        DiffMode::Staged => {
            command.args(["diff", "--cached", "--"]).arg(path);
        }
    }

    let output = command
        .output()
        .with_context(|| format!("failed to diff `{path}`"))?;

    let is_expected_untracked_exit = mode == DiffMode::Untracked && output.status.code() == Some(1);
    if !output.status.success() && !is_expected_untracked_exit {
        bail!(git_error(&format!("diff `{path}`"), &output));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn stage_file(repo_root: &Path, path: &str) -> Result<()> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["add", "--"])
        .arg(path)
        .output()
        .with_context(|| format!("failed to stage `{path}`"))?;

    if !output.status.success() {
        bail!(git_error(&format!("stage `{path}`"), &output));
    }

    Ok(())
}

pub fn unstage_file(repo_root: &Path, path: &str) -> Result<()> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["restore", "--staged", "--"])
        .arg(path)
        .output()
        .with_context(|| format!("failed to unstage `{path}`"))?;

    if !output.status.success() {
        bail!(git_error(&format!("unstage `{path}`"), &output));
    }

    Ok(())
}

pub fn undo_file_to_mainline(repo_root: &Path, path: &str, was_untracked: bool) -> Result<String> {
    let mainline = resolve_mainline_ref(repo_root)?;

    let restore_output = Command::new("git")
        .current_dir(repo_root)
        .args(["restore", "--source"])
        .arg(mainline.as_str())
        .args(["--staged", "--worktree", "--"])
        .arg(path)
        .output()
        .with_context(|| format!("failed to restore `{path}` from `{mainline}`"))?;

    if restore_output.status.success() {
        return Ok(mainline);
    }

    if was_untracked {
        let clean_output = Command::new("git")
            .current_dir(repo_root)
            .args(["clean", "-f", "--"])
            .arg(path)
            .output()
            .with_context(|| format!("failed to remove untracked `{path}`"))?;

        if clean_output.status.success() {
            return Ok(mainline);
        }

        bail!(git_error(
            &format!("remove untracked `{path}`"),
            &clean_output
        ));
    }

    bail!(git_error(
        &format!("restore `{path}` from `{mainline}`"),
        &restore_output
    ));
}

fn run_z_list(repo_root: &Path, args: &[&str]) -> Result<Vec<String>> {
    let output = run_git(repo_root, args)?;
    if !output.status.success() {
        bail!(git_error("list file changes", &output));
    }

    Ok(parse_nul_terminated(&output.stdout))
}

fn run_git(repo_root: &Path, args: &[&str]) -> Result<Output> {
    Command::new("git")
        .current_dir(repo_root)
        .args(args)
        .output()
        .with_context(|| format!("failed to run `git {}`", args.join(" ")))
}

fn resolve_mainline_ref(repo_root: &Path) -> Result<String> {
    let origin_head = run_git(
        repo_root,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    )?;

    if origin_head.status.success() {
        let branch = String::from_utf8_lossy(&origin_head.stdout)
            .trim()
            .to_owned();
        if !branch.is_empty() && ref_exists(repo_root, branch.as_str())? {
            return Ok(branch);
        }
    }

    for candidate in ["origin/main", "main", "origin/master", "master", "HEAD"] {
        if ref_exists(repo_root, candidate)? {
            return Ok(candidate.to_owned());
        }
    }

    Ok(String::from("HEAD"))
}

fn ref_exists(repo_root: &Path, reference: &str) -> Result<bool> {
    let output = run_git(
        repo_root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{reference}^{{commit}}"),
        ],
    )?;

    Ok(output.status.success())
}

fn parse_nul_terminated(raw: &[u8]) -> Vec<String> {
    raw.split(|byte| *byte == 0)
        .filter(|chunk| !chunk.is_empty())
        .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
        .collect()
}

fn git_error(action: &str, output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if stderr.is_empty() {
        format!("git failed to {action} (exit status: {})", output.status)
    } else {
        format!("git failed to {action}: {stderr}")
    }
}
