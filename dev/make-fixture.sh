#!/bin/bash
# Create a throwaway git repo exercising every diff case Backseat must handle:
# multiple commits, a dirty worktree, a rename, an added and a deleted file,
# an untracked file, and a very long line (soft-wrap rule).
set -euo pipefail

dir="${1:?usage: make-fixture.sh <dir>}"
rm -rf "$dir"
mkdir -p "$dir"
cd "$dir"

git init -q -b main
git config user.email fixture@backseat.test
git config user.name "Backseat Fixture"

mkdir -p src/util docs

cat > src/main.rs <<'EOF'
fn main() {
    let items = load_items();
    let mut out = Vec::new();
    for item in items {
        out.push(process(item));
    }
    render(out);
}

fn process(item: Item) -> Output {
    Output::from(item)
}
EOF

cat > src/util/helpers.rs <<'EOF'
pub fn clamp(v: i64, lo: i64, hi: i64) -> i64 {
    v.max(lo).min(hi)
}
EOF

cat > docs/notes.md <<'EOF'
# Notes
Initial documentation.
EOF

git add -A
git commit -qm "init: main scaffold"

cat >> src/main.rs <<'EOF'

fn render(out: Vec<Output>) {
    for o in out {
        println!("{o:?}");
    }
}
EOF
git add -A
git commit -qm "add render loop"

git mv src/util/helpers.rs src/util/math.rs
echo "pub fn double(v: i64) -> i64 { v * 2 }" >> src/util/math.rs
git add -A
git commit -qm "rename helpers to math"

# Dirty worktree on top:
#  - modify main.rs (with a long line for the wrap rule)
python3 - <<'EOF'
content = open("src/main.rs").read()
content = content.replace(
    "    let mut out = Vec::new();",
    "    let mut out = Vec::with_capacity(items.len()); // preallocate because the batch length is known ahead of time and reallocation in this hot loop showed up in profiles during the migration to the new subscription pipeline",
)
open("src/main.rs", "w").write(content)
EOF
#  - delete a tracked file
git rm -q docs/notes.md
#  - untracked new file
cat > src/config.rs <<'EOF'
pub struct Config {
    pub verbose: bool,
}
EOF

echo "fixture ready at $dir"
