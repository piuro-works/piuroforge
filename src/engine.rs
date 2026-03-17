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
use crate::launch_contract::validate_launch_contract;
use crate::novel_backend::{
    CodexNovelBackend, NovelBackend, ReviewRequest, RewriteRequest, SceneGenerationRequest,
    WorldExpansionRequest,
};
use crate::state_manager::StateManager;
use crate::story_foundation::{
    load_story_foundation, StoryFoundationBundle, StoryFoundationStatus,
};
use crate::utils::files::{ensure_dir, list_markdown_files, read_string, write_string};
use crate::utils::markdown::{parse_scene, render_bundle, render_scene};
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
        match config.llm_backend.as_str() {
            "codex_cli" => {
                let mut codex_runner = CodexRunner::new(
                    config.codex_command.clone(),
                    Duration::from_secs(config.codex_timeout_secs),
                );
                if config.log_prompts {
                    codex_runner =
                        codex_runner.with_prompt_logging(config.logs_dir.join("llm_prompts"));
                }

                Self::with_backend(config, Arc::new(CodexNovelBackend::new(codex_runner)))
            }
            backend => Err(anyhow!(
                "unsupported llm backend `{backend}`. supported backends: codex_cli"
            )),
        }
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
        self.ensure_launch_contract_ready()?;
        let mut state = self.prepare_generation_state()?;
        let memory = self.memory_manager.load_prompt_bundle()?;
        let foundation = self.load_story_foundation_bundle()?;
        let (bundle, scene_number, scene_id) = self.state_manager.next_scene_identity(&state);
        let generated = self.backend.generate_scene(SceneGenerationRequest {
            state: state.clone(),
            novel: self.config.novel_settings.clone(),
            memory: memory.clone(),
            planner_story_foundation: foundation.views.planner.clone(),
            writer_story_foundation: foundation.views.writer.clone(),
            editor_story_foundation: foundation.views.editor.clone(),
            bundle,
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
            .upsert_story_memory_entry(&self.render_story_memory_entry(&final_scene))?;

        let mut warnings = generated.warnings;
        if let Some(warning) = story_foundation_warning(&foundation.status) {
            warnings.push(warning);
        }

        Ok(OperationResult {
            value: final_scene,
            warnings,
        })
    }

    fn ensure_launch_contract_ready(&self) -> Result<()> {
        let report = validate_launch_contract(&self.config.workspace_dir, &self.config.novel_settings)?;
        if report.has_blocking_issues() {
            let reasons = report.blocking_messages().join(" | ");
            anyhow::bail!("launch contract validation failed: {reasons}");
        }
        Ok(())
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
        let foundation = self.load_story_foundation_bundle()?;
        let reviewed = self.backend.review_scene(ReviewRequest {
            state,
            novel: self.config.novel_settings.clone(),
            memory,
            critic_story_foundation: foundation.views.critic,
            allow_dummy_fallback: self.config.allow_dummy_fallback,
            scene,
        })?;

        self.save_review_report(
            &scene_id,
            reviewed.score,
            &reviewed.issues,
            reviewed.critic_fallback_warning,
        )?;
        let mut warnings = reviewed.warnings;
        if let Err(error) = self.backfill_post_rewrite_review_score(&scene_id, reviewed.score) {
            warnings.push(format!(
                "could not update rewrite metadata with post-review score: {error}"
            ));
        }
        Ok(OperationResult {
            value: ReviewOutcome {
                score: reviewed.score,
                issues: reviewed.issues,
            },
            warnings,
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
        let foundation = self.load_story_foundation_bundle()?;
        let existing_scene = self.show_scene(scene_id)?;
        let rewritten = self.backend.rewrite_scene(RewriteRequest {
            state: state.clone(),
            novel: self.config.novel_settings.clone(),
            memory,
            editor_story_foundation: foundation.views.editor,
            scene: existing_scene.clone(),
            instruction: instruction.to_string(),
            allow_dummy_fallback: self.config.allow_dummy_fallback,
        })?;
        let revision = self.next_rewrite_revision(scene_id)?;
        let original_snapshot_path = self.rewrite_snapshot_path(scene_id, revision, "original");
        let rewritten_snapshot_path = self.rewrite_snapshot_path(scene_id, revision, "rewritten");
        write_string(&original_snapshot_path, &render_scene(&existing_scene))?;
        let rewritten_scene = rewritten.rewritten_scene;
        let mut warnings = rewritten.warnings;
        let source_review_score = match self.load_review_score(scene_id) {
            Ok(score) => score,
            Err(error) => {
                warnings.push(format!(
                    "could not read latest review score for rewrite metadata: {error}"
                ));
                None
            }
        };

        self.save_scene(&rewritten_scene)?;
        write_string(&rewritten_snapshot_path, &render_scene(&rewritten_scene))?;
        self.save_rewrite_record(RewriteRecord {
            timestamp_unix_secs: unix_timestamp_secs(),
            scene_id: scene_id.to_string(),
            instruction: instruction.to_string(),
            revision,
            source_review_score,
            post_rewrite_review_score: None,
            editor_fallback_warning: rewritten.editor_fallback_warning,
            original_snapshot_path: self.workspace_relative_path(&original_snapshot_path),
            rewritten_snapshot_path: self.workspace_relative_path(&rewritten_snapshot_path),
        })?;

        if state.current_scene_id.as_deref() == Some(scene_id) {
            self.memory_manager
                .overwrite_active_memory(&self.render_active_memory(&state, &rewritten_scene))?;
        }
        self.memory_manager
            .upsert_story_memory_entry(&self.render_story_memory_entry(&rewritten_scene))?;

        self.memory_manager.append_story_memory(&format!(
            "## Rewrite {}\n- Instruction: {}\n- Status: draft\n",
            rewritten_scene.id.as_str(),
            instruction
        ))?;

        Ok(OperationResult {
            value: rewritten_scene,
            warnings,
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
        self.memory_manager
            .overwrite_active_memory(&self.render_active_memory(&state, &approved))?;
        self.memory_manager
            .upsert_story_memory_entry(&self.render_story_memory_entry(&approved))?;
        Ok(())
    }

    pub fn generate_next_bundle(&self) -> Result<PathBuf> {
        self.init_project()?;

        let mut state = self.state_manager.load_state()?;
        let (bundle, scenes, should_advance_state) =
            self.resolve_bundle_compilation_target(&state)?;
        self.validate_scene_sequence(bundle, &scenes)?;
        self.validate_bundle_scene_target(bundle, &scenes)?;

        let bundle_short_title = self.derive_bundle_short_title(bundle, &scenes);
        let content = render_bundle(bundle, &bundle_short_title, &scenes);
        let bundle_path = self.bundle_path(bundle, &bundle_short_title);
        write_string(&bundle_path, &content)?;

        self.memory_manager.append_story_memory(&format!(
            "## Bundle {:03}: {}\n- Compiled {} scene(s) into {}\n",
            bundle,
            bundle_short_title,
            scenes.len(),
            self.workspace_relative_path(&bundle_path)
        ))?;

        if should_advance_state {
            self.state_manager.begin_next_bundle(&mut state);
            self.state_manager.save_state(&state)?;
        }

        Ok(bundle_path)
    }

    pub fn expand_world(&self) -> Result<OperationResult<String>> {
        self.init_project()?;

        let memory = self.memory_manager.load_prompt_bundle()?;
        let foundation = self.load_story_foundation_bundle()?;
        let expanded = self.backend.expand_world(WorldExpansionRequest {
            memory,
            world_story_foundation: foundation.views.world,
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

    pub fn story_foundation_status(&self) -> Result<StoryFoundationStatus> {
        Ok(self.load_story_foundation_bundle()?.status)
    }

    pub fn bundle_scene_target(&self) -> u32 {
        self.config.novel_settings.bundle_scene_target.max(1)
    }

    pub fn serialized_workflow_enabled(&self) -> bool {
        self.config.novel_settings.serialized_workflow
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

    pub fn bundle_short_title(&self, bundle: u32) -> Result<Option<String>> {
        let scenes = self.load_scenes_for_bundle(bundle)?;
        if scenes.is_empty() {
            return Ok(None);
        }

        Ok(Some(self.derive_bundle_short_title(bundle, &scenes)))
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

    fn load_review_score(&self, scene_id: &str) -> Result<Option<u32>> {
        let path = self.review_report_path(scene_id);
        if !path.exists() {
            return Ok(None);
        }

        let content = read_string(&path)?;
        let report: ReviewReport = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        Ok(Some(report.score))
    }

    fn backfill_post_rewrite_review_score(&self, scene_id: &str, score: u32) -> Result<()> {
        let Some(path) = self.latest_rewrite_record_path(scene_id)? else {
            return Ok(());
        };

        let content = read_string(&path)?;
        let mut record: RewriteRecord = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        if record.post_rewrite_review_score.is_some() {
            return Ok(());
        }

        record.post_rewrite_review_score = Some(score);
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

    fn bundle_path(&self, bundle: u32, short_title: &str) -> PathBuf {
        let slug = slug_fragment(short_title);
        if slug.is_empty() {
            return self
                .config
                .bundles_dir
                .join(format!("bundle_{bundle:03}.md"));
        }

        self.config
            .bundles_dir
            .join(format!("bundle_{bundle:03}-{slug}.md"))
    }

    fn load_scenes_for_bundle(&self, bundle: u32) -> Result<Vec<Scene>> {
        let prefix = format!("scene_{:03}_", bundle);
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

    fn validate_scene_sequence(&self, bundle: u32, scenes: &[Scene]) -> Result<()> {
        let mut expected = 1u32;
        for scene in scenes {
            if scene.scene_number != expected {
                return Err(anyhow!(
                    "bundle {:03} scene order is invalid: expected scene {:03} but found {}",
                    bundle,
                    expected,
                    scene.id
                ));
            }
            expected += 1;
        }
        Ok(())
    }

    fn validate_bundle_scene_target(&self, bundle: u32, scenes: &[Scene]) -> Result<()> {
        let target = self.bundle_scene_target() as usize;
        if scenes.len() != target {
            return Err(anyhow!(
                "bundle scene target not reached for bundle {:03}: expected {} scene(s) but found {}",
                bundle,
                target,
                scenes.len()
            ));
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

    fn latest_rewrite_record_path(&self, scene_id: &str) -> Result<Option<PathBuf>> {
        let mut latest: Option<(u32, PathBuf)> = None;
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
                if !file_name.ends_with(".json") {
                    continue;
                }

                let Some(number) = file_name
                    .strip_prefix("rewrite_")
                    .and_then(|value| value.strip_suffix(".json"))
                    .and_then(|value| value.parse::<u32>().ok())
                else {
                    continue;
                };

                let path = entry.path();
                match &latest {
                    Some((latest_number, _)) if *latest_number >= number => {}
                    _ => latest = Some((number, path)),
                }
            }
        }

        Ok(latest.map(|(_, path)| path))
    }

    fn rewrite_snapshot_path(&self, scene_id: &str, revision: u32, kind: &str) -> PathBuf {
        self.rewrite_history_dir(scene_id)
            .join(format!("rewrite_{revision:03}_{kind}.md"))
    }

    fn render_active_memory(&self, state: &StoryState, scene: &Scene) -> String {
        format!(
            "# Active Memory\n\n- Arc: {}\n- Bundle: {}\n- Scene: {}\n- Scene ID: {}\n- Short Title: {}\n- Bundle Role: {}\n- Stage: {}\n- Goal: {}\n- Conflict: {}\n- Outcome: {}\n",
            state.current_arc,
            scene.bundle,
            scene.scene_number,
            scene.id.as_str(),
            scene.effective_short_title(),
            scene.effective_bundle_role(self.bundle_scene_target()),
            state.stage.as_str(),
            scene.goal.as_str(),
            scene.conflict.as_str(),
            scene.outcome.as_str()
        )
    }

    fn render_story_memory_entry(&self, scene: &Scene) -> String {
        format!(
            "## Scene {}: {}\n- Bundle Role: {}\n- Goal: {}\n- Conflict: {}\n- Outcome: {}\n- Status: {}\n",
            scene.id.as_str(),
            scene.effective_short_title(),
            scene.effective_bundle_role(self.bundle_scene_target()),
            scene.goal.as_str(),
            scene.conflict.as_str(),
            scene.outcome.as_str(),
            scene.status.as_str()
        )
    }

    fn prepare_generation_state(&self) -> Result<StoryState> {
        let missing = self.missing_required_novel_fields();
        if !missing.is_empty() {
            return Err(anyhow!(
                "cannot generate scene. missing required novel config: {}. fill {} first",
                missing.join(", "),
                self.config.workspace_config_path.display()
            ));
        }

        let mut state = self.state_manager.load_state()?;
        let bundle = state.current_bundle;
        let scenes = self.load_scenes_for_bundle(bundle)?;
        let target = self.bundle_scene_target() as usize;
        if scenes.len() >= target {
            if !self.serialized_workflow_enabled() {
                return Err(anyhow!(
                    "bundle scene limit reached for bundle {:03}: target is {} scene(s). compile the bundle before drafting more",
                    bundle,
                    target
                ));
            }

            if state.stage != "scene_approved" {
                let current_scene = state
                    .current_scene_id
                    .as_deref()
                    .unwrap_or("the current scene");
                return Err(anyhow!(
                    "serialized workflow reached the internal bundle boundary for bundle {:03}. review and approve {} before drafting the next scene",
                    bundle,
                    current_scene
                ));
            }

            self.state_manager.begin_next_bundle(&mut state);
            self.state_manager.save_state(&state)?;
        }

        Ok(state)
    }

    fn resolve_bundle_compilation_target(
        &self,
        state: &StoryState,
    ) -> Result<(u32, Vec<Scene>, bool)> {
        let bundle = state.current_bundle;
        let scenes = self.load_scenes_for_bundle(bundle)?;
        if self.serialized_workflow_enabled() {
            let target = self.bundle_scene_target() as usize;
            if scenes.len() >= target {
                return Ok((bundle, scenes, false));
            }

            if bundle > 1 {
                let previous_bundle = bundle - 1;
                let previous_scenes = self.load_scenes_for_bundle(previous_bundle)?;
                if !previous_scenes.is_empty() {
                    return Ok((previous_bundle, previous_scenes, false));
                }
            }

            if !scenes.is_empty() {
                return Ok((bundle, scenes, false));
            }
        } else if !scenes.is_empty() {
            return Ok((bundle, scenes, true));
        }

        Err(anyhow!("no scenes found for bundle {:03}", bundle))
    }

    fn derive_bundle_short_title(&self, bundle: u32, scenes: &[Scene]) -> String {
        let Some(first) = scenes.first() else {
            return format!("Bundle {:03}", bundle);
        };

        let Some(last) = scenes.last() else {
            return format!("Bundle {:03}", bundle);
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

    fn load_story_foundation_bundle(&self) -> Result<StoryFoundationBundle> {
        load_story_foundation(&self.config.workspace_dir)
    }
}

fn unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn story_foundation_warning(status: &StoryFoundationStatus) -> Option<String> {
    if status.score >= 60 {
        return None;
    }

    if status.missing_items.is_empty() {
        return Some(format!(
            "Story foundation is still {} ({}/100). Add more brief, bible, or plot documents before expecting stable novel output.",
            status.level(),
            status.score
        ));
    }

    Some(format!(
        "Story foundation is {} ({}/100). For better scenes, add {}.",
        status.level(),
        status.score,
        status.missing_items.join(", ")
    ))
}
