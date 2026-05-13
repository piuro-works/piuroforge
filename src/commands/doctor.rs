use anyhow::Result;
use std::io;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use crate::claude_code_runner::ClaudeCodeRunner;
use crate::codex_runner::CodexRunner;
use crate::config::{Config, SUPPORTED_LLM_BACKENDS};
use crate::gemini_runner::GeminiRunner;
use crate::launch_contract::validate_launch_contract;
use crate::llm_runner::PromptRunner;
use crate::output::CommandOutput;

pub fn run(config: &Config) -> Result<CommandOutput> {
    let workspace_manifest_exists = config.workspace_manifest_path.exists();
    let workspace_config_exists = config.workspace_config_path.exists();
    let global_config_exists = config.global_config_path.exists();
    let missing_fields = config.novel_settings.missing_required_fields();
    let backend = BackendDescriptor::from(config);

    let version_probe = probe_cli_version(backend.command);
    let backend_cli_status = match &version_probe {
        CliVersionProbe::Ready { .. } => "installed",
        CliVersionProbe::Missing => "missing",
        CliVersionProbe::Failed { .. } => "error",
    };
    let backend_version = match &version_probe {
        CliVersionProbe::Ready { version } => version.clone(),
        _ => "-".to_string(),
    };
    let probe_timeout_secs = backend.timeout_secs.min(15);

    let mut warnings = Vec::new();
    let mut next_steps = Vec::new();
    let launch_contract_report = if workspace_config_exists {
        Some(validate_launch_contract(
            &config.workspace_dir,
            &config.novel_settings,
        )?)
    } else {
        None
    };

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

    let backend_connection = match version_probe {
        CliVersionProbe::Missing => {
            warnings.push(format!(
                "{label} CLI was not found on this machine. Install {label} CLI first, then run `{login}`.",
                label = backend.label,
                login = backend.login_command,
            ));
            next_steps.push(format!("Install {} CLI", backend.label));
            next_steps.push(backend.login_command.to_string());
            BackendConnection::Missing
        }
        CliVersionProbe::Failed { detail } => {
            warnings.push(format!(
                "{label} CLI exists but `--version` failed: {detail}.",
                label = backend.label,
            ));
            next_steps.push(format!(
                "Open a terminal and run: {}",
                backend.login_command
            ));
            BackendConnection::Unavailable
        }
        CliVersionProbe::Ready { .. } => match probe_backend_connection(config, &backend) {
            Ok(()) => BackendConnection::Ready,
            Err(error) => {
                if looks_like_network_error(&error) {
                    warnings.push(format!(
                        "{label} CLI is installed, but this machine could not reach the {label} service. Check internet, DNS, VPN, or proxy settings.",
                        label = backend.label,
                    ));
                } else {
                    warnings.push(format!(
                        "{label} CLI is installed, but the live check did not complete. Run `{login}` again and retry.",
                        label = backend.label,
                        login = backend.login_command,
                    ));
                }
                warnings.push(format!(
                    "{} check detail: {}.",
                    backend.label,
                    compact_message(&error)
                ));
                next_steps.push(format!(
                    "Open a terminal and run: {}",
                    backend.login_command
                ));
                next_steps.push("piuroforge doctor".to_string());
                BackendConnection::Unavailable
            }
        },
    };

    if config.allow_dummy_fallback {
        warnings.push(format!(
            "Dummy fallback is ON. PiuroForge can produce placeholder text instead of live {} output.",
            backend.label
        ));
    }

    let mut launch_contract_has_blocking_issues = false;
    if let Some(report) = &launch_contract_report {
        for issue in &report.issues {
            warnings.push(issue.message.clone());
            if issue.severity == crate::launch_contract::LaunchContractSeverity::Error {
                launch_contract_has_blocking_issues = true;
                next_steps.push(issue.remediation.clone());
                next_steps.push(format!(
                    "Edit {}",
                    config.workspace_config_path.display()
                ));
            }
        }
    }

    let ready_to_draft = workspace_manifest_exists
        && workspace_config_exists
        && missing_fields.is_empty()
        && matches!(backend_connection, BackendConnection::Ready)
        && !config.allow_dummy_fallback
        && !launch_contract_has_blocking_issues;

    if ready_to_draft {
        next_steps.push(format!(
            "piuroforge --workspace {} next-scene",
            config.workspace_dir.display()
        ));
    } else if workspace_manifest_exists
        && workspace_config_exists
        && missing_fields.is_empty()
        && !matches!(backend_connection, BackendConnection::Ready)
    {
        next_steps.push(format!(
            "Open a terminal and run: {}",
            backend.login_command
        ));
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
        .detail("auth_mode", &config.llm_backend)
        .detail("setup_flow", "init_then_doctor")
        .detail("supported_llm_backends", SUPPORTED_LLM_BACKENDS.join(", "))
        .detail(
            "workspace_ready",
            yes_no(workspace_manifest_exists && workspace_config_exists),
        )
        .detail("workspace_manifest", yes_no(workspace_manifest_exists))
        .detail("workspace_config", yes_no(workspace_config_exists))
        .detail("global_config", yes_no(global_config_exists))
        .detail("backend_command", backend.command.to_string())
        .detail("backend_cli", backend_cli_status)
        .detail("backend_version", backend_version)
        .detail("backend_connection", backend_connection.as_str())
        .detail("backend_login_command", backend.login_command.to_string())
        // Compat aliases for existing agent integrations that look for `codex_*` keys.
        .detail("codex_command", config.codex_command.clone())
        .detail("codex_cli", backend_cli_status)
        .detail("codex_connection", backend_connection.as_str())
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
            "doctor reports backend_connection=ready and missing_required_fields=none",
        )
        .artifact("global_config", &config.global_config_path);

    if let Some(report) = &launch_contract_report {
        output = output
            .detail("launch_contract_enabled", report.enabled.to_string())
            .detail("launch_contract_status", report.status_label())
            .detail(
                "launch_contract_required_beats",
                report.required_beats_summary(),
            );
        if let Some(path) = &report.primary_plot_path {
            output = output.detail("launch_contract_primary_plot", path.display().to_string());
        }
    }

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
        backend.label,
        workspace_manifest_exists,
        workspace_config_exists,
        &missing_fields,
        &backend_connection,
        config.allow_dummy_fallback,
        config.workspace_auto_commit,
        launch_contract_report
            .as_ref()
            .map(|report| report.status_label())
            .unwrap_or("not_checked"),
    ));

    Ok(output)
}

struct BackendDescriptor<'a> {
    label: &'static str,
    login_command: &'static str,
    command: &'a str,
    timeout_secs: u64,
}

impl<'a> BackendDescriptor<'a> {
    fn from(config: &'a Config) -> Self {
        match config.llm_backend.as_str() {
            "gemini_cli" => Self {
                label: "Gemini",
                login_command: "gemini",
                command: &config.gemini_command,
                timeout_secs: config.gemini_timeout_secs,
            },
            "claude_code" => Self {
                label: "Claude Code",
                login_command: "claude login",
                command: &config.claude_code_command,
                timeout_secs: config.claude_code_timeout_secs,
            },
            _ => Self {
                label: "Codex",
                login_command: "codex login",
                command: &config.codex_command,
                timeout_secs: config.codex_timeout_secs,
            },
        }
    }
}

#[derive(Debug, Clone)]
enum CliVersionProbe {
    Ready { version: String },
    Missing,
    Failed { detail: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackendConnection {
    Ready,
    Unavailable,
    Missing,
}

impl BackendConnection {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Unavailable => "needs_attention",
            Self::Missing => "missing",
        }
    }
}

fn probe_cli_version(command: &str) -> CliVersionProbe {
    match Command::new(command).arg("--version").output() {
        Ok(output) if output.status.success() => CliVersionProbe::Ready {
            version: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        },
        Ok(output) => {
            let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let detail = if detail.is_empty() {
                format!("exit status {}", output.status)
            } else {
                detail
            };
            CliVersionProbe::Failed { detail }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => CliVersionProbe::Missing,
        Err(error) => CliVersionProbe::Failed {
            detail: error.to_string(),
        },
    }
}

fn probe_backend_connection(config: &Config, backend: &BackendDescriptor) -> Result<()> {
    let probe_timeout = Duration::from_secs(backend.timeout_secs.min(15));
    let runner = build_probe_runner(&config.llm_backend, backend.command, probe_timeout);
    let response = runner.run_prompt_named("doctor", "Reply with OK only.")?;
    if response.trim().is_empty() {
        anyhow::bail!("{} returned an empty healthcheck response", backend.label);
    }
    Ok(())
}

fn build_probe_runner(
    llm_backend: &str,
    command: &str,
    timeout: Duration,
) -> Arc<dyn PromptRunner> {
    match llm_backend {
        "gemini_cli" => Arc::new(GeminiRunner::new(command.to_string(), timeout)),
        "claude_code" => Arc::new(ClaudeCodeRunner::new(command.to_string(), timeout)),
        _ => Arc::new(CodexRunner::new(command.to_string(), timeout)),
    }
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
    backend_label: &str,
    workspace_manifest_exists: bool,
    workspace_config_exists: bool,
    missing_fields: &[&str],
    backend_connection: &BackendConnection,
    allow_dummy_fallback: bool,
    workspace_auto_commit: bool,
    launch_contract_state: &str,
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
    let backend_state = match backend_connection {
        BackendConnection::Ready => format!("live {backend_label} check succeeded"),
        BackendConnection::Unavailable => format!("live {backend_label} check failed"),
        BackendConnection::Missing => format!("{backend_label} CLI is missing"),
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
    let launch_state = match launch_contract_state {
        "disabled" => "launch contract checks are OFF",
        "empty" => "launch contract is enabled but empty",
        "blocking_issues" => "launch contract has blocking conflicts",
        "warnings" => "launch contract has warnings",
        "ok" => "launch contract checks passed",
        _ => "launch contract was not checked",
    };

    format!(
        "PiuroForge Doctor\n\n- LLM backend: {llm_backend}\n- Workspace: {workspace_state}\n- Novel config: {config_state}\n- {backend_label}: {backend_state}\n- Fallback: {fallback_state}\n- Workspace Git auto-commit: {git_state}\n- Launch contract: {launch_state}\n\nIf Doctor says ready, PiuroForge setup is finished and you can move on to `piuroforge next-scene`.\n\nIf you run PiuroForge through another assistant, IDE agent, or sandboxed tool, that host may still ask for its own approval prompts. Those prompts are outside PiuroForge."
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
