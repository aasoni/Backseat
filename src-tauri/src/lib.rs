pub mod agent;
pub mod apply;
pub mod diffparse;
pub mod error;
pub mod git;
pub mod model;
pub mod skill;
pub mod store;
pub mod watcher;

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, State, Wry};
use tauri_plugin_notification::NotificationExt;

use error::{Error, Result};
use git::Git;
use model::{
    AgentStatus, CommitInfo, DiffFileInfo, DoneStatus, FileDiff, NewCommentInput, ProjectInfo,
    RoundMeta, RoundStatus, Scope, SessionState, ThreadRecord,
};
use store::Store;

// ---------------------------------------------------------------------------
// Runtime state
// ---------------------------------------------------------------------------

struct ProjectCtx {
    repo_root: PathBuf,
    agent_kind: Option<String>,
}

struct RoundRuntime {
    round: u32,
    agent: Option<agent::RunningAgent>,
    _watcher: watcher::RoundWatcher,
}

#[derive(Default)]
struct AppState {
    project: Mutex<Option<ProjectCtx>>,
    runtime: Mutex<Option<RoundRuntime>>,
}

fn with_project<T>(state: &State<'_, AppState>, f: impl FnOnce(&ProjectCtx) -> Result<T>) -> Result<T> {
    let guard = state.project.lock().unwrap();
    let ctx = guard.as_ref().ok_or(Error::NoProject)?;
    f(ctx)
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

const EV_AGENT_STATUS: &str = "backseat://agent-status";
const EV_REPLY: &str = "backseat://reply";
const EV_ROUND_DONE: &str = "backseat://round-done";
const EV_DIFF_INVALIDATED: &str = "backseat://diff-invalidated";
const EV_TOGGLE_THEME: &str = "backseat://toggle-theme";

const MENU_ID_TOGGLE_THEME: &str = "toggle-theme";

/// Handle to the View > Switch to … Mode item, for relabeling on theme change.
struct ThemeMenuItem(MenuItem<Wry>);

#[derive(Clone, serde::Serialize)]
struct AgentStatusEvent {
    status: AgentStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

#[derive(Clone, serde::Serialize)]
struct RoundDoneEvent {
    round: u32,
    status: DoneStatus,
    summary: String,
}

fn emit_status(app: &AppHandle, status: AgentStatus, detail: Option<String>) {
    let _ = app.emit(EV_AGENT_STATUS, AgentStatusEvent { status, detail });
}

/// Fold new replies and (if present) the done signal for a round, emitting the
/// corresponding events. Idempotent — safe to call from the watcher, the agent
/// exit handler, and project open.
fn sync_round(app: &AppHandle, repo_root: &PathBuf, round: u32) {
    let Ok(store) = Store::init(repo_root) else {
        return;
    };
    if let Ok(folded) = store.fold_new_replies(round) {
        for r in folded {
            let _ = app.emit(EV_REPLY, &r);
        }
    }
    let Ok(state) = store.load_state() else { return };
    let in_progress = state
        .rounds
        .iter()
        .any(|r| r.number == round && r.status == RoundStatus::InProgress);
    if !in_progress {
        return;
    }
    if let Ok(Some(done)) = store.read_done(round) {
        if store.fold_done(round, &done).is_ok() {
            let _ = app.emit(
                EV_ROUND_DONE,
                RoundDoneEvent {
                    round,
                    status: done.status,
                    summary: done.summary.clone(),
                },
            );
            let _ = app.emit(EV_DIFF_INVALIDATED, ());
            emit_status(app, AgentStatus::Idle, None);
            notify_if_unfocused(app, &done.summary);
        }
    }
}

fn notify_if_unfocused(app: &AppHandle, summary: &str) {
    let focused = app
        .get_webview_window("main")
        .and_then(|w| w.is_focused().ok())
        .unwrap_or(false);
    if !focused {
        let _ = app
            .notification()
            .builder()
            .title("Backseat · agent finished")
            .body(summary)
            .show();
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
async fn open_project(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<ProjectInfo> {
    let git = Git::discover(std::path::Path::new(&path))?;
    let repo_root = git.repo_root().to_path_buf();
    let store = Store::init(&repo_root)?;

    skill::ensure_skill(&repo_root)?;
    skill::ensure_git_exclude(&git)?;

    let backseat_state = store.load_state()?;
    let agent_kind = backseat_state
        .agent
        .kind
        .clone()
        .or_else(|| skill::detect_agent(&repo_root));

    let name = repo_root
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.clone());

    let info = ProjectInfo {
        repo_root: repo_root.to_string_lossy().into_owned(),
        name,
        branch: git.branch(),
        base_branch: git.base_branch(),
        agent_kind: agent_kind.clone(),
    };

    *state.project.lock().unwrap() = Some(ProjectCtx {
        repo_root: repo_root.clone(),
        agent_kind,
    });
    *state.runtime.lock().unwrap() = None;

    // Recover a round left in flight by a previous app session: fold what's on
    // disk and, if it's genuinely still unfinished, resume watching for the
    // agent (which survives the app) to deliver replies and the done signal.
    let current = backseat_state.current_round;
    if current > 0 {
        sync_round(&app, &repo_root, current);
        let st = store.load_state()?;
        let still_in_progress = st
            .rounds
            .iter()
            .any(|r| r.number == current && r.status == RoundStatus::InProgress);
        if still_in_progress {
            let app2 = app.clone();
            let root2 = repo_root.clone();
            if let Ok(w) = watcher::watch_round(&store.round_dir(current), move || {
                sync_round(&app2, &root2, current);
            }) {
                *state.runtime.lock().unwrap() = Some(RoundRuntime {
                    round: current,
                    agent: None,
                    _watcher: w,
                });
                emit_status(&app, AgentStatus::Working, None);
            }
        }
    }

    Ok(info)
}

#[tauri::command]
async fn close_project(state: State<'_, AppState>) -> Result<()> {
    *state.runtime.lock().unwrap() = None;
    *state.project.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
async fn list_commits(state: State<'_, AppState>, limit: Option<u32>) -> Result<Vec<CommitInfo>> {
    with_project(&state, |ctx| {
        Git::discover(&ctx.repo_root)?.list_commits(limit.unwrap_or(30))
    })
}

#[tauri::command]
async fn get_diff(state: State<'_, AppState>, scope: Scope) -> Result<Vec<DiffFileInfo>> {
    with_project(&state, |ctx| {
        Git::discover(&ctx.repo_root)?.changed_files(&scope)
    })
}

#[tauri::command]
async fn get_file_diff(
    state: State<'_, AppState>,
    scope: Scope,
    path: String,
    old_path: Option<String>,
) -> Result<FileDiff> {
    with_project(&state, |ctx| {
        let git = Git::discover(&ctx.repo_root)?;
        let text = git.file_diff_text(&scope, &path, old_path.as_deref())?;
        Ok(diffparse::parse_unified(&text))
    })
}

#[tauri::command]
async fn load_session(state: State<'_, AppState>, scope: Scope) -> Result<SessionState> {
    with_project(&state, |ctx| {
        let git = Git::discover(&ctx.repo_root)?;
        let store = Store::init(&ctx.repo_root)?;
        let session = store.re_anchor(&git, &scope)?;
        let backseat_state = store.load_state()?;

        let threads: Vec<ThreadRecord> = session
            .threads
            .iter()
            .filter(|t| t.scope == scope)
            .cloned()
            .collect();
        let viewed = session.viewed.get(&scope.key()).cloned().unwrap_or_default();

        let working = backseat_state
            .rounds
            .iter()
            .any(|r| r.status == RoundStatus::InProgress);
        let last_error = backseat_state
            .rounds
            .iter()
            .rev()
            .next()
            .map(|r| r.status == RoundStatus::Error)
            .unwrap_or(false);
        let agent_status = if working {
            AgentStatus::Working
        } else if last_error {
            AgentStatus::Error
        } else {
            AgentStatus::Idle
        };

        let files = git.changed_files(&scope).unwrap_or_default();
        let last_edit_time = git.last_edit_time(&files);

        Ok(SessionState {
            threads,
            overall: session.overall.clone(),
            viewed,
            rounds: backseat_state.rounds.clone(),
            current_round: backseat_state.current_round,
            agent_status,
            agent_kind: ctx.agent_kind.clone(),
            last_edit_time,
        })
    })
}

#[tauri::command]
async fn add_comment(
    state: State<'_, AppState>,
    scope: Scope,
    input: NewCommentInput,
) -> Result<ThreadRecord> {
    with_project(&state, |ctx| {
        let git = Git::discover(&ctx.repo_root)?;
        let store = Store::init(&ctx.repo_root)?;
        let side = input.side.unwrap_or(model::Side::New);
        store.add_comment(
            &git,
            &scope,
            input.thread_id.as_deref(),
            input.path.as_deref(),
            side,
            input.start_line.unwrap_or(1),
            input.end_line.or(input.start_line).unwrap_or(1),
            &input.body,
            "You",
        )
    })
}

#[tauri::command]
async fn discard_pending(state: State<'_, AppState>, scope: Scope) -> Result<()> {
    with_project(&state, |ctx| Store::init(&ctx.repo_root)?.discard_pending(&scope))
}

#[tauri::command]
async fn set_thread_resolved(
    state: State<'_, AppState>,
    thread_id: String,
    resolved: bool,
) -> Result<()> {
    with_project(&state, |ctx| {
        Store::init(&ctx.repo_root)?.set_thread_resolved(&thread_id, resolved)
    })
}

#[tauri::command]
async fn set_file_viewed(
    state: State<'_, AppState>,
    scope: Scope,
    path: String,
    viewed: bool,
) -> Result<()> {
    with_project(&state, |ctx| {
        Store::init(&ctx.repo_root)?.set_file_viewed(&scope, &path, viewed)
    })
}

#[tauri::command]
async fn set_agent_kind(state: State<'_, AppState>, kind: String) -> Result<()> {
    let repo_root = with_project(&state, |ctx| Ok(ctx.repo_root.clone()))?;
    Store::init(&repo_root)?.set_agent_kind(&kind)?;
    if let Some(ctx) = state.project.lock().unwrap().as_mut() {
        ctx.agent_kind = Some(kind);
    }
    Ok(())
}

#[tauri::command]
async fn submit_round(
    app: AppHandle,
    state: State<'_, AppState>,
    scope: Scope,
    overall_body: Option<String>,
) -> Result<RoundMeta> {
    let (repo_root, agent_kind) =
        with_project(&state, |ctx| Ok((ctx.repo_root.clone(), ctx.agent_kind.clone())))?;

    // One round in flight at a time.
    {
        let store = Store::init(&repo_root)?;
        let st = store.load_state()?;
        if st.rounds.iter().any(|r| r.status == RoundStatus::InProgress) {
            return Err(Error::RoundInFlight);
        }
    }

    // Resolve the agent launcher *before* writing anything.
    let cmd = agent::resolve_agent_cmd()?;

    let git = Git::discover(&repo_root)?;
    let store = Store::init(&repo_root)?;
    skill::ensure_skill(&repo_root)?;

    let (meta, _review_path) = store.submit_round(
        &git,
        &scope,
        overall_body.as_deref(),
        "You",
        agent_kind.as_deref(),
    )?;
    let round = meta.number;
    let round_rel = Store::round_rel_path(round);
    let session_id = store.load_state()?.agent.session_id;

    // Watch for replies / done.
    let app2 = app.clone();
    let root2 = repo_root.clone();
    let w = watcher::watch_round(&store.round_dir(round), move || {
        sync_round(&app2, &root2, round);
    })?;

    // Spawn the agent.
    let app3 = app.clone();
    let root3 = repo_root.clone();
    let running = agent::spawn_round(
        &cmd,
        &repo_root,
        round,
        &round_rel,
        session_id.as_deref(),
        &store.round_dir(round).join("agent.log"),
        move |exit| {
            let store = match Store::init(&root3) {
                Ok(s) => s,
                Err(_) => return,
            };
            if exit.session_id.is_some() {
                let _ = store.set_agent_session_id(exit.session_id.clone());
            }
            // Grace period: done.json may land moments after process exit.
            std::thread::sleep(std::time::Duration::from_secs(2));
            sync_round(&app3, &root3, exit.round);
            if let Ok(st) = store.load_state() {
                let unfinished = st
                    .rounds
                    .iter()
                    .any(|r| r.number == exit.round && r.status == RoundStatus::InProgress);
                if unfinished {
                    let _ = store.mark_round(exit.round, RoundStatus::Error);
                    emit_status(
                        &app3,
                        AgentStatus::Error,
                        Some(
                            "Agent finished without signaling completion · check agent.log"
                                .to_string(),
                        ),
                    );
                }
            }
        },
    )?;

    *state.runtime.lock().unwrap() = Some(RoundRuntime {
        round,
        agent: Some(running),
        _watcher: w,
    });
    emit_status(&app, AgentStatus::Working, None);
    Ok(meta)
}

#[tauri::command]
async fn cancel_round(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    let repo_root = with_project(&state, |ctx| Ok(ctx.repo_root.clone()))?;
    let runtime = state.runtime.lock().unwrap().take();
    if let Some(rt) = runtime {
        if let Some(agent) = &rt.agent {
            agent.kill();
        }
        let store = Store::init(&repo_root)?;
        let st = store.load_state()?;
        let in_progress = st
            .rounds
            .iter()
            .any(|r| r.number == rt.round && r.status == RoundStatus::InProgress);
        if in_progress {
            store.mark_round(rt.round, RoundStatus::Cancelled)?;
        }
    }
    emit_status(&app, AgentStatus::Idle, None);
    Ok(())
}

/// In-diff hunk editing (worktree scope only): replace `old` lines at
/// `start_line` with `new` lines. Fails if the file changed underneath.
#[tauri::command]
async fn edit_lines(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    start_line: u32,
    old: Vec<String>,
    new: Vec<String>,
) -> Result<()> {
    if !apply::is_safe_repo_path(&path) {
        return Err(Error::Other(format!("refusing to edit path: {path}")));
    }
    let repo_root = with_project(&state, |ctx| Ok(ctx.repo_root.clone()))?;
    apply::apply_suggestion(
        &repo_root,
        &model::Suggestion {
            path,
            start_line,
            old,
            new,
        },
    )?;
    let _ = app.emit(EV_DIFF_INVALIDATED, ());
    Ok(())
}

/// Keep the View-menu item's label pointing at the *other* theme.
#[tauri::command]
async fn set_theme(item: State<'_, ThemeMenuItem>, theme: String) -> Result<()> {
    let label = if theme == "light" {
        "Switch to Dark Mode"
    } else {
        "Switch to Light Mode"
    };
    item.0
        .set_text(label)
        .map_err(|e| Error::Other(e.to_string()))
}

#[tauri::command]
async fn apply_suggestion(
    app: AppHandle,
    state: State<'_, AppState>,
    thread_id: String,
    comment_id: String,
) -> Result<()> {
    let repo_root = with_project(&state, |ctx| Ok(ctx.repo_root.clone()))?;
    let store = Store::init(&repo_root)?;
    let session = store.load_session()?;
    let suggestion = session
        .threads
        .iter()
        .find(|t| t.id == thread_id)
        .and_then(|t| t.comments.iter().find(|c| c.id == comment_id))
        .and_then(|c| c.suggestion.clone())
        .or_else(|| {
            // Overall-thread suggestions.
            session
                .overall
                .iter()
                .find(|c| c.id == comment_id)
                .and_then(|c| c.suggestion.clone())
        })
        .ok_or_else(|| Error::Other("no suggestion on that comment".into()))?;

    apply::apply_suggestion(&repo_root, &suggestion)?;
    store.mark_suggestion_applied(&thread_id, &comment_id)?;
    let _ = app.emit(EV_DIFF_INVALIDATED, ());
    Ok(())
}

// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .manage(AppState::default())
        .setup(|app| {
            let handle = app.handle();
            let menu = Menu::default(handle)?;
            let toggle = MenuItem::with_id(
                handle,
                MENU_ID_TOGGLE_THEME,
                // Placeholder; the frontend calls set_theme on boot with the
                // stored theme, correcting this before the menu is ever opened.
                "Switch to Light Mode",
                true,
                None::<&str>,
            )?;

            // The default macOS menu already has a View submenu — append there.
            let mut appended = false;
            for entry in menu.items()? {
                if let Some(sub) = entry.as_submenu() {
                    if sub.text().map(|t| t == "View").unwrap_or(false) {
                        sub.append(&PredefinedMenuItem::separator(handle)?)?;
                        sub.append(&toggle)?;
                        appended = true;
                        break;
                    }
                }
            }
            if !appended {
                let view = Submenu::with_items(handle, "View", true, &[&toggle])?;
                menu.append(&view)?;
            }
            app.set_menu(menu)?;
            app.manage(ThemeMenuItem(toggle));
            Ok(())
        })
        .on_menu_event(|app, event| {
            if event.id().as_ref() == MENU_ID_TOGGLE_THEME {
                let _ = app.emit(EV_TOGGLE_THEME, ());
            }
        })
        .invoke_handler(tauri::generate_handler![
            open_project,
            close_project,
            list_commits,
            get_diff,
            get_file_diff,
            load_session,
            add_comment,
            discard_pending,
            set_thread_resolved,
            set_file_viewed,
            set_agent_kind,
            submit_round,
            cancel_round,
            apply_suggestion,
            edit_lines,
            set_theme,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
