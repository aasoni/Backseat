use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::error::{Error, Result};

/// How the agent is launched. `BACKSEAT_AGENT_CMD` (a shell command) overrides
/// the real Claude Code binary — used by tests and the fake-agent harness.
#[derive(Clone, Debug)]
pub enum AgentCmd {
    Claude(PathBuf),
    Custom(String),
}

/// Resolve the agent launcher. GUI apps on macOS don't inherit the shell PATH,
/// so `claude` is resolved through a login shell once and cached by the caller.
pub fn resolve_agent_cmd() -> Result<AgentCmd> {
    if let Ok(cmd) = std::env::var("BACKSEAT_AGENT_CMD") {
        if !cmd.trim().is_empty() {
            return Ok(AgentCmd::Custom(cmd));
        }
    }
    let out = Command::new("/bin/zsh")
        .args(["-lc", "command -v claude"])
        .output()?;
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || path.is_empty() {
        return Err(Error::AgentNotFound);
    }
    Ok(AgentCmd::Claude(PathBuf::from(path)))
}

pub struct RunningAgent {
    child: Arc<Mutex<Option<Child>>>,
}

impl RunningAgent {
    pub fn kill(&self) {
        if let Some(child) = self.child.lock().unwrap().as_mut() {
            let _ = child.kill();
        }
    }
}

/// Outcome of a finished agent process, handed to the completion callback.
pub struct AgentExit {
    pub round: u32,
    pub success: bool,
    /// Claude Code session id parsed from its JSON result, for `--resume`.
    pub session_id: Option<String>,
}

/// Spawn the agent for a round. Stdout/stderr stream into `log_path`; when the
/// process exits, `on_exit` runs on the monitor thread.
pub fn spawn_round(
    cmd: &AgentCmd,
    repo_root: &Path,
    round: u32,
    review_rel_path: &str,
    resume_session_id: Option<&str>,
    log_path: &Path,
    on_exit: impl FnOnce(AgentExit) + Send + 'static,
) -> Result<RunningAgent> {
    let prompt = format!(
        "A new Backseat review round was submitted. Use the backseat skill: read \
         .backseat/{review_rel_path}/review.json, act on the feedback, write your replies, \
         and write the done signal exactly as the skill specifies."
    );

    let mut command = match cmd {
        AgentCmd::Claude(bin) => {
            let mut c = Command::new(bin);
            c.arg("-p")
                .arg(&prompt)
                .args(["--output-format", "json"])
                .args(["--permission-mode", "acceptEdits"])
                .args([
                    "--allowedTools",
                    "Skill,Read,Edit,Write,MultiEdit,Grep,Glob,Bash(git:*)",
                ]);
            if let Some(sid) = resume_session_id {
                c.args(["--resume", sid]);
            }
            c
        }
        AgentCmd::Custom(shell_cmd) => {
            let mut c = Command::new("/bin/sh");
            c.arg("-c").arg(shell_cmd);
            c.env("BACKSEAT_ROUND", round.to_string())
                .env("BACKSEAT_REVIEW", format!(".backseat/{review_rel_path}/review.json"))
                .env("BACKSEAT_PROMPT", &prompt);
            c
        }
    };
    command
        .current_dir(repo_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => Error::AgentNotFound,
        _ => Error::Io(e),
    })?;

    // Drain stdout/stderr into the log so the child never blocks on a full pipe.
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let log_path = log_path.to_path_buf();
    let stdout_buf: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));

    fn drain<R: Read + Send + 'static>(
        src: Option<R>,
        sink: Arc<Mutex<String>>,
    ) -> Option<std::thread::JoinHandle<()>> {
        src.map(|mut r| {
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                while let Ok(n) = r.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    sink.lock()
                        .unwrap()
                        .push_str(&String::from_utf8_lossy(&buf[..n]));
                }
            })
        })
    }
    let stderr_buf: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let t_out = drain(stdout, stdout_buf.clone());
    let t_err = drain(stderr, stderr_buf.clone());

    let child = Arc::new(Mutex::new(Some(child)));
    let running = RunningAgent {
        child: child.clone(),
    };

    std::thread::spawn(move || {
        // Poll rather than wait(): kill() needs the same handle.
        let status = loop {
            let done = {
                let mut guard = child.lock().unwrap();
                match guard.as_mut() {
                    Some(c) => c.try_wait().ok().flatten(),
                    None => break None,
                }
            };
            if let Some(s) = done {
                break Some(s);
            }
            std::thread::sleep(Duration::from_millis(200));
        };
        if let Some(t) = t_out {
            let _ = t.join();
        }
        if let Some(t) = t_err {
            let _ = t.join();
        }
        *child.lock().unwrap() = None;

        let out = stdout_buf.lock().unwrap().clone();
        let err = stderr_buf.lock().unwrap().clone();
        let _ = std::fs::write(
            &log_path,
            format!("--- stdout ---\n{out}\n--- stderr ---\n{err}\n"),
        );

        on_exit(AgentExit {
            round,
            success: status.map(|s| s.success()).unwrap_or(false),
            session_id: parse_session_id(&out),
        });
    });

    Ok(running)
}

/// Pull `session_id` out of Claude Code's `--output-format json` result.
fn parse_session_id(stdout: &str) -> Option<String> {
    for line in stdout.lines().rev() {
        let line = line.trim();
        if !line.starts_with('{') {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(sid) = v.get("session_id").and_then(|s| s.as_str()) {
                return Some(sid.to_string());
            }
        }
    }
    None
}
