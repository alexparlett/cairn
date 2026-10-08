# Patch mechanics spike — inverting a diff for unstage and discard

Date: 2026-10-07. Commissioned by: staging-and-commit planning (brainstorm section 17,
asked for mechanism detail before lock). Method: the script below, run in fresh
repositories under the session scratchpad with a scratch `HOME`, `GIT_CONFIG_NOSYSTEM=1`,
against git 2.56.0 (system) and 2.30.9 (the floor build in `~/.cache/cairn/git-floor`).
The two transcripts are identical apart from git's own warning wording, so only 2.56.0's
is reproduced; the diff between them is shown at the end. No GPG, no network, nothing
outside the scratchpad was touched.

Code facts this spike was planned against (read at `36c0d5c`):

- `cairn_model::Selection` (`crates/cairn-model/src/line_selection.rs`) is two sets:
  removed lines by old number, added lines by new number.
- `cairn_model::emit_patch` (`crates/cairn-model/src/patch.rs`) emits a forward patch at
  three lines of context: an unselected removal becomes context, an unselected addition is
  dropped; a partial deletion becomes a modification; `new file mode` is written for an
  `Added` file even when the selection is partial; paths are written as raw bytes
  (`write_headers`, `write_path_line`); `old mode`/`new mode` are written whenever the
  mode changed, whatever is selected.
- `cairn_model::apply_patch` and `apply_patch_in_reverse` (`patch_apply.rs`) are the
  reference applier.
- The working-tree query's unstaged answer carries the index blob as `old_id` and the
  null id as `new_id` (`crates/cairn-git/src/reads/working_tree.rs` test at the
  `assert_eq!(file.new_id ... NULL)` line); the staged answer carries HEAD's blob and the
  index blob.

## Findings

- **E1 — unstaging lines by inverting the diff.** The staged diff (HEAD → index) with old
  and new swapped, and the selection's sets swapped, emitted by the existing forward rule
  and applied with `git apply --cached` (no `-R`), unstaged exactly the selected change and
  left the working tree alone.
- **E1b — the mirrored rule with `-R` gives the same index.** A patch emitted against
  HEAD → index with the opposite rule (unselected additions as context, unselected
  removals dropped) and applied with `git apply --cached -R` produced the identical index.
  Two independent derivations agreeing is the oracle the packet's tests can use.
- **E2 — discarding lines in a CRLF working tree.** Under `core.autocrlf=true`, the
  inverted unstaged diff (worktree → index), emitted in git's form (LF), applied with
  `git apply` to the working tree removed exactly the selected line and kept every other
  line's CRLF on disk. (E2b: a CRLF-form patch also passed `--check` here, under
  autocrlf; `git-write-verbs.md` records CRLF patches refused without it — git's form is
  the one that works in both.)
- **E3 — staging part of an untracked file.** A `new file mode` patch carrying only the
  selected lines, applied with `git apply --cached`, created the index entry with exactly
  those lines; status reports `AM` and `git diff` shows the unselected line as the
  remaining unstaged change.
- **E3b — after `git add -N`.** A modification patch (`@@ -0,0 +1,2 @@`, no `new file
  mode`) applied against the intent-to-add entry. Whether a `new file mode` patch also
  applies against an intent-to-add entry was not tested.
- **E4 — discarding part of an untracked file.** The inverted diff is a deletion; a partial
  deletion is emitted as a modification (`emit_patch`'s existing rule), and `git apply` on
  the working tree removed the selected line; the file stayed untracked.
- **E5 — path quoting.** git C-quotes a path containing `\`, `"`, a tab or (under
  `core.quotePath`) a byte above 0x7f, and leaves a space unquoted. `git apply --cached`
  accepted raw bytes for a space, a quote, a backslash and `ü`, and **refused a raw tab**;
  git's quoted form applied for all five.
- **E6 / E6b — a stale patch applies at an offset, silently.** With the index changed
  after the selection was made, `git apply --cached` reported `Hunk #1 succeeded at 4
  (offset 2 lines)` / `succeeded at 5 (offset 3 lines)` and `Applied patch f cleanly`
  (exit 0). git does not refuse a patch whose preimage moved; only an id check does.

## The script

```bash
#!/usr/bin/env bash
# Patch-mechanism experiments for staging-and-commit planning, section 17.
# Usage: run.sh <git-binary>
set -u
GIT="$1"
ROOT="$(cd "$(dirname "$0")" && pwd)/work-$("$GIT" --version | awk '{print $3}')"
rm -rf "$ROOT"; mkdir -p "$ROOT/home"
export HOME="$ROOT/home" GIT_CONFIG_NOSYSTEM=1 GIT_TERMINAL_PROMPT=0 GIT_EDITOR=false
export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@t
g() { "$GIT" "$@"; }
say() { printf '\n### %s\n' "$*"; }
new_repo() { rm -rf "$ROOT/r"; mkdir "$ROOT/r"; cd "$ROOT/r"; g init -q; }

echo "git: $(g --version)"

say "E1 unstage one of two staged lines: inverted diff, forward patch, apply --cached (no -R)"
new_repo
printf 'a\nb\nc\nd\ne\n' > f; g add f; g commit -qm base
printf 'a\nB1\nc\nd\nE1\n' > f; g add f
# staged diff HEAD->index: -b +B1 ... -e +E1. Unstage the E1 change only.
# Inverted: old=index, new=HEAD. Selecting the inverse of change 2: -E1 +e.
cat > p1 <<'EOF'
diff --git a/f b/f
--- a/f
+++ b/f
@@ -2,4 +2,4 @@
 B1
 c
 d
-E1
+e
EOF
g apply --cached --check p1 && g apply --cached p1; echo "exit=$?"
echo "index now:"; g show :f | tr '\n' ' '; echo
echo "worktree unchanged:"; tr '\n' ' ' < f; echo

say "E1b same intent via mirrored rule + -R (patch against HEAD->index, unselected additions as context)"
new_repo
printf 'a\nb\nc\nd\ne\n' > f; g add f; g commit -qm base
printf 'a\nB1\nc\nd\nE1\n' > f; g add f
# mirrored: unselected removed (-b) dropped, unselected added (+B1) becomes context
cat > p1b <<'EOF'
diff --git a/f b/f
--- a/f
+++ b/f
@@ -2,4 +2,4 @@
 B1
 c
 d
-e
+E1
EOF
g apply --cached -R --check p1b && g apply --cached -R p1b; echo "exit=$?"
echo "index now:"; g show :f | tr '\n' ' '; echo

say "E2 discard one unstaged line in a CRLF (autocrlf=true) worktree file: inverted forward patch, worktree apply"
new_repo
g config core.autocrlf true
printf 'a\r\nb\r\nc\r\n' > f; g add f 2>/dev/null; g commit -qm base
printf 'a\r\nX\r\nb\r\nc\r\nY\r\n' > f
echo "git diff (git's form):"; g diff f | cat -A | sed -n '5,20p'
# discard +X only: inverted diff old=worktree new=index; forward patch drops line X.
cat > p2 <<'EOF'
diff --git a/f b/f
--- a/f
+++ b/f
@@ -1,4 +1,3 @@
 a
-X
 b
 c
EOF
g apply --check p2 && g apply p2; echo "exit=$?"
echo "worktree bytes now:"; cat -A f | tr '\n' ' '; echo
echo "status:"; g status --porcelain=v2 | cut -c1-12

say "E2b same with a CRLF patch (the form a raw read of the file would give)"
new_repo
g config core.autocrlf true
printf 'a\r\nb\r\nc\r\n' > f; g add f 2>/dev/null; g commit -qm base
printf 'a\r\nX\r\nb\r\nc\r\n' > f
printf 'diff --git a/f b/f\n--- a/f\n+++ b/f\n@@ -1,4 +1,3 @@\n a\r\n-X\r\n b\r\n c\r\n' > p2b
g apply --check p2b; echo "exit=$?"

say "E3 stage two of three lines of an UNTRACKED file: new-file patch with the selected lines only"
new_repo
printf 'base\n' > keep; g add keep; g commit -qm base
printf 'one\ntwo\nthree\n' > u
cat > p3 <<'EOF'
diff --git a/u b/u
new file mode 100644
--- /dev/null
+++ b/u
@@ -0,0 +1,2 @@
+one
+three
EOF
g apply --cached --check p3 && g apply --cached p3; echo "exit=$?"
echo "index :u ="; g show :u | tr '\n' ' '; echo
echo "status v2:"; g status --porcelain=v2 -uall | cut -c1-8
echo "git diff (unstaged remainder):"; g diff u | sed -n '5,20p'

say "E3b the same with intent-to-add first (git add -N), then a modification patch"
new_repo
printf 'base\n' > keep; g add keep; g commit -qm base
printf 'one\ntwo\nthree\n' > u
g add -N u
echo "git diff after -N:"; g diff u | sed -n '1,12p'
cat > p3b <<'EOF'
diff --git a/u b/u
--- a/u
+++ b/u
@@ -0,0 +1,2 @@
+one
+three
EOF
g apply --cached --check p3b && g apply --cached p3b; echo "exit=$?"
echo "index :u ="; g show :u 2>&1 | tr '\n' ' '; echo

say "E4 discard one line of an UNTRACKED file: modification patch applied to the worktree"
new_repo
printf 'base\n' > keep; g add keep; g commit -qm base
printf 'one\ntwo\nthree\n' > u
cat > p4 <<'EOF'
diff --git a/u b/u
--- a/u
+++ b/u
@@ -1,3 +1,2 @@
 one
-two
 three
EOF
g apply --check p4 && g apply p4; echo "exit=$?"
echo "worktree u ="; tr '\n' ' ' < u; echo; echo "still untracked:"; g status --porcelain=v2 -uall | cut -c1-30

say "E5 path quoting: raw bytes vs git's C-quoted form"
new_repo
for name in 'sp ace' $'ta\tb' 'quo"te' 'back\slash' 'ümlaut'; do
  printf 'x\n' > "$name"; done
g add .; g commit -qm base
for name in 'sp ace' $'ta\tb' 'quo"te' 'back\slash' 'ümlaut'; do
  printf 'x\ny\n' > "$name"; done
echo "what git diff writes for the headers:"; g -c core.quotePath=true diff | grep '^diff --git'
for name in 'sp ace' $'ta\tb' 'quo"te' 'back\slash' 'ümlaut'; do
  printf 'diff --git a/%s b/%s\n--- a/%s\n+++ b/%s\n@@ -1 +1,2 @@\n x\n+y\n' "$name" "$name" "$name" "$name" > praw
  printf '%-12q raw: ' "$name"; g apply --cached --check praw >/dev/null 2>&1 && echo ok || echo REFUSED
done
echo "git's own (quoted) patch, applied whole:"; g diff > pq; g apply --cached --check pq && echo ok

say "E6 offset hazard: index changed under a selection made earlier, repetitive content"
new_repo
printf 'x\nx\nx\nend\nx\nx\nx\nend\n' > f; g add f; g commit -qm base
# user selected 'add Z after the first end' from a diff of an index that equals HEAD
printf 'x\nx\nx\nend\nZ\nx\nx\nx\nend\n' > f
cat > p6 <<'EOF'
diff --git a/f b/f
--- a/f
+++ b/f
@@ -2,6 +2,7 @@
 x
 x
 end
+Z
 x
 x
 x
EOF
# meanwhile another tool rewrote the index: two lines inserted at the top
printf 'n\nn\nx\nx\nx\nend\nx\nx\nx\nend\n' | g hash-object -w --stdin >/dev/null
printf 'n\nn\nx\nx\nx\nend\nx\nx\nx\nend\n' > tmp; g update-index --cacheinfo 100644,$(g hash-object -w tmp),f; rm tmp
g apply --cached -v p6 2>&1 | grep -i -E 'offset|applied|error|Checking'; echo "exit=${PIPESTATUS[0]}"
echo "index now:"; g show :f | tr '\n' ' '; echo
```

## Transcript, git 2.56.0 (warnings about LF→CRLF removed)

```
git: git version 2.56.0

### E1 unstage one of two staged lines: inverted diff, forward patch, apply --cached (no -R)
exit=0
index now:
a B1 c d e 
worktree unchanged:
a B1 c d E1 

### E1b same intent via mirrored rule + -R (patch against HEAD->index, unselected additions as context)
exit=0
index now:
a B1 c d e 

### E2 discard one unstaged line in a CRLF (autocrlf=true) worktree file: inverted forward patch, worktree apply
git diff (git's form):
@@ -1,3 +1,5 @@$
 a$
+X$
 b$
 c$
+Y$
exit=0
worktree bytes now:
a^M$ b^M$ c^M$ Y^M$ 
status:
1 .M N... 10
? p2

### E2b same with a CRLF patch (the form a raw read of the file would give)
exit=0

### E3 stage two of three lines of an UNTRACKED file: new-file patch with the selected lines only
exit=0
index :u =
one three 
status v2:
1 AM N..
? p3
git diff (unstaged remainder):
@@ -1,2 +1,3 @@
 one
+two
 three

### E3b the same with intent-to-add first (git add -N), then a modification patch
git diff after -N:
diff --git a/u b/u
new file mode 100644
index 0000000..4cb29ea
--- /dev/null
+++ b/u
@@ -0,0 +1,3 @@
+one
+two
+three
exit=0
index :u =
one three 

### E4 discard one line of an UNTRACKED file: modification patch applied to the worktree
exit=0
worktree u =
one three 
still untracked:
? p4
? u

### E5 path quoting: raw bytes vs git's C-quoted form
what git diff writes for the headers:
diff --git "a/back\\slash" "b/back\\slash"
diff --git "a/quo\"te" "b/quo\"te"
diff --git a/sp ace b/sp ace
diff --git "a/ta\tb" "b/ta\tb"
diff --git "a/\303\274mlaut" "b/\303\274mlaut"
sp\ ace      raw: ok
$'ta\tb'     raw: REFUSED
quo\"te      raw: ok
back\\slash  raw: ok
ümlaut      raw: ok
git's own (quoted) patch, applied whole:
ok

### E6 offset hazard: index changed under a selection made earlier, repetitive content
Checking patch f...
Hunk #1 succeeded at 4 (offset 2 lines).
Applied patch f cleanly.
exit=0
index now:
n n x x x end Z x x x end 
```

## E6b script and transcript (2.56.0; 2.30.9 identical)

```bash
set -u; GIT="$1"; R="$(pwd)/e6b-$("$GIT" --version|awk '{print $3}')"; rm -rf "$R"; mkdir -p "$R/h"; cd "$R"
export HOME="$R/h" GIT_CONFIG_NOSYSTEM=1 GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@t
"$GIT" init -q .
printf 'mod m;\nfn a() {\n  x;\n}\nfn b() {\n  x;\n}\nend\n' > f; "$GIT" add f; "$GIT" commit -qm base
# user selects "+  y;" inside fn a, from a diff where index == HEAD
cat > p <<'P'
diff --git a/f b/f
--- a/f
+++ b/f
@@ -2,3 +2,4 @@
 fn a() {
   x;
+  y;
 }
P
# another tool renames fn a in the index before the queued stage runs
printf 'mod m;\nfn b() {\n  x;\n}\nfn a() {\n  x;\n}\nend\n' > t; "$GIT" update-index --cacheinfo 100644,$("$GIT" hash-object -w t),f
"$GIT" apply --cached -v p 2>&1 | grep -E 'offset|Applied|error'; echo "index now:"; "$GIT" show :f
```

```
Hunk #1 succeeded at 5 (offset 3 lines).
Applied patch f cleanly.
index now:
mod m;
fn b() {
  x;
}
fn a() {
  x;
  y;
}
end
```

## Difference between the 2.56.0 and 2.30.9 transcripts (warnings removed)

```
1c1
< git: git version 2.56.0
---
> git: git version 2.30.9
```
