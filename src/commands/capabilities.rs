use anyhow::Result;
use serde::Serialize;
use serde_json::json;

use crate::config::{Config, SUPPORTED_LLM_BACKENDS};
use crate::output::{CommandOutput, OUTPUT_SCHEMA_VERSION};

#[derive(Debug, Clone, Serialize)]
struct CommandCapability {
    name: &'static str,
    workspace_required: bool,
    mutates_workspace: bool,
    requires_codex: bool,
    supports_json: bool,
    supports_agent_mode: bool,
    args: Vec<&'static str>,
}

pub fn run(config: &Config) -> Result<CommandOutput> {
    let commands = vec![
        CommandCapability {
            name: "capabilities",
            workspace_required: false,
            mutates_workspace: false,
            requires_codex: false,
            supports_json: true,
            supports_agent_mode: true,
            args: vec![],
        },
        CommandCapability {
            name: "init",
            workspace_required: false,
            mutates_workspace: true,
            requires_codex: false,
            supports_json: true,
            supports_agent_mode: true,
            args: vec!["[PATH]"],
        },
        CommandCapability {
            name: "status",
            workspace_required: false,
            mutates_workspace: false,
            requires_codex: false,
            supports_json: true,
            supports_agent_mode: true,
            args: vec![],
        },
        CommandCapability {
            name: "doctor",
            workspace_required: false,
            mutates_workspace: false,
            requires_codex: false,
            supports_json: true,
            supports_agent_mode: true,
            args: vec![],
        },
        CommandCapability {
            name: "next-scene",
            workspace_required: true,
            mutates_workspace: true,
            requires_codex: true,
            supports_json: true,
            supports_agent_mode: true,
            args: vec![],
        },
        CommandCapability {
            name: "review",
            workspace_required: true,
            mutates_workspace: true,
            requires_codex: true,
            supports_json: true,
            supports_agent_mode: true,
            args: vec![],
        },
        CommandCapability {
            name: "rewrite",
            workspace_required: true,
            mutates_workspace: true,
            requires_codex: true,
            supports_json: true,
            supports_agent_mode: true,
            args: vec!["SCENE_ID", "--instruction TEXT"],
        },
        CommandCapability {
            name: "polish",
            workspace_required: true,
            mutates_workspace: true,
            requires_codex: true,
            supports_json: true,
            supports_agent_mode: true,
            args: vec!["[SCENE_ID]"],
        },
        CommandCapability {
            name: "proofread",
            workspace_required: true,
            mutates_workspace: true,
            requires_codex: true,
            supports_json: true,
            supports_agent_mode: true,
            args: vec!["[SCENE_ID]"],
        },
        CommandCapability {
            name: "approve",
            workspace_required: true,
            mutates_workspace: true,
            requires_codex: false,
            supports_json: true,
            supports_agent_mode: true,
            args: vec!["SCENE_ID"],
        },
        CommandCapability {
            name: "next-bundle",
            workspace_required: true,
            mutates_workspace: true,
            requires_codex: false,
            supports_json: true,
            supports_agent_mode: true,
            args: vec![],
        },
        CommandCapability {
            name: "expand-world",
            workspace_required: true,
            mutates_workspace: true,
            requires_codex: true,
            supports_json: true,
            supports_agent_mode: true,
            args: vec![],
        },
        CommandCapability {
            name: "memory",
            workspace_required: true,
            mutates_workspace: false,
            requires_codex: false,
            supports_json: true,
            supports_agent_mode: true,
            args: vec![],
        },
        CommandCapability {
            name: "show",
            workspace_required: true,
            mutates_workspace: false,
            requires_codex: false,
            supports_json: true,
            supports_agent_mode: true,
            args: vec!["SCENE_ID"],
        },
    ];

    let supported_backends = SUPPORTED_LLM_BACKENDS
        .iter()
        .map(|name| {
            let (login_cmd, description) = backend_help(name);
            json!({
                "name": name,
                "default": *name == "codex_cli",
                "auth_mode": *name,
                "requires_login_command": login_cmd,
                "description": description,
            })
        })
        .collect::<Vec<_>>();

    let auth_modes = SUPPORTED_LLM_BACKENDS
        .iter()
        .map(|name| {
            let (_, description) = backend_help(name);
            json!({
                "name": name,
                "default": *name == "codex_cli",
                "description": description,
            })
        })
        .collect::<Vec<_>>();

    let (active_login_cmd, _) = backend_help(config.llm_backend.as_str());
    let required_installs = match config.llm_backend.as_str() {
        "gemini_cli" => vec!["piuroforge", "Gemini CLI"],
        "claude_code" => vec!["piuroforge", "Claude Code CLI"],
        _ => vec!["piuroforge", "codex CLI"],
    };

    let data = json!({
        "auth_mode": config.llm_backend,
        "selected_backend": config.llm_backend,
        "supported_backends": supported_backends,
        "auth_modes": auth_modes,
        "required_installs": required_installs,
        "recommended_setup_sequence": [
            "install piuroforge",
            format!("install the CLI for the selected backend ({})", config.llm_backend),
            format!("log in to the backend CLI ({active_login_cmd})"),
            "piuroforge init <workspace>",
            "piuroforge --workspace <workspace> doctor",
            "follow next_steps until doctor is ready"
        ],
        "recommended_invocation": "piuroforge --format json --agent <command>",
        "schema_version": OUTPUT_SCHEMA_VERSION,
        "success_fields": ["schema_version", "status", "agent_mode", "command", "workspace", "summary", "details", "artifacts", "next_steps", "warnings"],
        "error_fields": ["schema_version", "status", "agent_mode", "command", "workspace", "error_code", "reason", "remediation", "details"],
        "commands": commands,
        "notes": [
            "Use --format json for stable machine-readable output.",
            "Use --agent to request compact text output and explicit agent_mode markers.",
            "Commands that mutate the workspace may auto-commit if workspace_auto_commit is enabled.",
            "Commands that require the LLM backend will fail with codex_unavailable unless the configured CLI (codex/gemini/claude) is installed, logged in, and reachable.",
            "Call capabilities first, then doctor, then status before mutating commands.",
            "Supported auth modes: codex_cli, gemini_cli, claude_code. PiuroForge never performs OAuth directly; the backend CLI handles login."
        ]
    });

    let body = "\
Recommended agent invocation:
piuroforge --workspace <workspace> --format json --agent status
piuroforge --workspace <workspace> --format json --agent next-scene

Prefer `capabilities`, then `doctor`, then `status` before mutating commands.";

    Ok(CommandOutput::ok(
        "capabilities",
        &config.workspace_dir,
        "PiuroForge agent contract and command capabilities.",
    )
    .detail("llm_backend", &config.llm_backend)
    .detail("auth_mode", &config.llm_backend)
    .detail("setup_flow", "init_then_doctor")
    .detail("recommended_format", "json")
    .detail("recommended_flag", "--agent")
    .detail("schema_version", OUTPUT_SCHEMA_VERSION.to_string())
    .detail("command_count", commands.len().to_string())
    .body(body)
    .data(data))
}

fn backend_help(name: &str) -> (&'static str, &'static str) {
    match name {
        "gemini_cli" => (
            "gemini",
            "Use the logged-in Gemini CLI subprocess (Google OAuth handled by the gemini CLI itself).",
        ),
        "claude_code" => (
            "claude login",
            "Use the logged-in Claude Code CLI subprocess (Anthropic OAuth handled by the claude CLI itself).",
        ),
        _ => (
            "codex login",
            "Use the logged-in Codex CLI subprocess as the novel generation backend.",
        ),
    }
}
