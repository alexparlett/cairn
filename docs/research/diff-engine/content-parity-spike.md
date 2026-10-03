# Content parity spike — gix's line diff against git's, and what reproduces git

Evidence record. Commissioned 2026-10-03 by the diff-engine packet after the C6
adjudication found the content query's hunks differ from `git diff -U3`.
Historical: never retro-edited.

## Why it was run

The content query computed a file's changed ranges with gix-imara-diff 0.2.5
(`diff_with_slider_heuristics`). An adjudicator found 9 of 267 modified text files
in the last 150 non-merge Cairn commits differ from `git diff -U3` under default
config (myers on both sides), and on 393 seeded random files: myers 2, minimal 2,
histogram 7, patience 20. gix has no patience: `gix-0.87.1/src/config/tree/sections/diff.rs`
returns `Unimplemented` for it and lenient config falls back to histogram, so the
PRD's sanctioned fallback was itself a divergence. C6 was worded "crafted fixtures
with unambiguous edits", which excluded every case that differs. The user classes
any divergence from what git shows as a critical bug.

## Setup

git 2.56.0, Ryzen 7 9800X3D (16 threads), release builds, warm, medians of 5–9.
Detached worktree at `f45b161`, removed afterwards; nothing committed. Bench
`~/Development/bench/rust` at `c999cef531e`, read only. Reference per file:
porcelain `git diff -U3 --no-ext-diff --no-textconv [--diff-algorithm=X] old new --
path`, compared from the first `@@` to the end, each header compared up to its
closing `@@`. Candidate: git's ranges + Cairn's `split_lines` lines +
`emit_patch(.., Selection::with_every_change)` (which groups with `Hunks::of` at
context 3).

## 1. Correctness

`-U0` does not reproduce `-U3`: with context 0, `xdi_diff` runs
`trim_common_tail`, cutting the byte-identical tail before xdiff runs, which both
stops a change sliding into the tail (`f345.txt`, corpus B: `(351,0,351,3)` at
`-U0`, `(352,0,352,3)` at `-U1`/`-U3`) and changes Myers' line-count heuristics
(`cfe3315` `crates/cairn-guards/tests/invariants.rs`: one change at `-U0`, two at
`-U3`). With context ≥ 1 git produces the same script it uses for `-U3`. The
mechanism: run with `-U1`, take each change as a maximal run of `-`/`+` lines (the
headers alone are not enough).

Files differing from `git diff -U3` (headers and lines):

| Corpus | Algorithm | gix | diff-tree `-U0` | diff-tree `-U1` | diff-tree `-U3` |
| --- | --- | --- | --- | --- | --- |
| A: Cairn, 150 non-merge commits to `f45b161`, 262 M text files | myers | 9 | 2 | 0 | 0 |
| A | minimal | 13 | 1 | 0 | 0 |
| A | histogram | 1 | 0 | 0 | 0 |
| A | patience | 9 | 0 | 0 | 0 |
| B: seeded random, seed 20261003, 400 files | myers / minimal / histogram / patience | 19 / 24 / 2 / 67 | 1 / 1 / 1 / 1 | 0 / 0 / 0 / 0 | 0 |
| C: seeded random, seed 7, 1,000 files | myers / minimal / histogram / patience | 60 / 67 / 7 / 204 | 9 / 9 / 7 / 7 | 0 / 0 / 0 / 0 | — |
| Bench: F1, F3, F6, F7, F8, a 45k-line file, a small file | myers / minimal / histogram / patience | 3 / 3 / 0 / 3 | 1 / 1 / 0 / 0 | 0 / 0 / 0 / 0 | — |

`-U1` matched on all 6,676 file×algorithm comparisons. Corpus A counted 262
files, not the adjudicator's 267 (different ref or filter); gix's 9 reproduce. The
C14 subjects diverge today: under myers gix differs on F7 (973 git ranges vs 975)
and F1 (19,969 vs 23,048).

Line indexing (corpus D, 11 edge cases: final newline lost, gained or absent on
both sides; CRLF; CRLF↔LF mixes; a lone `\r` mid-line; last line `b\r` with no
newline; empty→content; content→empty; `\n`→`\n\n`): all matched. git splits only
on `\n`, keeps `\r` in the line, and emits `\ No newline` exactly where
`split_lines` puts an unterminated line, so git's ranges index the lines gix read.
The parser skips `\` lines.

## 2. Plumbing

No plumbing diffs two arbitrary blobs (`diff-tree -p A:a B:b` fails "unable to
read tree"; only porcelain `git diff blob blob` does). Renames:
`diff-tree -p -M HEAD^ HEAD -- old new`, both paths in the pathspec (the new path
alone gives "new file"). Copies of an unmodified source need
`--find-copies-harder` plus both paths. Pass the changes query's detection flags;
if the old path is also a modification the output holds two file patches — pick
with `--raw -z` records in the same call. One `diff-tree -p` per commit equals
per-file calls joined in raw order (`09bf6de39eb`, 46 files, byte-identical).

| Setting | Read by `diff-tree`? | Consequence |
| --- | --- | --- |
| `diff.algorithm` | No (0 of 400 changed; the flag changes 80) | Cairn passes it; porcelain with config X matched diff-tree `--diff-algorithm=X` on 400/400 for all four |
| `diff.indentHeuristic=false` | Yes (144 changed) | Pass nothing; git honours it (gix always applied the heuristic — inferred divergence) |
| `diff.<driver>.algorithm` | Yes (80/80) | A driver beats `diff.algorithm` config; `--diff-algorithm=X` beats the driver |
| `diff.suppressBlankEmpty=true` | Yes | Blank context loses its space (naive parser wrong on 285/400); pass `-c diff.suppressBlankEmpty=false` |
| `color.*`, `diff.noprefix`, `diff.interHunkContext`, `diff.context`, `diff.relative`, `diff.colorMoved`, `diff.wsErrorHighlight`, `diff.orderFile`, `core.quotePath` | No | — |
| `diff=<driver>` `xfuncname` | Only the text after `@@` | Ranges unaffected; `git diff` shows function context Cairn did not |

Rule: if the path's driver has an algorithm, pass no algorithm flag; otherwise
`--diff-algorithm=<diff.algorithm or myers>` (that spelling: `--minimal` alone
misbehaved with a driver set, 1 of 80 came out as patience). Unverified: how Cairn
resolves the driver vs git; `diff.<driver>.algorithm` on 2.30–2.39.

## 3. Writes and programs

Trap repository: textconv with `cachetextconv`, a `diff.<driver>.command`,
`diff.external`, clean/smudge filters writing markers; one stat-dirty and one
content-dirty file; snapshots of every `.git` file (size, mtime) and the refs.

| Command (`GIT_OPTIONAL_LOCKS=0`) | Writes `.git`? | Programs run |
| --- | --- | --- |
| `diff-tree -p -U1 --no-ext-diff --no-textconv -a` | no | none |
| `diff-tree -p -U1`, no flags | no | none |
| `diff-tree -p --textconv` (control) | yes (notes cache) | textconv, smudge |
| `diff-tree -p --ext-diff` (control) | no | driver command, smudge |
| `diff-files -p -U1 --no-ext-diff --no-textconv -a` | no | clean filter |
| `diff-index -p -U1 --cached` | no | none |
| `diff-index -p -U1 HEAD` (worktree side) | no | clean filter |
| porcelain `git diff -U1` (control) | yes (index refresh despite `GIT_OPTIONAL_LOCKS=0`) | clean, driver command, smudge |

## 4. Cost

"git total" = `diff-tree -U1` spawn, read and parse plus the gix blob reads and
`split_lines` Cairn keeps either way; "gix total" = read and split plus
`diff_with_slider_heuristics`.

| Subject | git process | git total | gix total | Bar |
| --- | --- | --- | --- | --- |
| `996bad35ddc` `rustc_parse/src/parser/expr.rs` (~4.5k lines, +6 −10) | 3.80 ms | 4.10 ms | 0.63 ms | 100 ms |
| `35e9bf6555e` `aarch64/sve/generated.rs` (44,982 lines, +22 −22) | 8.59 ms | 11.31 ms | 4.67 ms | 100 ms |
| F7 `3b09522c34b`, myers | 12.21 ms | 15.31 ms | 7.05 ms | 100 ms, met |
| F7, histogram | 12.39 ms | 15.32 ms | 9.14 ms | met |
| F1 `6a6e8446b97` load anyway, myers | 168.4 ms | 174.8 ms | 114.5 ms | — (git's own 163.0 ms) |
| F1, histogram | 25.2 ms | 32.2 ms | 14.5 ms | — |

Per-process floor ≈ 2.5 ms on the bench repo, 1.07 ms on Cairn's. Estimated whole
content query on F7 through Cairn ≈ 16–17 ms (not measured through the real
runner). Expand All: `09bf6de39eb` (46 files) one call 14.6 ms vs per-file 119.2 ms;
S7 `f0845adb0c1` (1,017 files) one call 31.7 ms vs per-file 2,577 ms.

## 5. Ignore whitespace

| Corpus | Cairn ranges ≠ git `-w` | Cairn headers ≠ `git diff -w` | git `-w` + `Hunks::of` headers ≠ `git diff -w` |
| --- | --- | --- | --- |
| A (262), myers / histogram | 13 / 4 | 2 / 2 | 0 / 0 |
| B (400), myers / histogram | 100 / 84 | 82 / 81 | 0 / 0 |
| D (11) | 0 | 0 | 0 |

Mismatches are one-line slider shifts, likely because git's indent heuristic reads
the original indentation while Cairn's whitespace-stripped keys have none
(inferred). Body text under `-w` (which side's context git prints) not checked.

## 6. Intra-line

No git equivalent: `git diff` offers `--word-diff`, `--color-words`,
`--color-moved`, `--ws-error-highlight`, other presentations; `diff-highlight` is
a contrib script. Stays Cairn's, on gix.

## Recommendation as reported

Move the content query's ranges and the ignore-whitespace ranges to
`git -c diff.suppressBlankEmpty=false diff-tree -p -U1 -a --no-ext-diff
--no-textconv --no-color --literal-pathspecs [--diff-algorithm=<X>] [detection
flags] <old> <new> -- <path> [<old path>]` as a read (`GIT_OPTIONAL_LOCKS=0`,
`GIT_NO_LAZY_FETCH=1`), parsed as runs; one call per commit for Expand All; skip
git for added/deleted/mode-only/binary/LFS/submodule/too-large files. A stale-read
guard: check every emitted line against the lines Cairn holds. Phase 03:
`diff-files -p -U1`, `diff-index -p -U1 [--cached] HEAD`; stat-dirty files emit no
patch; untracked files need no git call; the held lines must be git's
clean-filtered, EOL-converted form. Confidence high on 2.56; medium on older gits.

Unverified: driver detection parity; 2.30–2.39 behaviour of the driver algorithm
and indent heuristic; cost through the real runner; `-B`; `-w` context text;
skipping git for added/deleted files in every attribute setup.

## Decided from it (2026-10-03, by the user)

The content query's changed ranges, and the ignore-whitespace ranges, come from
`git diff-tree -p -U1` as a read, parsed as runs and regrouped by `Hunks::of`;
Expand All asks once per commit. gix keeps blob reads, the size, binary and LFS
pre-checks, intra-line highlights and the patch emitter. Amends packet decision L3
and PRD R2.4/R2.8. And hunk header rows show git's function context — the text
after the closing `@@` — taken from git's own header.
