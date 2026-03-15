use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::agents::base::{strip_code_fences, Agent, AgentContext};
use crate::agents::critic::CriticAgent;
use crate::agents::editor::EditorAgent;
use crate::agents::planner::PlannerAgent;
use crate::agents::writer::WriterAgent;
use crate::codex_runner::CodexRunner;
use crate::config::Config;
use crate::memory_manager::MemoryManager;
use crate::models::{
    MemoryBundle, ReviewIssue, ReviewReport, RewriteRecord, Scene, SceneGenerationLog, ScenePlan,
    StoryState, WorkspaceManifest,
};
use crate::state_manager::StateManager;
use crate::utils::files::{ensure_dir, list_markdown_files, read_string, write_string};
use crate::utils::markdown::{parse_scene, render_chapter, render_scene};

#[derive(Debug, Clone)]
pub struct NovelEngine {
    config: Config,
    state_manager: StateManager,
    memory_manager: MemoryManager,
    codex_runner: CodexRunner,
}

impl NovelEngine {
    pub fn new(config: Config) -> Result<Self> {
        let state_manager = StateManager::new(config.state_path.clone());
        let memory_manager = MemoryManager::new(config.memory_dir.clone());
        let codex_runner = CodexRunner::new(config.codex_command.clone());

        Ok(Self {
            config,
            state_manager,
            memory_manager,
            codex_runner,
        })
    }

    pub fn init_project(&self) -> Result<()> {
        self.ensure_layout()?;
        self.ensure_global_config_file()?;
        self.ensure_workspace_config_file()?;
        self.ensure_workspace_manifest()?;
        self.ensure_workspace_gitignore()?;
        self.state_manager.ensure_state_file()?;
        self.memory_manager.ensure_files()?;
        Ok(())
    }

    pub fn bootstrap_workspace(&self) -> Result<()> {
        self.ensure_layout()?;
        self.ensure_global_config_file()?;
        self.write_workspace_config_file()?;
        self.write_workspace_manifest()?;
        self.ensure_workspace_gitignore()?;
        self.state_manager.ensure_state_file()?;
        self.memory_manager.ensure_files()?;
        Ok(())
    }

    pub fn get_status(&self) -> Result<StoryState> {
        self.init_project()?;
        self.state_manager.load_state()
    }

    pub fn generate_next_scene(&self) -> Result<Scene> {
        self.init_project()?;
        self.ensure_generation_ready()?;

        let mut state = self.state_manager.load_state()?;
        let memory = self.memory_manager.load_bundle()?;
        let (chapter, scene_number, scene_id) = self.state_manager.next_scene_identity(&state);

        let planner = PlannerAgent::new(self.codex_runner.clone(), true);
        let planner_context = AgentContext {
            state: state.clone(),
            novel: self.config.novel_settings.clone(),
            memory: memory.clone(),
            scene_plan: None,
            scene: None,
            instruction: None,
            allow_dummy_fallback: self.config.allow_dummy_fallback,
        };
        let planner_output = planner.run(&planner_context)?;
        let mut scene_plan = self.parse_scene_plan(&planner_output, chapter, scene_number);
        scene_plan.chapter = chapter;
        scene_plan.scene_number = scene_number;

        let writer = WriterAgent::new(self.codex_runner.clone(), true);
        let writer_context = AgentContext {
            state: state.clone(),
            novel: self.config.novel_settings.clone(),
            memory: memory.clone(),
            scene_plan: Some(scene_plan.clone()),
            scene: None,
            instruction: None,
            allow_dummy_fallback: self.config.allow_dummy_fallback,
        };
        let writer_output = writer.run(&writer_context)?;

        let draft_scene = Scene {
            id: scene_id,
            chapter,
            scene_number,
            goal: scene_plan.goal.clone(),
            conflict: scene_plan.conflict.clone(),
            outcome: scene_plan.outcome.clone(),
            text: writer_output.clone(),
            status: "draft".to_string(),
        };

        let editor = EditorAgent::new(self.codex_runner.clone(), true);
        let editor_context = AgentContext {
            state: state.clone(),
            novel: self.config.novel_settings.clone(),
            memory: memory.clone(),
            scene_plan: Some(scene_plan),
            scene: Some(draft_scene.clone()),
            instruction: None,
            allow_dummy_fallback: self.config.allow_dummy_fallback,
        };
        let editor_output = editor.run(&editor_context)?;

        let final_scene = Scene {
            text: editor_output.clone(),
            ..draft_scene
        };

        self.save_scene(&final_scene)?;
        self.save_scene_generation_log(SceneGenerationLog {
            timestamp_unix_secs: unix_timestamp_secs(),
            scene_id: final_scene.id.clone(),
            planner_output,
            writer_output,
            editor_output,
            final_scene: final_scene.clone(),
        })?;
        self.state_manager
            .mark_scene_generated(&mut state, &final_scene);
        self.state_manager.save_state(&state)?;

        self.memory_manager
            .overwrite_active_memory(&self.render_active_memory(&state, &final_scene))?;
        self.memory_manager
            .append_story_memory(&self.render_story_memory_entry(&final_scene))?;

        Ok(final_scene)
    }

    pub fn review_current_scene(&self) -> Result<Vec<ReviewIssue>> {
        self.init_project()?;

        let state = self.state_manager.load_state()?;
        let scene_id = state
            .current_scene_id
            .clone()
            .ok_or_else(|| anyhow!("no current scene available to review"))?;
        let scene = self.show_scene(&scene_id)?;
        let memory = self.memory_manager.load_bundle()?;
        let critic = CriticAgent::new(self.codex_runner.clone(), true);
        let context = AgentContext {
            state,
            novel: self.config.novel_settings.clone(),
            memory,
            scene_plan: None,
            scene: Some(scene.clone()),
            instruction: None,
            allow_dummy_fallback: self.config.allow_dummy_fallback,
        };

        let output = critic.run(&context)?;
        let issues = self.parse_review_issues(&output);
        self.save_review_report(&scene.id, &issues)?;
        Ok(issues)
    }

    pub fn rewrite_scene(&self, scene_id: &str, instruction: &str) -> Result<Scene> {
        self.init_project()?;

        let state = self.state_manager.load_state()?;
        let memory = self.memory_manager.load_bundle()?;
        let existing_scene = self.show_scene(scene_id)?;
        let editor = EditorAgent::new(self.codex_runner.clone(), true);
        let context = AgentContext {
            state: state.clone(),
            novel: self.config.novel_settings.clone(),
            memory,
            scene_plan: None,
            scene: Some(existing_scene.clone()),
            instruction: Some(instruction.to_string()),
            allow_dummy_fallback: self.config.allow_dummy_fallback,
        };

        let rewritten_text = editor.run(&context)?;
        let revision = self.next_rewrite_revision(scene_id)?;
        let original_snapshot_path = self.rewrite_snapshot_path(scene_id, revision, "original");
        let rewritten_snapshot_path = self.rewrite_snapshot_path(scene_id, revision, "rewritten");
        write_string(&original_snapshot_path, &render_scene(&existing_scene))?;

        let rewritten_scene = Scene {
            text: rewritten_text,
            status: "draft".to_string(),
            ..existing_scene
        };

        self.save_scene(&rewritten_scene)?;
        write_string(&rewritten_snapshot_path, &render_scene(&rewritten_scene))?;
        self.save_rewrite_record(RewriteRecord {
            timestamp_unix_secs: unix_timestamp_secs(),
            scene_id: scene_id.to_string(),
            instruction: instruction.to_string(),
            revision,
            original_snapshot_path: original_snapshot_path.display().to_string(),
            rewritten_snapshot_path: rewritten_snapshot_path.display().to_string(),
        })?;

        if state.current_scene_id.as_deref() == Some(scene_id) {
            self.memory_manager
                .overwrite_active_memory(&self.render_active_memory(&state, &rewritten_scene))?;
        }

        self.memory_manager.append_story_memory(&format!(
            "## Rewrite {}\n- Instruction: {}\n- Status: draft\n",
            rewritten_scene.id.as_str(),
            instruction
        ))?;

        Ok(rewritten_scene)
    }

    pub fn approve_scene(&self, scene_id: &str) -> Result<()> {
        self.init_project()?;

        let mut state = self.state_manager.load_state()?;
        let scene = self.show_scene(scene_id)?;
        let approved = Scene {
            status: "approved".to_string(),
            ..scene
        };

        self.save_scene(&approved)?;
        self.state_manager.mark_scene_approved(&mut state, scene_id);
        self.state_manager.save_state(&state)?;
        Ok(())
    }

    pub fn generate_next_chapter(&self) -> Result<PathBuf> {
        self.init_project()?;

        let mut state = self.state_manager.load_state()?;
        let chapter = state.current_chapter;
        let scenes = self.load_scenes_for_chapter(chapter)?;
        if scenes.is_empty() {
            return Err(anyhow!("no scenes found for chapter {:03}", chapter));
        }
        self.validate_scene_sequence(chapter, &scenes)?;

        let content = render_chapter(chapter, &scenes);
        let chapter_path = self
            .config
            .chapters_dir
            .join(format!("chapter_{:03}.md", chapter));
        write_string(&chapter_path, &content)?;

        self.memory_manager.append_story_memory(&format!(
            "## Chapter {:03}\n- Compiled {} scene(s) into {}\n",
            chapter,
            scenes.len(),
            chapter_path.display()
        ))?;

        self.state_manager.begin_next_chapter(&mut state);
        self.state_manager.save_state(&state)?;

        Ok(chapter_path)
    }

    pub fn expand_world(&self) -> Result<String> {
        self.init_project()?;

        let memory = self.memory_manager.load_bundle()?;
        let prompt = format!(
            "You are expanding the world bible for an AI novel engine.\n\
Return plain markdown only with one new section that deepens setting, factions, or rules.\n\n\
Core memory:\n{core}\n\nStory memory:\n{story}\n\nActive memory:\n{active}\n",
            core = memory.core_memory,
            story = memory.story_memory,
            active = memory.active_memory,
        );

        let expansion = match self.codex_runner.run_prompt(&prompt) {
            Ok(response) => response,
            Err(error) if !self.config.allow_dummy_fallback => return Err(error),
            Err(_) => "# World Expansion\n\n- A hidden civic pact binds the city guilds to a forgotten disaster beneath the harbor.\n- Anyone who breaks the pact inherits both political leverage and mortal risk.\n".to_string(),
        };

        self.memory_manager
            .append_story_memory(&format!("## World Expansion\n{}\n", expansion.trim()))?;

        Ok(expansion)
    }

    pub fn get_memory(&self) -> Result<MemoryBundle> {
        self.init_project()?;
        self.memory_manager.load_bundle()
    }

    pub fn show_scene(&self, scene_id: &str) -> Result<Scene> {
        self.init_project()?;
        let path = self.scene_path(scene_id);
        let content = read_string(&path)?;
        parse_scene(&content).with_context(|| format!("failed to parse {}", path.display()))
    }

    pub fn workspace_dir(&self) -> &std::path::Path {
        &self.config.workspace_dir
    }

    pub fn novel_dir(&self) -> &std::path::Path {
        &self.config.novel_dir
    }

    pub fn global_config_path(&self) -> &std::path::Path {
        &self.config.global_config_path
    }

    pub fn workspace_config_path(&self) -> &std::path::Path {
        &self.config.workspace_config_path
    }

    pub fn workspace_manifest_path(&self) -> &std::path::Path {
        &self.config.workspace_manifest_path
    }

    pub fn novel_title(&self) -> &str {
        self.config.novel_title()
    }

    pub fn missing_required_novel_fields(&self) -> Vec<&'static str> {
        self.config.novel_settings.missing_required_fields()
    }

    pub fn scene_markdown_path(&self, scene_id: &str) -> PathBuf {
        self.scene_path(scene_id)
    }

    pub fn scene_generation_log_path(&self, scene_id: &str) -> PathBuf {
        self.config
            .logs_dir
            .join("scene_generation")
            .join(format!("{scene_id}.json"))
    }

    pub fn review_report_path(&self, scene_id: &str) -> PathBuf {
        self.config
            .logs_dir
            .join("reviews")
            .join(format!("{scene_id}.json"))
    }

    pub fn rewrite_history_dir(&self, scene_id: &str) -> PathBuf {
        self.config.logs_dir.join("rewrites").join(scene_id)
    }

    fn ensure_layout(&self) -> Result<()> {
        ensure_dir(&self.config.workspace_dir)?;
        ensure_dir(&self.config.global_config_dir)?;
        ensure_dir(&self.config.novel_dir)?;
        ensure_dir(&self.config.scenes_dir)?;
        ensure_dir(&self.config.chapters_dir)?;
        ensure_dir(&self.config.logs_dir)?;
        if let Some(parent) = self.config.state_path.parent() {
            ensure_dir(parent)?;
        }
        ensure_dir(&self.config.memory_dir)?;
        Ok(())
    }

    fn ensure_global_config_file(&self) -> Result<()> {
        if self.config.global_config_path.exists() {
            return Ok(());
        }

        let content = self.config.render_global_config()?;
        write_string(&self.config.global_config_path, &content)
    }

    fn ensure_workspace_config_file(&self) -> Result<()> {
        if self.config.workspace_config_path.exists() {
            return Ok(());
        }

        self.write_workspace_config_file()
    }

    fn write_workspace_config_file(&self) -> Result<()> {
        let content = self.config.render_workspace_config()?;
        write_string(&self.config.workspace_config_path, &content)
    }

    fn ensure_workspace_manifest(&self) -> Result<()> {
        if self.config.workspace_manifest_path.exists() {
            return Ok(());
        }

        self.write_workspace_manifest()
    }

    fn write_workspace_manifest(&self) -> Result<()> {
        let manifest = WorkspaceManifest {
            name: self.config.novel_title().to_string(),
            ..WorkspaceManifest::default()
        };
        let content = serde_json::to_string_pretty(&manifest)
            .context("failed to serialize workspace manifest")?;
        write_string(&self.config.workspace_manifest_path, &content)
    }

    fn ensure_workspace_gitignore(&self) -> Result<()> {
        let path = self.config.workspace_dir.join(".gitignore");
        if path.exists() {
            return Ok(());
        }

        let content = "\
# Novel workspace runtime state
/.novel/state/
/.novel/logs/
/.novel/cache/
/.novel/memory/active_memory.md
";

        write_string(&path, content)
    }

    fn save_scene_generation_log(&self, log: SceneGenerationLog) -> Result<()> {
        let path = self.scene_generation_log_path(&log.scene_id);
        let content = serde_json::to_string_pretty(&log)
            .context("failed to serialize scene generation log")?;
        write_string(&path, &content)
    }

    fn save_review_report(&self, scene_id: &str, issues: &[ReviewIssue]) -> Result<()> {
        let report = ReviewReport {
            timestamp_unix_secs: unix_timestamp_secs(),
            scene_id: scene_id.to_string(),
            issues: issues.to_vec(),
        };
        let path = self.review_report_path(scene_id);
        let content =
            serde_json::to_string_pretty(&report).context("failed to serialize review report")?;
        write_string(&path, &content)
    }

    fn save_rewrite_record(&self, record: RewriteRecord) -> Result<()> {
        let path = self
            .rewrite_history_dir(&record.scene_id)
            .join(format!("rewrite_{:03}.json", record.revision));
        let content =
            serde_json::to_string_pretty(&record).context("failed to serialize rewrite record")?;
        write_string(&path, &content)
    }

    fn save_scene(&self, scene: &Scene) -> Result<PathBuf> {
        let path = self.scene_path(&scene.id);
        let markdown = render_scene(scene);
        write_string(&path, &markdown)?;
        Ok(path)
    }

    fn scene_path(&self, scene_id: &str) -> PathBuf {
        self.config.scenes_dir.join(format!("{scene_id}.md"))
    }

    fn load_scenes_for_chapter(&self, chapter: u32) -> Result<Vec<Scene>> {
        let prefix = format!("scene_{:03}_", chapter);
        let mut scenes = Vec::new();

        for path in list_markdown_files(&self.config.scenes_dir)? {
            let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if !file_name.starts_with(&prefix) {
                continue;
            }

            let content = read_string(&path)?;
            scenes.push(parse_scene(&content)?);
        }

        scenes.sort_by_key(|scene| scene.scene_number);
        Ok(scenes)
    }

    fn validate_scene_sequence(&self, chapter: u32, scenes: &[Scene]) -> Result<()> {
        let mut expected = 1u32;
        for scene in scenes {
            if scene.scene_number != expected {
                return Err(anyhow!(
                    "chapter {:03} scene order is invalid: expected scene {:03} but found {}",
                    chapter,
                    expected,
                    scene.id
                ));
            }
            expected += 1;
        }
        Ok(())
    }

    fn next_rewrite_revision(&self, scene_id: &str) -> Result<u32> {
        let dir = self.rewrite_history_dir(scene_id);
        if !dir.exists() {
            return Ok(1);
        }

        let mut max_revision = 0;
        for entry in
            fs::read_dir(&dir).with_context(|| format!("failed to read {}", dir.display()))?
        {
            let entry = entry.with_context(|| format!("failed to inspect {}", dir.display()))?;
            let Some(file_name) = entry.file_name().to_str().map(|value| value.to_string()) else {
                continue;
            };

            let Some(number) = file_name
                .strip_prefix("rewrite_")
                .and_then(|value| value.split('_').next())
                .and_then(|value| value.parse::<u32>().ok())
            else {
                continue;
            };

            max_revision = max_revision.max(number);
        }

        Ok(max_revision + 1)
    }

    fn rewrite_snapshot_path(&self, scene_id: &str, revision: u32, kind: &str) -> PathBuf {
        self.rewrite_history_dir(scene_id)
            .join(format!("rewrite_{revision:03}_{kind}.md"))
    }

    fn parse_scene_plan(&self, raw: &str, chapter: u32, scene_number: u32) -> ScenePlan {
        let cleaned = strip_code_fences(raw);
        let mut plan = serde_json::from_str::<ScenePlan>(&cleaned).unwrap_or_else(|_| ScenePlan {
            chapter,
            scene_number,
            goal: "The protagonist pushes the story into a new decision point.".to_string(),
            conflict: "An ally and a threat demand mutually exclusive choices.".to_string(),
            outcome: "The protagonist gains momentum, but the cost becomes personal.".to_string(),
        });

        if plan.chapter == 0 {
            plan.chapter = chapter;
        }
        if plan.scene_number == 0 {
            plan.scene_number = scene_number;
        }
        if plan.goal.trim().is_empty() {
            plan.goal = "The protagonist pushes the story into a new decision point.".to_string();
        }
        if plan.conflict.trim().is_empty() {
            plan.conflict = "An ally and a threat demand mutually exclusive choices.".to_string();
        }
        if plan.outcome.trim().is_empty() {
            plan.outcome =
                "The protagonist gains momentum, but the cost becomes personal.".to_string();
        }

        plan
    }

    fn parse_review_issues(&self, raw: &str) -> Vec<ReviewIssue> {
        let cleaned = strip_code_fences(raw);
        serde_json::from_str::<Vec<ReviewIssue>>(&cleaned).unwrap_or_else(|_| {
            vec![ReviewIssue {
                issue_type: "analysis".to_string(),
                description: cleaned,
                line_start: None,
                line_end: None,
            }]
        })
    }

    fn render_active_memory(&self, state: &StoryState, scene: &Scene) -> String {
        format!(
            "# Active Memory\n\n- Arc: {}\n- Chapter: {}\n- Scene: {}\n- Scene ID: {}\n- Stage: {}\n- Goal: {}\n- Conflict: {}\n- Outcome: {}\n",
            state.current_arc,
            scene.chapter,
            scene.scene_number,
            scene.id.as_str(),
            state.stage.as_str(),
            scene.goal.as_str(),
            scene.conflict.as_str(),
            scene.outcome.as_str()
        )
    }

    fn render_story_memory_entry(&self, scene: &Scene) -> String {
        format!(
            "## Scene {}\n- Goal: {}\n- Conflict: {}\n- Outcome: {}\n- Status: {}\n",
            scene.id.as_str(),
            scene.goal.as_str(),
            scene.conflict.as_str(),
            scene.outcome.as_str(),
            scene.status.as_str()
        )
    }

    fn ensure_generation_ready(&self) -> Result<()> {
        let missing = self.missing_required_novel_fields();
        if missing.is_empty() {
            return Ok(());
        }

        Err(anyhow!(
            "cannot generate scene. missing required novel config: {}. fill {} first",
            missing.join(", "),
            self.config.workspace_config_path.display()
        ))
    }
}

fn unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
