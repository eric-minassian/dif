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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchEntry {
    pub name: String,
    pub current: bool,
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
    let output = run_git(repo_root, &["status", "--porcelain=v1", "-z"])?;
    if !output.status.success() {
        bail!(git_error("list file changes", &output));
    }

    Ok(parse_porcelain_status(&output.stdout))
}

pub fn list_local_branches(repo_root: &Path) -> Result<Vec<BranchEntry>> {
    let output = run_git(
        repo_root,
        &[
            "branch",
            "--list",
            "--format=%(HEAD)\t%(refname:short)",
            "--sort=refname",
        ],
    )?;
    if !output.status.success() {
        bail!(git_error("list local branches", &output));
    }

    Ok(parse_branch_listing(&output.stdout))
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
                .args(["diff", "--no-index", "--", null_device_path()])
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

pub fn create_branch(repo_root: &Path, name: &str) -> Result<()> {
    if name.trim().is_empty() {
        bail!("branch name cannot be empty");
    }

    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["switch", "-c"])
        .arg(name)
        .output()
        .with_context(|| format!("failed to create branch `{name}`"))?;

    if !output.status.success() {
        bail!(git_error(&format!("create branch `{name}`"), &output));
    }

    Ok(())
}

pub fn switch_branch(repo_root: &Path, name: &str) -> Result<()> {
    if name.trim().is_empty() {
        bail!("branch name cannot be empty");
    }

    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["switch"])
        .arg(name)
        .output()
        .with_context(|| format!("failed to switch to branch `{name}`"))?;

    if !output.status.success() {
        bail!(git_error(&format!("switch to branch `{name}`"), &output));
    }

    Ok(())
}

pub fn delete_branch(repo_root: &Path, name: &str) -> Result<()> {
    if name.trim().is_empty() {
        bail!("branch name cannot be empty");
    }

    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["branch", "-d", "--"])
        .arg(name)
        .output()
        .with_context(|| format!("failed to delete branch `{name}`"))?;

    if !output.status.success() {
        bail!(git_error(&format!("delete branch `{name}`"), &output));
    }

    Ok(())
}

pub fn commit(repo_root: &Path, message: &str) -> Result<()> {
    if message.trim().is_empty() {
        bail!("commit message cannot be empty");
    }

    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["commit", "-m"])
        .arg(message)
        .output()
        .with_context(|| format!("failed to create commit `{message}`"))?;

    if !output.status.success() {
        bail!(git_error("create commit", &output));
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

fn parse_branch_listing(raw: &[u8]) -> Vec<BranchEntry> {
    let mut branches = Vec::new();
    let text = String::from_utf8_lossy(raw);

    for raw_line in text.lines() {
        let line = raw_line.trim_end();
        if line.is_empty() {
            continue;
        }

        let (current, name) = if let Some((head_marker, branch_name)) = line.split_once('\t') {
            (head_marker.trim() == "*", branch_name.trim())
        } else if let Some(branch_name) = line.strip_prefix("* ") {
            (true, branch_name.trim())
        } else if let Some(branch_name) = line.strip_prefix("  ") {
            (false, branch_name.trim())
        } else {
            (false, line.trim())
        };

        if name.is_empty() {
            continue;
        }

        branches.push(BranchEntry {
            name: name.to_owned(),
            current,
        });
    }

    branches
}

fn parse_porcelain_status(raw: &[u8]) -> RepoStatus {
    let records = parse_nul_terminated(raw);
    let mut staged = Vec::new();
    let mut unstaged = Vec::new();

    let mut idx = 0;
    while idx < records.len() {
        let entry = records[idx].as_str();
        idx += 1;

        if entry.len() < 3 {
            continue;
        }

        let bytes = entry.as_bytes();
        let x = bytes[0] as char;
        let y = bytes[1] as char;

        if x == '!' && y == '!' {
            continue;
        }

        if bytes.get(2).copied() != Some(b' ') {
            continue;
        }

        let mut path = entry[3..].to_owned();
        let rename_or_copy = matches!(x, 'R' | 'C') || matches!(y, 'R' | 'C');
        if rename_or_copy && idx < records.len() {
            path = records[idx].clone();
            idx += 1;
        }

        if x == '?' && y == '?' {
            unstaged.push(FileEntry {
                path,
                kind: UnstagedKind::Untracked,
            });
            continue;
        }

        if x != ' ' && x != '?' {
            staged.push(path.clone());
        }

        if y != ' ' {
            unstaged.push(FileEntry {
                path,
                kind: UnstagedKind::Tracked,
            });
        }
    }

    staged.sort_unstable();
    staged.dedup();

    unstaged.sort_unstable_by(|a, b| a.path.cmp(&b.path));
    unstaged.dedup_by(|a, b| a.path == b.path && a.kind == b.kind);

    RepoStatus { unstaged, staged }
}

fn null_device_path() -> &'static str {
    if cfg!(windows) { "NUL" } else { "/dev/null" }
}

fn git_error(action: &str, output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if stderr.is_empty() {
        format!("git failed to {action} (exit status: {})", output.status)
    } else {
        format!("git failed to {action}: {stderr}")
    }
}

#[cfg(test)]
mod tests {
    use super::{UnstagedKind, parse_branch_listing, parse_porcelain_status};

    #[test]
    fn parses_staged_unstaged_and_untracked_entries() {
        let raw = b"M  staged.txt\0 M unstaged.txt\0MM both.txt\0?? new.txt\0";
        let status = parse_porcelain_status(raw);

        assert_eq!(status.staged, vec!["both.txt", "staged.txt"]);
        assert_eq!(status.unstaged.len(), 3);
        assert_eq!(status.unstaged[0].path, "both.txt");
        assert_eq!(status.unstaged[0].kind, UnstagedKind::Tracked);
        assert_eq!(status.unstaged[1].path, "new.txt");
        assert_eq!(status.unstaged[1].kind, UnstagedKind::Untracked);
        assert_eq!(status.unstaged[2].path, "unstaged.txt");
        assert_eq!(status.unstaged[2].kind, UnstagedKind::Tracked);
    }

    #[test]
    fn uses_destination_path_for_renames() {
        let raw = b"R  old-name.txt\0new-name.txt\0";
        let status = parse_porcelain_status(raw);

        assert_eq!(status.staged, vec!["new-name.txt"]);
        assert!(status.unstaged.is_empty());
    }

    #[test]
    fn parses_branch_listing_with_head_markers() {
        let raw = b"*\tmain\n \tfeature/login\n";
        let branches = parse_branch_listing(raw);

        assert_eq!(branches.len(), 2);
        assert_eq!(branches[0].name, "main");
        assert!(branches[0].current);
        assert_eq!(branches[1].name, "feature/login");
        assert!(!branches[1].current);
    }

    #[test]
    fn parses_branch_listing_default_style_fallback() {
        let raw = b"* main\n  hotfix\n";
        let branches = parse_branch_listing(raw);

        assert_eq!(branches.len(), 2);
        assert_eq!(branches[0].name, "main");
        assert!(branches[0].current);
        assert_eq!(branches[1].name, "hotfix");
        assert!(!branches[1].current);
    }
}
