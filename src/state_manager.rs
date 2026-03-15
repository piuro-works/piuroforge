use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::models::{Scene, StoryState};
use crate::utils::files::{read_string, write_string};

#[derive(Debug, Clone)]
pub struct StateManager {
    state_path: PathBuf,
}

impl StateManager {
    pub fn new(state_path: PathBuf) -> Self {
        Self { state_path }
    }

    pub fn ensure_state_file(&self) -> Result<()> {
        if !self.state_path.exists() {
            let state = StoryState::default();
            self.save_state(&state)?;
        }
        Ok(())
    }

    pub fn load_state(&self) -> Result<StoryState> {
        self.ensure_state_file()?;
        let content = read_string(&self.state_path)?;
        serde_json::from_str(&content)
            .with_context(|| format!("failed to parse {}", self.state_path.display()))
    }

    pub fn save_state(&self, state: &StoryState) -> Result<()> {
        let content =
            serde_json::to_string_pretty(state).context("failed to serialize story state")?;
        write_string(&self.state_path, &content)
    }

    pub fn next_scene_identity(&self, state: &StoryState) -> (u32, u32, String) {
        let chapter = state.current_chapter;
        let scene_number = state.current_scene + 1;
        let scene_id = format!("scene_{:03}_{:03}", chapter, scene_number);
        (chapter, scene_number, scene_id)
    }

    pub fn update_stage(&self, state: &mut StoryState, stage: impl Into<String>) {
        state.stage = stage.into();
    }

    pub fn update_current_scene_id(&self, state: &mut StoryState, scene_id: Option<String>) {
        state.current_scene_id = scene_id;
    }

    pub fn mark_scene_generated(&self, state: &mut StoryState, scene: &Scene) {
        state.current_chapter = scene.chapter;
        state.current_scene = scene.scene_number;
        state.current_goal = Some(scene.goal.clone());
        self.update_current_scene_id(state, Some(scene.id.clone()));
        self.update_stage(state, "scene_draft_ready");

        if !state
            .open_conflicts
            .iter()
            .any(|item| item == &scene.conflict)
        {
            state.open_conflicts.push(scene.conflict.clone());
        }
    }

    pub fn mark_scene_approved(&self, state: &mut StoryState, scene_id: &str) {
        if state.current_scene_id.as_deref() == Some(scene_id) {
            self.update_stage(state, "scene_approved");
        }
    }

    pub fn begin_next_chapter(&self, state: &mut StoryState) {
        state.current_chapter += 1;
        state.current_scene = 0;
        state.current_goal = None;
        state.current_scene_id = None;
        self.update_stage(state, "chapter_ready");
    }
}
