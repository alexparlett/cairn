# Rename parity spike — gix against git, and what closes the gap

Evidence record, saved in full. Commissioned 2026-09-30 by the diff-engine packet
after phase 02's stopping rule fired on C14's rename clause. Historical: never
retro-edited.

## Why it was run

Phase 02 found that on rust-lang/rust `5a3292f163d` gix pairs 231 renames where
git pairs 2,774. Two causes were verified in the linked source before the spike:

- `gix-diff-0.67.1/src/lib.rs:33` documents `Rewrites::limit` as "only 1000*1000
  combinations can be tested", but `src/rewrites/tracker.rs:395-396` computes
  `let permutations = num_src * num_dst;` and skips the fuzzy stage when
  `permutations > self.rewrites.limit` — unsquared. The same file squares it at
  `tracker.rs:431` (`self.rewrites.limit.saturating_pow(2)`) for `max_checks`.
  So the default 1000 behaves like git's `diff.renameLimit=31`.
- gix has no basename stage for inexact renames. git's `diffcore-rename` pairs by
  file name before the exhaustive stage, ungoverned by the limit.
- Upstream: gitoxide `main` (checked 2026-09-30) still compares the unsquared
  product. The filename check that exists there is a tie-break inside exact
  (identity) matching, not git's inexact basename stage. `gix` 0.88.0 /
  `gix-diff` 0.68.0 were published; the pinned versions are 0.87.1 / 0.67.1.

The user classed a wrong rename as a critical bug, which ruled out "accept and
file". Candidates measured: **D** — when gix reports its limit cut detection
short, take the rename pairing from `git diff-tree -M`; **B** — pass gix
`limit = diff.renameLimit²`.

## Setup

Code: `origin/feature/diff-engine` at `2c8ae1e`, in a detached scratch worktree
(removed). Nothing committed. Bench: `~/Development/bench/rust` at `c999cef531e`,
read only. AMD Ryzen 7 9800X3D, `git version 2.55.0`. Cargo `release` profile.
Warm, median of 7 (D) or 6 (B) in-process after a warm-up. The git spawn is timed
from `Command` start through reading all output, then parsing, with an environment
of `LC_ALL=C GIT_OPTIONAL_LOCKS=0 GIT_TERMINAL_PROMPT=0 PATH HOME`. Harness:
throwaway — a spike-only environment variable in `changes.rs` overriding the limit
(`platform.options(|o| o.track_rewrites(Some(gix::diff::Rewrites{limit,..Default::default()})))`,
checked against the gix 0.87.1 source) and an ignored test file.

A correction to B's premise: when gix skips, `renames_skipped_for_limit` equals
gix's `num_src * num_dst` (`tracker.rs:393-403`). On M1 that is 6,946,095, so
**B with limit = 1000² still skips M1**. git's own post-basename matrix size was
not read from git's source and is unverified.

## 1. What D costs (gix query plus `git diff-tree -r -M -z --raw --no-abbrev`, spawned and parsed)

| Subject | Cut short? | gix query ms | git spawn→output ms | git + parse ms | D total when it fires | git R / gix R |
|---|---|---|---|---|---|---|
| M1 `5a3292f163d` | yes (6,946,095) | 2.29 | 82.7 | 83.3 | **85.6** | **2,774** / 231 |
| S1 `cf2dff2b1e3` | no | 20.04 | 32.8 | 37.5 | (does not fire) | 27,592 / 27,592 |
| S7 `f0845adb0c1` | no | 1.04 | 7.8 | 8.0 | (does not fire) | 0 / 0 |
| S3 `3fc7ab23731` | yes (23,600) | 6.14 | 12.1 | 13.2 | 19.4 | 6,551 / 6,482 |
| S5 `9be35f82c1a` | no | 3.01 | 8.9 | 9.5 | (does not fire) | 3,214 / 3,213 |
| S6 `9e5f7d5631b` | yes (13,348) | 1.46 | 7.4 | 7.8 | 9.2 | 1,593 / 1,582 |
| `7d52cbce6db` (2018 rollup) | yes (801,024) | 2.86 | 32.5 | 33.0 | 35.9 | 2,795 / 1,966 |
| `c798dffac9d` (S5's merge) | yes (542,145) | 2.94 | 31.5 | 32.1 | 35.1 | 3,277 / 2,594 |
| `f45f52532a3` (S3's merge) | yes (221,061) | 4.50 | 31.1 | 32.2 | 36.7 | 4,384 / 4,318 |
| `bb55bd449e6` | yes (104,500) | 0.77 | 27.5 | 27.7 | 28.5 | 219 / 0 |
| `2f351415e53` | yes (91,996) | 0.38 | 14.1 | 14.2 | 14.6 | 235 / 25 |

- git's pair count on M1 confirmed at 2,774 (231 exact), stderr empty.
- Parsing costs 0.1–0.6 ms, except 4.6 ms on S1's 27,592 records (5.5 MB).
- With `-C` (copies are not configured on the bench repo; information only) git
  itself gives up on M1: 231 R plus 6 C, and a warning to raise `diff.renameLimit`
  to at least 2807, in 7.2 ms. S3 with `-C` takes 131 ms.
- S6: gix's candidate count is 13,348 against git's 47×47 = 2,209 from the
  baseline, though everything left is `Cargo.toml` files. Cause not isolated.

## 2. How often D would fire

| Sample | Commits | Cut short | Rate |
|---|---|---|---|
| First-parent chain, most recent 20,000 (mostly bors merges) | 20,000 | 121 | **0.60%** |
| Whole first-parent chain | 45,261 | 143 | 0.32% |
| Most recent 20,000 non-merge commits | 20,000 | 7 | **0.035%** |

`num_src * num_dst` over the 121 recent first-parent commits that fired:

| min | p25 | p50 | p75 | p90 | p99 | max |
|---|---|---|---|---|---|---|
| 1,008 | 1,462 | 2,660 | 8,720 | 28,438 | 801,024 | 6,946,095 |

- By bucket: 94 at or below 1e4, 22 at or below 1e5, 4 at or below 1e6, 1 above
  1e6 (M1).
- 113 of the 121 are real gaps: git finds 51,440 renames against gix's 42,072.
- In the 7 non-merge commits that fired, git finds 301 renames against gix's 46.
- git's fallback on the 143 fired first-parent commits (one warm run each): p50
  9.3 ms, p90 19 ms, max 83 ms (M1). No rename-limit warning on any of the 150
  fired commits.
- The gix query alone: p50 0.19 ms, p99 2.7 ms on the recent 20,000.

## 3. What B costs (gix with limit 1,000,000, 60 s cap per run)

| Subject | gix ms (median, min–max) | similarity checks | gix R / git R | pairs matching git's |
|---|---|---|---|---|
| M1 | 8.9, still cut short | 0 | 231 / 2,774 | 231 common, 2,543 missing |
| M1 with **limit 0 (unlimited)** | **8,527** (one run) | 3,247,411 | 2,774 / 2,774 | 2,733 common, **41 pairs differ** |
| `7d52cbce6db` | **1,193** (990–1,352) | 344,035 | 2,795 / 2,795 | all pairs match; 1 score differs |
| `c798dffac9d` | 534 | 247,329 | 3,277 / 3,277 | 7 pairs differ |
| `f45f52532a3` | 161 | 63,170 | 4,384 / 4,384 | 16 differ |
| `bb55bd449e6` | 164 | 80,660 | 218 / 219 | 19 gix-only, 20 git-only |
| `2f351415e53` | 86 | 43,154 | 232 / 235 | 23 gix-only, 26 git-only |
| S6 | 4.8 | 1,983 | 1,593 / 1,593 | exact match |

Nothing hit the 60 s cap; the unlimited M1 run was not repeated. B's `limit` is a
single knob that also feeds the copy stage (unverified).

## 4. Does "gix for everything, git for pairing" merge cleanly?

Yes, on every commit where D would fire. Merge: gix's answer, with gix's own
renames split back into D and A, then git's R pairs applied; compared field by
field (status and score, both modes, both ids, both paths) with git's full
`--raw`. Exact match on M1, S3, S6, the five heavy subjects and all 150 fired
commits. No git R source was missing from gix's deletions, no destination from its
additions, no mode or id disagreed, and gix's exact pairs agreed with git's for the
same destination. `repair_copies` was not exercised (copies off).

## 5. gix also diverges when nothing is cut short

Rename and entry counts compared against a bulk `git log -M --raw` over both
20,000-commit samples:

| Sample | Commits disagreeing with git | Cut short (D catches) | **Not cut short (D misses)** |
|---|---|---|---|
| Recent 20,000 first-parent | 176 | 113 | **63 (0.32%)** |
| Recent 20,000 non-merge | 18 | 6 | **12 (0.06%)** |

A lower bound: equal counts with different pairs are not counted. Causes, where
gix ran a full search:

- **The similarity measure differs.** `175c8f578bc`: git pairs
  `incorrect-locations.rs` → `incorrect_locations.rs` at 77% and the `.stderr`
  files at 62%; gix ran 42 checks and paired neither. `656ccbe796f`: git scores
  56%, gix does not pair it. `7d52cbce6db` under B: gix 72% where git 86%.
- **Pairing picks a different match.** gix takes the first candidate over the
  threshold; git takes the best and prefers the same basename. `1b96797c085`: gix
  makes 8 pairs at 51–52%, git 6 at 75–76%, none shared. M1 unlimited: 41
  duplicate-content `auxiliary/` files paired to different sources.
- S5: git pairs one file at 96% (`lexer-crlf-…doc-comment.rs`) that gix leaves as
  a delete plus an add.

## Verdict as reported

- D's fallback is rare (0.6% of recent first-parent diffs, 0.035% of ordinary
  commits), costing 9 ms typically, 19 ms p90, 86 ms worst.
- B is not viable: at 1000² it still leaves M1 at 231; at the limit M1 needs it
  costs 8.5 s; where it finishes (0.09–1.2 s, 3–40× git) its pairs still differ.
- D merges cleanly but is not parity: `was_cut_short()` does not flag the 0.3% /
  0.06% where gix's similarity or pairing differs. D closes roughly 64% of
  disagreeing commits.

## Unverified

Cold-cache timings; copies enabled; Cairn's real `GitEnvironment` and runner
(approximated with `std::process::Command`); merges' non-first-parent sides beyond
the non-merge sample; any repository other than rust-lang/rust; git's source for
its exact limit arithmetic.

## What was decided from it (2026-09-30, by the user)

Option **E**: the changes query (which paths changed, statuses, modes, ids, rename
and copy pairs) comes from `git diff-tree -M` always; gix keeps content diffs,
history and the model. Exact parity by construction; per-selection cost is git's
own column in section 1 (about 8–37 ms, 83 ms on M1), inside every C14 bar. This
amends D1. A process manager for spawning git is to be designed and built first,
as its own packet — see `git-process-survey.md` for what exists today.
