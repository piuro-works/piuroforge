use anyhow::Result;

use crate::config::NovelSettings;
use crate::models::{MemoryBundle, Scene, ScenePlan, StoryState};

#[derive(Debug, Clone)]
pub struct AgentContext {
    pub state: StoryState,
    pub novel: NovelSettings,
    pub memory: MemoryBundle,
    pub scene_plan: Option<ScenePlan>,
    pub scene: Option<Scene>,
    pub instruction: Option<String>,
    pub allow_dummy_fallback: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRun {
    pub output: String,
    pub fallback_warning: Option<String>,
}

impl AgentRun {
    pub fn direct(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            fallback_warning: None,
        }
    }

    pub fn fallback(output: impl Into<String>, warning: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            fallback_warning: Some(warning.into()),
        }
    }
}

pub trait Agent {
    fn run(&self, context: &AgentContext) -> Result<AgentRun>;
}

pub fn strip_code_fences(raw: &str) -> String {
    let trimmed = raw.trim();
    if !trimmed.starts_with("```") {
        return trimmed.to_string();
    }

    let mut lines = trimmed.lines();
    let _ = lines.next();
    let mut body = Vec::new();
    for line in lines {
        if line.trim_start().starts_with("```") {
            break;
        }
        body.push(line);
    }
    body.join("\n").trim().to_string()
}

pub fn fallback_warning(label: &str, error: &anyhow::Error) -> String {
    format!(
        "{} used dummy fallback because codex failed: {}",
        label,
        summarize_error(error)
    )
}

fn summarize_error(error: &anyhow::Error) -> String {
    let flattened = error
        .to_string()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    truncate_chars(flattened.trim(), 220)
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut rendered = String::new();
    for ch in value.chars().take(max_chars) {
        rendered.push(ch);
    }

    if value.chars().count() <= max_chars {
        rendered
    } else {
        format!("{}...", rendered.trim_end())
    }
}
