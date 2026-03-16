use anyhow::Result;
use piuroforge::codex_runner::CodexRunner;
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};
use tempfile::tempdir;

#[test]
fn run_prompt_retries_once_after_failure() -> Result<()> {
    let temp_dir = tempdir()?;
    let state_path = temp_dir.path().join("count.txt");
    let script_path = temp_dir.path().join("fake_codex.sh");
    let script = format!(
        "#!/bin/sh\nCOUNT_FILE=\"{}\"\ncount=0\nif [ -f \"$COUNT_FILE\" ]; then\n  count=$(cat \"$COUNT_FILE\")\nfi\ncount=$((count + 1))\nprintf '%s' \"$count\" > \"$COUNT_FILE\"\nif [ \"$count\" -eq 1 ]; then\n  echo 'temporary failure' >&2\n  exit 1\nfi\necho 'retry ok'\n",
        state_path.display()
    );
    std::fs::write(&script_path, script)?;
    let mut permissions = std::fs::metadata(&script_path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&script_path, permissions)?;

    let runner = CodexRunner::new(script_path.display().to_string(), Duration::from_secs(5));
    let response = runner.run_prompt("hello")?;

    assert_eq!(response, "retry ok");
    assert_eq!(std::fs::read_to_string(state_path)?, "2");

    Ok(())
}

#[test]
fn run_prompt_times_out_and_does_not_retry() -> Result<()> {
    let temp_dir = tempdir()?;
    let script_path = temp_dir.path().join("hanging_codex.sh");
    let script = "#!/bin/sh\nsleep 5\nprintf 'too late\\n'\n";
    std::fs::write(&script_path, script)?;
    let mut permissions = std::fs::metadata(&script_path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&script_path, permissions)?;

    let runner = CodexRunner::new(
        script_path.display().to_string(),
        Duration::from_millis(300),
    );
    let started = Instant::now();
    let error = runner
        .run_prompt("hello")
        .expect_err("expected timeout for hanging command");

    assert!(started.elapsed() < Duration::from_millis(550));
    assert!(error.to_string().contains("완료되지 않았습니다"));

    Ok(())
}

#[test]
fn run_prompt_writes_opt_in_prompt_log() -> Result<()> {
    let temp_dir = tempdir()?;
    let script_path = temp_dir.path().join("logging_codex.sh");
    let log_dir = temp_dir.path().join("prompt-logs");
    let script = "#!/bin/sh\necho 'logged ok'\n";
    std::fs::write(&script_path, script)?;
    let mut permissions = std::fs::metadata(&script_path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&script_path, permissions)?;

    let runner = CodexRunner::new(script_path.display().to_string(), Duration::from_secs(5))
        .with_prompt_logging(log_dir.clone());
    let response = runner.run_prompt_named("planner", "Prompt body for logging")?;

    assert_eq!(response, "logged ok");

    let entries = std::fs::read_dir(&log_dir)?
        .map(|entry| entry.map(|value| value.path()))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    assert_eq!(entries.len(), 1);

    let log = std::fs::read_to_string(&entries[0])?;
    assert!(log.contains("\"label\": \"planner\""));
    assert!(log.contains("\"prompt\": \"Prompt body for logging\""));
    assert!(log.contains("\"response\": \"logged ok\""));
    assert!(log.contains("\"outcome\": \"ok\""));

    Ok(())
}

#[test]
fn run_prompt_named_prefers_output_last_message_file_when_present() -> Result<()> {
    let temp_dir = tempdir()?;
    let script_path = temp_dir.path().join("json_progress_codex.sh");
    let script = "#!/bin/sh\nOUT=\"\"\nwhile [ \"$#\" -gt 0 ]; do\n  if [ \"$1\" = \"--output-last-message\" ]; then\n    OUT=\"$2\"\n    shift 2\n    continue\n  fi\n  shift\ndone\necho '{\"type\":\"thread.started\"}'\necho '{\"type\":\"turn.started\"}'\nif [ -n \"$OUT\" ]; then\n  printf 'final from file\\n' > \"$OUT\"\nfi\n";
    std::fs::write(&script_path, script)?;
    let mut permissions = std::fs::metadata(&script_path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&script_path, permissions)?;

    let runner = CodexRunner::new(script_path.display().to_string(), Duration::from_secs(5));
    let response = runner.run_prompt_named("writer", "hello")?;

    assert_eq!(response, "final from file");

    Ok(())
}

#[test]
fn critic_uses_longer_timeout_than_base_runner_timeout() -> Result<()> {
    let temp_dir = tempdir()?;
    let script_path = temp_dir.path().join("slow_critic_codex.sh");
    let script = "#!/bin/sh\nsleep 1\nprintf 'slow critic ok\\n'\n";
    std::fs::write(&script_path, script)?;
    let mut permissions = std::fs::metadata(&script_path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&script_path, permissions)?;

    let runner = CodexRunner::new(
        script_path.display().to_string(),
        Duration::from_millis(200),
    );
    let started = Instant::now();
    let response = runner.run_prompt_named("critic", "hello")?;

    assert_eq!(response, "slow critic ok");
    assert!(started.elapsed() >= Duration::from_millis(900));

    Ok(())
}
