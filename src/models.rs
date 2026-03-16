use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationResult<T> {
    pub value: T,
    pub warnings: Vec<String>,
}

impl<T> OperationResult<T> {
    pub fn new(value: T) -> Self {
        Self {
            value,
            warnings: Vec::new(),
        }
    }

    pub fn warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoryState {
    pub current_arc: u32,
    pub current_chapter: u32,
    pub current_scene: u32,
    pub stage: String,
    pub current_goal: Option<String>,
    pub open_conflicts: Vec<String>,
    pub current_scene_id: Option<String>,
}

impl Default for StoryState {
    fn default() -> Self {
        Self {
            current_arc: 1,
            current_chapter: 1,
            current_scene: 0,
            stage: "initialized".to_string(),
            current_goal: None,
            open_conflicts: Vec::new(),
            current_scene_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Scene {
    pub id: String,
    pub chapter: u32,
    pub scene_number: u32,
    #[serde(default)]
    pub short_title: String,
    pub goal: String,
    pub conflict: String,
    pub outcome: String,
    pub text: String,
    pub status: String,
}

impl Scene {
    pub fn effective_short_title(&self) -> String {
        if !self.short_title.trim().is_empty() {
            return self.short_title.trim().to_string();
        }

        derive_short_title(&self.goal)
    }

    pub fn file_name(&self) -> String {
        let slug = slug_fragment(&self.effective_short_title());
        if slug.is_empty() {
            return format!("{}.md", self.id);
        }

        format!("{}-{}.md", self.id, slug)
    }
}

pub(crate) fn slug_fragment(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for ch in value.chars() {
        if ch.is_alphanumeric() {
            slug.extend(ch.to_lowercase());
            last_was_dash = false;
        } else if !slug.is_empty() && !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }

        if slug.len() >= 48 {
            break;
        }
    }

    slug.trim_matches('-').to_string()
}

pub(crate) fn derive_short_title(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let words = trimmed
        .split_whitespace()
        .take(6)
        .map(|word| word.trim_matches(|ch: char| !ch.is_alphanumeric()))
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();

    if words.is_empty() {
        return trimmed.chars().take(32).collect();
    }

    words.join(" ")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewIssue {
    #[serde(default)]
    pub issue_type: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub line_start: Option<u32>,
    #[serde(default)]
    pub line_end: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct MemoryBundle {
    pub core_memory: String,
    pub story_memory: String,
    pub active_memory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ScenePlan {
    #[serde(default)]
    pub chapter: u32,
    #[serde(default)]
    pub scene_number: u32,
    #[serde(default)]
    pub short_title: String,
    #[serde(default)]
    pub goal: String,
    #[serde(default)]
    pub conflict: String,
    #[serde(default)]
    pub outcome: String,
}

impl ScenePlan {
    pub fn scene_id(&self) -> String {
        format!("scene_{:03}_{:03}", self.chapter, self.scene_number)
    }

    pub fn effective_short_title(&self) -> String {
        if !self.short_title.trim().is_empty() {
            return self.short_title.trim().to_string();
        }

        derive_short_title(&self.goal)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceManifest {
    pub kind: String,
    pub version: u32,
    pub name: String,
    pub created_by: String,
    pub engine_version: String,
    pub workspace_config: String,
}

impl Default for WorkspaceManifest {
    fn default() -> Self {
        Self {
            kind: "heeforge-workspace".to_string(),
            version: 1,
            name: "heeforge-workspace".to_string(),
            created_by: "heeforge".to_string(),
            engine_version: env!("CARGO_PKG_VERSION").to_string(),
            workspace_config: "novel.toml".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SceneGenerationLog {
    pub timestamp_unix_secs: u64,
    pub scene_id: String,
    pub planner_output: String,
    #[serde(default)]
    pub planner_fallback_warning: Option<String>,
    pub writer_output: String,
    #[serde(default)]
    pub writer_fallback_warning: Option<String>,
    pub editor_output: String,
    #[serde(default)]
    pub editor_fallback_warning: Option<String>,
    pub final_scene: Scene,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewReport {
    pub timestamp_unix_secs: u64,
    pub scene_id: String,
    #[serde(default)]
    pub critic_fallback_warning: Option<String>,
    pub issues: Vec<ReviewIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RewriteRecord {
    pub timestamp_unix_secs: u64,
    pub scene_id: String,
    pub instruction: String,
    pub revision: u32,
    #[serde(default)]
    pub editor_fallback_warning: Option<String>,
    pub original_snapshot_path: String,
    pub rewritten_snapshot_path: String,
}
