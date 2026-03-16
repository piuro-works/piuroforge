use anyhow::{anyhow, Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::codex_runner::CodexRunner;
use crate::config::Config;
use crate::memory_manager::MemoryManager;
use crate::models::{
    derive_short_title, slug_fragment, MemoryBundle, OperationResult, ReviewIssue, ReviewOutcome,
    ReviewReport, RewriteRecord, Scene, SceneGenerationLog, StoryState, WorkspaceManifest,
};
use crate::novel_backend::{
    CodexNovelBackend, NovelBackend, ReviewRequest, RewriteRequest, SceneGenerationRequest,
    WorldExpansionRequest,
};
use crate::state_manager::StateManager;
use crate::utils::files::{ensure_dir, list_markdown_files, read_string, write_string};
use crate::utils::markdown::{parse_scene, render_chapter, render_scene};
use crate::workspace_git::{WorkspaceGit, WorkspaceGitOutcome};
use crate::workspace_scaffold::{scaffold_files, SCAFFOLD_DIRS};

#[derive(Clone)]
pub struct NovelEngine {
    config: Config,
    state_manager: StateManager,
    memory_manager: MemoryManager,
    backend: Arc<dyn NovelBackend + Send + Sync>,
}

impl NovelEngine {
    pub fn new(config: Config) -> Result<Self> {
        let mut codex_runner = CodexRunner::new(
            config.codex_command.clone(),
            Duration::from_secs(config.codex_timeout_secs),
        );
        if config.log_prompts {
            codex_runner = codex_runner.with_prompt_logging(config.logs_dir.join("llm_prompts"));
        }

        Self::with_backend(config, Arc::new(CodexNovelBackend::new(codex_runner)))
    }

    pub fn with_backend(
        config: Config,
        backend: Arc<dyn NovelBackend + Send + Sync>,
    ) -> Result<Self> {
        let state_manager = StateManager::new(config.state_path.clone());
        let memory_manager = MemoryManager::new(config.memory_dir.clone());

        Ok(Self {
            config,
            state_manager,
            memory_manager,
            backend,
        })
    }

    pub fn init_project(&self) -> Result<()> {
        self.ensure_layout()?;
        self.ensure_global_config_file()?;
        self.ensure_workspace_config_file()?;
        self.ensure_workspace_manifest()?;
        self.ensure_workspace_gitignore()?;
        self.ensure_workspace_support_files()?;
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
        self.ensure_workspace_support_files()?;
        self.state_manager.ensure_state_file()?;
        self.memory_manager.ensure_files()?;
        Ok(())
    }

    pub fn get_status(&self) -> Result<StoryState> {
        self.init_project()?;
        self.state_manager.load_state()
    }

    pub fn generate_next_scene(&self) -> Result<OperationResult<Scene>> {
        self.init_project()?;
        self.ensure_generation_ready()?;

        let mut state = self.state_manager.load_state()?;
        let memory = self.memory_manager.load_prompt_bundle()?;
        let (chapter, scene_number, scene_id) = self.state_manager.next_scene_identity(&state);
        let generated = self.backend.generate_scene(SceneGenerationRequest {
            state: state.clone(),
            novel: self.config.novel_settings.clone(),
            memory: memory.clone(),
            chapter,
            scene_number,
            scene_id,
            allow_dummy_fallback: self.config.allow_dummy_fallback,
        })?;
        let final_scene = generated.final_scene;

        self.save_scene(&final_scene)?;
        self.save_scene_generation_log(SceneGenerationLog {
            timestamp_unix_secs: unix_timestamp_secs(),
            scene_id: final_scene.id.clone(),
            planner_output: generated.planner_output,
            planner_fallback_warning: generated.planner_fallback_warning,
            writer_output: generated.writer_output,
            writer_fallback_warning: generated.writer_fallback_warning,
            editor_output: generated.editor_output,
            editor_fallback_warning: generated.editor_fallback_warning,
            final_scene: final_scene.clone(),
        })?;
        self.state_manager
            .mark_scene_generated(&mut state, &final_scene);
        self.state_manager.save_state(&state)?;

        self.memory_manager
            .overwrite_active_memory(&self.render_active_memory(&state, &final_scene))?;
        self.memory_manager
            .append_story_memory(&self.render_story_memory_entry(&final_scene))?;

        Ok(OperationResult {
            value: final_scene,
            warnings: generated.warnings,
        })
    }

    pub fn review_current_scene(&self) -> Result<OperationResult<ReviewOutcome>> {
        self.init_project()?;

        let state = self.state_manager.load_state()?;
        let scene_id = state
            .current_scene_id
            .clone()
            .ok_or_else(|| anyhow!("no current scene available to review"))?;
        let scene = self.show_scene(&scene_id)?;
        let memory = self.memory_manager.load_bundle()?;
        let reviewed = self.backend.review_scene(ReviewRequest {
            state,
            novel: self.config.novel_settings.clone(),
            memory,
            allow_dummy_fallback: self.config.allow_dummy_fallback,
            scene,
        })?;

        self.save_review_report(
            &scene_id,
            reviewed.score,
            &reviewed.issues,
            reviewed.critic_fallback_warning,
        )?;
        Ok(OperationResult {
            value: ReviewOutcome {
                score: reviewed.score,
                issues: reviewed.issues,
            },
            warnings: reviewed.warnings,
        })
    }

    pub fn rewrite_scene(
        &self,
        scene_id: &str,
        instruction: &str,
    ) -> Result<OperationResult<Scene>> {
        self.init_project()?;

        let state = self.state_manager.load_state()?;
        let memory = self.memory_manager.load_bundle()?;
        let existing_scene = self.show_scene(scene_id)?;
        let rewritten = self.backend.rewrite_scene(RewriteRequest {
            state: state.clone(),
            novel: self.config.novel_settings.clone(),
            memory,
            scene: existing_scene.clone(),
            instruction: instruction.to_string(),
            allow_dummy_fallback: self.config.allow_dummy_fallback,
        })?;
        let revision = self.next_rewrite_revision(scene_id)?;
        let original_snapshot_path = self.rewrite_snapshot_path(scene_id, revision, "original");
        let rewritten_snapshot_path = self.rewrite_snapshot_path(scene_id, revision, "rewritten");
        write_string(&original_snapshot_path, &render_scene(&existing_scene))?;
        let rewritten_scene = rewritten.rewritten_scene;

        self.save_scene(&rewritten_scene)?;
        write_string(&rewritten_snapshot_path, &render_scene(&rewritten_scene))?;
        self.save_rewrite_record(RewriteRecord {
            timestamp_unix_secs: unix_timestamp_secs(),
            scene_id: scene_id.to_string(),
            instruction: instruction.to_string(),
            revision,
            editor_fallback_warning: rewritten.editor_fallback_warning,
            original_snapshot_path: self.workspace_relative_path(&original_snapshot_path),
            rewritten_snapshot_path: self.workspace_relative_path(&rewritten_snapshot_path),
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

        Ok(OperationResult {
            value: rewritten_scene,
            warnings: rewritten.warnings,
        })
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

        let chapter_short_title = self.derive_chapter_short_title(chapter, &scenes);
        let content = render_chapter(chapter, &chapter_short_title, &scenes);
        let chapter_path = self.chapter_path(chapter, &chapter_short_title);
        write_string(&chapter_path, &content)?;

        self.memory_manager.append_story_memory(&format!(
            "## Chapter {:03}: {}\n- Compiled {} scene(s) into {}\n",
            chapter,
            chapter_short_title,
            scenes.len(),
            self.workspace_relative_path(&chapter_path)
        ))?;

        self.state_manager.begin_next_chapter(&mut state);
        self.state_manager.save_state(&state)?;

        Ok(chapter_path)
    }

    pub fn expand_world(&self) -> Result<OperationResult<String>> {
        self.init_project()?;

        let memory = self.memory_manager.load_prompt_bundle()?;
        let expanded = self.backend.expand_world(WorldExpansionRequest {
            memory,
            allow_dummy_fallback: self.config.allow_dummy_fallback,
        })?;

        self.memory_manager.append_story_memory(&format!(
            "## World Expansion\n{}\n",
            expanded.expansion.trim()
        ))?;

        Ok(OperationResult {
            value: expanded.expansion,
            warnings: expanded.warnings,
        })
    }

    pub fn get_memory(&self) -> Result<MemoryBundle> {
        self.init_project()?;
        self.memory_manager.load_bundle()
    }

    pub fn show_scene(&self, scene_id: &str) -> Result<Scene> {
        self.init_project()?;
        let path = self.resolved_scene_path(scene_id);
        let content = read_string(&path)?;
        parse_scene(&content).with_context(|| format!("failed to parse {}", path.display()))
    }

    pub fn workspace_dir(&self) -> &std::path::Path {
        &self.config.workspace_dir
    }

    pub fn workspace_auto_commit_enabled(&self) -> bool {
        self.config.workspace_auto_commit
    }

    pub fn auto_commit_workspace(&self, message: &str) -> WorkspaceGitOutcome {
        WorkspaceGit::new(
            self.config.workspace_dir.clone(),
            self.config.workspace_auto_commit,
        )
        .auto_commit(message)
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

    pub fn workspace_readme_path(&self) -> &std::path::Path {
        &self.config.workspace_readme_path
    }

    pub fn novel_title(&self) -> &str {
        self.config.novel_title()
    }

    pub fn missing_required_novel_fields(&self) -> Vec<&'static str> {
        self.config.novel_settings.missing_required_fields()
    }

    pub fn scene_markdown_path(&self, scene_id: &str) -> PathBuf {
        self.resolved_scene_path(scene_id)
    }

    pub fn scene_generation_log_path(&self, scene_id: &str) -> PathBuf {
        self.config
            .logs_dir
            .join("scene_generation")
            .join(format!("{scene_id}.json"))
    }

    pub fn chapter_short_title(&self, chapter: u32) -> Result<Option<String>> {
        let scenes = self.load_scenes_for_chapter(chapter)?;
        if scenes.is_empty() {
            return Ok(None);
        }

        Ok(Some(self.derive_chapter_short_title(chapter, &scenes)))
    }

    pub fn review_report_path(&self, scene_id: &str) -> PathBuf {
        self.config
            .review_feedback_dir
            .join(format!("{scene_id}.json"))
    }

    pub fn rewrite_history_dir(&self, scene_id: &str) -> PathBuf {
        self.config.review_revisions_dir.join(scene_id)
    }

    fn ensure_layout(&self) -> Result<()> {
        ensure_dir(&self.config.workspace_dir)?;
        ensure_dir(&self.config.global_config_dir)?;
        ensure_dir(&self.config.novel_dir)?;
        ensure_dir(&self.config.logs_dir)?;
        if let Some(parent) = self.config.state_path.parent() {
            ensure_dir(parent)?;
        }
        ensure_dir(&self.config.memory_dir)?;

        for dir in SCAFFOLD_DIRS {
            ensure_dir(&self.config.workspace_dir.join(dir))?;
        }

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

    fn ensure_workspace_support_files(&self) -> Result<()> {
        for file in scaffold_files(self.config.novel_title()) {
            let path = self.config.workspace_dir.join(file.relative_path);
            if path.exists() {
                continue;
            }

            write_string(&path, &file.content)?;
        }

        Ok(())
    }

    fn save_scene_generation_log(&self, log: SceneGenerationLog) -> Result<()> {
        let path = self.scene_generation_log_path(&log.scene_id);
        let content = serde_json::to_string_pretty(&log)
            .context("failed to serialize scene generation log")?;
        write_string(&path, &content)
    }

    fn save_review_report(
        &self,
        scene_id: &str,
        score: u32,
        issues: &[ReviewIssue],
        critic_fallback_warning: Option<String>,
    ) -> Result<()> {
        let report = ReviewReport {
            timestamp_unix_secs: unix_timestamp_secs(),
            scene_id: scene_id.to_string(),
            score,
            critic_fallback_warning,
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
        let path = self.scene_write_path(scene);
        let markdown = render_scene(scene);
        write_string(&path, &markdown)?;
        Ok(path)
    }

    fn scene_write_path(&self, scene: &Scene) -> PathBuf {
        self.config.scenes_dir.join(scene.file_name())
    }

    fn legacy_scene_path(&self, scene_id: &str) -> PathBuf {
        self.config
            .novel_dir
            .join("scenes")
            .join(format!("{scene_id}.md"))
    }

    fn resolved_scene_path(&self, scene_id: &str) -> PathBuf {
        if let Some(current) = self.find_scene_path(&self.config.scenes_dir, scene_id) {
            return current;
        }

        let current = self.config.scenes_dir.join(format!("{scene_id}.md"));
        if current.exists() {
            return current;
        }

        let legacy = self.legacy_scene_path(scene_id);
        if legacy.exists() {
            return legacy;
        }

        current
    }

    fn find_scene_path(&self, dir: &Path, scene_id: &str) -> Option<PathBuf> {
        let prefix = format!("{scene_id}-");
        let exact = format!("{scene_id}.md");

        let entries = list_markdown_files(dir).ok()?;
        for path in &entries {
            let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };

            if file_name.starts_with(&prefix) {
                return Some(path.clone());
            }
        }

        for path in entries {
            let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };

            if file_name == exact {
                return Some(path);
            }
        }

        None
    }

    fn chapter_path(&self, chapter: u32, short_title: &str) -> PathBuf {
        let slug = slug_fragment(short_title);
        if slug.is_empty() {
            return self
                .config
                .chapters_dir
                .join(format!("chapter_{chapter:03}.md"));
        }

        self.config
            .chapters_dir
            .join(format!("chapter_{chapter:03}-{slug}.md"))
    }

    fn load_scenes_for_chapter(&self, chapter: u32) -> Result<Vec<Scene>> {
        let prefix = format!("scene_{:03}_", chapter);
        let mut scenes = BTreeMap::new();

        for dir in [self.legacy_scenes_dir(), self.config.scenes_dir.clone()] {
            if !dir.exists() {
                continue;
            }

            for path in list_markdown_files(&dir)? {
                let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
                    continue;
                };
                if !file_name.starts_with(&prefix) {
                    continue;
                }

                let content = read_string(&path)?;
                let scene = parse_scene(&content)?;
                scenes.insert(scene.id.clone(), scene);
            }
        }

        let mut scenes = scenes.into_values().collect::<Vec<_>>();
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
        let mut max_revision = 0;
        for dir in [
            self.legacy_rewrite_history_dir(scene_id),
            self.rewrite_history_dir(scene_id),
        ] {
            if !dir.exists() {
                continue;
            }

            for entry in
                fs::read_dir(&dir).with_context(|| format!("failed to read {}", dir.display()))?
            {
                let entry =
                    entry.with_context(|| format!("failed to inspect {}", dir.display()))?;
                let Some(file_name) = entry.file_name().to_str().map(|value| value.to_string())
                else {
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
        }

        Ok(max_revision + 1)
    }

    fn rewrite_snapshot_path(&self, scene_id: &str, revision: u32, kind: &str) -> PathBuf {
        self.rewrite_history_dir(scene_id)
            .join(format!("rewrite_{revision:03}_{kind}.md"))
    }

    fn render_active_memory(&self, state: &StoryState, scene: &Scene) -> String {
        format!(
            "# Active Memory\n\n- Arc: {}\n- Chapter: {}\n- Scene: {}\n- Scene ID: {}\n- Short Title: {}\n- Stage: {}\n- Goal: {}\n- Conflict: {}\n- Outcome: {}\n",
            state.current_arc,
            scene.chapter,
            scene.scene_number,
            scene.id.as_str(),
            scene.effective_short_title(),
            state.stage.as_str(),
            scene.goal.as_str(),
            scene.conflict.as_str(),
            scene.outcome.as_str()
        )
    }

    fn render_story_memory_entry(&self, scene: &Scene) -> String {
        format!(
            "## Scene {}: {}\n- Goal: {}\n- Conflict: {}\n- Outcome: {}\n- Status: {}\n",
            scene.id.as_str(),
            scene.effective_short_title(),
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

    fn derive_chapter_short_title(&self, chapter: u32, scenes: &[Scene]) -> String {
        let Some(first) = scenes.first() else {
            return format!("Chapter {:03}", chapter);
        };

        let Some(last) = scenes.last() else {
            return format!("Chapter {:03}", chapter);
        };

        let first_title = first.effective_short_title();
        let last_title = last.effective_short_title();

        if scenes.len() == 1 || first_title == last_title {
            return first_title;
        }

        let combined = format!("{first_title} to {last_title}");
        let shortened = derive_short_title(&combined);
        if shortened.is_empty() {
            first_title
        } else {
            shortened
        }
    }

    fn legacy_scenes_dir(&self) -> PathBuf {
        self.config.novel_dir.join("scenes")
    }

    fn legacy_rewrite_history_dir(&self, scene_id: &str) -> PathBuf {
        self.config.logs_dir.join("rewrites").join(scene_id)
    }

    fn workspace_relative_path(&self, path: &Path) -> String {
        path.strip_prefix(&self.config.workspace_dir)
            .unwrap_or(path)
            .display()
            .to_string()
    }
}

fn unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
