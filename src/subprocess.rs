use std::ffi::OsStr;
use std::io::{self, BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

const POLL_INTERVAL: Duration = Duration::from_millis(100);

pub struct ProcessOutcome {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug)]
pub enum SubprocessError {
    Spawn(io::Error),
    Wait(io::Error),
    Timeout,
}

pub fn run_with_timeout<I, S>(
    command: &str,
    args: I,
    timeout: Duration,
) -> Result<ProcessOutcome, SubprocessError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut process_command = Command::new(command);
    process_command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_process_group(&mut process_command);

    let mut child = process_command.spawn().map_err(SubprocessError::Spawn)?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (tx, rx) = mpsc::channel();
    let mut handles = Vec::new();
    if let Some(stdout) = stdout {
        let tx = tx.clone();
        handles.push(thread::spawn(move || drain(stdout, true, tx)));
    }
    if let Some(stderr) = stderr {
        let tx = tx.clone();
        handles.push(thread::spawn(move || drain(stderr, false, tx)));
    }
    drop(tx);

    let started = Instant::now();
    let exit_status = loop {
        match child.try_wait().map_err(SubprocessError::Wait)? {
            Some(status) => break status,
            None if started.elapsed() >= timeout => {
                terminate_process_tree(&mut child);
                let _ = child.wait();
                for handle in handles {
                    let _ = handle.join();
                }
                return Err(SubprocessError::Timeout);
            }
            None => thread::sleep(POLL_INTERVAL.min(timeout)),
        }
    };

    for handle in handles {
        let _ = handle.join();
    }

    let mut stdout_lines: Vec<String> = Vec::new();
    let mut stderr_lines: Vec<String> = Vec::new();
    while let Ok((is_stdout, line)) = rx.try_recv() {
        if is_stdout {
            stdout_lines.push(line);
        } else {
            stderr_lines.push(line);
        }
    }

    Ok(ProcessOutcome {
        success: exit_status.success(),
        stdout: stdout_lines.join("\n").trim().to_string(),
        stderr: stderr_lines.join("\n").trim().to_string(),
    })
}

fn drain<R: Read + Send + 'static>(
    reader: R,
    is_stdout: bool,
    tx: mpsc::Sender<(bool, String)>,
) {
    let reader = BufReader::new(reader);
    for line in reader.lines() {
        let Ok(line) = line else {
            break;
        };
        if tx.send((is_stdout, line)).is_err() {
            break;
        }
    }
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
