use anyhow::{anyhow, bail, Result};
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
    bundle_role_for, normalize_review_score, review_score_from_issue_count, MemoryBundle,
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
    pub bundle: u32,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedScenePlan {
    plan: ScenePlan,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedReviewOutcome {
    outcome: ReviewOutcome,
    warnings: Vec<String>,
}

#[derive(serde::Deserialize)]
struct ReviewPayload {
    #[serde(default)]
    score: Option<u32>,
    #[serde(default)]
    issues: Vec<ReviewIssue>,
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
        let bundle_scene_target = request.novel.bundle_scene_target.max(1);

        let planner = PlannerAgent::new(self.runner.clone());
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
        let parsed_plan = parse_scene_plan(
            &planner_run.output,
            request.bundle,
            request.scene_number,
            bundle_scene_target,
        )?;
        warnings.extend(parsed_plan.warnings);
        let scene_plan = parsed_plan.plan;

        let writer = WriterAgent::new(self.runner.clone());
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
            bundle: request.bundle,
            scene_number: request.scene_number,
            short_title: scene_plan.effective_short_title(),
            bundle_role: scene_plan.bundle_role.clone(),
            goal: scene_plan.goal.clone(),
            conflict: scene_plan.conflict.clone(),
            outcome: scene_plan.outcome.clone(),
            text: writer_run.output.clone(),
            status: "draft".to_string(),
        };

        let editor = EditorAgent::new(self.runner.clone());
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
        let critic = CriticAgent::new(self.runner.clone());
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

        let parsed_outcome = parse_review_outcome(&run.output)?;
        warnings.extend(parsed_outcome.warnings);
        let outcome = parsed_outcome.outcome;

        Ok(ReviewResponse {
            score: outcome.score,
            issues: outcome.issues,
            critic_fallback_warning: run.fallback_warning,
            warnings,
        })
    }

    fn rewrite_scene(&self, request: RewriteRequest) -> Result<RewriteResponse> {
        let editor = EditorAgent::new(self.runner.clone());
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
            "You are expanding the world bible for PiuroForge, a CLI-first AI novel engine.\n\
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
    bundle: u32,
    scene_number: u32,
    bundle_scene_target: u32,
) -> Result<ParsedScenePlan> {
    let cleaned = strip_code_fences(raw);
    let mut plan = serde_json::from_str::<ScenePlan>(&cleaned).map_err(|error| {
        anyhow!(
            "planner returned invalid scene plan JSON: {error}; excerpt: {}",
            compact_response_excerpt(&cleaned)
        )
    })?;
    let mut warnings = Vec::new();
    let expected_role = bundle_role_for(scene_number, bundle_scene_target);

    if plan.bundle == 0 {
        plan.bundle = bundle;
        warnings.push(format!(
            "planner omitted bundle; using expected bundle {}.",
            bundle
        ));
    } else if plan.bundle != bundle {
        warnings.push(format!(
            "planner returned bundle {} but current bundle is {}; using current bundle.",
            plan.bundle, bundle
        ));
        plan.bundle = bundle;
    }

    if plan.scene_number == 0 {
        plan.scene_number = scene_number;
        warnings.push(format!(
            "planner omitted scene_number; using expected scene {}.",
            scene_number
        ));
    } else if plan.scene_number != scene_number {
        warnings.push(format!(
            "planner returned scene_number {} but expected {}; using expected scene number.",
            plan.scene_number, scene_number
        ));
        plan.scene_number = scene_number;
    }

    if plan.short_title.trim().is_empty() {
        bail!("planner returned scene plan without short_title");
    }
    if plan.goal.trim().is_empty() {
        bail!("planner returned scene plan without goal");
    }
    if plan.conflict.trim().is_empty() {
        bail!("planner returned scene plan without conflict");
    }
    if plan.outcome.trim().is_empty() {
        bail!("planner returned scene plan without outcome");
    }

    let returned_role = plan.bundle_role.trim();
    if returned_role.is_empty() {
        warnings.push(format!(
            "planner omitted bundle_role; using expected `{expected_role}`."
        ));
        plan.bundle_role = expected_role;
    } else if !is_valid_bundle_role(returned_role) {
        warnings.push(format!(
            "planner returned unsupported bundle_role `{returned_role}`; using expected `{expected_role}`."
        ));
        plan.bundle_role = expected_role;
    } else if returned_role != expected_role {
        warnings.push(format!(
            "planner returned bundle_role `{returned_role}` but bundle policy expects `{expected_role}`; using expected role."
        ));
        plan.bundle_role = expected_role;
    } else {
        plan.bundle_role = returned_role.to_string();
    }

    plan.short_title = plan.short_title.trim().to_string();
    plan.goal = plan.goal.trim().to_string();
    plan.conflict = plan.conflict.trim().to_string();
    plan.outcome = plan.outcome.trim().to_string();

    Ok(ParsedScenePlan { plan, warnings })
}

fn parse_review_outcome(raw: &str) -> Result<ParsedReviewOutcome> {
    let cleaned = strip_code_fences(raw);

    if let Ok(payload) = serde_json::from_str::<ReviewPayload>(&cleaned) {
        return validate_review_payload(payload);
    }

    if let Ok(issues) = serde_json::from_str::<Vec<ReviewIssue>>(&cleaned) {
        let mut validated = validate_review_issues(issues)?;
        validated.warnings.push(
            "critic returned a legacy issue array without score; computed score from issue count."
                .to_string(),
        );
        let score = review_score_from_issue_count(validated.outcome.issues.len());
        validated.outcome.score = score;
        return Ok(validated);
    }

    Err(anyhow!(
        "critic returned invalid review JSON: excerpt: {}",
        compact_response_excerpt(&cleaned)
    ))
}

fn validate_review_payload(payload: ReviewPayload) -> Result<ParsedReviewOutcome> {
    let mut validated = validate_review_issues(payload.issues)?;
    validated.outcome.score = match payload.score {
        Some(score) => {
            let normalized = normalize_review_score(score);
            if normalized != score {
                validated.warnings.push(format!(
                    "critic returned out-of-range score {}; clamped to {}.",
                    score, normalized
                ));
            }
            normalized
        }
        None => {
            let computed = review_score_from_issue_count(validated.outcome.issues.len());
            validated
                .warnings
                .push("critic omitted score; computed score from issue count.".to_string());
            computed
        }
    };
    Ok(validated)
}

fn validate_review_issues(issues: Vec<ReviewIssue>) -> Result<ParsedReviewOutcome> {
    let mut warnings = Vec::new();
    let mut normalized = Vec::with_capacity(issues.len());

    for (index, mut issue) in issues.into_iter().enumerate() {
        if issue.description.trim().is_empty() {
            bail!("critic returned issue {} without description", index + 1);
        }
        issue.description = issue.description.trim().to_string();

        if issue.issue_type.trim().is_empty() {
            issue.issue_type = "analysis".to_string();
            warnings.push(format!(
                "critic omitted issue_type for issue {}; using `analysis`.",
                index + 1
            ));
        } else {
            issue.issue_type = issue.issue_type.trim().to_string();
        }

        if let (Some(start), Some(end)) = (issue.line_start, issue.line_end) {
            if end < start {
                issue.line_start = Some(end);
                issue.line_end = Some(start);
                warnings.push(format!(
                    "critic returned reversed line range for issue {}; normalized to {}-{}.",
                    index + 1,
                    end,
                    start
                ));
            }
        }

        normalized.push(issue);
    }

    Ok(ParsedReviewOutcome {
        outcome: ReviewOutcome {
            score: review_score_from_issue_count(normalized.len()),
            issues: normalized,
        },
        warnings,
    })
}

fn is_valid_bundle_role(value: &str) -> bool {
    matches!(value, "incident" | "escalation" | "cliffhanger")
}

fn compact_response_excerpt(raw: &str) -> String {
    let flattened = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_chars(flattened.trim(), 220)
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
        parse_review_outcome, parse_scene_plan, CliNovelBackend, NovelBackend, ReviewRequest,
        RewriteRequest, SceneGenerationRequest, WorldExpansionRequest,
    };
    use crate::config::NovelSettings;
    use crate::llm_runner::PromptRunner;
    use crate::models::{MemoryBundle, Scene, StoryState};
    use anyhow::{anyhow, Result};
    use std::collections::{BTreeMap, VecDeque};
    use std::sync::{Arc, Mutex};

    #[test]
    fn parse_review_outcome_reads_object_score() -> Result<()> {
        let outcome = parse_review_outcome(
            r#"{"score":82,"issues":[{"issue_type":"pacing","description":"Tighten the middle.","line_start":3,"line_end":5}]}"#,
        )?;

        assert_eq!(outcome.outcome.score, 82);
        assert_eq!(outcome.outcome.issues.len(), 1);
        assert_eq!(outcome.outcome.issues[0].issue_type, "pacing");
        assert!(outcome.warnings.is_empty());
        Ok(())
    }

    #[test]
    fn parse_review_outcome_keeps_array_compatibility() -> Result<()> {
        let outcome = parse_review_outcome(
            r#"[{"issue_type":"logic","description":"Clarify the leap.","line_start":7,"line_end":9}]"#,
        )?;

        assert_eq!(outcome.outcome.score, 88);
        assert_eq!(outcome.outcome.issues.len(), 1);
        assert_eq!(outcome.outcome.issues[0].issue_type, "logic");
        assert!(outcome
            .warnings
            .iter()
            .any(|warning| warning.contains("legacy issue array")));
        Ok(())
    }

    #[test]
    fn parse_scene_plan_rejects_missing_required_fields() {
        let error = parse_scene_plan(
            r#"{"bundle":1,"scene_number":1,"bundle_role":"incident","goal":"","conflict":"A","outcome":"B"}"#,
            1,
            1,
            3,
        )
        .expect_err("planner payload without goal should fail");

        assert!(
            error.to_string().contains("without short_title")
                || error.to_string().contains("without goal")
        );
    }

    #[test]
    fn parse_scene_plan_normalizes_mismatched_role_with_warning() -> Result<()> {
        let parsed = parse_scene_plan(
            r#"{"bundle":99,"scene_number":3,"short_title":"Wrong Turn","bundle_role":"incident","goal":"Push through the vault.","conflict":"The guard recognizes her.","outcome":"She gets the ledger but trips the alarm."}"#,
            1,
            2,
            3,
        )?;

        assert_eq!(parsed.plan.bundle, 1);
        assert_eq!(parsed.plan.scene_number, 2);
        assert_eq!(parsed.plan.bundle_role, "escalation");
        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.contains("using current bundle")));
        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.contains("bundle policy expects `escalation`")));
        Ok(())
    }

    #[test]
    fn parse_review_outcome_rejects_freeform_text() {
        let error = parse_review_outcome("This scene feels thin in the middle.")
            .expect_err("freeform review payload should fail");

        assert!(error
            .to_string()
            .contains("critic returned invalid review JSON"));
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
                r#"{"bundle":1,"scene_number":1,"short_title":"Signal at the Gate","bundle_role":"incident","goal":"The investigator confirms the missing courier route.","conflict":"A clerk refuses access without a dead supervisor's code.","outcome":"The route is found, but the code points to a sealed dock ledger."}"#,
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
            bundle_scene_target: 3,
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
            bundle: 1,
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

    #[test]
    fn cli_novel_backend_rejects_invalid_planner_payload() {
        let backend = CliNovelBackend::new(Arc::new(FixturePromptRunner::from_pairs(&[(
            "planner",
            "not valid json",
        )])));

        let error = backend
            .generate_scene(SceneGenerationRequest {
                state: StoryState::default(),
                novel: NovelSettings {
                    title: "Glass Harbor".to_string(),
                    genre: "Mystery".to_string(),
                    tone: "Tense, atmospheric".to_string(),
                    premise: "An investigator follows edited records.".to_string(),
                    protagonist_name: "Yunseo".to_string(),
                    language: "ko".to_string(),
                    bundle_scene_target: 3,
                    ..NovelSettings::default()
                },
                memory: MemoryBundle::default(),
                planner_story_foundation: "Plot foundation".to_string(),
                writer_story_foundation: "Writer foundation".to_string(),
                editor_story_foundation: "Editor foundation".to_string(),
                bundle: 1,
                scene_number: 1,
                scene_id: "scene_001_001".to_string(),
                allow_dummy_fallback: false,
            })
            .expect_err("invalid planner payload should fail");

        assert!(error
            .to_string()
            .contains("planner returned invalid scene plan JSON"));
    }

    #[test]
    fn cli_novel_backend_rejects_invalid_critic_payload() {
        let backend = CliNovelBackend::new(Arc::new(FixturePromptRunner::from_pairs(&[(
            "critic",
            "This needs stronger pacing.",
        )])));

        let error = backend
            .review_scene(ReviewRequest {
                state: StoryState::default(),
                novel: NovelSettings {
                    title: "Glass Harbor".to_string(),
                    genre: "Mystery".to_string(),
                    tone: "Tense, atmospheric".to_string(),
                    premise: "An investigator follows edited records.".to_string(),
                    protagonist_name: "Yunseo".to_string(),
                    language: "ko".to_string(),
                    bundle_scene_target: 3,
                    ..NovelSettings::default()
                },
                memory: MemoryBundle::default(),
                critic_story_foundation: "Critic foundation".to_string(),
                scene: Scene {
                    id: "scene_001_001".to_string(),
                    bundle: 1,
                    scene_number: 1,
                    short_title: "Signal at the Gate".to_string(),
                    bundle_role: "incident".to_string(),
                    goal: "Confirm the route.".to_string(),
                    conflict: "The clerk refuses access.".to_string(),
                    outcome: "The route points to a sealed ledger.".to_string(),
                    text: "Yunseo forced the ledger drawer open.".to_string(),
                    status: "draft".to_string(),
                },
                allow_dummy_fallback: false,
            })
            .expect_err("invalid critic payload should fail");

        assert!(error
            .to_string()
            .contains("critic returned invalid review JSON"));
    }
}
