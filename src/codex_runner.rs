use anyhow::Result;
use std::io;
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CodexRunnerError {
    #[error("codex CLI를 실행할 수 없습니다. 먼저 codex login 실행 후 codex 설치와 PATH를 확인하세요. 원인: {0}")]
    Unavailable(#[source] io::Error),
    #[error("codex CLI 호출이 실패했습니다. 먼저 codex login 실행 후 다시 시도하세요. 상세: {0}")]
    Invocation(String),
    #[error("codex CLI가 빈 응답을 반환했습니다. 먼저 codex login 실행 후 다시 시도하세요.")]
    EmptyResponse,
}

#[derive(Debug, Clone)]
pub struct CodexRunner {
    command: String,
}

impl CodexRunner {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
        }
    }

    pub fn healthcheck(&self) -> Result<bool> {
        let version = Command::new(&self.command).arg("--version").output();
        match version {
            Ok(output) if output.status.success() => {}
            Ok(_) => return Ok(false),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(CodexRunnerError::Unavailable(error).into()),
        }

        match self.run_prompt("Reply with OK only.") {
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
        let mut last_error = None;

        for _ in 0..2 {
            match self.run_prompt_once(prompt) {
                Ok(response) => return Ok(response),
                Err(error) => last_error = Some(error),
            }
        }

        Err(last_error.expect("retry loop should capture an error"))
    }

    fn run_prompt_once(&self, prompt: &str) -> Result<String> {
        let output = Command::new(&self.command)
            .arg("exec")
            .arg("--skip-git-repo-check")
            .arg(prompt)
            .output()
            .map_err(CodexRunnerError::Unavailable)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let detail = if stderr.is_empty() {
                format!("exit status {}", output.status)
            } else {
                stderr
            };
            return Err(CodexRunnerError::Invocation(detail).into());
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.is_empty() {
            return Err(CodexRunnerError::EmptyResponse.into());
        }

        Ok(stdout)
    }
}
