use anyhow::Result;
use serde::Serialize;
use std::io;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

const POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Error)]
pub enum CodexRunnerError {
    #[error("codex CLI를 실행할 수 없습니다. 먼저 codex login 실행 후 codex 설치와 PATH를 확인하세요. 원인: {0}")]
    Unavailable(#[source] io::Error),
    #[error("codex CLI 호출이 실패했습니다. 먼저 codex login 실행 후 다시 시도하세요. 상세: {0}")]
    Invocation(String),
    #[error("codex CLI 응답이 {0} 안에 완료되지 않았습니다. 먼저 codex login 실행 후 codex 상태를 확인하세요.")]
    Timeout(String),
    #[error("codex CLI가 빈 응답을 반환했습니다. 먼저 codex login 실행 후 다시 시도하세요.")]
    EmptyResponse,
}

#[derive(Debug, Clone)]
pub struct CodexRunner {
    command: String,
    timeout: Duration,
    prompt_log_dir: Option<PathBuf>,
}

impl CodexRunner {
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
        let version = Command::new(&self.command).arg("--version").output();
        match version {
            Ok(output) if output.status.success() => {}
            Ok(_) => return Ok(false),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(CodexRunnerError::Unavailable(error).into()),
        }

        match self.run_prompt_named("healthcheck", "Reply with OK only.") {
            Ok(response) => Ok(!response.trim().is_empty()),
            Err(_) => Ok(false),
        }
    }

    pub fn ensure_available(&self) -> Result<()> {
        if self.healthcheck()? {
            return Ok(());
        }

        Err(CodexRunnerError::Invocation(
            "codex CLI가 실행되지 않았거나 로그인 상태가 아닙니다. 먼저 codex login 실행 후 다시 시도하세요.".to_string(),
        )
        .into())
    }

    pub fn run_prompt(&self, prompt: &str) -> Result<String> {
        self.run_prompt_named("generic", prompt)
    }

    pub fn run_prompt_named(&self, label: &str, prompt: &str) -> Result<String> {
        for attempt in 0..2 {
            match self.run_prompt_once(label, prompt, attempt + 1) {
                Ok(response) => return Ok(response),
                Err(error) => {
                    let timed_out = matches!(
                        error.downcast_ref::<CodexRunnerError>(),
                        Some(CodexRunnerError::Timeout(_))
                    );
                    if timed_out || attempt == 1 {
                        return Err(error);
                    }
                }
            }
        }

        unreachable!("retry loop should return on success or final failure")
    }

    fn run_prompt_once(&self, label: &str, prompt: &str, attempt: usize) -> Result<String> {
        let started_at = SystemTime::now();
        let started = Instant::now();
        let mut command = Command::new(&self.command);
        command
            .arg("exec")
            .arg("--skip-git-repo-check")
            .arg(prompt)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        configure_process_group(&mut command);

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                self.write_prompt_log(PromptLogEntry {
                    timestamp_unix_millis: unix_timestamp_millis(started_at),
                    label: label.to_string(),
                    attempt,
                    command: self.command.clone(),
                    prompt_chars: prompt.chars().count(),
                    response_chars: 0,
                    duration_ms: started.elapsed().as_millis() as u64,
                    outcome: "unavailable".to_string(),
                    prompt: prompt.to_string(),
                    response: None,
                    stderr: Some(error.to_string()),
                });
                return Err(CodexRunnerError::Unavailable(error).into());
            }
        };

        let output = loop {
            match child.try_wait().map_err(CodexRunnerError::Unavailable)? {
                Some(_) => {
                    break child
                        .wait_with_output()
                        .map_err(CodexRunnerError::Unavailable)?
                }
                None if started.elapsed() >= self.timeout => {
                    terminate_process_tree(&mut child);
                    let detail = child
                        .wait_with_output()
                        .ok()
                        .and_then(|output| {
                            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                            if stderr.is_empty() {
                                None
                            } else {
                                Some(format!(" ({stderr})"))
                            }
                        })
                        .unwrap_or_default();
                    self.write_prompt_log(PromptLogEntry {
                        timestamp_unix_millis: unix_timestamp_millis(started_at),
                        label: label.to_string(),
                        attempt,
                        command: self.command.clone(),
                        prompt_chars: prompt.chars().count(),
                        response_chars: 0,
                        duration_ms: started.elapsed().as_millis() as u64,
                        outcome: "timeout".to_string(),
                        prompt: prompt.to_string(),
                        response: None,
                        stderr: if detail.is_empty() {
                            None
                        } else {
                            Some(
                                detail
                                    .trim()
                                    .trim_matches(|ch| ch == '(' || ch == ')')
                                    .to_string(),
                            )
                        },
                    });
                    return Err(CodexRunnerError::Timeout(format!(
                        "{}{}",
                        format_timeout(self.timeout),
                        detail
                    ))
                    .into());
                }
                None => thread::sleep(POLL_INTERVAL.min(self.timeout)),
            }
        };

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if !output.status.success() {
            let detail = if stderr.is_empty() {
                format!("exit status {}", output.status)
            } else {
                stderr.clone()
            };
            self.write_prompt_log(PromptLogEntry {
                timestamp_unix_millis: unix_timestamp_millis(started_at),
                label: label.to_string(),
                attempt,
                command: self.command.clone(),
                prompt_chars: prompt.chars().count(),
                response_chars: 0,
                duration_ms: started.elapsed().as_millis() as u64,
                outcome: "invocation_error".to_string(),
                prompt: prompt.to_string(),
                response: None,
                stderr: Some(detail.clone()),
            });
            return Err(CodexRunnerError::Invocation(detail).into());
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.is_empty() {
            self.write_prompt_log(PromptLogEntry {
                timestamp_unix_millis: unix_timestamp_millis(started_at),
                label: label.to_string(),
                attempt,
                command: self.command.clone(),
                prompt_chars: prompt.chars().count(),
                response_chars: 0,
                duration_ms: started.elapsed().as_millis() as u64,
                outcome: "empty_response".to_string(),
                prompt: prompt.to_string(),
                response: None,
                stderr: if stderr.is_empty() {
                    None
                } else {
                    Some(stderr)
                },
            });
            return Err(CodexRunnerError::EmptyResponse.into());
        }

        self.write_prompt_log(PromptLogEntry {
            timestamp_unix_millis: unix_timestamp_millis(started_at),
            label: label.to_string(),
            attempt,
            command: self.command.clone(),
            prompt_chars: prompt.chars().count(),
            response_chars: stdout.chars().count(),
            duration_ms: started.elapsed().as_millis() as u64,
            outcome: "ok".to_string(),
            prompt: prompt.to_string(),
            response: Some(stdout.clone()),
            stderr: if stderr.is_empty() {
                None
            } else {
                Some(stderr)
            },
        });
        Ok(stdout)
    }

    fn write_prompt_log(&self, entry: PromptLogEntry) {
        let Some(dir) = &self.prompt_log_dir else {
            return;
        };

        let file_name = format!(
            "{:020}-{}-attempt{:02}.json",
            entry.timestamp_unix_millis,
            sanitize_label(&entry.label),
            entry.attempt
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

fn format_timeout(timeout: Duration) -> String {
    if timeout.as_millis() < 1_000 {
        return format!("{}ms", timeout.as_millis());
    }

    if timeout.subsec_millis() == 0 {
        return format!("{}초", timeout.as_secs());
    }

    format!("{:.1}초", timeout.as_secs_f64())
}

fn configure_process_group(command: &mut Command) {
    #[cfg(unix)]
    {
        command.process_group(0);
    }
}

fn terminate_process_tree(child: &mut Child) {
    #[cfg(unix)]
    {
        let pgid = child.id() as i32;
        unsafe {
            libc::killpg(pgid, libc::SIGKILL);
        }
    }

    let _ = child.kill();
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
    attempt: usize,
    command: String,
    prompt_chars: usize,
    response_chars: usize,
    duration_ms: u64,
    outcome: String,
    prompt: String,
    response: Option<String>,
    stderr: Option<String>,
}
