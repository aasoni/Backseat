//! Manual end-to-end check against the REAL Claude Code CLI.
//!
//! Builds a throwaway fixture repo, leaves one inline comment plus overall
//! feedback, submits a round, spawns `claude` headless exactly as the app does,
//! and reports what came back. Run with:
//!
//!     cargo run --example real_e2e
//!
//! Requires a logged-in `claude` CLI. Spends one small headless session.

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use backseat_lib::agent;
use backseat_lib::git::Git;
use backseat_lib::model::{Scope, Side};
use backseat_lib::skill;
use backseat_lib::store::Store;

fn git(cwd: &Path, args: &[&str]) {
    let out = Command::new("git").args(args).current_dir(cwd).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

fn main() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    git(repo, &["init", "-q", "-b", "main"]);
    git(repo, &["config", "user.email", "t@t.test"]);
    git(repo, &["config", "user.name", "T"]);
    std::fs::write(
        repo.join("stats.py"),
        "def mean(xs):\n    return sum(xs) / len(xs)\n\n\ndef total(xs):\n    return sum(xs)\n",
    )
    .unwrap();
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-qm", "init"]);
    // The "agent's change" under review: a median with a subtle bug.
    std::fs::write(
        repo.join("stats.py"),
        "def mean(xs):\n    return sum(xs) / len(xs)\n\n\ndef median(xs):\n    xs = sorted(xs)\n    return xs[len(xs) // 2]\n\n\ndef total(xs):\n    return sum(xs)\n",
    )
    .unwrap();

    let g = Git::discover(repo).unwrap();
    let store = Store::init(repo).unwrap();
    skill::ensure_skill(repo).unwrap();
    skill::ensure_git_exclude(&g).unwrap();

    store
        .add_comment(
            &g,
            &Scope::Worktree,
            None,
            Some("stats.py"),
            Side::New,
            7,
            7,
            "This is wrong for even-length lists — the median should average the two middle elements. Please fix and handle the empty-list case explicitly.",
            "Reviewer",
        )
        .unwrap();

    let (meta, review_path) = store
        .submit_round(
            &g,
            &Scope::Worktree,
            Some("Keep the file dependency-free; stdlib only."),
            "Reviewer",
            Some("claude-code"),
        )
        .unwrap();
    println!("submitted round {} -> {}", meta.number, review_path.display());

    let cmd = agent::resolve_agent_cmd().expect("claude CLI not found");
    println!("spawning agent: {cmd:?}");
    let (tx, rx) = std::sync::mpsc::channel();
    let _running = agent::spawn_round(
        &cmd,
        repo,
        meta.number,
        &Store::round_rel_path(meta.number),
        None,
        &store.round_dir(meta.number).join("agent.log"),
        move |exit| {
            let _ = tx.send((exit.success, exit.session_id));
        },
    )
    .unwrap();

    let start = Instant::now();
    let deadline = start + Duration::from_secs(600);
    loop {
        if let Some(done) = store.read_done(meta.number).unwrap() {
            println!(
                "done after {:?}: status={:?} summary={}",
                start.elapsed(),
                done.status,
                done.summary
            );
            store.fold_done(meta.number, &done).unwrap();
            break;
        }
        if Instant::now() > deadline {
            eprintln!("TIMED OUT waiting for done.json; agent.log follows:");
            eprintln!(
                "{}",
                std::fs::read_to_string(store.round_dir(meta.number).join("agent.log"))
                    .unwrap_or_default()
            );
            std::process::exit(1);
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    let folded = store.fold_new_replies(meta.number).unwrap();
    println!("\n--- {} replies ---", folded.len());
    for f in &folded {
        println!(
            "[{}] resolved={} refs={:?}\n{}\n",
            f.target, f.marks_resolved, f.comment.refs, f.comment.body
        );
    }

    if let Ok((success, session_id)) = rx.recv_timeout(Duration::from_secs(60)) {
        println!("process success={success} session_id={session_id:?}");
    }

    println!("\n--- final stats.py ---");
    println!("{}", std::fs::read_to_string(repo.join("stats.py")).unwrap());
    println!("--- git status (should show only stats.py modified, no commits) ---");
    let out = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo)
        .output()
        .unwrap();
    println!("{}", String::from_utf8_lossy(&out.stdout));
    let out = Command::new("git")
        .args(["log", "--oneline"])
        .current_dir(repo)
        .output()
        .unwrap();
    println!("{}", String::from_utf8_lossy(&out.stdout));
}
