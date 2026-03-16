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
            name: "approve",
            workspace_required: true,
            mutates_workspace: true,
            requires_codex: false,
            supports_json: true,
            supports_agent_mode: true,
            args: vec!["SCENE_ID"],
        },
        CommandCapability {
            name: "next-chapter",
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
            json!({
                "name": name,
                "default": *name == "codex_cli",
                "auth_mode": "codex_cli",
                "requires_login_command": "codex login",
                "description": "Use the logged-in Codex CLI subprocess as the novel generation backend."
            })
        })
        .collect::<Vec<_>>();

    let data = json!({
        "auth_mode": "codex_cli",
        "selected_backend": config.llm_backend,
        "supported_backends": supported_backends,
        "auth_modes": [
            {
                "name": "codex_cli",
                "default": true,
                "description": "PiuroForge does not perform OAuth directly. Install Codex CLI and run `codex login` first."
            }
        ],
        "required_installs": ["piuroforge", "codex CLI"],
        "recommended_setup_sequence": [
            "install piuroforge",
            "install Codex CLI",
            "codex login",
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
            "Commands that require Codex will fail with codex_unavailable unless Codex is installed, logged in, and reachable.",
            "Call capabilities first, then doctor, then status before mutating commands.",
            "Current auth mode is codex_cli only. PiuroForge expects `codex login` instead of direct OAuth."
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
    .detail("auth_mode", "codex_cli")
    .detail("setup_flow", "init_then_doctor")
    .detail("recommended_format", "json")
    .detail("recommended_flag", "--agent")
    .detail("schema_version", OUTPUT_SCHEMA_VERSION.to_string())
    .detail("command_count", commands.len().to_string())
    .body(body)
    .data(data))
}
