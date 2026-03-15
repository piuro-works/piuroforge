use serde::{Deserialize, Serialize};

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
    pub goal: String,
    pub conflict: String,
    pub outcome: String,
    pub text: String,
    pub status: String,
}

impl Scene {
    pub fn file_name(&self) -> String {
        format!("{}.md", self.id)
    }
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
            kind: "novel-engine-workspace".to_string(),
            version: 1,
            name: "novel-workspace".to_string(),
            created_by: "heeforge/novel_engine".to_string(),
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
    pub writer_output: String,
    pub editor_output: String,
    pub final_scene: Scene,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewReport {
    pub timestamp_unix_secs: u64,
    pub scene_id: String,
    pub issues: Vec<ReviewIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RewriteRecord {
    pub timestamp_unix_secs: u64,
    pub scene_id: String,
    pub instruction: String,
    pub revision: u32,
    pub original_snapshot_path: String,
    pub rewritten_snapshot_path: String,
}
