use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceGitOutcome {
    Disabled,
    NoChanges,
    Committed {
        revision: String,
        initialized_repo: bool,
    },
    Failed {
        reason: String,
    },
}

#[derive(Debug, Clone)]
pub struct WorkspaceGit {
    workspace_dir: PathBuf,
    enabled: bool,
}

impl WorkspaceGit {
    pub fn new(workspace_dir: impl Into<PathBuf>, enabled: bool) -> Self {
        Self {
            workspace_dir: workspace_dir.into(),
            enabled,
        }
    }

    pub fn auto_commit(&self, message: &str) -> WorkspaceGitOutcome {
        if !self.enabled {
            return WorkspaceGitOutcome::Disabled;
        }

        match self.auto_commit_inner(message) {
            Ok(outcome) => outcome,
            Err(error) => WorkspaceGitOutcome::Failed {
                reason: error.to_string(),
            },
        }
    }

    fn auto_commit_inner(&self, message: &str) -> Result<WorkspaceGitOutcome> {
        let initialized_repo = self.ensure_repo()?;
        self.run_git(["add", "-A", "."])?;

        let status = self.run_git(["status", "--porcelain"])?;
        if status.trim().is_empty() {
            return Ok(WorkspaceGitOutcome::NoChanges);
        }

        self.run_git(["commit", "-m", message])?;
        let revision = self.run_git(["rev-parse", "--short", "HEAD"])?;

        Ok(WorkspaceGitOutcome::Committed {
            revision: revision.trim().to_string(),
            initialized_repo,
        })
    }

    fn ensure_repo(&self) -> Result<bool> {
        if git_dir_exists(&self.workspace_dir) {
            return Ok(false);
        }

        self.run_git(["init"])?;
        Ok(true)
    }

    fn run_git<const N: usize>(&self, args: [&str; N]) -> Result<String> {
        let mut command = Command::new("git");
        command.current_dir(&self.workspace_dir);
        command.args(args);
        command.env("GIT_AUTHOR_NAME", "piuroforge");
        command.env("GIT_AUTHOR_EMAIL", "piuroforge@local");
        command.env("GIT_COMMITTER_NAME", "piuroforge");
        command.env("GIT_COMMITTER_EMAIL", "piuroforge@local");

        let output = command
            .output()
            .with_context(|| format!("failed to run git in {}", self.workspace_dir.display()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                format!("exit status {}", output.status)
            };
            return Err(anyhow!(
                "git {} failed in {}: {}",
                args.join(" "),
                self.workspace_dir.display(),
                detail
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

fn git_dir_exists(workspace_dir: &Path) -> bool {
    workspace_dir.join(".git").exists()
}
