use anyhow::Result;
use std::io;
use std::process::Command;
use std::time::Duration;

use crate::codex_runner::CodexRunner;
use crate::config::{Config, SUPPORTED_LLM_BACKENDS};
use crate::output::CommandOutput;

pub fn run(config: &Config) -> Result<CommandOutput> {
    let workspace_manifest_exists = config.workspace_manifest_path.exists();
    let workspace_config_exists = config.workspace_config_path.exists();
    let global_config_exists = config.global_config_path.exists();
    let missing_fields = config.novel_settings.missing_required_fields();

    let version_probe = probe_codex_version(&config.codex_command);
    let codex_cli_status = match &version_probe {
        CodexVersionProbe::Ready { .. } => "installed",
        CodexVersionProbe::Missing => "missing",
        CodexVersionProbe::Failed { .. } => "error",
    };
    let codex_version = match &version_probe {
        CodexVersionProbe::Ready { version } => version.clone(),
        _ => "-".to_string(),
    };
    let probe_timeout_secs = config.codex_timeout_secs.min(15);

    let mut warnings = Vec::new();
    let mut next_steps = Vec::new();

    if !workspace_manifest_exists {
        warnings.push(
            "No PiuroForge workspace marker was found here yet. Run `piuroforge init` before drafting."
                .to_string(),
        );
        next_steps.push(format!(
            "piuroforge init {}",
            config.workspace_dir.display()
        ));
    }

    if !workspace_config_exists {
        warnings.push("`novel.toml` is missing in this workspace.".to_string());
        next_steps.push(format!(
            "piuroforge init {}",
            config.workspace_dir.display()
        ));
    }

    if !missing_fields.is_empty() {
        warnings.push(format!(
            "Required novel settings are still missing: {}.",
            missing_fields.join(", ")
        ));
        next_steps.push(format!("Edit {}", config.workspace_config_path.display()));
    }

    if !global_config_exists {
        warnings.push(format!(
            "Global settings file does not exist yet at {}. It will be created automatically by `piuroforge init`.",
            config.global_config_path.display()
        ));
    }

    let codex_connection = match version_probe {
        CodexVersionProbe::Missing => {
            warnings.push(
                "Codex CLI was not found on this machine. Install Codex CLI first, then run `codex login`."
                    .to_string(),
            );
            next_steps.push("Install Codex CLI".to_string());
            next_steps.push("codex login".to_string());
            CodexConnection::Missing
        }
        CodexVersionProbe::Failed { detail } => {
            warnings.push(format!(
                "Codex CLI exists but `--version` failed: {}.",
                detail
            ));
            next_steps.push("Open a terminal and run: codex login".to_string());
            CodexConnection::Unavailable
        }
        CodexVersionProbe::Ready { .. } => match probe_codex_connection(config) {
            Ok(()) => CodexConnection::Ready,
            Err(error) => {
                if looks_like_network_error(&error) {
                    warnings.push(
                        "Codex CLI is installed, but this machine could not reach the Codex service. Check internet, DNS, VPN, or proxy settings."
                            .to_string(),
                    );
                } else {
                    warnings.push(
                        "Codex CLI is installed, but the live Codex check did not complete. Run `codex login` again and retry."
                            .to_string(),
                    );
                }
                warnings.push(format!("Codex check detail: {}.", compact_message(&error)));
                next_steps.push("Open a terminal and run: codex login".to_string());
                next_steps.push("piuroforge doctor".to_string());
                CodexConnection::Unavailable
            }
        },
    };

    if config.allow_dummy_fallback {
        warnings.push(
            "Dummy fallback is ON. PiuroForge can produce placeholder text instead of live Codex output."
                .to_string(),
        );
    }

    let ready_to_draft = workspace_manifest_exists
        && workspace_config_exists
        && missing_fields.is_empty()
        && matches!(codex_connection, CodexConnection::Ready)
        && !config.allow_dummy_fallback;

    if ready_to_draft {
        next_steps.push(format!(
            "piuroforge --workspace {} next-scene",
            config.workspace_dir.display()
        ));
    } else if workspace_manifest_exists && workspace_config_exists && missing_fields.is_empty() {
        next_steps.push("Open a terminal and run: codex login".to_string());
    }

    if config.allow_dummy_fallback {
        next_steps.push(format!("Edit {}", config.global_config_path.display()));
    }

    dedup(&mut next_steps);

    let summary = if ready_to_draft {
        "Doctor check passed. PiuroForge is ready for real drafting."
    } else {
        "Doctor found setup issues to fix before real drafting."
    };

    let mut output = CommandOutput::ok("doctor", &config.workspace_dir, summary)
        .detail("llm_backend", &config.llm_backend)
        .detail("auth_mode", "codex_cli")
        .detail("setup_flow", "init_then_doctor")
        .detail("supported_llm_backends", SUPPORTED_LLM_BACKENDS.join(", "))
        .detail(
            "workspace_ready",
            yes_no(workspace_manifest_exists && workspace_config_exists),
        )
        .detail("workspace_manifest", yes_no(workspace_manifest_exists))
        .detail("workspace_config", yes_no(workspace_config_exists))
        .detail("global_config", yes_no(global_config_exists))
        .detail("codex_command", config.codex_command.clone())
        .detail("codex_cli", codex_cli_status)
        .detail("codex_version", codex_version)
        .detail("codex_connection", codex_connection.as_str())
        .detail("codex_probe_timeout_secs", probe_timeout_secs.to_string())
        .detail(
            "allow_dummy_fallback",
            config.allow_dummy_fallback.to_string(),
        )
        .detail(
            "workspace_auto_commit",
            config.workspace_auto_commit.to_string(),
        )
        .detail(
            "missing_required_fields",
            render_missing_fields(&missing_fields),
        )
        .detail(
            "setup_complete_when",
            "doctor reports codex_connection=ready and missing_required_fields=none",
        )
        .artifact("global_config", &config.global_config_path);

    if workspace_config_exists {
        output = output.artifact("workspace_config", &config.workspace_config_path);
    }

    if workspace_manifest_exists {
        output = output.artifact("workspace_manifest", &config.workspace_manifest_path);
    }

    for warning in warnings {
        output = output.warning(warning);
    }

    for next_step in next_steps {
        output = output.next_step(next_step);
    }

    output = output.body(render_doctor_body(
        &config.llm_backend,
        workspace_manifest_exists,
        workspace_config_exists,
        &missing_fields,
        &codex_connection,
        config.allow_dummy_fallback,
        config.workspace_auto_commit,
    ));

    Ok(output)
}

#[derive(Debug, Clone)]
enum CodexVersionProbe {
    Ready { version: String },
    Missing,
    Failed { detail: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodexConnection {
    Ready,
    Unavailable,
    Missing,
}

impl CodexConnection {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Unavailable => "needs_attention",
            Self::Missing => "missing",
        }
    }
}

fn probe_codex_version(command: &str) -> CodexVersionProbe {
    match Command::new(command).arg("--version").output() {
        Ok(output) if output.status.success() => CodexVersionProbe::Ready {
            version: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        },
        Ok(output) => {
            let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let detail = if detail.is_empty() {
                format!("exit status {}", output.status)
            } else {
                detail
            };
            CodexVersionProbe::Failed { detail }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => CodexVersionProbe::Missing,
        Err(error) => CodexVersionProbe::Failed {
            detail: error.to_string(),
        },
    }
}

fn probe_codex_connection(config: &Config) -> Result<()> {
    let runner = CodexRunner::new(
        config.codex_command.clone(),
        Duration::from_secs(config.codex_timeout_secs.min(15)),
    );
    let response = runner.run_prompt_named("doctor", "Reply with OK only.")?;
    if response.trim().is_empty() {
        anyhow::bail!("codex returned an empty healthcheck response");
    }
    Ok(())
}

fn render_missing_fields(missing_fields: &[&str]) -> String {
    if missing_fields.is_empty() {
        "none".to_string()
    } else {
        missing_fields.join(", ")
    }
}

fn render_doctor_body(
    llm_backend: &str,
    workspace_manifest_exists: bool,
    workspace_config_exists: bool,
    missing_fields: &[&str],
    codex_connection: &CodexConnection,
    allow_dummy_fallback: bool,
    workspace_auto_commit: bool,
) -> String {
    let workspace_state = if workspace_manifest_exists && workspace_config_exists {
        "workspace files are present"
    } else {
        "workspace files are incomplete"
    };
    let config_state = if missing_fields.is_empty() {
        "required novel settings are filled"
    } else {
        "required novel settings still need attention"
    };
    let codex_state = match codex_connection {
        CodexConnection::Ready => "live Codex check succeeded",
        CodexConnection::Unavailable => "live Codex check failed",
        CodexConnection::Missing => "Codex CLI is missing",
    };
    let fallback_state = if allow_dummy_fallback {
        "dummy fallback is ON"
    } else {
        "dummy fallback is OFF"
    };
    let git_state = if workspace_auto_commit {
        "workspace auto-commit is ON"
    } else {
        "workspace auto-commit is OFF"
    };

    format!(
        "PiuroForge Doctor\n\n- LLM backend: {llm_backend}\n- Workspace: {workspace_state}\n- Novel config: {config_state}\n- Codex: {codex_state}\n- Fallback: {fallback_state}\n- Workspace Git auto-commit: {git_state}\n\nIf Doctor says ready, PiuroForge setup is finished and you can move on to `piuroforge next-scene`.\n\nIf you run PiuroForge through another assistant, IDE agent, or sandboxed tool, that host may still ask for its own approval prompts. Those prompts are outside PiuroForge."
    )
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn looks_like_network_error(error: &anyhow::Error) -> bool {
    let normalized = error.to_string().to_ascii_lowercase();
    normalized.contains("failed to lookup address information")
        || normalized.contains("dns")
        || normalized.contains("network")
        || normalized.contains("error sending request for url")
        || normalized.contains("stream disconnected")
        || normalized.contains("connection reset")
        || normalized.contains("connection refused")
        || normalized.contains("timed out")
}

fn compact_message(error: &anyhow::Error) -> String {
    let flattened = error
        .to_string()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if flattened.chars().count() <= 220 {
        flattened
    } else {
        format!(
            "{}...",
            flattened.chars().take(220).collect::<String>().trim_end()
        )
    }
}

fn dedup(items: &mut Vec<String>) {
    let mut seen = Vec::new();
    items.retain(|item| {
        if seen.contains(item) {
            false
        } else {
            seen.push(item.clone());
            true
        }
    });
}
