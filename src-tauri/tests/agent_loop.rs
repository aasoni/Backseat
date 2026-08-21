//! Exercises the async agent loop: spawn the fake agent, watch the round dir,
//! observe replies arriving live and the done signal, then the exit callback.

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use backseat_lib::agent::{self, AgentCmd};
use backseat_lib::git::Git;
use backseat_lib::model::{DoneStatus, Scope, Side};
use backseat_lib::store::Store;
use backseat_lib::watcher;

fn git(cwd: &Path, args: &[&str]) {
    let out = Command::new("git").args(args).current_dir(cwd).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn spawn_watch_fold_loop() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    git(repo, &["init", "-q", "-b", "main"]);
    git(repo, &["config", "user.email", "t@t.test"]);
    git(repo, &["config", "user.name", "T"]);
    std::fs::write(repo.join("a.txt"), "alpha\nbeta\ngamma\n").unwrap();
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-qm", "init"]);
    std::fs::write(repo.join("a.txt"), "alpha\nBETA\ngamma\n").unwrap();

    let g = Git::discover(repo).unwrap();
    let store = Store::init(repo).unwrap();
    store
        .add_comment(&g, &Scope::Worktree, None, Some("a.txt"), Side::New, 2, 2, "why caps?", "R")
        .unwrap();
    let (meta, _) = store.submit_round(&g, &Scope::Worktree, None, "R", None).unwrap();
    let round = meta.number;

    let fake = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("dev/fake-agent.py");
    let cmd = AgentCmd::Custom(format!("FAKE_AGENT_MODE=slow python3 '{}'", fake.display()));

    let change_count = Arc::new(AtomicUsize::new(0));
    let cc = change_count.clone();
    let _watch = watcher::watch_round(&store.round_dir(round), move || {
        cc.fetch_add(1, Ordering::SeqCst);
    })
    .unwrap();

    let exited = Arc::new(AtomicBool::new(false));
    let ex = exited.clone();
    let _running = agent::spawn_round(
        &cmd,
        repo,
        round,
        &Store::round_rel_path(round),
        None,
        &store.round_dir(round).join("agent.log"),
        move |exit| {
            assert_eq!(exit.round, round);
            assert!(exit.success);
            ex.store(true, Ordering::SeqCst);
        },
    )
    .unwrap();

    // Wait for the done signal to land (slow mode sleeps between replies).
    let deadline = Instant::now() + Duration::from_secs(30);
    while store.read_done(round).unwrap().is_none() {
        assert!(Instant::now() < deadline, "agent never wrote done.json");
        std::thread::sleep(Duration::from_millis(100));
    }
    // Watcher must have fired at least once for the reply files.
    let deadline = Instant::now() + Duration::from_secs(5);
    while change_count.load(Ordering::SeqCst) == 0 {
        assert!(Instant::now() < deadline, "watcher never fired");
        std::thread::sleep(Duration::from_millis(50));
    }
    // And the exit callback runs once the process is gone.
    let deadline = Instant::now() + Duration::from_secs(10);
    while !exited.load(Ordering::SeqCst) {
        assert!(Instant::now() < deadline, "exit callback never ran");
        std::thread::sleep(Duration::from_millis(100));
    }

    // Fold everything; the round completes.
    let folded = store.fold_new_replies(round).unwrap();
    assert!(!folded.is_empty());
    let done = store.read_done(round).unwrap().unwrap();
    assert_eq!(done.status, DoneStatus::Completed);
    store.fold_done(round, &done).unwrap();

    // agent.log captured output.
    assert!(store.round_dir(round).join("agent.log").exists());
}
