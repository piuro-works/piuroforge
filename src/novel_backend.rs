use anyhow::Result;
use std::sync::Arc;

use crate::agents::base::{strip_code_fences, Agent, AgentContext};
use crate::agents::critic::CriticAgent;
use crate::agents::editor::EditorAgent;
use crate::agents::planner::PlannerAgent;
use crate::agents::writer::WriterAgent;
use crate::codex_runner::CodexRunner;
use crate::config::NovelSettings;
use crate::llm_runner::PromptRunner;
use crate::models::{
    chapter_role_for, normalize_review_score, review_score_from_issue_count, MemoryBundle,
    ReviewIssue, ReviewOutcome, Scene, ScenePlan, StoryState,
};

pub trait NovelBackend {
    fn generate_scene(&self, request: SceneGenerationRequest) -> Result<SceneGenerationResponse>;
    fn review_scene(&self, request: ReviewRequest) -> Result<ReviewResponse>;
    fn rewrite_scene(&self, request: RewriteRequest) -> Result<RewriteResponse>;
    fn expand_world(&self, request: WorldExpansionRequest) -> Result<WorldExpansionResponse>;
}

#[derive(Debug, Clone)]
pub struct SceneGenerationRequest {
    pub state: StoryState,
    pub novel: NovelSettings,
    pub memory: MemoryBundle,
    pub planner_story_foundation: String,
    pub writer_story_foundation: String,
    pub editor_story_foundation: String,
    pub chapter: u32,
    pub scene_number: u32,
    pub scene_id: String,
    pub allow_dummy_fallback: bool,
}

#[derive(Debug, Clone)]
pub struct SceneGenerationResponse {
    pub final_scene: Scene,
    pub planner_output: String,
    pub planner_fallback_warning: Option<String>,
    pub writer_output: String,
    pub writer_fallback_warning: Option<String>,
    pub editor_output: String,
    pub editor_fallback_warning: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ReviewRequest {
    pub state: StoryState,
    pub novel: NovelSettings,
    pub memory: MemoryBundle,
    pub critic_story_foundation: String,
    pub scene: Scene,
    pub allow_dummy_fallback: bool,
}

#[derive(Debug, Clone)]
pub struct ReviewResponse {
    pub score: u32,
    pub issues: Vec<ReviewIssue>,
    pub critic_fallback_warning: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RewriteRequest {
    pub state: StoryState,
    pub novel: NovelSettings,
    pub memory: MemoryBundle,
    pub editor_story_foundation: String,
    pub scene: Scene,
    pub instruction: String,
    pub allow_dummy_fallback: bool,
}

#[derive(Debug, Clone)]
pub struct RewriteResponse {
    pub rewritten_scene: Scene,
    pub editor_fallback_warning: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct WorldExpansionRequest {
    pub memory: MemoryBundle,
    pub world_story_foundation: String,
    pub allow_dummy_fallback: bool,
}

#[derive(Debug, Clone)]
pub struct WorldExpansionResponse {
    pub expansion: String,
    pub warnings: Vec<String>,
}

#[derive(Clone)]
pub struct CliNovelBackend {
    runner: Arc<dyn PromptRunner>,
}

impl CliNovelBackend {
    pub fn new(runner: Arc<dyn PromptRunner>) -> Self {
        Self { runner }
    }

    pub fn codex(runner: CodexRunner) -> Self {
        Self::new(Arc::new(runner))
    }
}

impl NovelBackend for CliNovelBackend {
    fn generate_scene(&self, request: SceneGenerationRequest) -> Result<SceneGenerationResponse> {
        let mut warnings = Vec::new();
        let chapter_scene_target = request.novel.chapter_scene_target.max(1);

        let planner = PlannerAgent::new(self.runner.clone(), true);
        let planner_context = AgentContext {
            state: request.state.clone(),
            novel: request.novel.clone(),
            memory: request.memory.clone(),
            story_foundation: request.planner_story_foundation.clone(),
            scene_plan: None,
            scene: None,
            instruction: None,
            allow_dummy_fallback: request.allow_dummy_fallback,
        };
        let planner_run = planner.run(&planner_context)?;
        if let Some(warning) = planner_run.fallback_warning.clone() {
            warnings.push(warning);
        }
        let mut scene_plan = parse_scene_plan(
            &planner_run.output,
            request.chapter,
            request.scene_number,
            chapter_scene_target,
        );
        scene_plan.chapter = request.chapter;
        scene_plan.scene_number = request.scene_number;
        scene_plan.chapter_role = scene_plan.effective_chapter_role(chapter_scene_target);

        let writer = WriterAgent::new(self.runner.clone(), true);
        let writer_context = AgentContext {
            state: request.state.clone(),
            novel: request.novel.clone(),
            memory: request.memory.clone(),
            story_foundation: request.writer_story_foundation.clone(),
            scene_plan: Some(scene_plan.clone()),
            scene: None,
            instruction: None,
            allow_dummy_fallback: request.allow_dummy_fallback,
        };
        let writer_run = writer.run(&writer_context)?;
        if let Some(warning) = writer_run.fallback_warning.clone() {
            warnings.push(warning);
        }

        let draft_scene = Scene {
            id: request.scene_id,
            chapter: request.chapter,
            scene_number: request.scene_number,
            short_title: scene_plan.effective_short_title(),
            chapter_role: scene_plan.chapter_role.clone(),
            goal: scene_plan.goal.clone(),
            conflict: scene_plan.conflict.clone(),
            outcome: scene_plan.outcome.clone(),
            text: writer_run.output.clone(),
            status: "draft".to_string(),
        };

        let editor = EditorAgent::new(self.runner.clone(), true);
        let editor_context = AgentContext {
            state: request.state,
            novel: request.novel,
            memory: request.memory,
            story_foundation: request.editor_story_foundation,
            scene_plan: Some(scene_plan),
            scene: Some(draft_scene.clone()),
            instruction: None,
            allow_dummy_fallback: request.allow_dummy_fallback,
        };
        let editor_run = editor.run(&editor_context)?;
        if let Some(warning) = editor_run.fallback_warning.clone() {
            warnings.push(warning);
        }

        let final_scene = Scene {
            text: editor_run.output.clone(),
            ..draft_scene
        };

        Ok(SceneGenerationResponse {
            final_scene,
            planner_output: planner_run.output,
            planner_fallback_warning: planner_run.fallback_warning,
            writer_output: writer_run.output,
            writer_fallback_warning: writer_run.fallback_warning,
            editor_output: editor_run.output,
            editor_fallback_warning: editor_run.fallback_warning,
            warnings,
        })
    }

    fn review_scene(&self, request: ReviewRequest) -> Result<ReviewResponse> {
        let critic = CriticAgent::new(self.runner.clone(), true);
        let context = AgentContext {
            state: request.state,
            novel: request.novel,
            memory: request.memory,
            story_foundation: request.critic_story_foundation,
            scene_plan: None,
            scene: Some(request.scene),
            instruction: None,
            allow_dummy_fallback: request.allow_dummy_fallback,
        };

        let run = critic.run(&context)?;
        let mut warnings = Vec::new();
        if let Some(warning) = run.fallback_warning.clone() {
            warnings.push(warning);
        }

        let outcome = parse_review_outcome(&run.output);

        Ok(ReviewResponse {
            score: outcome.score,
            issues: outcome.issues,
            critic_fallback_warning: run.fallback_warning,
            warnings,
        })
    }

    fn rewrite_scene(&self, request: RewriteRequest) -> Result<RewriteResponse> {
        let editor = EditorAgent::new(self.runner.clone(), true);
        let context = AgentContext {
            state: request.state,
            novel: request.novel,
            memory: request.memory,
            story_foundation: request.editor_story_foundation,
            scene_plan: None,
            scene: Some(request.scene.clone()),
            instruction: Some(request.instruction),
            allow_dummy_fallback: request.allow_dummy_fallback,
        };

        let run = editor.run(&context)?;
        let mut warnings = Vec::new();
        if let Some(warning) = run.fallback_warning.clone() {
            warnings.push(warning);
        }

        Ok(RewriteResponse {
            rewritten_scene: Scene {
                text: run.output,
                status: "draft".to_string(),
                ..request.scene
            },
            editor_fallback_warning: run.fallback_warning,
            warnings,
        })
    }

    fn expand_world(&self, request: WorldExpansionRequest) -> Result<WorldExpansionResponse> {
        let prompt = format!(
            "You are expanding the world bible for HeeForge, a CLI-first AI novel engine.\n\
Return plain markdown only with one new section that deepens setting, factions, or rules.\n\n\
Story foundation:\n{foundation}\n\n\
Core memory:\n{core}\n\nStory memory:\n{story}\n\nActive memory:\n{active}\n",
            foundation = request.world_story_foundation,
            core = request.memory.core_memory,
            story = request.memory.story_memory,
            active = request.memory.active_memory,
        );

        let (expansion, warning) = match self.runner.run_prompt_named("expand-world", &prompt) {
            Ok(response) => (response, None),
            Err(error) if !request.allow_dummy_fallback => return Err(error),
            Err(error) => (
                "# World Expansion\n\n- A hidden civic pact binds the city guilds to a forgotten disaster beneath the harbor.\n- Anyone who breaks the pact inherits both political leverage and mortal risk.\n".to_string(),
                Some(format!(
                    "expand-world used dummy fallback because codex failed: {}",
                    compact_error_message(&error)
                )),
            ),
        };

        let mut warnings = Vec::new();
        if let Some(warning) = warning {
            warnings.push(warning);
        }

        Ok(WorldExpansionResponse {
            expansion,
            warnings,
        })
    }
}

#[derive(Clone)]
pub struct CodexNovelBackend {
    inner: CliNovelBackend,
}

impl CodexNovelBackend {
    pub fn new(runner: CodexRunner) -> Self {
        Self {
            inner: CliNovelBackend::codex(runner),
        }
    }
}

impl NovelBackend for CodexNovelBackend {
    fn generate_scene(&self, request: SceneGenerationRequest) -> Result<SceneGenerationResponse> {
        self.inner.generate_scene(request)
    }

    fn review_scene(&self, request: ReviewRequest) -> Result<ReviewResponse> {
        self.inner.review_scene(request)
    }

    fn rewrite_scene(&self, request: RewriteRequest) -> Result<RewriteResponse> {
        self.inner.rewrite_scene(request)
    }

    fn expand_world(&self, request: WorldExpansionRequest) -> Result<WorldExpansionResponse> {
        self.inner.expand_world(request)
    }
}

fn parse_scene_plan(
    raw: &str,
    chapter: u32,
    scene_number: u32,
    chapter_scene_target: u32,
) -> ScenePlan {
    let cleaned = strip_code_fences(raw);
    let mut plan = serde_json::from_str::<ScenePlan>(&cleaned).unwrap_or_else(|_| ScenePlan {
        chapter,
        scene_number,
        short_title: "Decision at the Threshold".to_string(),
        chapter_role: chapter_role_for(scene_number, chapter_scene_target),
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
    if plan.short_title.trim().is_empty() {
        plan.short_title = plan.effective_short_title();
    }
    if plan.chapter_role.trim().is_empty() {
        plan.chapter_role = chapter_role_for(plan.scene_number, chapter_scene_target);
    }
    if plan.conflict.trim().is_empty() {
        plan.conflict = "An ally and a threat demand mutually exclusive choices.".to_string();
    }
    if plan.outcome.trim().is_empty() {
        plan.outcome = "The protagonist gains momentum, but the cost becomes personal.".to_string();
    }

    plan
}

fn parse_review_outcome(raw: &str) -> ReviewOutcome {
    let cleaned = strip_code_fences(raw);

    #[derive(serde::Deserialize)]
    struct ReviewPayload {
        #[serde(default)]
        score: Option<u32>,
        #[serde(default)]
        issues: Vec<ReviewIssue>,
    }

    if let Ok(payload) = serde_json::from_str::<ReviewPayload>(&cleaned) {
        let score = payload
            .score
            .map(normalize_review_score)
            .unwrap_or_else(|| review_score_from_issue_count(payload.issues.len()));
        return ReviewOutcome {
            score,
            issues: payload.issues,
        };
    }

    if let Ok(issues) = serde_json::from_str::<Vec<ReviewIssue>>(&cleaned) {
        return ReviewOutcome {
            score: review_score_from_issue_count(issues.len()),
            issues,
        };
    }

    let issues = vec![ReviewIssue {
        issue_type: "analysis".to_string(),
        description: cleaned,
        line_start: None,
        line_end: None,
    }];

    ReviewOutcome {
        score: review_score_from_issue_count(issues.len()),
        issues,
    }
}

fn compact_error_message(error: &anyhow::Error) -> String {
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

#[cfg(test)]
mod tests {
    use super::{
        parse_review_outcome, CliNovelBackend, NovelBackend, ReviewRequest, RewriteRequest,
        SceneGenerationRequest, WorldExpansionRequest,
    };
    use crate::config::NovelSettings;
    use crate::llm_runner::PromptRunner;
    use crate::models::{MemoryBundle, StoryState};
    use anyhow::{anyhow, Result};
    use std::collections::{BTreeMap, VecDeque};
    use std::sync::{Arc, Mutex};

    #[test]
    fn parse_review_outcome_reads_object_score() {
        let outcome = parse_review_outcome(
            r#"{"score":82,"issues":[{"issue_type":"pacing","description":"Tighten the middle.","line_start":3,"line_end":5}]}"#,
        );

        assert_eq!(outcome.score, 82);
        assert_eq!(outcome.issues.len(), 1);
        assert_eq!(outcome.issues[0].issue_type, "pacing");
    }

    #[test]
    fn parse_review_outcome_keeps_array_compatibility() {
        let outcome = parse_review_outcome(
            r#"[{"issue_type":"logic","description":"Clarify the leap.","line_start":7,"line_end":9}]"#,
        );

        assert_eq!(outcome.score, 88);
        assert_eq!(outcome.issues.len(), 1);
        assert_eq!(outcome.issues[0].issue_type, "logic");
    }

    #[derive(Clone)]
    struct FixturePromptRunner {
        outputs: Arc<Mutex<BTreeMap<String, VecDeque<String>>>>,
    }

    impl FixturePromptRunner {
        fn from_pairs(pairs: &[(&str, &str)]) -> Self {
            let mut outputs = BTreeMap::new();
            for (label, output) in pairs {
                outputs
                    .entry((*label).to_string())
                    .or_insert_with(VecDeque::new)
                    .push_back((*output).to_string());
            }

            Self {
                outputs: Arc::new(Mutex::new(outputs)),
            }
        }
    }

    impl PromptRunner for FixturePromptRunner {
        fn run_prompt_named(&self, label: &str, _prompt: &str) -> Result<String> {
            let mut outputs = self.outputs.lock().expect("fixture runner mutex");
            let queue = outputs
                .get_mut(label)
                .ok_or_else(|| anyhow!("no fixture output registered for {label}"))?;
            queue
                .pop_front()
                .ok_or_else(|| anyhow!("fixture outputs exhausted for {label}"))
        }
    }

    #[test]
    fn cli_novel_backend_supports_fixture_runner_integration() -> Result<()> {
        let backend = CliNovelBackend::new(Arc::new(FixturePromptRunner::from_pairs(&[
            (
                "planner",
                r#"{"chapter":1,"scene_number":1,"short_title":"Signal at the Gate","chapter_role":"incident","goal":"The investigator confirms the missing courier route.","conflict":"A clerk refuses access without a dead supervisor's code.","outcome":"The route is found, but the code points to a sealed dock ledger."}"#,
            ),
            (
                "writer",
                "Yunseo leaned over the registry desk and found the courier route hidden under a dead supervisor's lock code.",
            ),
            (
                "editor",
                "Yunseo leaned over the registry desk and found the courier route hidden behind a dead supervisor's lock code.",
            ),
            (
                "critic",
                r#"{"score":91,"issues":[{"issue_type":"pacing","description":"Tighten the second paragraph.","line_start":2,"line_end":3}]}"#,
            ),
            (
                "editor",
                "Yunseo revised the registry exchange until the dead supervisor's lock code landed with sharper pressure.",
            ),
            (
                "expand-world",
                "# World Expansion\n\n## Dock Ledger Protocol\nThe sealed dock ledger can only be opened by the night quartermaster and the memory court auditor.\n",
            ),
        ])));

        let state = StoryState::default();
        let novel = NovelSettings {
            title: "Glass Harbor".to_string(),
            genre: "Mystery".to_string(),
            tone: "Tense, atmospheric".to_string(),
            premise: "An investigator follows edited records to find a missing sibling."
                .to_string(),
            protagonist_name: "Yunseo".to_string(),
            language: "ko".to_string(),
            chapter_scene_target: 3,
            ..NovelSettings::default()
        };
        let memory = MemoryBundle {
            core_memory: "# Core Memory\n".to_string(),
            story_memory: "# Story Memory\n".to_string(),
            active_memory: "# Active Memory\n".to_string(),
        };

        let generated = backend.generate_scene(SceneGenerationRequest {
            state: state.clone(),
            novel: novel.clone(),
            memory: memory.clone(),
            planner_story_foundation: "Plot foundation".to_string(),
            writer_story_foundation: "Writer foundation".to_string(),
            editor_story_foundation: "Editor foundation".to_string(),
            chapter: 1,
            scene_number: 1,
            scene_id: "scene_001_001".to_string(),
            allow_dummy_fallback: false,
        })?;
        assert_eq!(generated.final_scene.id, "scene_001_001");
        assert_eq!(generated.final_scene.short_title, "Signal at the Gate");
        assert!(generated
            .final_scene
            .text
            .contains("dead supervisor's lock code"));

        let reviewed = backend.review_scene(ReviewRequest {
            state: state.clone(),
            novel: novel.clone(),
            memory: memory.clone(),
            critic_story_foundation: "Critic foundation".to_string(),
            scene: generated.final_scene.clone(),
            allow_dummy_fallback: false,
        })?;
        assert_eq!(reviewed.score, 91);
        assert_eq!(reviewed.issues.len(), 1);

        let rewritten = backend.rewrite_scene(RewriteRequest {
            state,
            novel,
            memory: memory.clone(),
            editor_story_foundation: "Editor foundation".to_string(),
            scene: generated.final_scene,
            instruction: "Sharpen the pressure in the registry exchange.".to_string(),
            allow_dummy_fallback: false,
        })?;
        assert!(rewritten.rewritten_scene.text.contains("sharper pressure"));

        let expanded = backend.expand_world(WorldExpansionRequest {
            memory,
            world_story_foundation: "World foundation".to_string(),
            allow_dummy_fallback: false,
        })?;
        assert!(expanded.expansion.contains("Dock Ledger Protocol"));

        Ok(())
    }
}
