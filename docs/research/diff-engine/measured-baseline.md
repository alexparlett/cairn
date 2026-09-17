# git's own diff costs on rust-lang/rust: the baseline for `diff-engine`'s bar

Research for the `diff-engine` packet, recorded 2026-09-17. Decision L6
(`docs/work/daily-loop/brainstorm.md`) gives every Tier 0 and Tier 1 read surface
an acceptance bar in the shape of `history-graph`'s A7: a named real repository and
recorded numbers. The roadmap brief says only that this packet's bar is "diffing a
large commit in a large repository". This report measures **`git` 2.55.0 itself,
not Cairn**, on a full clone of rust-lang/rust. It picks the commits and files a bar
could name, and records git's warm numbers for each operation. **It decides no
bar.** Section 4 says what the planner's provisional bars would mean against these
numbers. The planner decides.

Every number here was measured on the machine described in section 1. Where a
claim is an inference and not a measurement, it says so.

## Headlines

- **git meets both provisional bars by a wide margin when the operation is a
  file list or a single file's patch.** The slowest file list measured takes 82.0 ms
  at worst: a 2018 rollup with 2,543 inexact renames (M1). The subject with about
  10k changed lines (F7) takes 10.1 ms at worst.
- **git misses 500 ms only when it writes a whole commit's patch.** "Move /src/test
  to /tests" (S1) takes 724 ms and "Remove licenses" (S2) takes 526 ms. **git misses
  100 ms only on the largest file diff with its default algorithm.** That subject
  (F1) takes 164.6 ms with Myers and 22.4 ms with `--histogram`.
- **Per-file line counts cost up to 23 times as much as the bare file list**
  (1.3 times on M1, 14 times on S1, 23 times on S2). On S1 the cost goes from
  28.6 ms to 407 ms. That fits git inflating every blob, including 27,592 renames
  whose content is identical and whose count is 0/0.
- **The algorithm changes the diff, not just its cost.** On F3, git's Myers reports
  57,548 changed lines in 1,661 hunks. Histogram reports 16,602 lines in 581 hunks,
  about five times faster. So the ranking of "largest diffs by changed lines"
  depends on the algorithm too.
- **The rename-limit warning never fires at git's defaults.** It stays silent on the
  compiler/ move, and on all 99 first-parent diffs in the history with 1,000 or more
  paths. git 2.55's wording is "exhaustive rename detection was skipped due to too
  many files". It does not say "inexact". A stage the limit does not govern finds
  most inexact renames: on M1 it finds 2,505 of 2,543, and every one keeps its file
  name.
- **The suggested window of 30,000 first-parent commits is not recent in this
  repository.** It reaches back to 2013-08-05 and holds 91.9% of all non-merge
  commits. It was scanned anyway (95 s), but the subjects come from the last 2,000
  first-parent commits (since 2025-07-28).
- **A `git diff-tree` process on this repository costs 2.4 ms before it diffs
  anything.** Every number below includes that floor. A long-lived worker does not
  pay it.

## Method, and what the numbers are worth

**Harness.** This machine has neither `hyperfine` nor GNU `time`. Installing either
is a tooling decision not taken here. Instead a Python 3.14.7 script does the timing
(Appendix B, verbatim). It runs each command in four stages:

1. One **capture** run, untimed. It counts stdout bytes and lines, keeps stderr and
   checks the exit code.
2. One discarded **warm-up** run, with stdout and stderr sent to `/dev/null`.
3. **Five timed runs**, also with stdout and stderr sent to `/dev/null`.
4. The results: median, min and max wall time, plus median user and system time
   and peak resident memory.

Wall time is `time.perf_counter_ns()` around `os.posix_spawnp` plus `os.wait4`, so
process start is included, as it is in `hyperfine`. User time, system time and peak
RSS come from `wait4`'s `rusage`, the same source GNU `time` reads for `%U %S %M`.

**Command environment.** Every command ran with `LC_ALL=C` and
`GIT_OPTIONAL_LOCKS=0`. `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`,
`GIT_EXTERNAL_DIFF`, `GIT_PAGER` and `PAGER` were removed. Every diff command
carries `--no-ext-diff --no-color`. Output never goes to a terminal, so no pager
runs. `GIT_OPTIONAL_LOCKS=0` is what keeps `git status` read-only: without it,
`status` may refresh and rewrite `.git/index`. On this index it would have had
nothing to refresh (section 1). Nothing in this report wrote to the repository.

**Warm, and how warm.** Every timed run came after the history scans (section 2) had
read the whole history. `fincore` showed the entire pack and index in the page cache
before and after the runs (section 1). No cold-cache number exists. Dropping the
page cache needs root.

**Sink check.** Could git be special-casing `/dev/null`? Four of the
largest-output commands were also timed with stdout drained through a pipe, five
runs each after a warm-up. The medians agree within 2%:

| Command | median to `/dev/null` (ms) | median through a pipe (ms) |
| --- | --- | --- |
| S2 whole patch (14.2 MiB) | 528.9 | 532.3 |
| S1 whole patch (6.7 MiB) | 718.3 | 730.1 |
| F9 file patch (9.9 MiB) | 19.6 | 19.5 |
| F1 file patch, Myers (5.6 MiB) | 161.3 | 162.6 |

**Two passes.** The whole suite ran twice: pass 1 from 17:47:36 to 17:48:05 and
pass 2 from 17:50:00 to 17:50:29 (UTC+01:00). The tables report pass 1 and show
pass 2's median beside it where it matters. Appendix C has both passes in full.

- Every median of 10 ms or more agrees across the passes within 3.9%.
- Every median under 10 ms agrees within 0.70 ms.
- One exception: the first command of pass 2 (S1 `--no-renames --name-status`) read
  31.2 ms against 24.9 ms in pass 1. Its user and system time were unchanged, so the
  extra time was spent waiting, not working. Three isolated re-runs of ten runs each
  gave medians of 24.1, 24.1 and 25.6 ms. Pass 1's figure stands.

**CPU frequency was not pinned.** The machine runs `amd-pstate-epp` with the
`powersave` governor and `balance_performance` preference. The warm-up run absorbs
the ramp, and the agreement between passes above is the evidence that it did.

**Not measured.** Cold cache; any repository other than this one; any
`--cc` or `-m` combined diff of a merge; `-C` copy detection; `--minimal`; Cairn or
gix. Every number below is git's.

## 1. Environment

| Item | Value |
| --- | --- |
| Date and times | 2026-09-17. History scans 17:40:23-17:41:58; timed pass 1 17:47:36-17:48:05; pass 2 17:50:00-17:50:29 (UTC+01:00) |
| CPU (`lscpu`) | AMD Ryzen 7 9800X3D 8-Core Processor: 8 cores, 16 threads, 1 socket, 1 NUMA node, L3 96 MiB, max 5,271.6 MHz |
| Frequency policy | `amd-pstate-epp`, governor `powersave`, EPP `balance_performance`, frequency boost enabled |
| RAM | 63,310,364 kB (60.4 GiB); swap is a 60 GiB zram device |
| Storage holding `/home` | `nvme1n1`, Samsung SSD 980 PRO 1TB (NVMe). Partition `nvme1n1p2` is btrfs, subvolume `/@home`, mounted `rw,noatime,compress=zstd:3,ssd,discard=async,space_cache=v2,commit=120` |
| Kernel | `Linux 7.2.5-1-cachyos #1 SMP PREEMPT_DYNAMIC`, x86_64 |
| git | `git version 2.55.0` at `/usr/bin/git`, the only `git` on `PATH`. `--build-options`: built from commit `e9019fcafe0040228b8631c30f97ae1adb61bcdc`, zlib-ng 2.3.3, SHA-1 `SHA1_DC`, SHA-256 `SHA256_BLK`, default hash sha1 |
| Machine load | `top` 93% idle just before the runs. One-minute load average 0.60 to 0.84 across pass 1 and 0.69 to 1.21 across pass 2 |

The repository:

| Item | Value |
| --- | --- |
| Clone | `/home/alexparlett/Development/bench/rust`, origin `https://github.com/rust-lang/rust.git`, branch `main`. Not shallow (`git rev-parse --is-shallow-repository` prints `false`), no partial-clone filter |
| HEAD | `c999cef531ea9059e189e82fe0e82c5daf249bc9`, "Auto merge of #162859 - JonathanBrouwer:rollup-0CDWDbo, r=JonathanBrouwer", committed 2026-09-17T02:59:25Z |
| Commits reachable from HEAD | 340,228, of which 232,766 are non-merge and 107,462 are merges; 19 root commits; the first-parent chain is 45,262 commits long |
| `git count-objects -vH` | `count: 0`, `size: 0 bytes`, `in-pack: 3526238`, `packs: 1`, `size-pack: 1.04 GiB`, `prune-packable: 0`, `garbage: 0`, `size-garbage: 0 bytes` |
| Pack files | One pack, `pack-9359bf258c47b6dcd46ec66c3593c38d2a8bb5a4`: `.pack` 1,021,034,929 B, `.idx` 98,735,736 B, `.rev` 14,105,004 B. No bitmap, no multi-pack-index |
| Commit-graph | Absent. `.git/objects/info/` is empty: there is no `commit-graph` file and no `commit-graphs/` chain. A diff between named commits reads only those commits, so this matters for the history scans, not for the timed diffs |
| Index and working tree | `.git/index` is 7,647,481 B with 62,892 entries. 12 of those are gitlinks for submodules that were never initialised (no `.git/modules`). On disk, outside `.git`: 62,880 files and symlinks in 4,742 directories |
| Index freshness | The index was written at 17:38:22, after every checked-out file (file mtimes 17:32:30-31). 71 entries record size 0: 59 empty blobs plus the 12 gitlinks. So no entry is marked racily clean, and `status` re-hashes nothing |
| Config in effect (`git config --list --show-origin --show-scope`) | Global: `user.email`, `user.name`, `init.defaultbranch`. Local: `core.repositoryformatversion`, `core.filemode`, `core.bare`, `core.logallrefupdates`, `remote.origin.*`, `branch.main.*`. There is no system file. No scope sets anything about diffs, renames, status or packs, so git's defaults hold. Per the installed man pages those are `diff.algorithm` myers, `diff.renameLimit` 1000, `core.deltaBaseCacheLimit` 96 MiB and `core.packedGitWindowSize` 1 GiB on 64-bit |
| Attributes that reach the subjects | `.gitattributes` sets `*.rs` to `diff=rust`, confirmed with `git check-attr` on every `.rs` subject. So git's built-in Rust hunk-header pattern runs for F3, F4, F5, F7 and every `.rs` file in a whole-commit patch. The `.json`, `.xml`, `.html` and `.md` subjects are `text=auto` with `diff` unspecified |
| Page cache (`fincore`, before and after the runs) | `.pack` 973.7 MiB of 973.7 MiB resident; `.idx` 94.2 MiB of 94.2 MiB; `.git/index` 7.3 MiB of 7.3 MiB. `free -h` at 17:38: 43 GiB buff/cache, 39 GiB available |

## 2. The subjects, and why each

Every subject has a short tag, used through the rest of the report:

| Tag | Commit | Stresses |
| --- | --- | --- |
| S1 | `cf2dff2b1e3fa55fa5415d524200070d0d7aacfe` | Most paths of any commit: 55,184 paths, all 27,592 of them exact renames |
| S2 | `2a663555ddf36f6b041445894a8c175cd1bc718c` | Most content modifications: 16,206 files edited |
| S3 | `3fc7ab237314a4ce85e612b4ce590c27f1425291` | 6,551 renames mixed with adds and deletes |
| S4 | `ec2cc761bc7067712ecc7734502f703fe3b024c8` | 9,925 edited files, generated by a tool |
| S5 | `9be35f82c1abf2ecbab489bca9eca138ea648312` | 3,214 renames |
| S6 | `9e5f7d5631b8f4009ac1c693e585d4b7108d4275` | The `src/librustc_*` to `compiler/` move (section 2b) |
| S7 | `f0845adb0c1b7a7fa1bef73e749b2d7e1d7f374d` | The literal "1,000-file commit": 1,017 edited files |
| S8 | `3fba180bb74610ef2fb44d5dfb18d385e2e1534a` | The most recent commit with 1,000 or more paths (a clippy subtree sync) |
| M1 | `5a3292f163da3327523ddec5bc44d17c2378ec37` | The largest rollup merge ever, compared with its first parent: 5,602 paths, 2,543 inexact renames |
| M2 | `b45dd71d1824f176fba88f6c40467030a16afa2c` | The largest rollup since 2019-07: 906 paths |
| M3 | `c999cef531ea9059e189e82fe0e82c5daf249bc9` | HEAD itself: a small rollup of 17 paths |
| F1-F5 | section 2c | The five largest single-file text diffs in the recent window |
| F6-F9 | section 2c | A huge file with a small change, a diff of about 10k changed lines, a hand-edited 16k-line file, and the largest text file at HEAD |
| W1 | `git status --porcelain=v2` | Working-tree scale, as orientation only (packet 4 owns status) |

### 2a. The non-merge commits that touch the most files

**Scan.** Tree diffs only, no rename detection, over the whole history. It took
8.4 s for 232,766 non-merge commits (pipeline in Appendix A):

```
git log --no-merges --no-renames --name-only --format='@@%H %P' c999cef531ea9059e189e82fe0e82c5daf249bc9
```

Paths per non-merge commit:

| p50 | p90 | p99 | p99.9 | max |
| --- | --- | --- | --- | --- |
| 2 | 9 | 60 | 329 | 55,184 |

The tail: 1,278 commits touch 100 or more paths, 127 touch 500 or more, 58 touch
1,000 or more, 8 touch 5,000 or more and 3 touch 10,000 or more.

**The top five.** All five are rust-lang/rust's own commits, not history imported
from a subtree. Three are dominated by renames and two are pure edits, which is
why both kinds are in the set. On the first-parent chain (section 2d), 43
first-parent diffs have 1,000 or more paths.

| Tag | Commit | Authored / committed | Author | Subject | Paths (`--no-renames`) | With `-M` | Landed in |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S1 | `cf2dff2b1e3fa55fa5415d524200070d0d7aacfe` | 2023-01-05 / 2023-01-11 | Albert Larsan | Move /src/test to /tests | 55,184 | 27,592 R, all exact (R100) | `b22c152958e` (#106458) |
| S2 | `2a663555ddf36f6b041445894a8c175cd1bc718c` | 2018-12-25 / 2018-12-25 | Mark Rousskov | Remove licenses | 16,206 | 16,206 M | `79d8a0fcefa` (#57108) |
| S3 | `3fc7ab237314a4ce85e612b4ce590c27f1425291` | 2018-08-08 / 2018-08-14 | David Wood | Merged migrated compile-fail tests and ui tests. Fixes #46841. | 13,135 | 6,551 R (6,483 exact) + 17 A + 16 D | `f45f52532a3` (#53196) |
| S4 | `ec2cc761bc7067712ecc7734502f703fe3b024c8` | 2024-02-16 / 2024-02-16 | 许杰友 Jieyou Xu (Joe) | [AUTO-GENERATED] Migrate ui tests from `//` to `//@` directives | 9,925 | 9,925 M | `bccb9bbb418` (#120881) |
| S5 | `9be35f82c1abf2ecbab489bca9eca138ea648312` | 2019-07-27 / 2019-07-27 | Vadim Petrochenkov | tests: Move run-pass tests without naming conflicts to ui | 6,440 | 3,214 R (3,208 exact) + 1 A + 11 D | `c798dffac9d` (#63029) |

**`--shortstat` for the top two** (`git diff-tree -r <flag> --shortstat <c>`):

| Subject | `--no-renames` | `-M` |
| --- | --- | --- |
| S1 | 55184 files changed, 849763 insertions(+), 849763 deletions(-) | 27592 files changed, 0 insertions(+), 0 deletions(-) |
| S2 | 16206 files changed, 14383 insertions(+), 129150 deletions(-) | the same: no renames |

With `-M`, the rest are: S3 has 6584 files changed, +884 and −762. S4 has 9925
files, +16401 and −16401. S5 has 3226 files, +64 and −196. Without renames, S3 and
S5 read +179,752 / −179,630 and +92,765 / −92,897. A diff view that does not detect
renames sees S1 as 1.7 million changed lines. With detection, S1 has none.

**Two more commit subjects, for the provisional bar's own shape:**

- **S7**, `f0845adb0c1b7a7fa1bef73e749b2d7e1d7f374d`, "Show diff suggestion format
  on verbose replacement" by Esteban Küber. Authored 2024-07-09, committed
  2025-02-10, landed in `ffa9afef183` (#127541). 1,017 edited files, +10,364 and
  −6,943. A rust-lang/rust commit that is literally a 1,000-file commit of ordinary
  edits.
- **S8**, `3fba180bb74610ef2fb44d5dfb18d385e2e1534a`, "Merge commit
  '49e2f89a2605a9a340e04955fdbefa9803e6027a' into clippy-subtree-update" by Philipp
  Krones, 2026-08-07, landed in `1a98b1e135b` (#160692). It has a single parent. Of
  1,106 paths, `-M` makes 1,022 M, 78 A, 2 D and 2 R. It is the most recent commit
  in the history with 1,000 or more paths, and it mixes file kinds the way a
  subtree sync does.

A quirk worth knowing: `d0762f032b4a5ed636a1ebae0b82647f2cce2998`, "always emit
consider `AutoImplCandidates` for them if they don't also have a
`ProjectionCandidate`" (2023-07-06), is a **root commit** with 1,613 paths in the
middle of the history. `git diff-tree <c>` prints nothing for it without `--root`,
and `<c>^` does not resolve. Any "diff against the parent" code path meets it.

### 2b. The rename-heavy commit: `src/librustc_*` to `compiler/`

**Found by** searching commit messages around the known 2020 move, then confirming
it among the commits with 1,000 or more paths:

```
git log --format='%H %P | %cs | %an | %s' --since=2020-07-01 --until=2020-10-15 \
  -i -E --grep='mv (compiler|std)|compiler/|librustc|move .*compiler' c999cef531e
```

**S6: `9e5f7d5631b8f4009ac1c693e585d4b7108d4275`**, "mv compiler to compiler/", by
mark. Authored 2020-08-27, committed 2020-08-30. Parent
`db534b3ac286cf45688c3bbae6aa6e77439e52d2`. It landed as
`85fbf49ce0e2274d0acf798f6e703747674feec3`, "Auto merge of #74862 -
mark-i-m:mv-compiler, r=petrochenkov", whose first parent is the same `db534b3ac28`.
So the merge's first-parent diff equals the commit's diff, count for count.

| Setting | R | of which exact | A | D | M | Entries | stderr |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `--no-renames` | - | - | 1,629 | 1,629 | 21 | 3,279 | empty |
| `-M` (default limit, 1000) | 1,593 | 1,582 | 36 | 36 | 21 | 1,686 | **empty** |
| `-M -l0` (unlimited) | 1,593 | 1,582 | 36 | 36 | 21 | 1,686 | empty |
| `-M -l100`, `-l300`, `-l1000` | 1,593 | 1,582 | 36 | 36 | 21 | 1,686 | empty |
| `-M -l1`, `-l10`, `-l40` | 1,582 | 1,582 | 47 | 47 | 21 | 1,697 | warning, plus advice to set `diff.renameLimit` to at least 47 |

**What that means.**

- **At git's defaults, no warning is printed, and `-l0` changes nothing.** Exact
  matching pairs 1,582 files first. That leaves 47 deleted and 47 added files for
  the exhaustive stage. 47 × 47 is well inside the default limit of 1000², so the
  stage runs in full.
- **Every leftover file is a crate's `Cargo.toml`.** The exhaustive stage pairs 11
  of them, at similarity scores from 53% to 82%. The other 36 fall below git's 50%
  default and show as 36 deletes plus 36 adds. That is a similarity outcome, not a
  limit outcome.
- **A forced low limit shows what the warning looks like on 2.55.** git prints
  "exhaustive rename detection was skipped due to too many files." The measurement
  brief quoted the wording as "inexact rename detection…"; git 2.55 says
  "exhaustive". A second warning line names the smallest `diff.renameLimit` that
  would have been enough, and the diff falls back to exact renames only.

**Heavier rename evidence, measured the same way:**

- **S1 (27,592 renames).** Every rename is exact, so every `-l` value from 1 to
  1000, and 0, gives the same 27,592 R. A single untimed run takes 30-33 ms.
- **M1 (2,774 renames in a rollup's first-parent diff).** Only 231 of the renames
  are exact. The other 2,543 are inexact: 2,538 score 90-99% and 5 score 80-89%.
  - With `-l1`, `-l10` or `-l40`, git still finds **2,736** renames (2,505 of them
    inexact) and warns with advice of "at least 59".
  - At `-l100` and above, it finds the full 2,774.
  - **Every one of the 2,505 inexact renames found under `-l1` keeps the same file
    name** (the last path component). So do the 38 found only by the exhaustive
    stage.
  - `git help diff` says `-l` limits only the exhaustive part, after "some
    preliminary steps that can detect subsets of renames/copies cheaply". The data
    fit a preliminary stage that pairs files by name and reaches most inexact
    renames before the limit applies. Inference from the output only; git's source
    was not read.

**The warning across the whole history.** Every first-parent diff with 1,000 or
more paths was run through `git diff-tree -r -M --name-status <c>^1 <c>` at
defaults: 58 non-merge commits and 42 two-parent merges, 99 diffs once the root
commit is excluded. **None printed the warning.** Some pairs are the same diff
twice, when a single-commit PR's commit and its merge share a first parent. The
slowest single run was M1 at 79-83 ms. Appendix A has the scan.

### 2c. The largest single-file text diffs, and the largest text file at HEAD

**The window.** The suggested window was the last 30,000 first-parent commits. In
this repository that means everything after `HEAD~30000` =
`bbda3fa9383dba653b20bd064102caceef91897a`, "auto merge of #8288 :
Kimundi/rust/opteitres4, r=brson", **2013-08-05**. That range holds 213,995 of the
232,766 non-merge commits (91.9%). It was scanned in full anyway, since a
line-count scan over it took only 94.9 s: 1,331,909 text rows and 515 binary rows.
But it is not a recent window.

**The subjects come from the last 2,000 first-parent commits** instead:
`HEAD~2000` = `d242a8bd5a73f633ba1ec5aacf19acf35a3c747d`, "Auto merge of #144469 -
Kivooeo:chains-cleanup, r=SparrowLii", 2025-07-28. That is about 14 months, and
24,423 non-merge commits (`git rev-list --no-merges HEAD~2000..HEAD`). A window
defined by ancestry still pulls in imported subtree history: 819 of those 24,423
commits (3.4%) have committer dates before 2025-07-28, the oldest from 2018-06-20.
The ranking is by `git log --numstat` added plus deleted lines, with git's default
algorithm, and binary rows (`-`) excluded. Appendix A has the exact pipeline.

**The five largest in the recent window (F1-F5):**

| Tag | Commit | Authored / committed | Author, subject | Path | Kind | Changed lines (Myers numstat) | Before → after | Hunks |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| F1 | `6a6e8446b97e8a3dfc0984660253b1ac437a445a` | 2026-02-28 / 2026-04-14 | David Wood, "intrinsics_data: add sve intrinsics" | `library/stdarch/intrinsics_data/arm_intrinsics.json` | M | +238,271 −32,701 = 270,972 | 121,063 L (1,890,602 B) → 326,633 L (5,245,383 B) | 2,436 |
| F2 | `223bb3c24b6012e54fb7a3da0556c1626719e52e` | 2025-08-05 / 2025-10-26 | Madhav Madhusoodanan, "feat: Added x86 to CI pipeline" | `library/stdarch/intrinsics_data/x86-intel.xml` | A | +158,422 | none → 158,421 newline-terminated lines (6,673,004 B; the last line has no newline) | 1 |
| F3 | `f09bf0bf511e0a41adb39f78268a0e68c02c89e4` | 2026-05-09 / 2026-05-14 | sayantn, "gen-arm: toggle `big_endian_inverse` where required" | `library/stdarch/crates/core_arch/src/arm_shared/neon/generated.rs` | M | +29,620 −27,928 = 57,548 | 69,444 L (2,602,521 B) → 71,136 L (2,681,600 B) | 1,661 |
| F4 | `a753cf4d77ebb6c39e984200087b1976c1d80c38` | 2026-01-15 / 2026-04-13 | David Wood, "core_arch: generated sve intrinsics" | `library/stdarch/crates/core_arch/src/aarch64/sve/generated.rs` | M (from a 1-byte file) | +44,957 | 1 L (1 B) → 44,958 L (2,359,922 B) | 1 |
| F5 | same commit as F4 | same | same | `library/stdarch/crates/core_arch/src/aarch64/sve2/generated.rs` | M (from a 1-byte file) | +23,856 | 1 L (1 B) → 23,857 L (1,206,017 B) | 1 |

A merge-level scan cross-checks this list: the first-parent `--numstat` of the last
2,000 first-parent commits, 101,826 text rows, 7.0 s. It finds the same blobs at
the top: F1's change landed in `0204aca0663` (#155385) and F2's in `f2bae990e89`
(#148425).

**For comparison, the top five rows of the 30,000-first-parent window:**

1. F9 below: 313,330 lines added, 2019.
2. F1.
3. `f283e449b11ebe8127570aab09b8871442d1e74b`, "PR feedback & pipeline",
   committed 2025-01-16. `arm_intrinsics.json` +12 −190,668
   (296,926 → 106,270 L).
4. `9e24b307dff93e78ebdc75650ef6504f804e1860`, "Add SVE support to
   stdarch-verify", committed 2025-01-16. The same two blobs in reverse,
   +190,668 −12.
5. F2.

**Four supplementary file subjects (F6-F9).** F1-F5 are all generated `stdarch`
data or code, and three of them are effectively whole-file additions. Two shapes a
provisional bar names, "a 10k-line file diff", are not in that set. Neither is the
largest file at HEAD.

| Tag | Commit | Committed | Path | Kind | Changed lines | Before → after | Hunks | Why |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| F6 | `992cb1d9de25ea8181dc2ea2403b91d6306f4868`, "arm-intrinsics: `svset{2,3,4}`" (David Wood) | 2026-06-16 | `library/stdarch/intrinsics_data/arm_intrinsics.json` | M | +198 | 319,143 L (5,120,409 B) → 319,341 L (5,124,039 B) | 33 | The most recent change to the text file with the most lines at HEAD: a huge file, small change |
| F7 | `3b09522c34b43d8cc9334371ba7e54b8e06471d6`, "Revert "Remove big-endian swizzles from `vreinterpret`"" (usamoi) | 2025-09-23 | `library/stdarch/crates/core_arch/src/arm_shared/neon/generated.rs` | M | +9,379 −904 = 10,283 | 66,874 L (2,532,736 B) → 75,349 L (2,843,298 B) | 324 | About 10k changed lines in a real edit |
| F8 | `27ee86c0606742c491037219354eacdd7b0bdcc0`, "Port 1.98.1 release notes into main" (Mark Rousskov) | 2026-08-31 | `RELEASES.md` | M | +7 | 16,265 L (918,483 B) → 16,272 L (918,663 B) | 1 | A hand-edited file of more than 10k lines, with a routine change |
| F9 | `d45703aecaabcafebe1d849899d2d3b941a18dcb`, "move the last few things from the forge" (Mark Mansi) | 2019-12-09 | `src/doc/rustc-dev-guide/src/queries/example-0.html` | A | +313,330 | none → 313,330 L (10,044,208 B) | 1 | The largest text file at HEAD. Blob `b196aaa837391cc3a3b8cfd88c29854f65719685` is unchanged since this, its only commit. Also the largest single-file diff in the 30,000-first-parent window |

**The largest text files at HEAD**, by blob size from `git ls-tree -r -l`. Each was
checked for a NUL byte in its first 8,000 bytes and for `binary`/`diff`
attributes:

| Bytes | Lines | Path | Note |
| --- | --- | --- | --- |
| 10,044,208 | 313,330 | `src/doc/rustc-dev-guide/src/queries/example-0.html` | Largest by bytes (F9) |
| 8,121,886 | 4,908 | `tests/ui/parser/survive-peano-lesson-queue.rs` | Long lines: 1,653 B per line on average, the longest 6,013 B. Not measured; see the open questions |
| 6,673,004 | 158,421 | `library/stdarch/intrinsics_data/x86-intel.xml` | F2 |
| 5,124,039 | 319,341 | `library/stdarch/intrinsics_data/arm_intrinsics.json` | **Most lines at HEAD** (F1, F6) |
| 2,970,947 | 62,626 | `library/stdarch/crates/core_arch/src/x86/avx512f.rs` | |
| 2,691,316 | 71,308 | `library/stdarch/crates/core_arch/src/arm_shared/neon/generated.rs` | F3, F7 |

Four more files in the top 20 by size are binary and were excluded:
`src/doc/rustc-dev-guide/mermaid.min.js` (1,102,136 B),
`src/tools/rust-analyzer/bench_data/numerous_macro_rules`,
`src/etc/installer/gfx/dialogbg.bmp` and `src/tools/miri/doc/img/perfetto_timeline.png`.
All four resolve to `binary` through attributes, and the last two also contain NUL
bytes.

**The ranking depends on the algorithm.** The same file pairs under git's three
line algorithms (`git diff --diff-algorithm=<a> --numstat` and a count of `@@`
lines):

| Subject | myers (git default): +/− , hunks | histogram: +/− , hunks | patience: +/− , hunks |
| --- | --- | --- | --- |
| F1 | 238,271 / 32,701, 2,436 | 208,393 / 2,823, 582 | 208,566 / 2,996, 579 |
| F3 | 29,620 / 27,928, 1,661 | 9,147 / 7,455, 581 | 10,236 / 8,544, 589 |
| F7 | 9,379 / 904, 324 | 8,475 / 0, 327 | 9,239 / 764, 320 |
| F6 | 198 / 0, 33 | 198 / 0, 33 | 198 / 0, 33 |

On generated code git's Myers is far from minimal: F3's Myers diff has about 3.5
times the changed lines of histogram's. So "the largest diff" is partly a property
of Myers here, and a bar that names changed-line counts must name the algorithm.

### 2d. A large rollup merge

**Scan.** A tree-only scan over the whole first-parent chain, 45,262 commits in
3.2 s. It records each commit's first-parent diff, merges included:

```
git log --first-parent --diff-merges=first-parent --no-renames --name-only --format='@@%H%x09%P%x09%cs%x09%s' c999cef531e
```

Paths per first-parent diff: p50 3, p90 45, p99 236, p99.9 920, max 55,260. 5,505
of those commits have "rollup" in the subject. Their paths run p50 36, p90 120, p99
365. In the recent window, the 894 rollups since 2025-07-28 run p50 49, p90 182,
p99 408, max 591.

- **M1**, `5a3292f163da3327523ddec5bc44d17c2378ec37`, "Auto merge of #54021 -
  kennytm:rollup, r=kennytm", 2018-09-07, first parent `7366752a6164`. **The largest
  rollup by first-parent paths in the whole chain: 5,602 paths.**
  - `-M` gives 2,828 entries: 2,774 R (231 exact, 2,543 inexact), 21 A, 33 M.
  - `--shortstat` with `-M`: 2828 files changed, 5510 insertions(+), 484
    deletions(-). With `--no-renames`: 5602 files, +107,324 −102,298.
  - It is the heaviest inexact-rename file list found anywhere in this study.
- **M2**, `b45dd71d1824f176fba88f6c40467030a16afa2c`, "Auto merge of #140529 -
  matthiaskrgr:rollup-jpaa2ky, r=matthiaskrgr", 2025-04-30. **The largest rollup
  since `b43eb4235ac` (919 paths, 2019-07-03): 906 paths.**
  - `-M` gives 896 entries: 742 M, 49 A, 95 D, 10 R (1 exact).
  - `--shortstat` with `-M`: 896 files changed, +33,318 −39,601.
  - A recent rollup of ordinary edits, close to the 1,000-file shape.
- **M3**, HEAD `c999cef531ea9059e189e82fe0e82c5daf249bc9`, rollup #162859. 17 paths
  (14 M, 3 A), +213 −130. A small everyday rollup, measured as the low end.

## 3. git's numbers

Commands, exactly as run (`R` is the repository path; every run also had
`LC_ALL=C GIT_OPTIONAL_LOCKS=0`):

| Operation | Command |
| --- | --- |
| File list, no renames | `git -C R diff-tree -r --no-renames --name-status --no-ext-diff --no-color <c>` |
| File list, default rename detection | `git -C R diff-tree -r -M --name-status --no-ext-diff --no-color <c>` |
| File list with per-file stats | `git -C R diff-tree -r -M --numstat --no-ext-diff --no-color <c>` |
| Whole commit's patch | `git -C R diff-tree -r -M -p --no-ext-diff --no-color <c>` |
| Merge, first parent | the same four, with `<m>^1 <m>` in place of `<c>` |
| One file's patch | `git -C R diff --no-ext-diff --no-color <c>^ <c> -- <path>` |
| One file's patch, histogram | `git -C R diff --histogram --no-ext-diff --no-color <c>^ <c> -- <path>` |
| Working tree | `git -C R status --porcelain=v2` |

Output line counts below include the one header line `diff-tree` prints when given
a single commit (S1-S8). Merges are given two trees and print no header.

### 3.1 Commit and merge subjects: wall time, milliseconds, median / max of 5

| Subject | `--no-renames --name-status` | `-M --name-status` | `-M --numstat` | `-M -p` |
| --- | --- | --- | --- | --- |
| S1 (55,184 paths; 27,592 exact R) | 24.9 / 26.7 | 28.6 / 28.8 | 407.4 / 411.3 | 724.3 / 728.1 |
| S2 (16,206 M) | 13.6 / 14.2 | 13.3 / 14.2 | 304.5 / 309.4 | 525.9 / 530.4 |
| S3 (13,135 paths; 6,551 R) | 7.2 / 7.4 | 10.6 / 11.1 | 66.9 / 68.6 | 126.0 / 126.9 |
| S4 (9,925 M) | 11.3 / 12.4 | 11.6 / 12.2 | 103.6 / 106.5 | 245.0 / 246.5 |
| S5 (6,440 paths; 3,214 R) | 7.6 / 8.1 | 8.3 / 8.4 | 40.1 / 41.7 | 90.3 / 90.5 |
| S6 (compiler/ move) | 5.0 / 5.7 | 6.6 / 6.8 | 95.7 / 96.6 | 136.7 / 136.7 |
| S7 (1,017 M) | 7.8 / 7.9 | 7.9 / 8.2 | 23.7 / 24.1 | 26.8 / 27.1 |
| S8 (1,106 paths) | 3.2 / 3.5 | 5.1 / 5.4 | 38.5 / 39.7 | 58.0 / 58.5 |
| M1 first parent (5,602 paths; 2,543 inexact R) | 5.7 / 5.9 | 78.5 / 82.0 | 104.7 / 106.6 | 148.2 / 150.1 |
| M2 first parent (906 paths) | 4.4 / 4.8 | 9.9 / 10.3 | 68.9 / 70.4 | 109.4 / 110.0 |
| M3 first parent (17 paths) | 2.5 / 2.7 | 2.5 / 2.6 | 6.6 / 6.7 | 7.6 / 7.8 |

Peak RSS, and what the capture run wrote to stdout:

| Subject | `--no-renames --name-status` | `-M --name-status` | `-M --numstat` | `-M -p` |
| --- | --- | --- | --- | --- |
| S1 | 162 MiB; 2.66 MiB, 55,185 lines | 165 MiB; 2.68 MiB, 27,593 lines | 234 MiB; 1.71 MiB, 27,593 lines | 233 MiB; 6.65 MiB, 110,369 lines |
| S2 | 134 MiB; 764 KiB, 16,207 lines | 135 MiB; 764 KiB, 16,207 lines | 296 MiB; 807 KiB, 16,207 lines | 296 MiB; 14.17 MiB, 331,864 lines |
| S3 | 80 MiB; 701 KiB, 13,136 lines | 97 MiB; 707 KiB, 6,585 lines | 155 MiB; 467 KiB, 6,585 lines | 155 MiB; 1.84 MiB, 29,532 lines |
| S4 | 130 MiB; 468 KiB, 9,926 lines | 130 MiB; 468 KiB, 9,926 lines | 161 MiB; 488 KiB, 9,926 lines | 159 MiB; 4.15 MiB, 117,918 lines |
| S5 | 90 MiB; 304 KiB, 6,441 lines | 93 MiB; 307 KiB, 3,227 lines | 156 MiB; 194 KiB, 3,227 lines | 156 MiB; 778 KiB, 13,251 lines |
| S6 | 84 MiB; 153 KiB, 3,280 lines | 95 MiB; 154 KiB, 1,687 lines | 233 MiB; 129 KiB, 1,687 lines | 233 MiB; 472 KiB, 9,195 lines |
| S7 | 94 MiB; 54 KiB, 1,018 lines | 94 MiB; 54 KiB, 1,018 lines | 132 MiB; 57 KiB, 1,018 lines | 132 MiB; 1.60 MiB, 43,201 lines |
| S8 | 58 MiB; 46 KiB, 1,107 lines | 72 MiB; 46 KiB, 1,105 lines | 115 MiB; 49 KiB, 1,105 lines | 115 MiB; 1.80 MiB, 48,948 lines |
| M1 | 77 MiB; 271 KiB, 5,602 lines | 167 MiB; 273 KiB, 2,828 lines | 190 MiB; 194 KiB, 2,828 lines | 190 MiB; 1.66 MiB, 43,710 lines |
| M2 | 90 MiB; 59 KiB, 906 lines | 97 MiB; 59 KiB, 896 lines | 143 MiB; 61 KiB, 896 lines | 143 MiB; 4.56 MiB, 122,946 lines |
| M3 | 31 MiB; 789 B, 17 lines | 31 MiB; 789 B, 17 lines | 42 MiB; 833 B, 17 lines | 43 MiB; 40 KiB, 890 lines |

No command in 3.1 wrote anything to stderr, and every exit code was 0.

**What the table shows:**

- **A file list is a tree diff, and a tree diff is cheap.** Even at 55,184 paths
  (S1) it costs 24.9 ms, and at 16,206 paths (S2) 13.6 ms. Rename detection adds
  about 4 ms for S1's 27,592 exact renames. It adds about 73 ms for M1's 2,543
  inexact ones (5.7 → 78.5 ms). Inexact renames are what make a file list slow.
- **Per-file stats are where cost starts.** `--numstat` is 14 times the `-M` file
  list on S1 and 23 times on S2. S1's 380 ms of extra time buys 27,592 rows that all
  read `0 0`. Over those runs the process takes 12,640 more minor page faults and
  69 MiB more peak RSS than `--name-status` does. The renamed blobs total 24.9 MiB
  inflated, 6.2 MiB as stored in the pack. That fits git inflating every blob of an
  exact rename to decide whether it is binary. It is an inference from resource
  usage; git's source was not read.
- **User plus system time roughly equals wall time for every `diff-tree` and
  `diff`.** They run on one thread. S1's `--numstat` spends 102.5 ms in system time,
  which fits page-faulting across the mapped pack.
- **Peak RSS is not heap.** git maps the 94.2 MiB `.idx` and pack windows of up to
  1 GiB, so resident file-backed pages count toward `ru_maxrss`. The 96 MiB
  delta-base cache and blob buffers are heap. Separating the two needs
  `/proc/<pid>/smaps`-style sampling, which was not done.

### 3.2 One file's patch

| Subject | Command | Median ms | Max ms | Pass-2 median ms | Peak RSS MiB | stdout |
| --- | --- | --- | --- | --- | --- | --- |
| F1 (+238,271 −32,701; 121k → 327k lines) | Myers | 163.0 | 164.6 | 163.8 | 38.1 | 5.64 MiB, 334,692 lines |
| F1 | `--histogram` | 21.8 | 22.4 | 22.2 | 38.1 | 3.56 MiB, 215,524 lines |
| F2 (158k-line add, 6.7 MB) | Myers | 12.4 | 13.3 | 12.3 | 25.9 | 6.52 MiB, 158,429 lines |
| F3 (+29,620 −27,928; 69k → 71k lines, `diff=rust`) | Myers | 78.8 | 80.1 | 80.5 | 47.1 | 3.66 MiB, 85,274 lines |
| F3 | `--histogram` | 15.8 | 16.5 | 16.3 | 47.4 | 872 KiB, 21,763 lines |
| F4 (+44,957 to a 1-byte file, `diff=rust`) | Myers | 4.3 | 5.0 | 4.3 | 21.6 | 2.29 MiB, 44,963 lines |
| F5 (+23,856 to a 1-byte file, `diff=rust`) | Myers | 3.5 | 3.7 | 3.4 | 22.1 | 1.17 MiB, 23,862 lines |
| F6 (+198 in a 319k-line file) | Myers | 9.9 | 10.2 | 10.2 | 53.9 | 9 KiB, 433 lines |
| F7 (+9,379 −904; 67k → 75k lines, `diff=rust`) | Myers | 9.8 | 10.1 | 9.7 | 30.2 | 584 KiB, 13,735 lines |
| F7 | `--histogram` | 9.5 | 10.0 | 10.0 | 30.2 | 458 KiB, 11,410 lines |
| F8 (+7 in a 16k-line file) | Myers | 3.1 | 3.2 | 3.2 | 22.3 | 376 B, 15 lines |
| F9 (313k-line add, 10.0 MB) | Myers | 18.9 | 20.1 | 18.6 | 32.5 | 9.88 MiB, 313,336 lines |

No command in 3.2 wrote anything to stderr, and every exit code was 0.

**What the table shows:**

- **Size alone is cheap.** Adding a 10 MB, 313k-line file (F9) takes 18.9 ms. A
  198-line change in a 319k-line file (F6) takes 9.9 ms. That fits the cost of a
  mostly unchanged file being line hashing and matching, not the search for an edit
  script (an inference from the timings).
- **Heavy interleaved change is what costs, and it costs Myers far more than
  histogram.** Myers is 7.5 times slower on F1 and 5 times slower on F3, and its
  output is 1.6 and 4.3 times larger. On F7, where the change is mostly one-sided,
  the two algorithms cost the same.

### 3.3 Fixed costs and the working tree

| Measurement | Command | Median ms | Max ms | Peak RSS MiB |
| --- | --- | --- | --- | --- |
| Process floor, no repository | `git version` | 0.43 | 0.70 | 15.0 |
| Open the repository, read one commit | `git -C R rev-parse --verify c999cef531e…^{tree}` | 0.57 | 0.61 | 15.2 |
| **`diff-tree` floor** (a commit against itself: empty diff) | `git -C R diff-tree -r --name-status --no-ext-diff --no-color c999cef531e… c999cef531e…` | **2.40** | 2.69 | 23.2 |
| A small commit's file list (M3 again, separate run) | `git -C R diff-tree -r -M --name-status … c999cef531e…^1 c999cef531e…` | 3.04 | 3.23 | 31.3 |
| **W1** working tree (62,892 index entries, clean) | `git -C R status --porcelain=v2` | 29.5 | 29.7 | 23.5 |

- **The floor matters for the small subjects.** A `diff-tree` process costs 2.4 ms
  before it compares a single tree: process start, configuration, attributes and
  mapping the 94 MiB `.idx`. That is 30% of S7's 7.9 ms file list and most of M3's.
  `git diff` has a floor of its own, at most F8's 3.1 ms.
- **Status runs in parallel.** W1 spends 18.1 ms of user time and 45.7 ms of system
  time inside 29.5 ms of wall time, so its work is spread over threads. Per the man
  page, `core.preloadIndex` checks the index against the filesystem in parallel and
  defaults to true. Its pass-2 median was 29.2 ms. It is here as orientation only; packet 4 owns status.

## 4. What the provisional bars would mean (evidence, not a decision)

The planner's provisional bars were "file list of a 1,000-file commit under 500 ms"
and "a 10k-line file diff under 100 ms". The table puts git's median and max for
each bar-shaped operation beside three ratio budgets (2, 5 and 10 times git's max)
and the provisional ceiling. The last column is the ceiling divided by git's max:
above 1 means git itself meets the ceiling, below 1 means it misses.

| Operation | Subject | git median / max ms | 2× git max | 5× git max | 10× git max | Provisional ceiling | Ceiling ÷ git max |
| --- | --- | --- | --- | --- | --- | --- | --- |
| File list, renames on | S7 (1,017 M) | 7.9 / 8.2 | 16.4 | 41.1 | 82.2 | 500 ms | 60.8 |
| File list, renames on | S2 (16,206 M) | 13.3 / 14.2 | 28.3 | 70.8 | 142 | 500 ms | 35.3 |
| File list, renames on | S1 (55,184 paths) | 28.6 / 28.8 | 57.7 | 144 | 288 | 500 ms | 17.3 |
| File list, renames on | M1 (2,543 inexact R) | 78.5 / 82.0 | 164 | 410 | 820 | 500 ms | 6.1 |
| File list + per-file stats | S7 | 23.7 / 24.1 | 48.2 | 121 | 241 | 500 ms | 20.7 |
| File list + per-file stats | M1 | 104.7 / 106.6 | 213 | 533 | 1,066 | 500 ms | 4.7 |
| File list + per-file stats | S2 | 304.5 / 309.4 | 619 | 1,547 | 3,094 | 500 ms | 1.6 |
| File list + per-file stats | S1 | 407.4 / 411.3 | 823 | 2,056 | 4,113 | 500 ms | 1.2 |
| Whole commit patch | S7 | 26.8 / 27.1 | 54.2 | 135 | 271 | 500 ms | 18.5 |
| Whole commit patch | M2 | 109.4 / 110.0 | 220 | 550 | 1,100 | 500 ms | 4.5 |
| Whole commit patch | S2 | 525.9 / 530.4 | 1,061 | 2,652 | 5,304 | 500 ms | **0.94: git misses it** |
| Whole commit patch | S1 | 724.3 / 728.1 | 1,456 | 3,641 | 7,281 | 500 ms | **0.69: git misses it** |
| One file's patch | F8 (16k-line file, +7) | 3.1 / 3.2 | 6.3 | 15.8 | 31.6 | 100 ms | 31.6 |
| One file's patch | F7 (~10k changed lines) | 9.8 / 10.1 | 20.2 | 50.6 | 101 | 100 ms | 9.9 |
| One file's patch | F6 (319k-line file, +198) | 9.9 / 10.2 | 20.3 | 50.8 | 102 | 100 ms | 9.8 |
| One file's patch | F9 (313k-line add) | 18.9 / 20.1 | 40.3 | 101 | 201 | 100 ms | 5.0 |
| One file's patch | F3, Myers | 78.8 / 80.1 | 160 | 401 | 801 | 100 ms | 1.2 |
| One file's patch | F3, histogram | 15.8 / 16.5 | 32.9 | 82.3 | 165 | 100 ms | 6.1 |
| One file's patch | F1, Myers | 163.0 / 164.6 | 329 | 823 | 1,646 | 100 ms | **0.61: git misses it** |
| One file's patch | F1, histogram | 21.8 / 22.4 | 44.8 | 112 | 224 | 100 ms | 4.5 |

**Does git meet the provisional bars on this hardware?**

- **"File list of a 1,000-file commit under 500 ms": yes, by 60 times** on the
  literal subject, S7 (8.2 ms max).
  - The slowest file list found, M1, still clears it by 6.1 times. M1 was also
    the slowest of the 99 large first-parent diffs scanned in 2b. The 55,184-path
    S1 clears it by 17.3 times.
  - If "file list" includes per-file line counts, git still clears 500 ms
    everywhere, but only by 1.2 times on S1 and 1.6 times on S2.
  - If it includes producing every hunk of the commit up front, git itself misses
    500 ms on S1 and S2.
- **"A 10k-line file diff under 100 ms": yes, on every reading of "10k-line".**
  - About 10k changed lines (F7): 10.1 ms.
  - A file of more than 10k lines with a small change: F8 at 3.2 ms, F6 at 10.2 ms.
  - Git misses 100 ms only on F1 with its default Myers, a 271k-changed-line edit
    of a 5 MB generated file, and only by about 65%. With histogram, F1 clears 100
    ms by 4.5 times.

**Which subjects exercise which bar best:**

- **File list:**
  - S7 is the literal shape.
  - M1 is the slowest file list found (inexact renames), and the subject where a
    rename-detection difference between gix and git would show.
  - S1 has the most rows.
  - S2 and S1 are the per-file-stats stress.
- **Single file:**
  - F7 is the literal shape.
  - F1 is the extreme, and the algorithm-sensitive one.
  - F3 is Myers's worst case relative to file size, in a `diff=rust` file whose
    hunk headers run the Rust pattern.
  - F6 is the huge-file, small-change case.
  - F9 is the huge output.

**What a ratio bar would mean, stated with the floor in mind:**

- git's numbers include a fixed 2.4 ms per `diff-tree` process (3.3). A worker
  holding the repository open does not pay it.
- Below about 10 ms, "N times git" mostly budgets the start-up cost git pays and
  Cairn does not. Subtracting the floor gives S7's file list about 5.5 ms of actual
  work.
- On the large subjects the floor is noise: under 1% of S1's 407 ms, for example.
- This hardware is fast: a 9800X3D, 60 GiB of RAM, and the whole pack in the page
  cache. An absolute ceiling recorded here would be generous on a laptop. A ratio
  to git measured on the same machine transfers between machines.

## Open questions for the planner

1. **Which diff algorithm does the bar hold both sides to?** git defaults to Myers.
   `gix-diff-api.md` records that gix's `Algorithm` enum defaults to `Histogram`,
   while an unset `diff.algorithm` resolves to Myers.
   - On F1 and F3, histogram is 5-7.5 times faster than git's Myers and reports
     up to 3.5 times fewer changed lines.
   - A ratio bar must fix the algorithm, or it compares different outputs.
   - The subject ranking in 2c is itself a Myers artefact.
2. **What does "file list" include?**
   - Paths and status only: git takes 8.2 ms on S7 and 82.0 ms at worst on M1.
   - With per-file +/− counts: 411 ms on S1, because every blob is read, even
     byte-identical renames.
   - Is the whole-commit patch built eagerly or per file on demand? git's whole
     patch misses 500 ms on S1 (728 ms) and S2 (530 ms).
3. **Must gix's renames match git's?**
   - git found 2,505 of M1's 2,543 inexact renames in a stage `-l` does not govern,
     and all of them keep their file names.
   - `gix-diff-api.md` records `Rewrites::limit` (default 1000, squared, falling
     back to exact renames only), and does not record a file-name stage.
   - If gix has no such stage, M1's leftover matrix (2,543 × 2,564 ≈ 6.5M, over
     1000² = 1M) would fall back to 231 exact renames plus 2,543 deletes and 2,564
     adds.
   - That needs measuring against gix before any rename-parity or file-list bar is
     written on M1. And is parity in the bar at all, or only time?
4. **Merges: which diff does the view show?** This report measured first-parent
   diffs only (M1-M3). `--cc` and `-m` were not measured, and M1's first-parent
   diff is 5,602 paths.
5. **Ratio or absolute ceiling, and on what hardware?**
   - The 2.4 ms process floor dominates every subject under 10 ms.
   - This machine is much faster than the median contributor laptop.
   - The previous A7 bar recorded no hardware beyond "16 cores". This one does
     (section 1).
6. **Warm only, or cold too?** Every number is warm, with the whole 974 MiB pack in
   cache. A cold number needs the page cache dropped (root) or a reboot, which is
   the user's action to take.
7. **Is memory in the bar, and measured how?** git's peak RSS runs up to 296 MiB
   (S2 per-file stats), and much of it is file-backed pack and `.idx` pages. gix
   also maps packs. Comparing RSS against RSS is like for like; comparing heap
   against RSS is not.
8. **Engine only, or through the window?** F9's patch is 9.9 MiB in 313,336 lines
   and S2's is 14.2 MiB in 331,864 lines. git's numbers stop at bytes written to
   `/dev/null`, and a bar through the UI measures something else.
9. **Long lines.** `tests/ui/parser/survive-peano-lesson-queue.rs` is 8.1 MB in only
   4,908 lines, the longest 6,013 B. That shape stresses rendering and any word or
   character diff, not the line diff. Is it a subject?
10. **The window and the pinned state.**
    - The subjects were taken from the last 2,000 first-parent commits, not the
      suggested 30,000 (which covers 2013 onward). Is that choice accepted?
    - A bar that names this clone should also pin its state: HEAD `c999cef531e`,
      one pack exactly as cloned, no commit-graph, no `gc` or maintenance run.
      A repack changes delta chains, and delta chains are what the per-file-stats
      and patch numbers spend their time inflating.

## Appendix A: scan commands

Four scans ran at once, 17:40:23-17:41:58. `H` is
`c999cef531ea9059e189e82fe0e82c5daf249bc9`. Their times are the cost of finding the
subjects and are not measurements for a bar.

**Whole history, non-merge commits, paths per commit (8.4 s):**

```
git -C "$R" log --no-merges --no-renames --name-only --format='@@%H %P' "$H" \
 | awk 'function flush(){ if(h!="") print n"\t"h"\t"k }
        /^@@/{ flush(); h=substr($1,3); k=(NF>1)?"has-parent":"root"; n=0; next }
        NF{n++} END{ flush() }' \
 | sort -t$'\t' -k1,1nr
```

**First-parent chain, each commit's first-parent diff, paths per commit (3.2 s):**

```
git -C "$R" log --first-parent --diff-merges=first-parent --no-renames --name-only \
    --format='@@%H%x09%P%x09%cs%x09%s' "$H" \
 | awk -F'\t' 'function flush(){ if(h!="") print n"\t"h"\t"np"\t"d"\t"sub_ }
        /^@@/{ flush(); h=substr($1,3); np=split($2,a," "); d=$3; sub_=$4; n=0; next }
        NF{n++} END{ flush() }' \
 | sort -t$'\t' -k1,1nr
```

**Line counts per file, keeping rows of 200 or more changed lines.** Run as
`RANGE="$H~30000..$H"` with `--no-merges` (94.9 s), and as `RANGE="$H"` with
`--first-parent --diff-merges=first-parent -n 2000` (7.0 s). The recent-window list
is the first run filtered to `git rev-list --no-merges $H~2000..$H`.

```
git -C "$R" log <extra args> --no-renames --raw --numstat --format='@@%H' $RANGE \
 | awk -F'\t' '
   /^@@/ { h=substr($1,3); delete st; next }
   /^:/  { split($1,f," "); st[$2]=f[5]; next }
   NF==3 { if ($1=="-") { bin++; next }
           tot=$1+$2; rows++;
           if (tot>=200) print tot"\t"$1"\t"$2"\t"st[$3]"\t"h"\t"$3 }
   END { print "#rows\t"rows"\t#binary\t"bin > "/dev/stderr" }' \
 | sort -t$'\t' -k1,1nr
```

**The rename-limit warning across every first-parent diff with 1,000 or more
paths.** The input rows are the scans above, filtered to 1,000 or more paths, with
merges limited to two parents:

```
git diff-tree -r -M --name-status --no-ext-diff --no-color "$c^1" "$c" 2>err \
 | awk -F'\t' '{s=substr($1,1,1); k[s]++; if($1=="R100") e++}
               END{printf "A=%d D=%d M=%d R=%d R100=%d", k["A"], k["D"], k["M"], k["R"], e}'
# then: is err empty?
```

**Rename limits** (2b), for each `l` in `"" -l0 -l1 -l10 -l40 -l100 -l300 -l1000`:

```
git diff-tree -r -M $l --name-status --no-ext-diff --no-color "$c^1" "$c"
```

**Largest text files at HEAD.** `git ls-tree -r -l "$H"`, sorted by size. For each
candidate: `git cat-file blob <oid> | head -c 8000 | tr -d -c '\000' | wc -c` (NUL
check), `git cat-file blob <oid> | wc -l`, and `git check-attr diff text binary --
<path>`.

## Appendix B: the timing harness, verbatim

Fed JSON lines of `{"label": ..., "argv": [...]}` on stdin; appends one JSON result
per command. Re-run: `python3 bench.py results.jsonl < specs.jsonl`.

```python
#!/usr/bin/env python3
"""Warm timing harness for git commands (no hyperfine / GNU time on this machine).

For each command: one CAPTURE run (untimed; counts stdout bytes and lines, keeps
stderr and the exit code), one discarded WARM-UP run to /dev/null, then N TIMED
runs with stdout and stderr to /dev/null. Wall time is perf_counter around
posix_spawn + wait4; user, sys and ru_maxrss come from wait4's rusage, which is
what GNU time's %U %S %M report.
"""
import json, os, statistics, sys, tempfile, time

RUNS = 5

def env():
    e = dict(os.environ)
    e["GIT_OPTIONAL_LOCKS"] = "0"   # never let status refresh-and-write the index
    e["LC_ALL"] = "C"
    for k in ("GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_EXTERNAL_DIFF", "GIT_PAGER", "PAGER"):
        e.pop(k, None)
    return e

def spawn_wait(argv, out_fd, err_fd):
    fa = [(os.POSIX_SPAWN_DUP2, out_fd, 1), (os.POSIX_SPAWN_DUP2, err_fd, 2)]
    t0 = time.perf_counter_ns()
    pid = os.posix_spawnp(argv[0], argv, env(), file_actions=fa)
    _, status, ru = os.wait4(pid, 0)
    t1 = time.perf_counter_ns()
    return {"wall_ms": (t1 - t0) / 1e6, "user_ms": ru.ru_utime * 1e3, "sys_ms": ru.ru_stime * 1e3,
            "maxrss_kib": ru.ru_maxrss, "exit": os.waitstatus_to_exitcode(status)}

def capture(argv):
    r, w = os.pipe()
    with tempfile.TemporaryFile() as errf:
        fa = [(os.POSIX_SPAWN_DUP2, w, 1), (os.POSIX_SPAWN_DUP2, errf.fileno(), 2), (os.POSIX_SPAWN_CLOSE, r)]
        pid = os.posix_spawnp(argv[0], argv, env(), file_actions=fa)
        os.close(w)
        nbytes = nlines = 0
        while True:
            chunk = os.read(r, 1 << 20)
            if not chunk:
                break
            nbytes += len(chunk)
            nlines += chunk.count(b"\n")
        os.close(r)
        _, status, ru = os.wait4(pid, 0)
        errf.seek(0)
        err = errf.read().decode("utf-8", "replace")
    return {"stdout_bytes": nbytes, "stdout_lines": nlines, "stderr": err,
            "exit": os.waitstatus_to_exitcode(status), "maxrss_kib": ru.ru_maxrss}

def measure(label, argv, runs=RUNS):
    cap = capture(argv)
    null = os.open(os.devnull, os.O_WRONLY)
    try:
        warm = spawn_wait(argv, null, null)
        timed = [spawn_wait(argv, null, null) for _ in range(runs)]
    finally:
        os.close(null)
    walls = [t["wall_ms"] for t in timed]
    res = {
        "label": label, "argv": argv, "runs": runs,
        "load_before": os.getloadavg()[0],
        "capture": cap, "warmup_wall_ms": warm["wall_ms"],
        "wall_ms": walls,
        "median_ms": statistics.median(walls), "min_ms": min(walls), "max_ms": max(walls),
        "mean_ms": statistics.fmean(walls),
        "user_ms_median": statistics.median(t["user_ms"] for t in timed),
        "sys_ms_median": statistics.median(t["sys_ms"] for t in timed),
        "peak_rss_mib": max(t["maxrss_kib"] for t in timed) / 1024,
        "exits": sorted({t["exit"] for t in timed}),
        "load_after": os.getloadavg()[0],
    }
    return res

if __name__ == "__main__":
    # stdin: JSON lines {"label": ..., "argv": [...]}; stdout: JSON lines of results
    out = open(sys.argv[1], "a")
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        spec = json.loads(line)
        res = measure(spec["label"], spec["argv"], spec.get("runs", RUNS))
        out.write(json.dumps(res) + "\n"); out.flush()
        print(f'{res["label"]}: median {res["median_ms"]:.1f} ms  max {res["max_ms"]:.1f} ms  '
              f'rss {res["peak_rss_mib"]:.1f} MiB  out {res["capture"]["stdout_bytes"]} B / {res["capture"]["stdout_lines"]} lines  '
              f'exit {res["exits"]}  stderr {res["capture"]["stderr"][:200]!r}', flush=True)
```

## Appendix C: every timed command, both passes

Wall times in milliseconds. "user / sys" is the median over pass 1's five timed
runs. stdout comes from pass 1's capture run. Every exit code in both passes was 0,
and no command wrote to stderr.

| Label | Median | Min | Max | Warm-up | Pass-2 median | Pass-2 max | user / sys | Peak RSS MiB | stdout bytes | stdout lines |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| S1 name-status --no-renames | 24.9 | 24.4 | 26.7 | 29.1 | 31.2 (first command of pass 2; section "Two passes") | 33.2 | 17.2 / 8.8 | 162.1 | 2,784,267 | 55,185 |
| S1 name-status -M | 28.6 | 27.1 | 28.8 | 29.1 | 28.8 | 30.4 | 19.2 / 9.1 | 164.6 | 2,811,859 | 27,593 |
| S1 numstat -M | 407.4 | 404.6 | 411.3 | 410.8 | 407.7 | 413.4 | 304.6 / 102.5 | 233.6 | 1,792,238 | 27,593 |
| S1 patch -M | 724.3 | 719.0 | 728.1 | 722.3 | 718.2 | 721.4 | 608.8 / 112.5 | 233.4 | 6,975,685 | 110,369 |
| S2 name-status --no-renames | 13.6 | 12.3 | 14.2 | 14.8 | 13.1 | 13.7 | 8.1 / 4.1 | 134.4 | 782,235 | 16,207 |
| S2 name-status -M | 13.3 | 12.6 | 14.2 | 13.8 | 13.8 | 14.2 | 9.8 / 4.2 | 134.5 | 782,235 | 16,207 |
| S2 numstat -M | 304.5 | 302.9 | 309.4 | 307.6 | 304.0 | 305.5 | 282.1 / 22.9 | 296.2 | 826,411 | 16,207 |
| S2 patch -M | 525.9 | 524.0 | 530.4 | 526.1 | 530.5 | 536.4 | 502.8 / 19.9 | 296.2 | 14,855,462 | 331,864 |
| S3 name-status --no-renames | 7.2 | 7.0 | 7.4 | 7.4 | 6.8 | 7.3 | 3.5 / 3.5 | 80.2 | 717,490 | 13,136 |
| S3 name-status -M | 10.6 | 10.4 | 11.1 | 10.9 | 10.4 | 11.0 | 6.7 / 3.8 | 96.9 | 724,041 | 6,585 |
| S3 numstat -M | 66.9 | 65.8 | 68.6 | 67.3 | 67.2 | 68.4 | 57.5 / 9.0 | 155.3 | 478,285 | 6,585 |
| S3 patch -M | 126.0 | 125.5 | 126.9 | 126.6 | 127.5 | 128.0 | 117.3 / 8.0 | 155.2 | 1,930,086 | 29,532 |
| S4 name-status --no-renames | 11.3 | 10.7 | 12.4 | 12.1 | 11.6 | 12.2 | 9.6 / 1.1 | 130.5 | 479,595 | 9,926 |
| S4 name-status -M | 11.6 | 11.1 | 12.2 | 11.5 | 11.5 | 12.7 | 9.5 / 2.2 | 130.5 | 479,595 | 9,926 |
| S4 numstat -M | 103.6 | 100.7 | 106.5 | 100.7 | 104.4 | 105.3 | 96.9 / 6.0 | 160.6 | 499,517 | 9,926 |
| S4 patch -M | 245.0 | 243.2 | 246.5 | 244.0 | 246.9 | 249.1 | 237.4 / 8.0 | 159.1 | 4,351,459 | 117,918 |
| S5 name-status --no-renames | 7.6 | 7.3 | 8.1 | 8.7 | 7.8 | 8.3 | 6.2 / 1.1 | 89.7 | 311,272 | 6,441 |
| S5 name-status -M | 8.3 | 7.8 | 8.4 | 7.7 | 8.8 | 9.0 | 4.2 / 4.1 | 93.0 | 314,486 | 3,227 |
| S5 numstat -M | 40.1 | 39.3 | 41.7 | 40.0 | 40.6 | 42.9 | 31.8 / 8.0 | 155.6 | 198,572 | 3,227 |
| S5 patch -M | 90.3 | 90.1 | 90.5 | 92.0 | 92.4 | 93.0 | 79.3 / 10.9 | 155.5 | 796,714 | 13,251 |
| S6 name-status --no-renames | 5.0 | 4.7 | 5.7 | 4.8 | 5.3 | 6.0 | 3.0 / 2.1 | 83.8 | 156,451 | 3,280 |
| S6 name-status -M | 6.6 | 6.4 | 6.8 | 6.9 | 6.9 | 7.3 | 3.6 / 2.7 | 94.7 | 158,044 | 1,687 |
| S6 numstat -M | 95.7 | 95.0 | 96.6 | 95.4 | 95.1 | 96.5 | 75.4 / 19.9 | 232.5 | 131,766 | 1,687 |
| S6 patch -M | 136.7 | 135.6 | 136.7 | 139.8 | 140.3 | 142.4 | 115.0 / 20.0 | 232.8 | 482,841 | 9,195 |
| S7 name-status --no-renames | 7.8 | 7.4 | 7.9 | 8.6 | 7.3 | 7.8 | 5.2 / 2.2 | 94.3 | 55,547 | 1,018 |
| S7 name-status -M | 7.9 | 7.2 | 8.2 | 7.5 | 7.2 | 8.1 | 5.7 / 2.0 | 94.4 | 55,547 | 1,018 |
| S7 numstat -M | 23.7 | 23.6 | 24.1 | 24.0 | 23.1 | 23.4 | 20.6 / 2.9 | 131.6 | 57,989 | 1,018 |
| S7 patch -M | 26.8 | 26.6 | 27.1 | 27.4 | 27.8 | 28.7 | 23.8 / 3.0 | 131.5 | 1,673,900 | 43,201 |
| S8 name-status --no-renames | 3.2 | 3.1 | 3.5 | 3.4 | 3.4 | 3.8 | 2.1 / 1.0 | 58.0 | 47,152 | 1,107 |
| S8 name-status -M | 5.1 | 4.7 | 5.4 | 5.0 | 5.1 | 5.4 | 3.5 / 1.2 | 72.0 | 47,154 | 1,105 |
| S8 numstat -M | 38.5 | 38.2 | 39.7 | 38.3 | 38.4 | 39.7 | 35.0 / 3.0 | 115.1 | 49,693 | 1,105 |
| S8 patch -M | 58.0 | 57.6 | 58.5 | 58.0 | 59.1 | 60.0 | 55.1 / 3.0 | 115.1 | 1,886,336 | 48,948 |
| M1 fp name-status --no-renames | 5.7 | 5.5 | 5.9 | 5.8 | 6.0 | 6.4 | 2.1 / 3.2 | 77.3 | 277,266 | 5,602 |
| M1 fp name-status -M | 78.5 | 76.6 | 82.0 | 82.6 | 80.0 | 81.3 | 60.0 / 16.9 | 166.5 | 280,040 | 2,828 |
| M1 fp numstat -M | 104.7 | 101.6 | 106.6 | 101.8 | 104.5 | 105.2 | 80.4 / 22.0 | 189.9 | 198,452 | 2,828 |
| M1 fp patch -M | 148.2 | 147.1 | 150.1 | 146.2 | 146.9 | 152.4 | 128.0 / 19.0 | 190.1 | 1,738,626 | 43,710 |
| M2 fp name-status --no-renames | 4.4 | 3.9 | 4.8 | 4.5 | 3.7 | 4.4 | 2.1 / 2.1 | 89.5 | 60,463 | 906 |
| M2 fp name-status -M | 9.9 | 9.6 | 10.3 | 10.0 | 9.9 | 10.6 | 7.0 / 3.0 | 97.4 | 60,473 | 896 |
| M2 fp numstat -M | 68.9 | 67.8 | 70.4 | 68.8 | 70.1 | 70.5 | 60.0 / 9.0 | 143.1 | 62,765 | 896 |
| M2 fp patch -M | 109.4 | 108.6 | 110.0 | 108.8 | 108.1 | 108.7 | 101.8 / 6.9 | 143.2 | 4,784,624 | 122,946 |
| M3 fp name-status --no-renames | 2.5 | 2.3 | 2.7 | 2.7 | 2.4 | 2.9 | 1.1 / 1.1 | 31.4 | 789 | 17 |
| M3 fp name-status -M | 2.5 | 2.3 | 2.6 | 2.4 | 2.3 | 2.5 | 1.2 / 1.1 | 31.4 | 789 | 17 |
| M3 fp numstat -M | 6.6 | 6.4 | 6.7 | 6.6 | 6.4 | 6.6 | 4.7 / 1.9 | 42.2 | 833 | 17 |
| M3 fp patch -M | 7.6 | 7.4 | 7.8 | 7.6 | 7.4 | 8.0 | 5.0 / 2.5 | 42.6 | 41,305 | 890 |
| F1 file patch (myers) | 163.0 | 161.7 | 164.6 | 162.7 | 163.8 | 164.7 | 160.7 / 2.0 | 38.1 | 5,918,514 | 334,692 |
| F1 file patch --histogram | 21.8 | 21.4 | 22.4 | 21.7 | 22.2 | 22.9 | 19.4 / 2.1 | 38.1 | 3,736,497 | 215,524 |
| F2 file patch (myers) | 12.4 | 12.3 | 13.3 | 12.2 | 12.3 | 12.7 | 11.6 / 1.0 | 25.9 | 6,831,701 | 158,429 |
| F3 file patch (myers) | 78.8 | 78.1 | 80.1 | 79.0 | 80.5 | 81.5 | 74.9 / 4.0 | 47.1 | 3,842,771 | 85,274 |
| F3 file patch --histogram | 15.8 | 15.5 | 16.5 | 15.5 | 16.3 | 17.5 | 12.2 / 3.3 | 47.4 | 892,474 | 21,763 |
| F4 file patch (myers) | 4.3 | 4.3 | 5.0 | 4.3 | 4.3 | 4.3 | 3.5 / 1.0 | 21.6 | 2,405,211 | 44,963 |
| F5 file patch (myers) | 3.5 | 3.4 | 3.7 | 3.7 | 3.4 | 3.6 | 3.4 / 0.0 | 22.1 | 1,230,209 | 23,862 |
| F6 file patch (myers) | 9.9 | 9.6 | 10.2 | 10.1 | 10.2 | 10.7 | 7.3 / 2.2 | 53.9 | 8,934 | 433 |
| F7 file patch (myers) | 9.8 | 9.6 | 10.1 | 9.7 | 9.7 | 10.0 | 7.3 / 2.4 | 30.2 | 598,115 | 13,735 |
| F7 file patch --histogram | 9.5 | 9.3 | 10.0 | 9.8 | 10.0 | 11.1 | 7.3 / 2.1 | 30.2 | 468,828 | 11,410 |
| F8 file patch (myers) | 3.1 | 3.1 | 3.2 | 3.2 | 3.2 | 3.3 | 2.1 / 1.0 | 22.3 | 376 | 15 |
| F9 file patch (myers) | 18.9 | 18.5 | 20.1 | 18.4 | 18.6 | 19.4 | 17.4 / 1.1 | 32.5 | 10,357,799 | 313,336 |
| W1 status --porcelain=v2 | 29.5 | 28.9 | 29.7 | 29.4 | 29.2 | 29.4 | 18.1 / 45.7 | 23.5 | 0 | 0 |

The three isolated re-runs of S1's file lists, ten runs each, gave these medians
and maxes:

| Run | `--no-renames --name-status` median / max | `-M --name-status` median / max |
| --- | --- | --- |
| 1 | 24.1 / 27.0 | 28.9 / 29.4 |
| 2 | 24.1 / 26.7 | 29.1 / 29.5 |
| 3 | 25.6 / 27.5 | 26.9 / 30.5 |
