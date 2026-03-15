pub mod approve;
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

fn workspace_command(engine: &NovelEngine, args: &str) -> String {
    format!(
        "novel --workspace {} {}",
        engine.workspace_dir().display(),
        args
    )
}

fn sentence_list<T: AsRef<str>>(items: &[T]) -> String {
    items
        .iter()
        .map(|item| item.as_ref().trim())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}
