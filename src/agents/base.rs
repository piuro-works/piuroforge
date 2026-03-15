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

pub trait Agent {
    fn run(&self, context: &AgentContext) -> Result<String>;
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
