# Progress — diff-engine

Running log, newest first. Historical record: entries are never retro-edited.
Correct course in a new entry.

## 2026-09-17 — phase 01: the diff model and the patch emitter

The model of R1 exists in `cairn-model` and nothing else does: no engine query, no
component, nothing that reads a repository. Eleven modules, all pure, described as
built in `docs/systems/diff.md`. `scripts/gate.sh` passes.

**Verified against real git, outside the suite.** Before QA, every non-empty patch
the emitter produces for the whole fixture corpus — 168 of them, over both the
with-ids and without-ids passes — was run through `git apply --check --cached` and
then `git apply --cached` in a scratch repository, and each result compared
byte-for-byte against the independent oracle. 168 pass, 0 fail. The harness was
deliberately NOT committed: `cairn-model` may not spawn a process, and C1, C2 and
C3 are phase 02's to own in `cairn-git`. This is evidence that phase 02 starts from
a working emitter, not a substitute for those criteria.

**Decisions taken without asking, any of which a reviewer might have taken
differently.**

- *The reference applier is public.* C2 lives in `cairn-git`, so the oracle has to
  cross a crate boundary, and a `#[cfg(test)]` item cannot. It is pure
  `(content, patch) -> bytes` and touches no repository, so it is not a route around
  D1; `docs/systems/diff.md` says so in as many words.
- *A path is a `RepoPath` over bytes, not a `String`.* git promises no encoding, and
  a lossy path in a patch header names a different file. The cost is a type the UI
  must read through.
- *`LineNumber` stores a zero-based index and offers `one_based()`* rather than the
  model carrying two coordinate systems. One type, one conversion point.
- *The emitter matches git where git is odd.* Keeping an unterminated last line as
  context while selecting the additions after it produces a file whose run-on line is
  in the middle. That is what `git apply` makes of the same patch, so the emitter
  does not second-guess it; the reverse-apply test skips those selections and says
  why, and a view offering line selection should keep the combination out of reach.
  Worth an issue when the packet files its deferred work.
- *The `index` line is omitted for a partial selection*, because the resulting blob
  is neither the old one nor the new one and a wrong id there is a lie a three-way
  apply could act on. Real git accepts the omission; verified.
- *`DiffLimits` carries R2.6's numbers* so the engine and the view read the same
  ceilings rather than each inventing them.
- *No new CLAUDE.md invariant was added for R1.7.* The type split is the enforcement
  and the `compile_fail` doctest is its twin, which is the strongest tier available;
  promoting it to a stated invariant with a guard belongs to the packet's QA phase,
  when the whole surface exists.

**QA.** `qa-checklist`, `test-coverage-auditor` and `responsiveness-reviewer` ran
fresh over the phase diff; `qa-confirm` adjudicated, in its own context. Twenty-six
raw findings, twenty-two confirmed after merging two duplicate pairs. Seven became
test changes, each watched go red on the mutation it names before being committed;
the rest became stated residuals in `docs/systems/diff.md` and in doc comments.

Findings DISMISSED, with the adjudicator's reasons:

- **R-7**, `old_content`/`new_content` allocate without `with_capacity`: no render-path
  caller exists or is planned, the work is inherently proportional to the content and
  runs off the UI thread, so the change would be speculative.
- **R-8**, `Hunk.changes` being a public `Range` invites an O(all changes) loop: that
  field IS the hunk's contract, and both the emitter and the row index iterate it
  legitimately. The cost is inherent to walking a hunk. `Hunks` itself is index-only
  by design and was explicitly not a finding.
- **TC-9(a)**, the four zero-change fixtures making round trips assert
  `content == content`: they exercise a real path — a header-only patch fed through
  the applier must leave the content byte-identical — which is a claim, not a
  tautology.
- **QC-5** in two of its three instances: `DiffContent` genuinely is `Text` or one of
  seven other states, and "four index entries" restates the name of the test that
  pins it, so neither can rot independently. The third instance was a real rotting
  count and was fixed.
- **QC-9**, the first commit's body running to six sentences rather than four: the
  content is why and not what, and the commit already had a child on a shared
  branch. Recorded for the packet's final QA rather than rewritten.

One confirmed finding was **fixed differently from the adjudication**. QC-7 asked
that `tests/diffs/mod.rs` drop its "unwrap and expect are denied here" header and
use `expect`, on the reading that `clippy.toml`'s test exemption covers the whole
integration-test crate. It does not: clippy at `-D warnings` refuses `expect` in a
helper outside a `#[test]` function, which the gate proved. The original spelling
stands and the header comment was corrected to say precisely why.

**Two changes of unknown provenance appeared in the working tree during QA**, and
both were discarded rather than committed. No reviewer reported writing either; all
three were dispatched read-only. Code nobody can account for authoring does not
belong on a branch other phases build on, however good it looks — but both are worth
weighing deliberately, so they are described here rather than just dropped:

- About a hundred lines adding `debug_assert!` preconditions to `TextDiff::new`, with
  tests, making a malformed changed-range set panic in debug rather than be drawn
  short. That is a real choice, and it is against the crate's existing "malformed
  input is placed rather than rejected" precedent, so it is the user's to take.
- `crates/cairn-model/tests/diff_row_addressing.rs`, about 260 lines, aimed squarely
  at the confirmed gap that `index_size` measures what was stored rather than what was
  built. It reasons that the decisive pin — counting allocations under a counting
  global allocator — cannot be written here, because `GlobalAlloc` is an unsafe trait
  and `unsafe_code = "forbid"` sits at the compiler tier, and that the alternative is a
  dependency on the seam crate, which is a user decision. Its substitute is a matcher
  over a roster of the functions on the lookup path, refusing allocating spellings in
  their bodies. That reasoning matches this session's own, and the approach is a
  reasonable answer to the gap; adopting it should be a considered call, not an
  accident of the working tree.

**What phase 02 should know.** The emitter omits the `index` line on a partial
selection and writes paths unquoted, both verified acceptable to `git apply`; a
mode-only change and a hundred-percent rename correctly carry no `index` line; the
reverse-apply property does not hold for a selection whose result is not a
well-formed file, and the condition is `diffs::is_a_well_formed_file`.

## 2026-09-17 — packet planned, over three rounds with the user

Six evidence records were gathered before any decision was taken, four in
parallel and two after the user asked for them: the engine and worker as built,
the UI and app as built, the gix diff API verified against the vendored 0.87.1
source, a cross-client precedent study, then a study of Fork's detail pane and
diff view, and git's own timings on a clone of rust-lang/rust made for the
purpose. All six are in `docs/research/diff-engine/` and outlive this directory.

Three rounds: options with recommendations, then detail where the user asked for
it (the hunk source, the working-tree read, the worker threading) plus a
Fork-based layout, then the lock. Sixteen decisions are recorded as L1-L16 in
`brainstorm.md`, with every rejected alternative and the evidence behind it.

Four decisions in the PRD were added while writing it rather than chosen by the
user, and are flagged as such in `brainstorm.md`: that a whitespace-ignoring view
announces what it hides (L4), the 64 MiB ceiling on loading an over-limit file
anyway (L10), that the no-literal-modifier convention becomes a guarded invariant
when the accelerator table exists (L14), and the split of the UI work into three
phases rather than two (L15).

Two facts set the shape more than any preference did. gix computes a diff but
renders no patch Cairn can hand to `git apply`, so the grouping, the headers and
the marker for a missing final newline are Cairn's to write (L3). And a
working-tree read has to run the user's clean filter driver to see what `git diff`
sees, which amends D1 (L6) — a process Cairn causes without building its
environment, stated as a residual rather than papered over.

Nine issues were filed for what the layout deliberately leaves out: #29 to #37.

No implementation has started.
