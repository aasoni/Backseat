<h1>
<p align="center">
  <img width="280" alt="backseat-logo" src="https://github.com/user-attachments/assets/3df29996-0ad6-4d3a-b4c3-2eb6f4aacd79" />	
  <br>Backseat
</h1>
  <p align="center">
    A Desktop Application to review AI-generated code locally like a pull request.
    <br />
    Give inline or highlevel feedback your agent can act on.
</p>

## About

Backseat is a Desktop application built using [Tauri 2](https://v2.tauri.app/). It allows you to review local changes inside of a git repository
like a regular pull request.

These comments are then read by your agent (Claude Code, Codex etx.)
so that you can refine the quality of what it wrote before submitting
it for final human review.

## How it works
Backseat requires that the code is written inside a git repository.
It uses git to generate diffs and renders them like a typical
pull-request with old and new version side by side.

From this view you can add inline comments or give high level feedback
just like you would in an on-line pull request review (like Codeberg or 
GitHub).

This feedback is written to a local .backseat folder in your repo.
When starting up Backseat and pointing it to a local git repo, it
adds instructions for your agent on how to read feedback provided
via the app.

Review comments can be given on the current uncommitted changes
or on previous commit. The agent will get precise instructions on whether
the change needs to be amended into an old commit or in the current
working tree.

## Development

```sh
npm install
npm run tauri dev        # run the desktop app
npm run dev              # frontend only, in a browser, against a mock backend
cd src-tauri && cargo test   # backend + protocol tests (uses dev/fake-agent.py)
```

The `.backseat` on-disk protocol, the agent skill contract, and the module
layout are documented in `CLAUDE.md`.

## Support
Currently works with Claude Code only.
