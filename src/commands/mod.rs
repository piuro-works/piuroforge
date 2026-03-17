use anyhow::{anyhow, Result};

pub mod approve;
pub mod capabilities;
pub mod doctor;
pub mod expand_world;
pub mod init;
pub mod memory;
pub mod next_bundle;
pub mod next_scene;
pub mod polish;
pub mod proofread;
pub mod review;
pub mod rewrite;
pub mod show;
pub mod status;

use crate::engine::NovelEngine;
use crate::output::CommandOutput;
use crate::workspace_git::WorkspaceGitOutcome;

fn workspace_command(engine: &NovelEngine, args: &str) -> String {
    format!(
        "piuroforge --workspace {} {}",
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

fn resolve_target_scene_id(
    engine: &NovelEngine,
    scene_id: Option<&str>,
    action: &str,
) -> Result<String> {
    if let Some(scene_id) = scene_id {
        return Ok(scene_id.to_string());
    }

    engine
        .get_status()?
        .current_scene_id
        .ok_or_else(|| anyhow!("no current scene available to {action}"))
}

fn sentence_list<T: AsRef<str>>(items: &[T]) -> String {
    items
        .iter()
        .map(|item| item.as_ref().trim())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}
