use anyhow::Result;
use heeforge::codex_runner::CodexRunner;
use std::os::unix::fs::PermissionsExt;
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

    let runner = CodexRunner::new(script_path.display().to_string());
    let response = runner.run_prompt("hello")?;

    assert_eq!(response, "retry ok");
    assert_eq!(std::fs::read_to_string(state_path)?, "2");

    Ok(())
}
