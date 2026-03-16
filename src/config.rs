use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};

pub const DEFAULT_LLM_BACKEND: &str = "codex_cli";
pub const SUPPORTED_LLM_BACKENDS: &[&str] = &[DEFAULT_LLM_BACKEND];

pub const WORKSPACE_DIR_NAME: &str = ".novel";
pub const WORKSPACE_MANIFEST_FILE: &str = "workspace.json";
pub const WORKSPACE_CONFIG_FILE: &str = "novel.toml";
pub const GLOBAL_CONFIG_DIR_NAME: &str = "piuroforge";
pub const GLOBAL_CONFIG_FILE: &str = "config.toml";

#[derive(Debug, Clone)]
pub struct Config {
    pub workspace_dir: PathBuf,
    pub novel_dir: PathBuf,
    pub global_config_dir: PathBuf,
    pub global_config_path: PathBuf,
    pub workspace_config_path: PathBuf,
    pub workspace_manifest_path: PathBuf,
    pub state_path: PathBuf,
    pub memory_dir: PathBuf,
    pub scenes_dir: PathBuf,
    pub chapters_dir: PathBuf,
    pub review_feedback_dir: PathBuf,
    pub review_revisions_dir: PathBuf,
    pub workspace_readme_path: PathBuf,
    pub logs_dir: PathBuf,
    pub global_settings: GlobalSettings,
    pub novel_settings: NovelSettings,
    pub allow_dummy_fallback: bool,
    pub log_prompts: bool,
    pub workspace_auto_commit: bool,
    pub llm_backend: String,
    pub codex_command: String,
    pub codex_timeout_secs: u64,
}

impl Config {
    pub fn new(workspace_dir: impl Into<PathBuf>) -> Result<Self> {
        let global_config_dir = resolve_global_config_dir()?;
        Self::with_global_config_dir(workspace_dir, global_config_dir)
    }

    pub fn with_global_config_dir(
        workspace_dir: impl Into<PathBuf>,
        global_config_dir: impl Into<PathBuf>,
    ) -> Result<Self> {
        let workspace_dir = workspace_dir.into();
        let global_config_dir = global_config_dir.into();
        let novel_dir = workspace_dir.join(WORKSPACE_DIR_NAME);
        let global_config_path = global_config_dir.join(GLOBAL_CONFIG_FILE);
        let workspace_config_path = workspace_dir.join(WORKSPACE_CONFIG_FILE);
        let workspace_manifest_path = novel_dir.join(WORKSPACE_MANIFEST_FILE);
        let state_path = novel_dir.join("state").join("project_state.json");
        let memory_dir = novel_dir.join("memory");
        let scenes_dir = workspace_dir.join("02_Draft").join("Scenes");
        let chapters_dir = workspace_dir.join("02_Draft").join("Chapters");
        let review_feedback_dir = workspace_dir.join("06_Review").join("Feedback");
        let review_revisions_dir = workspace_dir.join("06_Review").join("Revisions");
        let workspace_readme_path = workspace_dir.join("README.md");
        let logs_dir = novel_dir.join("logs");

        let global_settings = load_toml_or_default::<GlobalSettings>(&global_config_path)?;
        let mut novel_settings = load_toml_or_default::<NovelSettings>(&workspace_config_path)?;
        if novel_settings.title.trim().is_empty() {
            novel_settings.title = default_title_from_path(&workspace_dir);
        }
        let llm_backend = env::var("PIUROFORGE_LLM_BACKEND")
            .or_else(|_| env::var("NOVEL_ENGINE_LLM_BACKEND"))
            .unwrap_or_else(|_| global_settings.llm_backend.clone());
        let llm_backend = normalize_llm_backend(&llm_backend)?;

        Ok(Self {
            workspace_dir,
            novel_dir,
            global_config_dir,
            global_config_path,
            workspace_config_path,
            workspace_manifest_path,
            state_path,
            memory_dir,
            scenes_dir,
            chapters_dir,
            review_feedback_dir,
            review_revisions_dir,
            workspace_readme_path,
            logs_dir,
            allow_dummy_fallback: env_flag("PIUROFORGE_ALLOW_DUMMY")
                .or_else(|| env_flag("NOVEL_ENGINE_ALLOW_DUMMY"))
                .unwrap_or(global_settings.allow_dummy_fallback),
            log_prompts: env_flag("PIUROFORGE_LOG_PROMPTS")
                .or_else(|| env_flag("NOVEL_ENGINE_LOG_PROMPTS"))
                .unwrap_or(global_settings.log_prompts),
            workspace_auto_commit: env_flag("PIUROFORGE_WORKSPACE_AUTO_COMMIT")
                .or_else(|| env_flag("NOVEL_ENGINE_WORKSPACE_AUTO_COMMIT"))
                .unwrap_or(global_settings.workspace_auto_commit),
            llm_backend,
            codex_command: env::var("PIUROFORGE_CODEX_CMD")
                .or_else(|_| env::var("NOVEL_ENGINE_CODEX_CMD"))
                .unwrap_or_else(|_| global_settings.codex_command.clone()),
            codex_timeout_secs: env_u64("PIUROFORGE_CODEX_TIMEOUT_SECS")
                .or_else(|| env_u64("NOVEL_ENGINE_CODEX_TIMEOUT_SECS"))
                .unwrap_or(global_settings.codex_timeout_secs),
            global_settings,
            novel_settings,
        })
    }

    pub fn workspace_name(&self) -> String {
        self.workspace_dir
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .unwrap_or("piuroforge-workspace")
            .to_string()
    }

    pub fn novel_title(&self) -> &str {
        self.novel_settings.title.as_str()
    }

    pub fn render_global_config(&self) -> Result<String> {
        Ok(format!(
            "# PiuroForge global settings\n\
#\n\
# First run for writers:\n\
# 1. Open a terminal once and run: codex login\n\
# 2. Keep llm_backend = \"codex_cli\" unless you intentionally install a custom PiuroForge backend build later\n\
# 3. Leave allow_dummy_fallback = false for real drafting\n\
# 4. Turn allow_dummy_fallback = true on only if you intentionally want placeholder text for workflow testing\n\
# 5. Turn workspace_auto_commit = true on if you want Git history created automatically inside each novel workspace\n\
#\n\
# If PiuroForge shows `codex_unavailable`, the usual causes are:\n\
# - codex login has not been completed yet\n\
# - this machine cannot reach the Codex service because of internet, DNS, VPN, or proxy issues\n\
\n\
version = {version}\n\
llm_backend = {llm_backend:?}\n\
codex_command = {codex_command:?}\n\
codex_timeout_secs = {codex_timeout_secs}\n\
allow_dummy_fallback = {allow_dummy_fallback}\n\
log_prompts = {log_prompts}\n\
workspace_auto_commit = {workspace_auto_commit}\n\
default_language = {default_language:?}\n\
{default_workspace_root}",
            version = self.global_settings.version,
            llm_backend = self.global_settings.llm_backend,
            codex_command = self.global_settings.codex_command,
            codex_timeout_secs = self.global_settings.codex_timeout_secs,
            allow_dummy_fallback = self.global_settings.allow_dummy_fallback,
            log_prompts = self.global_settings.log_prompts,
            workspace_auto_commit = self.global_settings.workspace_auto_commit,
            default_language = self.global_settings.default_language,
            default_workspace_root = render_default_workspace_root(
                self.global_settings.default_workspace_root.as_deref(),
            ),
        ))
    }

    pub fn render_workspace_config(&self) -> Result<String> {
        let mut rendered = String::from(
            "# PiuroForge novel workspace settings\n\
#\n\
# Writing policy defaults:\n\
# - scene is the primary drafting unit; serialized workflows often treat one scene as one upload episode\n\
# - serialized_workflow = true keeps the day-to-day loop scene-first and rolls internal chapter boundaries automatically after approval\n\
# - chapter_scene_target = 3 means each internal chapter should usually draft as incident -> escalation -> cliffhanger\n\
# - Fill the story bible before serious drafting so planner and writer have real canon to follow\n\
\n",
        );
        rendered.push_str(&format!("version = {}\n", self.novel_settings.version));
        rendered.push_str(&format!("title = {:?}\n", self.novel_settings.title));
        if let Some(author) = &self.novel_settings.author {
            rendered.push_str(&format!("author = {:?}\n", author));
        }
        rendered.push_str(&format!("language = {:?}\n", self.novel_settings.language));
        rendered.push_str(&format!("genre = {:?}\n", self.novel_settings.genre));
        rendered.push_str(&format!("tone = {:?}\n", self.novel_settings.tone));
        rendered.push_str(&format!("premise = {:?}\n", self.novel_settings.premise));
        rendered.push_str(&format!(
            "protagonist_name = {:?}\n",
            self.novel_settings.protagonist_name
        ));
        rendered.push_str(&format!(
            "serialized_workflow = {}\n",
            self.novel_settings.serialized_workflow
        ));
        rendered.push_str(&format!(
            "chapter_scene_target = {}\n",
            self.novel_settings.chapter_scene_target.max(1)
        ));
        Ok(rendered)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GlobalSettings {
    #[serde(default = "default_config_version")]
    pub version: u32,
    #[serde(default = "default_llm_backend")]
    pub llm_backend: String,
    #[serde(default = "default_codex_command")]
    pub codex_command: String,
    #[serde(default = "default_codex_timeout_secs")]
    pub codex_timeout_secs: u64,
    #[serde(default = "default_allow_dummy_fallback")]
    pub allow_dummy_fallback: bool,
    #[serde(default = "default_log_prompts")]
    pub log_prompts: bool,
    #[serde(default = "default_workspace_auto_commit")]
    pub workspace_auto_commit: bool,
    #[serde(default = "default_default_language")]
    pub default_language: String,
    #[serde(default)]
    pub default_workspace_root: Option<String>,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            version: default_config_version(),
            llm_backend: default_llm_backend(),
            codex_command: default_codex_command(),
            codex_timeout_secs: default_codex_timeout_secs(),
            allow_dummy_fallback: default_allow_dummy_fallback(),
            log_prompts: default_log_prompts(),
            workspace_auto_commit: default_workspace_auto_commit(),
            default_language: default_default_language(),
            default_workspace_root: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NovelSettings {
    #[serde(default = "default_config_version")]
    pub version: u32,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default = "default_default_language")]
    pub language: String,
    #[serde(default = "default_genre")]
    pub genre: String,
    #[serde(default = "default_tone")]
    pub tone: String,
    #[serde(default)]
    pub premise: String,
    #[serde(default)]
    pub protagonist_name: String,
    #[serde(default = "default_serialized_workflow")]
    pub serialized_workflow: bool,
    #[serde(default = "default_chapter_scene_target")]
    pub chapter_scene_target: u32,
}

impl Default for NovelSettings {
    fn default() -> Self {
        Self {
            version: default_config_version(),
            title: String::new(),
            author: None,
            language: default_default_language(),
            genre: default_genre(),
            tone: default_tone(),
            premise: String::new(),
            protagonist_name: String::new(),
            serialized_workflow: default_serialized_workflow(),
            chapter_scene_target: default_chapter_scene_target(),
        }
    }
}

impl NovelSettings {
    pub fn missing_required_fields(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();

        if self.title.trim().is_empty() {
            missing.push("title");
        }
        if self.genre.trim().is_empty() {
            missing.push("genre");
        }
        if self.tone.trim().is_empty() {
            missing.push("tone");
        }
        if self.premise.trim().is_empty() {
            missing.push("premise");
        }
        if self.protagonist_name.trim().is_empty() {
            missing.push("protagonist_name");
        }
        if self.language.trim().is_empty() {
            missing.push("language");
        }

        missing
    }
}

fn default_serialized_workflow() -> bool {
    false
}

fn default_chapter_scene_target() -> u32 {
    3
}

fn load_toml_or_default<T>(path: &Path) -> Result<T>
where
    T: for<'de> Deserialize<'de> + Default,
{
    if !path.exists() {
        return Ok(T::default());
    }

    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("failed to parse {}", path.display()))
}

fn resolve_global_config_dir() -> Result<PathBuf> {
    if let Ok(path) = env::var("PIUROFORGE_CONFIG_DIR") {
        return Ok(PathBuf::from(path));
    }

    if let Ok(path) = env::var("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(path).join(GLOBAL_CONFIG_DIR_NAME));
    }

    let home = env::var("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home)
        .join(".config")
        .join(GLOBAL_CONFIG_DIR_NAME))
}

fn env_flag(name: &str) -> Option<bool> {
    match env::var(name) {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "0" | "false" | "no" | "off" => Some(false),
            "1" | "true" | "yes" | "on" => Some(true),
            _ => None,
        },
        Err(_) => None,
    }
}

fn env_u64(name: &str) -> Option<u64> {
    match env::var(name) {
        Ok(value) => value.trim().parse::<u64>().ok(),
        Err(_) => None,
    }
}

fn default_title_from_path(path: &Path) -> String {
    let raw = path
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("piuroforge-workspace");

    raw.split(['-', '_', ' '])
        .filter(|segment| !segment.is_empty())
        .map(title_case)
        .collect::<Vec<_>>()
        .join(" ")
}

fn title_case(segment: &str) -> String {
    let mut chars = segment.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let first = first.to_uppercase().collect::<String>();
    let rest = chars.as_str().to_ascii_lowercase();
    format!("{first}{rest}")
}

fn default_config_version() -> u32 {
    1
}

fn default_codex_command() -> String {
    "codex".to_string()
}

fn default_llm_backend() -> String {
    DEFAULT_LLM_BACKEND.to_string()
}

fn default_codex_timeout_secs() -> u64 {
    120
}

fn default_allow_dummy_fallback() -> bool {
    false
}

fn default_default_language() -> String {
    "ko".to_string()
}

fn default_log_prompts() -> bool {
    false
}

fn default_workspace_auto_commit() -> bool {
    false
}

fn render_default_workspace_root(value: Option<&str>) -> String {
    match value {
        Some(value) if !value.trim().is_empty() => {
            format!("default_workspace_root = {:?}\n", value.trim())
        }
        _ => String::new(),
    }
}

fn default_genre() -> String {
    "Mystery".to_string()
}

fn default_tone() -> String {
    "Focused, cinematic, character-driven".to_string()
}

fn normalize_llm_backend(value: &str) -> Result<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        anyhow::bail!(
            "unsupported llm backend ``. supported backends: {}",
            SUPPORTED_LLM_BACKENDS.join(", ")
        );
    }

    if SUPPORTED_LLM_BACKENDS.contains(&normalized.as_str()) {
        Ok(normalized)
    } else {
        anyhow::bail!(
            "unsupported llm backend `{}`. supported backends: {}",
            value.trim(),
            SUPPORTED_LLM_BACKENDS.join(", ")
        );
    }
}
