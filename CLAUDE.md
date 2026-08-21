# Backseat — dev notes

Tauri 2 desktop app for reviewing AI-generated code locally like a PR. React 19 + TS + Vite
renderer, Rust backend. The design spec lives in `backseat design files/README.md` (pixel-accurate;
Nocturne tokens) — treat it as the UI source of truth.

## Commands

- `npm run tauri dev` — run the app (builds Rust + starts Vite).
- `npm run dev` — frontend only in a browser, backed by the **mock backend** (`src/ipc/mock.ts`,
  auto-selected when not inside Tauri). Use this for UI work.
- `npm run build` — typecheck + bundle frontend.
- `cd src-tauri && cargo test` — all Rust tests, incl. protocol round-trips driven by the fake agent.
- `cd src-tauri && cargo run --example real_e2e` — one full round against the real `claude` CLI in a
  temp fixture repo (spends a small headless session; manual verification only).

## Architecture

- `src-tauri/src/model.rs` — serde structs = the `.backseat/` protocol schema (snake_case JSON).
- `src-tauri/src/store.rs` — the heart: round submission (`review.json` snapshots), reply/done
  folding, snapshot-based re-anchoring. Single-writer rule: app writes `state.json` / `review.json`
  / `app/session.json`; the agent writes only `rounds/N/replies/*.json` and `rounds/N/done.json`.
- `src-tauri/src/skill.rs` — writes `.claude/skills/backseat/SKILL.md` (the agent contract, version
  marker `backseat-skill-version`) and appends `.git/info/exclude`. Bump `SKILL_VERSION` when the
  contract changes.
- `src-tauri/src/agent.rs` — spawns `claude -p … --output-format json --permission-mode acceptEdits`
  (resolved via login shell; `BACKSEAT_AGENT_CMD` env overrides for tests), captures `agent.log`,
  parses `session_id` for `--resume`.
- `src-tauri/src/git.rs` + `diffparse.rs` — git CLI wrapper (`-z` porcelain parsing) and unified-diff
  → side-by-side row model.
- `src/ipc/` — `Backend` interface with Tauri and mock implementations. `src/state/` — Zustand
  stores; shapes mirror the design README's state contract.
- `dev/fake-agent.py` — reference implementation of the skill contract (modes: worktree, blocked,
  no-done, slow, commit w/ fixup+autosquash rebase). Keep it in lockstep with SKILL.md.

## Conventions

- Review scopes: `{type:"worktree"}` or `{type:"commit", sha}` — commit rounds instruct the agent to
  amend + rebase (`git rebase -i --autosquash` with `GIT_SEQUENCE_EDITOR=true`; plain
  `--autosquash` without `-i` is a no-op on git < 2.44).
- UI: no `git` writes from the app except applying suggestions to the working tree. Never commit or
  push from app code.
- Styling: plain CSS in `src/styles/` using Nocturne variables + `--surface-*` tokens. No CSS-in-JS.
  Primary actions are accent outlines, never fills.
- Theming: dark (bare `:root`, the design's source of truth) and a derived light theme
  (`styles/theme-light.css`, activated via `data-theme="light"` on `<html>`; persisted to
  localStorage, defaults to the OS preference). Switched via the native **View > Switch to
  Light/Dark Mode** menu item — no in-window toggle button, deliberately (user preference). The
  menu item lives in Rust (`lib.rs` setup hook, id `toggle-theme`); it emits
  `backseat://toggle-theme`, and the frontend keeps its label current through the `set_theme`
  command. In the browser mock there is no menu — flip `localStorage['backseat.theme']` in
  devtools if needed. Never hardcode a hex in components — add a token to `surfaces.css` and
  override it in `theme-light.css`.
- Syntax highlighting: `src/highlight.ts` (highlight.js core, per-line, cached; language from file
  extension). Token colors are `--syn-*` variables in `styles/syntax.css` with both theme palettes —
  tokens set `color` only, never backgrounds, so diff row tints read through. Syntax palettes are
  deliberately VIVID (high chroma — purple/pink/green/orange/cyan/blue), a user-requested exception
  to Nocturne's low-chroma rule, which still governs all non-code UI. Baseline code text is
  neutral-200, not 300. Comments stay muted by design.
- Hunk editing (worktree scope only): each hunk header shows an Edit button on hover
  (hidden while the agent is working); `HunkEditor.tsx` is an overlay editor — transparent-text
  textarea over a highlighted `<pre>` mirror — seeded with the hunk's right-side lines. Save goes
  through the `edit_lines` command → `apply::apply_suggestion` (exact-match splice, drift-tolerant,
  atomic write, path-guarded by `apply::is_safe_repo_path`) → `diff-invalidated`. Commit scope
  never offers editing.
