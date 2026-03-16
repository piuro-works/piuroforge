use anyhow::Result;
use clap::ValueEnum;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct OutputField {
    pub label: String,
    pub value: String,
}

impl OutputField {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Artifact {
    pub kind: String,
    pub path: String,
}

impl Artifact {
    pub fn new(kind: impl Into<String>, path: impl AsRef<Path>) -> Self {
        Self {
            kind: kind.into(),
            path: path.as_ref().display().to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandOutput {
    pub status: &'static str,
    pub command: String,
    pub workspace: String,
    pub summary: String,
    pub details: Vec<OutputField>,
    pub artifacts: Vec<Artifact>,
    pub next_steps: Vec<String>,
    pub body: Option<String>,
    pub warnings: Vec<String>,
}

impl CommandOutput {
    pub fn ok(
        command: impl Into<String>,
        workspace: impl AsRef<Path>,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            status: "ok",
            command: command.into(),
            workspace: workspace.as_ref().display().to_string(),
            summary: summary.into(),
            details: Vec::new(),
            artifacts: Vec::new(),
            next_steps: Vec::new(),
            body: None,
            warnings: Vec::new(),
        }
    }

    pub fn detail(mut self, label: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.push(OutputField::new(label, value));
        self
    }

    pub fn artifact(mut self, kind: impl Into<String>, path: impl AsRef<Path>) -> Self {
        self.artifacts.push(Artifact::new(kind, path));
        self
    }

    pub fn next_step(mut self, command: impl Into<String>) -> Self {
        self.next_steps.push(command.into());
        self
    }

    pub fn warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }

    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    pub fn render_text(&self) -> String {
        let mut lines = vec![self.summary.clone()];
        lines.push(format!("Command: {}", self.command));
        lines.push(format!("Workspace: {}", self.workspace));

        if !self.details.is_empty() {
            lines.push(String::new());
            lines.push("Details:".to_string());
            for detail in &self.details {
                lines.push(format!("- {}: {}", detail.label, detail.value));
            }
        }

        if !self.artifacts.is_empty() {
            lines.push(String::new());
            lines.push("Artifacts:".to_string());
            for artifact in &self.artifacts {
                lines.push(format!("- {}: {}", artifact.kind, artifact.path));
            }
        }

        if !self.next_steps.is_empty() {
            lines.push(String::new());
            lines.push("Next steps:".to_string());
            for next in &self.next_steps {
                lines.push(format!("- {}", next));
            }
        }

        if !self.warnings.is_empty() {
            lines.push(String::new());
            lines.push("Warnings:".to_string());
            for warning in &self.warnings {
                lines.push(format!("- {}", warning));
            }
        }

        if let Some(body) = &self.body {
            lines.push(String::new());
            lines.push("Body:".to_string());
            lines.push(body.trim_end().to_string());
        }

        lines.join("\n")
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorOutput {
    pub status: &'static str,
    pub command: String,
    pub workspace: Option<String>,
    pub error_code: String,
    pub reason: String,
    pub remediation: Vec<String>,
    pub example_command: Option<String>,
    pub details: Vec<OutputField>,
}

impl ErrorOutput {
    pub fn from_error(command: &str, workspace: Option<&Path>, error: &anyhow::Error) -> Self {
        let reason = error.to_string();
        let workspace_display = workspace.map(|path| path.display().to_string());

        if reason.contains("missing required novel config") {
            let mut details = Vec::new();
            if let Some(path) = workspace {
                details.push(OutputField::new(
                    "workspace_config",
                    path.join("novel.toml").display().to_string(),
                ));
            }
            return Self {
                status: "error",
                command: command.to_string(),
                workspace: workspace_display.clone(),
                error_code: "missing_novel_config".to_string(),
                reason,
                remediation: vec![
                    workspace
                        .map(|path| {
                            format!(
                                "Edit {} and fill the missing fields.",
                                path.join("novel.toml").display()
                            )
                        })
                        .unwrap_or_else(|| {
                            "Edit novel.toml and fill the missing fields.".to_string()
                        }),
                    example_for(command, workspace),
                ],
                example_command: Some(example_for(command, workspace)),
                details,
            };
        }

        if looks_like_codex_error(&reason) {
            let mut remediation = vec![
                "Run: codex login".to_string(),
                "Verify that the codex binary is installed and available on PATH.".to_string(),
            ];
            if looks_like_network_error(&reason) {
                remediation.push(
                    "Check the current network, DNS, proxy, or VPN path before retrying codex."
                        .to_string(),
                );
            } else {
                remediation.push(
                    "Retry after confirming the codex session is healthy and can reach its backend."
                        .to_string(),
                );
            }
            remediation.push(
                "If you intentionally want placeholder output, opt in with `allow_dummy_fallback = true` in ~/.config/heeforge/config.toml or `HEEFORGE_ALLOW_DUMMY=true`."
                    .to_string(),
            );
            remediation.push(example_for(command, workspace));
            return Self {
                status: "error",
                command: command.to_string(),
                workspace: workspace_display,
                error_code: "codex_unavailable".to_string(),
                reason,
                remediation,
                example_command: Some(example_for(command, workspace)),
                details: vec![],
            };
        }

        if reason.contains("no current scene available to review") {
            return Self {
                status: "error",
                command: command.to_string(),
                workspace: workspace_display,
                error_code: "no_current_scene".to_string(),
                reason,
                remediation: vec![
                    "Generate a scene before running review.".to_string(),
                    workspace
                        .map(|path| {
                            format!("Run: heeforge --workspace {} next-scene", path.display())
                        })
                        .unwrap_or_else(|| "Run: heeforge next-scene".to_string()),
                ],
                example_command: Some(example_for("next-scene", workspace)),
                details: vec![],
            };
        }

        if reason.contains("scene order is invalid") {
            return Self {
                status: "error",
                command: command.to_string(),
                workspace: workspace_display,
                error_code: "invalid_scene_sequence".to_string(),
                reason,
                remediation: vec![
                    "Check scene files and ensure each chapter has scene numbers 001..N without gaps.".to_string(),
                    "Re-run next-chapter after fixing the missing or duplicated scene numbers.".to_string(),
                ],
                example_command: Some(example_for("next-chapter", workspace)),
                details: vec![],
            };
        }

        if reason.contains("no scenes found for chapter") {
            return Self {
                status: "error",
                command: command.to_string(),
                workspace: workspace_display,
                error_code: "empty_chapter".to_string(),
                reason,
                remediation: vec![
                    "Generate at least one scene before compiling a chapter.".to_string(),
                    example_for("next-scene", workspace),
                ],
                example_command: Some(example_for("next-scene", workspace)),
                details: vec![],
            };
        }

        Self {
            status: "error",
            command: command.to_string(),
            workspace: workspace_display,
            error_code: "command_failed".to_string(),
            reason,
            remediation: vec![
                "Read the reason above and fix the workspace or input data.".to_string(),
                example_for(command, workspace),
            ],
            example_command: Some(example_for(command, workspace)),
            details: vec![],
        }
    }

    pub fn render_text(&self) -> String {
        let mut lines = vec!["Command failed.".to_string()];
        lines.push(format!("Command: {}", self.command));
        if let Some(workspace) = &self.workspace {
            lines.push(format!("Workspace: {}", workspace));
        }
        lines.push(format!("Error code: {}", self.error_code));
        lines.push(format!("Reason: {}", self.reason));

        if !self.details.is_empty() {
            lines.push(String::new());
            lines.push("Details:".to_string());
            for detail in &self.details {
                lines.push(format!("- {}: {}", detail.label, detail.value));
            }
        }

        if !self.remediation.is_empty() {
            lines.push(String::new());
            lines.push("How to fix:".to_string());
            for item in &self.remediation {
                lines.push(format!("- {}", item));
            }
        }

        if let Some(example) = &self.example_command {
            lines.push(String::new());
            lines.push("Example:".to_string());
            lines.push(example.clone());
        }

        lines.join("\n")
    }
}

pub fn emit_command(output: &CommandOutput, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Text => {
            println!("{}", output.render_text());
            Ok(())
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(output)?);
            Ok(())
        }
    }
}

pub fn emit_error(output: &ErrorOutput, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Text => {
            eprintln!("{}", output.render_text());
            Ok(())
        }
        OutputFormat::Json => {
            eprintln!("{}", serde_json::to_string_pretty(output)?);
            Ok(())
        }
    }
}

fn example_for(command: &str, workspace: Option<&Path>) -> String {
    match workspace {
        Some(path) => format!("heeforge --workspace {} {}", path.display(), command),
        None => format!("heeforge {}", command),
    }
}

fn looks_like_codex_error(reason: &str) -> bool {
    reason.contains("codex login")
        || reason.contains("codex CLI")
        || reason.contains("chatgpt.com/backend-api/codex")
}

fn looks_like_network_error(reason: &str) -> bool {
    let normalized = reason.to_ascii_lowercase();
    normalized.contains("failed to lookup address information")
        || normalized.contains("dns")
        || normalized.contains("network")
        || normalized.contains("error sending request for url")
        || normalized.contains("stream disconnected")
        || normalized.contains("connection reset")
        || normalized.contains("connection refused")
        || normalized.contains("timed out")
}
