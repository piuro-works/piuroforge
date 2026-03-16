use anyhow::Result;
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::io::{self, BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;

use crate::llm_runner::PromptRunner;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

const POLL_INTERVAL: Duration = Duration::from_millis(100);
const PROGRESS_HEARTBEAT: Duration = Duration::from_secs(15);

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
        let timeout = timeout_for_label(self.timeout, label);
        let output_path = codex_output_file_path(label, attempt, started_at);
        let mut command = Command::new(&self.command);
        command
            .arg("exec")
            .arg("--skip-git-repo-check")
            .arg("--json")
            .arg("--output-last-message")
            .arg(&output_path)
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

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let (tx, rx) = mpsc::channel();
        let mut reader_handles = Vec::new();

        if let Some(stdout) = stdout {
            let tx = tx.clone();
            reader_handles.push(thread::spawn(move || {
                read_stream_lines(stdout, StreamSource::Stdout, tx);
            }));
        }
        if let Some(stderr) = stderr {
            let tx = tx.clone();
            reader_handles.push(thread::spawn(move || {
                read_stream_lines(stderr, StreamSource::Stderr, tx);
            }));
        }
        drop(tx);

        let progress_enabled = progress_enabled_for(label);
        if progress_enabled {
            emit_progress(
                label,
                &format!("started (timeout {})", format_timeout(timeout)),
            );
        }

        let mut stdout_lines = Vec::new();
        let mut stderr_lines = Vec::new();
        let mut next_heartbeat = PROGRESS_HEARTBEAT;
        let mut last_progress_hint: Option<String> = None;
        let exit_status = loop {
            while let Ok(message) = rx.try_recv() {
                match message.source {
                    StreamSource::Stdout => {
                        let line = message.line.trim_end().to_string();
                        if line.is_empty() {
                            continue;
                        }
                        if let Some(progress) = progress_message_from_stdout(label, &line) {
                            last_progress_hint = Some(progress.clone());
                            if progress_enabled {
                                emit_progress(label, &progress);
                            }
                            continue;
                        }
                        stdout_lines.push(line);
                    }
                    StreamSource::Stderr => {
                        let line = message.line.trim_end().to_string();
                        if line.is_empty() {
                            continue;
                        }
                        stderr_lines.push(line);
                    }
                }
            }

            match child.try_wait().map_err(CodexRunnerError::Unavailable)? {
                Some(status) => {
                    break status;
                }
                None if started.elapsed() >= timeout => {
                    terminate_process_tree(&mut child);
                    let _ = child.wait();
                    for handle in reader_handles {
                        let _ = handle.join();
                    }
                    while let Ok(message) = rx.try_recv() {
                        match message.source {
                            StreamSource::Stdout => stdout_lines.push(message.line),
                            StreamSource::Stderr => stderr_lines.push(message.line),
                        }
                    }
                    let stderr = stderr_lines.join("\n").trim().to_string();
                    let detail = timeout_detail(&stderr, last_progress_hint.as_deref());
                    let _ = fs::remove_file(&output_path);
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
                        stderr: if stderr.is_empty() {
                            None
                        } else {
                            Some(stderr)
                        },
                    });
                    return Err(CodexRunnerError::Timeout(format!(
                        "{}{}",
                        format_timeout(timeout),
                        detail
                    ))
                    .into());
                }
                None => {
                    if progress_enabled && started.elapsed() >= next_heartbeat {
                        let elapsed = format_timeout(started.elapsed());
                        let heartbeat = match last_progress_hint.as_deref() {
                            Some(last) => format!("still running after {elapsed} ({last})"),
                            None => format!("still running after {elapsed}"),
                        };
                        emit_progress(label, &heartbeat);
                        next_heartbeat += PROGRESS_HEARTBEAT;
                    }
                    thread::sleep(POLL_INTERVAL.min(timeout));
                }
            }
        };

        for handle in reader_handles {
            let _ = handle.join();
        }
        while let Ok(message) = rx.try_recv() {
            match message.source {
                StreamSource::Stdout => {
                    let line = message.line.trim_end().to_string();
                    if !line.is_empty() {
                        stdout_lines.push(line);
                    }
                }
                StreamSource::Stderr => {
                    let line = message.line.trim_end().to_string();
                    if !line.is_empty() {
                        stderr_lines.push(line);
                    }
                }
            }
        }

        let stderr = stderr_lines.join("\n").trim().to_string();
        let stdout = stdout_lines.join("\n").trim().to_string();
        if !exit_status.success() {
            let detail = if stderr.is_empty() {
                if stdout.is_empty() {
                    format!("exit status {}", exit_status)
                } else {
                    stdout.clone()
                }
            } else {
                stderr.clone()
            };
            let _ = fs::remove_file(&output_path);
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

        let response = read_output_last_message(&output_path).unwrap_or_else(|| stdout.clone());
        let _ = fs::remove_file(&output_path);
        if response.is_empty() {
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

        if progress_enabled {
            emit_progress(label, "completed");
        }
        self.write_prompt_log(PromptLogEntry {
            timestamp_unix_millis: unix_timestamp_millis(started_at),
            label: label.to_string(),
            attempt,
            command: self.command.clone(),
            prompt_chars: prompt.chars().count(),
            response_chars: response.chars().count(),
            duration_ms: started.elapsed().as_millis() as u64,
            outcome: "ok".to_string(),
            prompt: prompt.to_string(),
            response: Some(response.clone()),
            stderr: if stderr.is_empty() {
                None
            } else {
                Some(stderr)
            },
        });
        Ok(response)
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

impl PromptRunner for CodexRunner {
    fn run_prompt_named(&self, label: &str, prompt: &str) -> Result<String> {
        CodexRunner::run_prompt_named(self, label, prompt)
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

fn timeout_for_label(base: Duration, label: &str) -> Duration {
    match label {
        "critic" => base.max(Duration::from_secs(300)),
        "writer" => base.max(Duration::from_secs(240)),
        "editor" => base.max(Duration::from_secs(180)),
        "expand-world" => base.max(Duration::from_secs(240)),
        _ => base,
    }
}

fn codex_output_file_path(label: &str, attempt: usize, started_at: SystemTime) -> PathBuf {
    std::env::temp_dir().join(format!(
        "heeforge-codex-{}-attempt{:02}-{}.txt",
        sanitize_label(label),
        attempt,
        unix_timestamp_millis(started_at)
    ))
}

fn read_output_last_message(path: &PathBuf) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let trimmed = content.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn timeout_detail(stderr: &str, last_progress_hint: Option<&str>) -> String {
    let mut details = Vec::new();
    if !stderr.trim().is_empty() {
        details.push(stderr.trim().to_string());
    }
    if let Some(progress) = last_progress_hint.filter(|value| !value.trim().is_empty()) {
        details.push(format!("last progress: {progress}"));
    }

    if details.is_empty() {
        String::new()
    } else {
        format!(" ({})", details.join(" | "))
    }
}

fn progress_enabled_for(label: &str) -> bool {
    !matches!(label, "healthcheck" | "doctor")
}

fn emit_progress(label: &str, message: &str) {
    eprintln!("Progress [{label}]: {message}");
}

fn progress_message_from_stdout(label: &str, line: &str) -> Option<String> {
    let event = serde_json::from_str::<Value>(line).ok()?;
    let event_type = event.get("type")?.as_str()?;

    match event_type {
        "thread.started" => Some("Codex session started".to_string()),
        "turn.started" => Some("reasoning started".to_string()),
        "error" => event
            .get("message")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string()),
        "item.completed" => {
            let item = event.get("item")?;
            if item.get("type").and_then(|value| value.as_str()) == Some("error") {
                item.get("message")
                    .and_then(|value| value.as_str())
                    .map(|value| value.to_string())
            } else {
                None
            }
        }
        "turn.completed" => Some(format!("{label} turn completed")),
        _ => None,
    }
}

fn read_stream_lines<R: Read + Send + 'static>(
    reader: R,
    source: StreamSource,
    tx: mpsc::Sender<StreamMessage>,
) {
    let reader = BufReader::new(reader);
    for line in reader.lines() {
        let Ok(line) = line else {
            break;
        };
        if tx.send(StreamMessage { source, line }).is_err() {
            break;
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum StreamSource {
    Stdout,
    Stderr,
}

#[derive(Debug)]
struct StreamMessage {
    source: StreamSource,
    line: String,
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
