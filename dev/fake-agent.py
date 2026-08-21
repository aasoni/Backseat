#!/usr/bin/env python3
"""A stand-in coding agent implementing the Backseat skill contract exactly.

Run from a repo root (Backseat spawns it via BACKSEAT_AGENT_CMD). It reads the
current round from .backseat/, "addresses" each unresolved thread by editing the
anchored line, writes one reply file per thread plus an overall reply, performs
the fixup/autosquash rebase for commit-scoped rounds, and writes done.json.

Modes via FAKE_AGENT_MODE:
  worktree (default) — normal behavior
  blocked            — writes done.json with status "blocked"
  no-done            — writes replies but never the done signal (error path)
  slow               — sleeps between replies (exercises live UI updates)
"""
import json
import os
import pathlib
import subprocess
import sys
import time

ROOT = pathlib.Path.cwd()
BS = ROOT / ".backseat"
MODE = os.environ.get("FAKE_AGENT_MODE", "worktree")
NOW = "2026-01-01T00:00:00Z"

state = json.loads((BS / "state.json").read_text())
round_dir = BS / state["current_round_path"]
review = json.loads((round_dir / "review.json").read_text())
replies_dir = round_dir / "replies"
replies_dir.mkdir(exist_ok=True)

seq = 0


def reply(target, body, **extra):
    global seq
    seq += 1
    obj = {"target": target, "author": "Fake Agent", "at": NOW, "body": body}
    obj.update({k: v for k, v in extra.items() if v is not None})
    (replies_dir / f"{seq:03d}-{target}.json").write_text(json.dumps(obj, indent=2))
    if MODE == "slow":
        time.sleep(1)


def git(*args, **kw):
    return subprocess.run(["git", *args], check=True, capture_output=True, text=True, **kw)


scope = review["scope"]

if MODE == "blocked":
    (round_dir / "done.json").write_text(json.dumps({
        "round": review["round"],
        "status": "blocked",
        "summary": "Fake agent was asked to simulate being blocked.",
        "at": NOW,
    }, indent=2))
    sys.exit(0)

# Address each unresolved thread: append a marker to the first anchored line.
first = True
edited_paths = []
for th in review["threads"]:
    if th["status"] != "unresolved":
        continue
    a = th["anchor"]
    path = ROOT / a["path"]
    edited = False
    if path.exists() and a["snapshot"]:
        lines = path.read_text().split("\n")
        needle = a["snapshot"][0]
        if needle.strip() and needle in lines:
            i = lines.index(needle)
            lines[i] = lines[i] + "  // addressed by fake agent"
            path.write_text("\n".join(lines))
            edited = True
            edited_paths.append(a["path"])
    extra = {}
    if first and edited:
        # One reply carries a proposed change (Apply-button path) that is NOT
        # applied by the agent: propose tweaking the line just below the anchor.
        first = False
        lines = path.read_text().split("\n")
        i = lines.index(a["snapshot"][0] + "  // addressed by fake agent")
        if i + 1 < len(lines) and lines[i + 1].strip():
            extra["suggestion"] = {
                "path": a["path"],
                "start_line": i + 2,
                "old": [lines[i + 1]],
                "new": [lines[i + 1] + "  // fake suggestion"],
            }
    reply(
        th["id"],
        f"Addressed the feedback on {a['path']}:{a['start_line']}."
        + ("" if edited else " (anchor not found; left as is)"),
        marks_resolved=edited,
        refs=[f"{a['path']}:{a['start_line']}"],
        **extra,
    )

if review.get("overall"):
    reply("overall", "Overall feedback acknowledged; see the per-thread replies.",
          refs=[f"{t['anchor']['path']}:{t['anchor']['start_line']}" for t in review["threads"][:2]])

commit_map = None
if scope["type"] == "commit":
    sha = scope["sha"]
    subject = git("show", "-s", "--format=%s", sha).stdout.strip()
    # Stage exactly what we changed — never a blanket add.
    git("add", "--", *edited_paths)
    git("commit", "--fixup", sha)
    env = dict(os.environ, GIT_SEQUENCE_EDITOR="true")
    # -i (with an auto-accepting editor) is required for --autosquash on git < 2.44.
    subprocess.run(
        ["git", "rebase", "-i", "--autosquash", "--autostash", sha + "^"],
        check=True, env=env, capture_output=True, text=True,
    )
    # Find the rewritten commit by matching the original subject.
    log = git("log", "--format=%H%x00%s").stdout
    new_sha = None
    for rec in log.strip().split("\n"):
        h, _, s = rec.partition("\x00")
        if s == subject:
            new_sha = h
            break
    commit_map = {sha: new_sha} if new_sha else None

if MODE != "no-done":
    done = {
        "round": review["round"],
        "status": "completed",
        "summary": f"Fake agent addressed {len(review['threads'])} thread(s).",
        "at": NOW,
    }
    if commit_map:
        done["commit_map"] = commit_map
    (round_dir / "done.json").write_text(json.dumps(done, indent=2))
