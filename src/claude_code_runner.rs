use anyhow::Result;
use serde::Serialize;
use std::io;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;

use crate::llm_runner::PromptRunner;
use crate::subprocess::{run_with_timeout, SubprocessError};

#[derive(Debug, Error)]
pub enum ClaudeCodeRunnerError {
    #[error(
        "claude code CLI를 실행할 수 없습니다. 먼저 `claude login` 또는 Claude 앱에서 로그인을 완료하고 PATH를 확인하세요. 원인: {0}"
    )]
    Unavailable(#[source] io::Error),
    #[error(
        "claude code CLI 호출이 실패했습니다. 먼저 `claude login`으로 로그인 상태를 확인하세요. 상세: {0}"
    )]
    Invocation(String),
    #[error(
        "claude code CLI 응답이 {0} 안에 완료되지 않았습니다. 먼저 `claude` 상태와 네트워크를 확인하세요."
    )]
    Timeout(String),
    #[error("claude code CLI가 빈 응답을 반환했습니다. 먼저 `claude login` 로그인 상태를 확인하세요.")]
    EmptyResponse,
}

#[derive(Debug, Clone)]
pub struct ClaudeCodeRunner {
    command: String,
    timeout: Duration,
    prompt_log_dir: Option<PathBuf>,
}

impl ClaudeCodeRunner {
    pub fn new(command: impl Into<String>, timeout: Duration) -> Self {
        Self {
            command: command.into(),
            timeout,
            prompt_log_dir: None,
        }
    }

    pub fn with_prompt_logging(mut self, prompt_log_dir: impl Into<PathBuf>) -> Self {
        self.prompt_log_dir = Some(prompt_log_dir.into());
        self
    }

    pub fn healthcheck(&self) -> Result<bool> {
        match Command::new(&self.command).arg("--version").output() {
            Ok(output) if output.status.success() => Ok(true),
            Ok(_) => Ok(false),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(ClaudeCodeRunnerError::Unavailable(error).into()),
        }
    }

    pub fn run_prompt(&self, prompt: &str) -> Result<String> {
        self.run_prompt_named("generic", prompt)
    }

    pub fn run_prompt_named(&self, label: &str, prompt: &str) -> Result<String> {
        let started_at = SystemTime::now();
        let started = std::time::Instant::now();
        let args: [&str; 4] = ["-p", prompt, "--output-format", "text"];
        let result = run_with_timeout(&self.command, args, self.timeout);

        match result {
            Ok(outcome) if outcome.success => {
                let response = outcome.stdout;
                if response.is_empty() {
                    self.write_prompt_log(PromptLogEntry {
                        timestamp_unix_millis: unix_timestamp_millis(started_at),
                        label: label.to_string(),
                        command: self.command.clone(),
                        prompt_chars: prompt.chars().count(),
                        response_chars: 0,
                        duration_ms: started.elapsed().as_millis() as u64,
                        outcome: "empty_response".to_string(),
                        prompt: prompt.to_string(),
                        response: None,
                        stderr: if outcome.stderr.is_empty() {
                            None
                        } else {
                            Some(outcome.stderr)
                        },
                    });
                    return Err(ClaudeCodeRunnerError::EmptyResponse.into());
                }

                self.write_prompt_log(PromptLogEntry {
                    timestamp_unix_millis: unix_timestamp_millis(started_at),
                    label: label.to_string(),
                    command: self.command.clone(),
                    prompt_chars: prompt.chars().count(),
                    response_chars: response.chars().count(),
                    duration_ms: started.elapsed().as_millis() as u64,
                    outcome: "ok".to_string(),
                    prompt: prompt.to_string(),
                    response: Some(response.clone()),
                    stderr: if outcome.stderr.is_empty() {
                        None
                    } else {
                        Some(outcome.stderr)
                    },
                });
                Ok(response)
            }
            Ok(outcome) => {
                let detail = if outcome.stderr.is_empty() {
                    if outcome.stdout.is_empty() {
                        "claude exited with a non-zero status".to_string()
                    } else {
                        outcome.stdout
                    }
                } else {
                    outcome.stderr
                };
                self.write_prompt_log(PromptLogEntry {
                    timestamp_unix_millis: unix_timestamp_millis(started_at),
                    label: label.to_string(),
                    command: self.command.clone(),
                    prompt_chars: prompt.chars().count(),
                    response_chars: 0,
                    duration_ms: started.elapsed().as_millis() as u64,
                    outcome: "invocation_error".to_string(),
                    prompt: prompt.to_string(),
                    response: None,
                    stderr: Some(detail.clone()),
                });
                Err(ClaudeCodeRunnerError::Invocation(detail).into())
            }
            Err(SubprocessError::Spawn(error)) | Err(SubprocessError::Wait(error)) => {
                self.write_prompt_log(PromptLogEntry {
                    timestamp_unix_millis: unix_timestamp_millis(started_at),
                    label: label.to_string(),
                    command: self.command.clone(),
                    prompt_chars: prompt.chars().count(),
                    response_chars: 0,
                    duration_ms: started.elapsed().as_millis() as u64,
                    outcome: "unavailable".to_string(),
                    prompt: prompt.to_string(),
                    response: None,
                    stderr: Some(error.to_string()),
                });
                Err(ClaudeCodeRunnerError::Unavailable(error).into())
            }
            Err(SubprocessError::Timeout) => {
                self.write_prompt_log(PromptLogEntry {
                    timestamp_unix_millis: unix_timestamp_millis(started_at),
                    label: label.to_string(),
                    command: self.command.clone(),
                    prompt_chars: prompt.chars().count(),
                    response_chars: 0,
                    duration_ms: started.elapsed().as_millis() as u64,
                    outcome: "timeout".to_string(),
                    prompt: prompt.to_string(),
                    response: None,
                    stderr: None,
                });
                Err(ClaudeCodeRunnerError::Timeout(format_timeout(self.timeout)).into())
            }
        }
    }

    fn write_prompt_log(&self, entry: PromptLogEntry) {
        let Some(dir) = &self.prompt_log_dir else {
            return;
        };

        let file_name = format!(
            "{:020}-claude-code-{}.json",
            entry.timestamp_unix_millis,
            sanitize_label(&entry.label)
        );
        let path = dir.join(file_name);

        let result = (|| -> Result<()> {
            std::fs::create_dir_all(dir)?;
            let content = serde_json::to_string_pretty(&entry)?;
            std::fs::write(path, content)?;
            Ok(())
        })();

        let _ = result;
    }
}

impl PromptRunner for ClaudeCodeRunner {
    fn run_prompt_named(&self, label: &str, prompt: &str) -> Result<String> {
        ClaudeCodeRunner::run_prompt_named(self, label, prompt)
    }
}

fn format_timeout(timeout: Duration) -> String {
    if timeout.as_millis() < 1_000 {
        return format!("{}ms", timeout.as_millis());
    }
    if timeout.subsec_millis() == 0 {
        return format!("{}초", timeout.as_secs());
    }
    format!("{:.1}초", timeout.as_secs_f64())
}

fn unix_timestamp_millis(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn sanitize_label(label: &str) -> String {
    let mut rendered = String::new();
    let mut last_was_dash = false;
    for ch in label.chars() {
        if ch.is_ascii_alphanumeric() {
            rendered.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !rendered.is_empty() && !last_was_dash {
            rendered.push('-');
            last_was_dash = true;
        }
        if rendered.len() >= 32 {
            break;
        }
    }
    let rendered = rendered.trim_matches('-');
    if rendered.is_empty() {
        "prompt".to_string()
    } else {
        rendered.to_string()
    }
}

#[derive(Debug, Clone, Serialize)]
struct PromptLogEntry {
    timestamp_unix_millis: u64,
    label: String,
    command: String,
    prompt_chars: usize,
    response_chars: usize,
    duration_ms: u64,
    outcome: String,
    prompt: String,
    response: Option<String>,
    stderr: Option<String>,
}
