use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};

pub const WORKSPACE_DIR_NAME: &str = ".novel";
pub const WORKSPACE_MANIFEST_FILE: &str = "workspace.json";
pub const WORKSPACE_CONFIG_FILE: &str = "novel.toml";
pub const GLOBAL_CONFIG_DIR_NAME: &str = "heeforge";
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
    pub logs_dir: PathBuf,
    pub global_settings: GlobalSettings,
    pub novel_settings: NovelSettings,
    pub allow_dummy_fallback: bool,
    pub codex_command: String,
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
        let scenes_dir = novel_dir.join("scenes");
        let chapters_dir = novel_dir.join("chapters");
        let logs_dir = novel_dir.join("logs");

        let global_settings = load_toml_or_default::<GlobalSettings>(&global_config_path)?;
        let mut novel_settings = load_toml_or_default::<NovelSettings>(&workspace_config_path)?;
        if novel_settings.title.trim().is_empty() {
            novel_settings.title = default_title_from_path(&workspace_dir);
        }

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
            logs_dir,
            allow_dummy_fallback: env_flag("HEEFORGE_ALLOW_DUMMY")
                .or_else(|| env_flag("NOVEL_ENGINE_ALLOW_DUMMY"))
                .unwrap_or(global_settings.allow_dummy_fallback),
            codex_command: env::var("HEEFORGE_CODEX_CMD")
                .or_else(|_| env::var("NOVEL_ENGINE_CODEX_CMD"))
                .unwrap_or_else(|_| global_settings.codex_command.clone()),
            global_settings,
            novel_settings,
        })
    }

    pub fn workspace_name(&self) -> String {
        self.workspace_dir
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .unwrap_or("heeforge-workspace")
            .to_string()
    }

    pub fn novel_title(&self) -> &str {
        self.novel_settings.title.as_str()
    }

    pub fn render_global_config(&self) -> Result<String> {
        toml::to_string_pretty(&self.global_settings).context("failed to serialize global config")
    }

    pub fn render_workspace_config(&self) -> Result<String> {
        toml::to_string_pretty(&self.novel_settings).context("failed to serialize workspace config")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GlobalSettings {
    #[serde(default = "default_config_version")]
    pub version: u32,
    #[serde(default = "default_codex_command")]
    pub codex_command: String,
    #[serde(default = "default_allow_dummy_fallback")]
    pub allow_dummy_fallback: bool,
    #[serde(default = "default_default_language")]
    pub default_language: String,
    #[serde(default)]
    pub default_workspace_root: Option<String>,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            version: default_config_version(),
            codex_command: default_codex_command(),
            allow_dummy_fallback: default_allow_dummy_fallback(),
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
    if let Ok(path) = env::var("HEEFORGE_CONFIG_DIR") {
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

fn default_title_from_path(path: &Path) -> String {
    let raw = path
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("heeforge-workspace");

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

fn default_allow_dummy_fallback() -> bool {
    true
}

fn default_default_language() -> String {
    "ko".to_string()
}

fn default_genre() -> String {
    "Mystery".to_string()
}

fn default_tone() -> String {
    "Focused, cinematic, character-driven".to_string()
}
