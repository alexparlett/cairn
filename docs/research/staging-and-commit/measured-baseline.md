# git's own write costs on rust-lang/rust: the baseline for `staging-and-commit`'s C21

Research for the `staging-and-commit` packet, recorded 2026-10-08 in its phase 03.
`docs/prd/staging-and-commit.md` R13.2 asks for git's own time for each verb C21
bounds, plus one status read, measured before any Cairn number is taken, so that C21's
margin is written against git's numbers and not against Cairn's. This report measures
**`git` 2.56.0 itself, not Cairn**. It decides no margin; section 4 says what one would
mean, and the user decides.

## Headlines

- **git stages or unstages a one-line hunk in 17.7 ms** on rust-lang/rust's 62,892-entry
  index (`git apply --cached --whitespace=nowarn -`): the cost is reading and writing the
  index, not the patch.
- **git discards the same hunk in 0.7 ms** (`git apply --whitespace=nowarn -` on the
  working tree): it reads and writes one file and never touches the index.
- **git commits the staged hunk in 11.0-12.3 ms** (`git commit -q -F -`, no hooks, no
  signing).
- **One status read costs 23.7-25.1 ms** (`git status --porcelain=v2 -z` with
  `GIT_OPTIONAL_LOCKS=0`, one file edited), against a clean tree's 29.1 ms that
  `docs/research/refs-and-status/measured.md` records for the bench itself — the scratch
  clone's freshly written index and tmpfs make it a little cheaper.
- **So git's own time plus one status read is 41-43 ms to stage or unstage, 24-26 ms to
  discard and 35-37 ms to commit.** C21's margin is added to these.

## 1. Machine, repository, method

- **Machine:** AMD Ryzen 7 9800X3D (16 threads), 60 GiB, Linux 7.2.8-2-cachyos — the
  machine of `docs/research/diff-engine/measured-baseline.md` section 1.
- **Repository:** a plain, non-shared clone of `~/Development/bench/rust` on tmpfs
  (`/tmp`, `GIT_OPTIONAL_LOCKS=0 git clone --no-hardlinks --no-checkout`, then `git
  checkout --detach c999cef531e`): one 1.1 GiB pack, no alternates, 62,892 index entries.
  The bench itself was never written: `find ~/Development/bench/rust/.git -newer
  <marker>` listed nothing before the clone, after it, and after every run below.
- **git:** 2.56.0, the application's.
- **Reporter:** `write_baseline` (`crates/cairn-git/tests/diff/write_baseline.rs`),
  `#[ignore]`d, driven by `CAIRN_BENCH_SCRATCH_CLONE`, release build. It refuses the
  bench (`CAIRN_BENCH_REPO`), a shared clone, another commit and a dirty tree, and puts
  the clone back as it found it. Each command runs with an emptied environment — `PATH`,
  an empty `HOME`, `LC_ALL=C`, `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null`
  and an identity — and the status read alone with `GIT_OPTIONAL_LOCKS=0`, as Cairn reads
  it. Wall time around spawning, feeding stdin and reaping, as `measured-baseline.md` of
  `diff-engine` times it. One run to warm, then seven, median; the state each run starts
  from is restored between runs, untimed (the index's bytes, the file's bytes, or `git
  reset --soft HEAD^` after a commit).
- **Subject:** `library/core/src/option.rs` (3,061 lines), one line inserted at line
  1,501: one hunk. The patches are git's own — `git diff -- <file>` to stage, `git diff
  -R -- <file>` (what Cairn's emitter writes as the inverted diff, applied without `-R`)
  to unstage and to discard — which for one whole hunk is what the model emits.

## 2. The verbs

Three runs of the reporter, each the median of seven, in milliseconds:

| What | git | Run 1 | Run 2 | Run 3 |
| --- | --- | --- | --- | --- |
| Stage a hunk | `apply --cached --whitespace=nowarn -` | 17.79 | 17.79 | 17.65 |
| Unstage a hunk | `apply --cached --whitespace=nowarn -`, the inverted patch | 17.67 | 17.41 | 17.78 |
| Discard a hunk | `apply --whitespace=nowarn -`, the inverted patch | 0.71 | 0.74 | 0.71 |
| Commit | `commit -q -F -`, no hooks | 12.32 | 11.30 | 10.99 |
| One status read | `status --porcelain=v2 -z`, `GIT_OPTIONAL_LOCKS=0` | 25.09 | 23.90 | 23.72 |

Spread within a run (min to max of seven): stage 17.1-18.9, unstage 16.9-18.4, discard
0.66-1.29, commit 10.7-23.0 (one slow run in the first set; 10.7-12.5 in the others),
status 23.1-26.0.

## 3. git's own time plus one status read

What C21 adds its margin to (R13.2), from the three runs:

| Verb | Verb + status, ms |
| --- | --- |
| Stage a hunk | 41.4 - 42.9 |
| Unstage a hunk | 41.3 - 42.8 |
| Discard a hunk | 24.4 - 25.8 |
| Commit | 34.7 - 37.4 |

Not measured here, because they are Cairn's: the stale check before an apply (gix reads
the index file — the same 62,892 entries git reads), a discard's `git hash-object` and
its in-process hash of the file, the lock listings before and after, the hop to the local
write lane and back, the refresh's scheduling and the frame that draws the lists. C21's
margin is what those may cost.

## 4. What a margin would mean

Not a decision. An absolute margin keeps one budget for Cairn's own work whatever the verb;
a proportional one punishes the cheapest verb, discard, whose git time is under a
millisecond while Cairn's re-check (an index read, a `hash-object` process) does not
shrink with it. With a margin of 50 ms the bars would be about 93 ms to stage or unstage,
76 ms to discard and 87 ms to commit — each under 100 ms, the bar refs-and-status set for
a clean status (C11); a margin of 100 ms keeps each under 150 ms.
