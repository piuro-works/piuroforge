pub mod approve;
pub mod doctor;
pub mod expand_world;
pub mod init;
pub mod memory;
pub mod next_chapter;
pub mod next_scene;
pub mod review;
pub mod rewrite;
pub mod show;
pub mod status;

use crate::engine::NovelEngine;
use crate::output::CommandOutput;
use crate::workspace_git::WorkspaceGitOutcome;

fn workspace_command(engine: &NovelEngine, args: &str) -> String {
    format!(
        "heeforge --workspace {} {}",
        engine.workspace_dir().display(),
        args
    )
}

fn finalize_workspace_change(
    engine: &NovelEngine,
    output: CommandOutput,
    commit_message: &str,
) -> CommandOutput {
    match engine.auto_commit_workspace(commit_message) {
        WorkspaceGitOutcome::Disabled | WorkspaceGitOutcome::NoChanges => output,
        WorkspaceGitOutcome::Committed {
            revision,
            initialized_repo,
        } => {
            let output = output.detail("git_commit", revision);
            if initialized_repo {
                output.detail("workspace_git", "initialized")
            } else {
                output
            }
        }
        WorkspaceGitOutcome::Failed { reason } => output.warning(format!(
            "Workspace auto-commit failed, but the command result was preserved: {reason}"
        )),
    }
}

fn sentence_list<T: AsRef<str>>(items: &[T]) -> String {
    items
        .iter()
        .map(|item| item.as_ref().trim())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}
