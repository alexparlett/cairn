# Progress — diff-engine

Running log, newest first. Historical record: entries are never retro-edited.
Correct course in a new entry.

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
