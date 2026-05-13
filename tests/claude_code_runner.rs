use anyhow::Result;
use piuroforge::claude_code_runner::ClaudeCodeRunner;
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};
use tempfile::tempdir;

#[test]
fn run_prompt_returns_stdout_from_subprocess() -> Result<()> {
    let temp_dir = tempdir()?;
    let script_path = temp_dir.path().join("fake_claude.sh");
    let script = "#!/bin/sh\nprintf 'claude code ok\\n'\n";
    std::fs::write(&script_path, script)?;
    let mut permissions = std::fs::metadata(&script_path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&script_path, permissions)?;

    let runner = ClaudeCodeRunner::new(script_path.display().to_string(), Duration::from_secs(5));
    let response = runner.run_prompt("hello")?;

    assert_eq!(response, "claude code ok");

    Ok(())
}

#[test]
fn run_prompt_times_out_when_command_hangs() -> Result<()> {
    let temp_dir = tempdir()?;
    let script_path = temp_dir.path().join("hanging_claude.sh");
    let script = "#!/bin/sh\nsleep 5\nprintf 'too late\\n'\n";
    std::fs::write(&script_path, script)?;
    let mut permissions = std::fs::metadata(&script_path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&script_path, permissions)?;

    let runner = ClaudeCodeRunner::new(
        script_path.display().to_string(),
        Duration::from_millis(300),
    );
    let started = Instant::now();
    let error = runner
        .run_prompt("hello")
        .expect_err("expected timeout for hanging command");

    assert!(started.elapsed() < Duration::from_millis(900));
    assert!(error.to_string().contains("완료되지 않았습니다"));

    Ok(())
}

#[test]
fn run_prompt_writes_opt_in_prompt_log() -> Result<()> {
    let temp_dir = tempdir()?;
    let script_path = temp_dir.path().join("logging_claude.sh");
    let log_dir = temp_dir.path().join("prompt-logs");
    let script = "#!/bin/sh\nprintf 'logged ok\\n'\n";
    std::fs::write(&script_path, script)?;
    let mut permissions = std::fs::metadata(&script_path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&script_path, permissions)?;

    let runner = ClaudeCodeRunner::new(script_path.display().to_string(), Duration::from_secs(5))
        .with_prompt_logging(log_dir.clone());
    let response = runner.run_prompt_named("writer", "Prompt body for logging")?;

    assert_eq!(response, "logged ok");

    let entries = std::fs::read_dir(&log_dir)?
        .map(|entry| entry.map(|value| value.path()))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    assert_eq!(entries.len(), 1);

    let log = std::fs::read_to_string(&entries[0])?;
    assert!(log.contains("\"label\": \"writer\""));
    assert!(log.contains("\"prompt\": \"Prompt body for logging\""));
    assert!(log.contains("\"response\": \"logged ok\""));
    assert!(log.contains("\"outcome\": \"ok\""));

    Ok(())
}
