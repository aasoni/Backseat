//! End-to-end protocol tests: submit a round, run the fake agent (which
//! implements the skill contract), fold its replies and done signal back.

use std::path::{Path, PathBuf};
use std::process::Command;

use backseat_lib::git::Git;
use backseat_lib::model::{RoundStatus, Scope, Side};
use backseat_lib::store::Store;

fn run(cwd: &Path, cmd: &str, args: &[&str]) -> String {
    let out = Command::new(cmd)
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap_or_else(|e| panic!("failed to run {cmd}: {e}"));
    assert!(
        out.status.success(),
        "{cmd} {args:?} failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn git(cwd: &Path, args: &[&str]) -> String {
    run(cwd, "git", args)
}

fn init_repo(dir: &Path) {
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.email", "t@t.test"]);
    git(dir, &["config", "user.name", "T"]);
}

fn commit_file(dir: &Path, path: &str, content: &str, msg: &str) {
    let full = dir.join(path);
    std::fs::create_dir_all(full.parent().unwrap()).unwrap();
    std::fs::write(&full, content).unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-qm", msg]);
}

fn fake_agent_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("dev/fake-agent.py")
}

fn run_fake_agent(repo: &Path, mode: Option<&str>) {
    let mut c = Command::new("python3");
    c.arg(fake_agent_path()).current_dir(repo);
    if let Some(m) = mode {
        c.env("FAKE_AGENT_MODE", m);
    }
    let out = c.output().unwrap();
    assert!(
        out.status.success(),
        "fake agent failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn worktree_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    init_repo(repo);
    commit_file(repo, "src/lib.rs", "fn a() {}\nfn b() {}\nfn c() {}\n", "init");

    // Dirty the worktree: the change under review.
    std::fs::write(
        repo.join("src/lib.rs"),
        "fn a() {}\nfn b_renamed() {}\nfn c() {}\n",
    )
    .unwrap();

    let g = Git::discover(repo).unwrap();
    let store = Store::init(repo).unwrap();
    let scope = Scope::Worktree;

    // Diff shows the modified file.
    let files = g.changed_files(&scope).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "src/lib.rs");

    // Leave an inline comment on the changed line (new side, line 2).
    let thread = store
        .add_comment(
            &g,
            &scope,
            None,
            Some("src/lib.rs"),
            Side::New,
            2,
            2,
            "Please add a doc comment here.",
            "Reviewer",
        )
        .unwrap();
    assert_eq!(thread.anchor.snapshot, vec!["fn b_renamed() {}"]);

    // Submit round 1 with overall feedback.
    let (meta, review_path) = store
        .submit_round(&g, &scope, Some("Looks good overall."), "Reviewer", Some("claude-code"))
        .unwrap();
    assert_eq!(meta.number, 1);
    assert!(review_path.exists());
    let review: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&review_path).unwrap()).unwrap();
    assert_eq!(review["threads"].as_array().unwrap().len(), 1);
    assert_eq!(review["overall"]["body"], "Looks good overall.");
    assert_eq!(review["scope"]["type"], "worktree");

    // Agent acts.
    run_fake_agent(repo, None);

    // Fold replies + done.
    let folded = store.fold_new_replies(1).unwrap();
    assert_eq!(folded.len(), 2, "one thread reply + one overall reply");
    let thread_reply = folded.iter().find(|f| f.target == thread.id).unwrap();
    assert!(thread_reply.marks_resolved);
    assert!(thread_reply.comment.suggestion.is_some());

    let done = store.read_done(1).unwrap().expect("done.json written");
    store.fold_done(1, &done).unwrap();
    let state = store.load_state().unwrap();
    assert_eq!(state.rounds[0].status, RoundStatus::Done);

    // Session reflects the folded state; the edited anchor re-anchors or orphans
    // gracefully (agent appended a marker to the line, so first-line fallback
    // must NOT match the old exact text — the line changed).
    let session = store.re_anchor(&g, &scope).unwrap();
    let t = &session.threads[0];
    assert!(t.resolved);
    assert_eq!(t.comments.len(), 2);

    // Folding twice must be a no-op.
    assert!(store.fold_new_replies(1).unwrap().is_empty());

    // Round 2: replying on the resolved thread reopens it; review restates it.
    store
        .add_comment(&g, &scope, Some(&thread.id), None, Side::New, 2, 2, "Still not documented.", "Reviewer")
        .unwrap();
    let (meta2, review2) = store
        .submit_round(&g, &scope, None, "Reviewer", None)
        .unwrap();
    assert_eq!(meta2.number, 2);
    let review2: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&review2).unwrap()).unwrap();
    let threads2 = review2["threads"].as_array().unwrap();
    assert_eq!(threads2.len(), 1);
    assert_eq!(threads2[0]["comments"].as_array().unwrap().len(), 3);
    assert_eq!(threads2[0]["new_in_round"], 2);
}

#[test]
fn commit_scope_round_amends_and_rebases() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    init_repo(repo);
    commit_file(repo, "app.py", "def one():\n    return 1\n", "first");
    commit_file(repo, "app.py", "def one():\n    return 1\n\ndef two():\n    return 2\n", "second: add two");
    commit_file(repo, "other.txt", "unrelated\n", "third");

    let g = Git::discover(repo).unwrap();
    let store = Store::init(repo).unwrap();
    backseat_lib::skill::ensure_git_exclude(&g).unwrap();
    let target_sha = git(repo, &["rev-parse", "HEAD^"]).trim().to_string();
    let scope = Scope::Commit {
        sha: target_sha.clone(),
        subject: Some("second: add two".into()),
    };

    // The commit's diff shows app.py.
    let files = g.changed_files(&scope).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "app.py");

    // Comment on the added function (new side of that commit, line 4).
    store
        .add_comment(&g, &scope, None, Some("app.py"), Side::New, 4, 4, "Name this better.", "Reviewer")
        .unwrap();
    store
        .submit_round(&g, &scope, None, "Reviewer", None)
        .unwrap();

    run_fake_agent(repo, None);

    store.fold_new_replies(1).unwrap();
    let done = store.read_done(1).unwrap().expect("done.json written");
    let map = done.commit_map.clone().expect("commit_map for commit scope");
    let new_sha = map.get(&target_sha).expect("old sha mapped").clone();
    assert_ne!(new_sha, target_sha);
    store.fold_done(1, &done).unwrap();

    // History was rewritten: the amended commit contains the agent's edit and
    // the descendant commit was rebased on top of it.
    let show = git(repo, &["show", &format!("{new_sha}:app.py")]);
    assert!(show.contains("addressed by fake agent"), "amended commit has the fix:\n{show}");
    let subjects = git(repo, &["log", "--format=%s"]);
    assert_eq!(subjects.trim().split('\n').count(), 3, "no extra commits left over");
    assert!(git(repo, &["status", "--porcelain"]).trim().is_empty(), "worktree clean after rebase");

    // Session threads retargeted to the new sha.
    let session = store.load_session().unwrap();
    match &session.threads[0].scope {
        Scope::Commit { sha, .. } => assert_eq!(sha, &new_sha),
        _ => panic!("thread should stay commit-scoped"),
    }
}

#[test]
fn blocked_and_missing_done_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    init_repo(repo);
    commit_file(repo, "f.txt", "hello\nworld\n", "init");
    std::fs::write(repo.join("f.txt"), "hello\nplanet\n").unwrap();

    let g = Git::discover(repo).unwrap();
    let store = Store::init(repo).unwrap();
    store
        .add_comment(&g, &Scope::Worktree, None, Some("f.txt"), Side::New, 2, 2, "hm", "R")
        .unwrap();
    store
        .submit_round(&g, &Scope::Worktree, None, "R", None)
        .unwrap();

    run_fake_agent(repo, Some("blocked"));
    let done = store.read_done(1).unwrap().expect("blocked still writes done.json");
    store.fold_done(1, &done).unwrap();
    assert_eq!(store.load_state().unwrap().rounds[0].status, RoundStatus::Blocked);
}
