# gix diff API as linked: what `diff-engine` can lean on

Research for the `diff-engine` packet. Every claim below was read from the
vendored source under `~/.cargo/registry/src/index.crates.io-*/` for the versions
`Cargo.lock` pins today — `gix 0.87.1`, `gix-diff 0.67.1`, `gix-imara-diff 0.2.5`
(gitoxide's fork of `imara-diff`; there is no plain `imara-diff` in the lock),
`gix-status 0.34.1`, `gix-filter 0.34.0`, `gix-worktree 0.56.0`, `gix-object 0.64.1`,
`gix-index 0.55.0`, `gix-features 0.49.1`, `gix-zlib 0.1.0`. Where a fact could not be
read from source it is marked **OPEN**. Section 10 (git's patch format) cites the
`git 2.55.0` man pages installed on this machine and git's `add-patch.c` / `apply.c`
fetched from `github.com/git/git` at `master` on the day of writing.

Vendored paths below are relative to the registry `src/` directory. `docs.rs` was not
needed: the source was never thin on anything asked.

---

## 1. Tree-to-tree diff

### Entry points

`gix::Repository::diff_tree_to_tree` — `gix-0.87.1/src/repository/diff.rs`:

```rust
pub fn diff_tree_to_tree<'a, 'old_repo: 'a, 'new_repo: 'a>(
    &self,
    old_tree: impl Into<Option<&'a Tree<'old_repo>>>,
    new_tree: impl Into<Option<&'a Tree<'new_repo>>>,
    options: impl Into<Option<crate::diff::Options>>,
) -> Result<Vec<crate::object::tree::diff::ChangeDetached>, diff_tree_to_tree::Error>
```

`None` for either tree means the empty tree (`self.empty_tree()`), which is how a root
commit is diffed. Internally it builds a fresh `diff_resource_cache(Mode::ToGit, WorktreeRoots::default())`
PER CALL, then calls `gix_diff::tree_with_rewrites(..)` and collects `change.into_owned()`
into a `Vec`. That per-call cache creation is the cost the fluent API lets you avoid:

`gix::Tree::changes()` → `gix::object::tree::diff::Platform` (`gix-0.87.1/src/object/tree/diff/mod.rs`),
then `Platform::for_each_to_obtain_tree_with_cache` (`.../diff/for_each.rs`):

```rust
pub fn for_each_to_obtain_tree_with_cache<'new, E>(
    &mut self,
    other: &Tree<'new>,
    resource_cache: &mut gix_diff::blob::Platform,
    for_each: impl FnMut(Change<'_, 'old, 'new>) -> Result<Action, E>,
) -> Result<Option<gix_diff::rewrites::Outcome>, Error>
where
    E: Into<Box<dyn std::error::Error + Sync + Send + 'static>>,
```

`Action = std::ops::ControlFlow<()>`; `Break` stops the walk. The borrowed
`gix::object::tree::diff::Change<'a, 'old, 'new>` carries `Id<'repo>` (attached ids);
`Change::detach()` gives `ChangeDetached`, which is literally
`pub use gix_diff::tree_with_rewrites::Change as ChangeDetached;`.

Plumbing, if the `Repository`-level wrappers are ever in the way:
`gix_diff::tree_with_rewrites` (= `gix_diff::tree_with_rewrites::function::diff`,
`gix-diff-0.67.1/src/tree_with_rewrites/function.rs`):

```rust
pub fn diff<E>(
    lhs: TreeRefIter<'_>,
    rhs: TreeRefIter<'_>,
    resource_cache: &mut crate::blob::Platform,
    tree_diff_state: &mut crate::tree::State,
    objects: &impl gix_object::FindObjectOrHeader,
    for_each: impl FnMut(ChangeRef<'_>) -> Result<Action, E>,
    options: Options,
) -> Result<Option<rewrites::Outcome>, Error>
```

and the rename-free core `gix_diff::tree` (`gix-diff-0.67.1/src/tree/function.rs`), which is
breadth-first, single-threaded, allocation-free apart from `tree::State` (two object
buffers + a `VecDeque` of pending tree pairs; `Clone + Default`, reusable across calls).

Getting trees: `Repository::find_commit(id)?.tree()?` (`object/commit.rs`),
`Repository::find_tree`, `Repository::rev_parse_single("..")?.object()?.peel_to_tree()`
(`object/peel.rs`), `Repository::head_tree_id_or_empty()`, `Repository::empty_tree()`.

### What a change carries — `gix_diff::tree_with_rewrites::Change`

`gix-diff-0.67.1/src/tree_with_rewrites/change.rs`. `ChangeRef<'a>` borrows `location: &'a BStr`;
`Change` (owned) has `BString`. Four variants, verbatim field lists:

- `Addition { location, entry_mode: EntryMode, relation: Option<tree::visit::Relation>, id: ObjectId }`
- `Deletion { location, entry_mode, relation, id }`
- `Modification { location, previous_entry_mode, previous_id, entry_mode, id }` — note a
  mode-only change (blob ↔ executable, blob ↔ symlink) is a `Modification` whose ids may
  be equal.
- `Rewrite { source_location, source_entry_mode, source_relation, source_id, diff: Option<DiffLineStats>, entry_mode, id, location, relation, copy: bool }`
  — `copy == false` is a rename (source was a `Deletion`); `diff` is `None` when
  `source_id == id` (matched by identity, no similarity diff was run).

Accessors on both forms: `entry_mode()`, `entry_mode_and_id()`, `source_entry_mode_and_id()`,
`location()`, `source_location()`, `relation()`.

`gix_diff::tree::visit::Relation` (`src/tree/visit.rs`): `Parent(ChangeId)` /
`ChildOfParent(ChangeId)` — set on whole-directory additions/deletions so a UI can fold a
deleted tree. Directories DO appear as their own `Addition`/`Deletion` change with
`entry_mode.is_tree()` (the tracker's `try_push_change` special-cases `EntryKind::Tree`
with no relation), so the consumer must filter or nest them.

`gix_diff::blob::DiffLineStats` (`src/blob/mod.rs`): `removals: u32, insertions: u32, before: usize, after: usize, similarity: f32`.

### Ordering — NOT a single sorted sequence

Read from `tree_with_rewrites/function.rs` + `rewrites/tracker.rs`:

1. With rewrites enabled and `copies == None`, every `Modification` is emitted IMMEDIATELY,
   in breadth-first traversal order, as the walk meets it (`Tracker::try_push_change`
   returns it back: `if let (None, ChangeKind::Modification) = (self.rewrites.copies, change_kind) { return Some(change) }`).
2. Additions and deletions are buffered. After the walk, `Tracker::emit` matches
   rename pairs (sorted by id then location), emitting `Rewrite`s as found, then
   `self.items.sort_by(location)` and emits the leftover additions/deletions in path order.
3. With rewrites disabled (`Options.rewrites == None`) everything is emitted in
   breadth-first order — directory by directory, not depth-first path order.

Consequence: Cairn must sort the collected change set itself if it wants git's path
order (git sorts by path with the tree-entry comparison). Sorting a `Vec<Change>` by
`location()` is the obvious move; the `Rewrite` should sort by its destination.

### Options

`gix::diff::Options` (`gix-0.87.1/src/diff.rs`): private `location: Option<Location>`
(default `Some(Location::Path)`) and `rewrites: Option<gix_diff::Rewrites>`. Setters
`no_locations()`, `track_filename()`, `track_path()`, `track_rewrites(Option<Rewrites>)`,
builder `with_rewrites(..)`. `Options::from_configuration(&config)` is `pub(crate)` but is
what `Tree::changes()` and a `None` argument to `diff_tree_to_tree` use.

**Config IS read automatically**: `config.diff_renames()` → `gix::diff::new_rewrites(config, lenient)`
(`diff.rs`, `utils::new_rewrites_inner`) reads `diff.renames` (`true`/`false`/`copy`/`copies`
→ `rename::Tracking::{Renames, Disabled, RenamesAndCopies}`) and `diff.renameLimit`. If
`diff.renames` is unset, gix defaults to `Some(Rewrites::default())`, i.e. rename tracking
ON (matches `git diff`'s default). `Tree::changes()` doc: "By default, similar to `git diff`,
rename tracking will be enabled if it is not configured."

`gix_diff::Rewrites` (`gix-diff-0.67.1/src/lib.rs`):

```rust
pub struct Rewrites {
    pub copies: Option<rewrites::Copies>,
    pub percentage: Option<f32>,   // None = identity only; default Some(0.5)
    pub limit: usize,              // default 1000, squared for fuzzy matching; 0 = no limit
    pub track_empty: bool,         // default false
}
```

`rewrites::Copies { source: CopySource, percentage: Option<f32> }`, `CopySource::{FromSetOfModifiedFiles, FromSetOfModifiedFilesAndAllSources}`.
`rewrites::Outcome` reports `num_similarity_checks` and the two
`..._skipped_..._due_to_limit` counters, which is how a UI learns rename detection was
truncated. When the limit does not cover the whole N×M, gix "will trade in precision and
not run the fuzzy version of identity tests at all" (doc on `limit`) — results are
never partial, but they are exact-match-only.

Rename tracking requires the resource cache to be in `Mode::ToGit` to match git
(doc on `tree_with_rewrites`), and `Tracker::emit` forces
`diff_cache.options.skip_internal_diff_if_external_is_configured = false`.
Similarity diffs inside the tracker use the cache's algorithm; a link or a submodule
entry is only ever matched by identity (`find_match`: `item_mode.is_link() || item_mode.is_commit()` → exact).

---

## 2. Blob-to-blob diff and hunks

### The re-export

`gix_diff::blob` (`gix-diff-0.67.1/src/blob/mod.rs`) is `pub use imara_diff::*;` over
`gix-imara-diff 0.2.5` plus gix's own `pipeline`, `platform`, `unified_diff`,
`Driver`, `Pipeline`, `Platform`, `DiffLineStats`, `ResourceKind`, and
`diff_with_slider_heuristics`. `gix::diff` is `pub use gix_diff::*`, so everything
is reachable as `gix::diff::blob::..`.

The `unified_diff` cargo feature of `gix-imara-diff` is NOT enabled by `gix-diff`
(its `[dependencies.imara-diff]` block has no `features`), so imara's own
`Diff::unified_diff`, `BasicLineDiffPrinter`, `UnifiedDiffConfig`, `UnifiedDiffPrinter`
are NOT compiled. The only unified-diff renderer linked is gix-diff's
`gix_diff::blob::UnifiedDiff` (section below).

### Algorithm — `gix_diff::blob::Algorithm` (`gix-imara-diff-0.2.5/src/lib.rs`)

```rust
pub enum Algorithm { #[default] Histogram, Myers, MyersMinimal }
```

`diff.algorithm` IS honoured: `gix::Repository::diff_algorithm()` (`repository/config/mod.rs`)
→ `config::tree::Diff::ALGORITHM.try_into_algorithm` (`config/tree/sections/diff.rs`):
`myers`/`default` → `Myers`, `minimal` → `MyersMinimal`, `histogram` → `Histogram`,
`patience` → `Error::Unimplemented` (falls back to `Histogram` only when the repo was
opened lenient). Unset defaults to **`myers`** (`with_default(b"myers")`), matching git.
`diff.<driver>.algorithm` overrides per path via `Driver::algorithm`; the resolution
order is documented on `prepare_diff::Operation::InternalDiff`: driver → platform
option → `Algorithm::default()`.

### Tokens — `TokenSource`, `InternedInput`, `Interner` (`gix-imara-diff-0.2.5/src/intern.rs`)

```rust
pub trait TokenSource {
    type Token: Hash + Eq;
    type Tokenizer: Iterator<Item = Self::Token>;
    fn tokenize(&self) -> Self::Tokenizer;
    fn estimate_tokens(&self) -> u32;
}
pub struct InternedInput<T> { pub before: Vec<Token>, pub after: Vec<Token>, pub interner: Interner<T> }
impl<T: Eq + Hash> InternedInput<T> {
    pub fn new<I: TokenSource<Token = T>>(before: I, after: I) -> Self
    pub fn update_before(&mut self, input: impl Iterator<Item = T>)
    pub fn update_after(&mut self, input: impl Iterator<Item = T>)
    pub fn clear(&mut self)
}
impl<T> Index<Token> for Interner<T> { type Output = T; }
pub struct Token(pub u32);
```

The token type is arbitrary (`T: Hash + Eq`): lines, words, chars, or Cairn's own
normalised-line newtype all work. `update_before/after` take a plain iterator, so
no `TokenSource` impl is even required. `Diff::compute_with(algorithm, &[Token], &[Token], num_tokens)`
bypasses interning entirely. `Diff::compute_with` asserts both sides `< i32::MAX` tokens.

Sources (`gix-imara-diff-0.2.5/src/sources.rs`): `lines(&str)`, `bstr_lines(&BStr)`,
`byte_lines(&[u8])` — all KEEP the newline in the token (a CRLF↔LF change or a missing
final newline is therefore a change); `words(&str)`. `&str`, `&BStr` and `&[u8]`
implement `TokenSource` as lines-with-terminator. There is NO `lines_with_terminator`
symbol; `byte_lines` is it. The terminator-STRIPPING source lives in gix-diff:
`gix_diff::blob::platform::resource::ByteLinesWithoutTerminator` (strips `\n` then a
preceding `\r`), reached through `Resource::intern_source_strip_newline_separators()`;
`Resource::intern_source()` gives `byte_lines` (keeps terminators).

### The raw hunk structure — `Diff` and `Hunk` (`gix-imara-diff-0.2.5/src/lib.rs`)

```rust
pub struct Diff { removed: Vec<bool>, added: Vec<bool> }   // private, one flag per token per side
impl Diff {
    pub fn compute<T>(algorithm: Algorithm, input: &InternedInput<T>) -> Diff
    pub fn compute_with(&mut self, algorithm: Algorithm, before: &[Token], after: &[Token], num_tokens: u32)
    pub fn count_additions(&self) -> u32
    pub fn count_removals(&self) -> u32
    pub fn is_removed(&self, token_idx: u32) -> bool
    pub fn is_added(&self, token_idx: u32) -> bool
    pub fn postprocess_no_heuristic<T>(&mut self, input: &InternedInput<T>)
    pub fn postprocess_lines<T: AsRef<[u8]>>(&mut self, input: &InternedInput<T>)  // indent heuristic, git's
    pub fn hunks(&self) -> HunkIter<'_>
}
pub struct Hunk { pub before: Range<u32>, pub after: Range<u32> }
impl Hunk { pub fn invert(&self) -> Hunk; pub fn is_pure_insertion(&self) -> bool; pub fn is_pure_removal(&self) -> bool; .. }
```

`HunkIter` yields hunks in monotonically increasing token order. Each `Hunk` is one
contiguous change: token indices `before` were replaced by token indices `after`
(either may be empty). This is the primitive Cairn's patch model builds on — no text
involved; context is whatever Cairn chooses to slice from `input.before`/`input.after`
around it. `gix_diff::blob::diff_with_slider_heuristics(algorithm, &input)` =
`Diff::compute` + `postprocess_lines`, which is what `git diff` does by default
(`--indent-heuristic`).

`gix::object::blob::diff::Platform::lines(process_hunk)` (`gix-0.87.1/src/object/blob.rs`)
is the same thing pre-chewed: it calls `prepare_diff()`, `interned_input()`,
`diff_with_slider_heuristics`, and hands each hunk as
`lines::Change::{Addition{lines}, Deletion{lines}, Modification{lines_before, lines_after}}`
with `&[&BStr]` (terminators stripped). It gives NO line numbers, so Cairn wants the raw
`Diff::hunks()` instead. `Platform::line_counts()` → `Option<DiffLineStats>` (`None` if binary).

### Unified diff in gix-diff 0.67.1 — `gix_diff::blob::unified_diff` (`src/blob/unified_diff/{mod,impls}.rs`)

Exists. `gix_diff::blob::UnifiedDiff::new(diff: &Diff, input: &InternedInput<T>, consume_hunk: D, context_size: ContextSize)`
then `.consume() -> io::Result<D::Out>`. `ContextSize::symmetrical(n)`, default 3.
Trait `ConsumeHunk { type Out; fn consume_hunk(&mut self, header: HunkHeader, lines: &[(DiffLineKind, &[u8])]) -> io::Result<()>; fn finish(self) -> Out }`.
`HunkHeader { before_hunk_start: u32 (1-based), before_hunk_len, after_hunk_start (1-based), after_hunk_len }`
with `Display` = `"@@ -{},{} +{},{} @@"` — ALWAYS four numbers (git omits `,1`; git
accepts both). `DiffLineKind::{Context, Add, Remove}`, `to_prefix()` → `' '`/`'+'`/`'-'`.
`ConsumeBinaryHunk::new(delegate, newline: &str)` renders to `String`/`Vec<u8>`/`BString`.

What it does NOT do:
- **No `\ No newline at end of file` marker.** `ConsumeBinaryHunk::consume_hunk` appends
  `newline` to any line that does not already end with it. With the terminator-stripped
  input (which `prepare_diff::Outcome::interned_input()` always produces) every line
  gets a newline appended, so the missing-final-newline fact is lost entirely.
- No function-context (`xfuncname`) in the header: nothing after the second `@@`.
- No `diff --git` / `index` / mode headers; hunks only.
- Hunk merging: hunks are merged when `before.start - ctx_pos > 2 * ctx_size` is false
  (i.e. within `2 × context` lines), which is git's rule; no `interhunk-lines` knob
  (crate-status: `[ ] merge hunks that are close enough based on line-setting`).
- `diff.context` is not read by gix (no such key in `config/tree/sections/diff.rs`).

So the renderer is a reference for the header arithmetic and merging rule, not a patch
emitter Cairn can hand to `git apply` as-is. Build the patch from `Diff::hunks()` +
terminator-keeping tokens (or a per-line "had terminator" bit) so the marker can be
emitted (section 10).

---

## 3. Whitespace options

**None.** `grep -rni whitespace` over `gix-imara-diff-0.2.5/src` and `gix-diff-0.67.1/src`
hits only the slider heuristic's "line is blank" classification. There is no
`--ignore-space-change` / `--ignore-all-space` / `--ignore-blank-lines` equivalent
anywhere in the linked crates, and gitoxide's `crate-status.md` lists for gix-diff
`[ ] white-space related settings`.

The standard approach — and therefore **a Cairn-side token transformation**: since the
diff runs over `Hash + Eq` tokens, whitespace-insensitivity is a tokenizer that maps
each original line to a normalised key (trim trailing, collapse runs, strip all, drop
blank lines) and interns THAT, while keeping a side table from token position back to
the original line. `InternedInput::update_before(iter)` accepts any iterator, so:
`before: Vec<&[u8]>` originals, `keys.iter().map(normalise)` interned. `Hunk.before`/`after`
ranges then index the ORIGINAL line vectors one-to-one, so mapping back is free —
provided the transformation is one-token-per-line. `--ignore-blank-lines` (dropping
lines) breaks the one-to-one mapping; git implements it as a post-pass, and Cairn would
need either a position remap or to keep blank lines as tokens with a special key that
compares equal on both sides (which is what git effectively does: it marks the lines
and skips them when they are the only change). Recorded as design work, not a library gap
to wait on.

---

## 4. Word-level intra-line diff

gix-diff provides nothing itself. `gix-imara-diff 0.2.5` provides:

- `sources::words(&str) -> Words<'_>` — a word is a run of `char::is_alphanumeric` or
  `_`, or a run of `' '`, or any single other char (`sources.rs`, `Words::next`). `&str` only.
- `Hunk::latin_word_diff(&self, input: &InternedInput<&'a str>, word_tokens: &mut InternedInput<&'a str>, diff: &mut Diff)`
  (`lib.rs`) — re-tokenises the hunk's before/after lines with `words`, runs
  `Algorithm::Myers` + `postprocess_no_heuristic`, fills the reusable `diff`. Only for
  `&str` inputs (UTF-8), so a `&[u8]` line diff needs `str::from_utf8` first or a
  Cairn-side byte tokenizer.

Confirmed: `InternedInput` accepts arbitrary tokens (`TokenSource::Token: Hash + Eq`,
cited in section 2), so a per-changed-line-pair diff over Cairn's own word or
char tokenization is straightforward, and `Algorithm` doc explicitly recommends
`Myers` over `Histogram` for character diffs ("**character diffs do not work well**"
with Histogram; it detects and falls back, but at a cost).

---

## 5. Worktree and index diffs (staged / unstaged)

### The status platform — `gix::Repository::status(progress)` (`gix-0.87.1/src/status/mod.rs`)

Returns `gix::status::Platform<'repo, P>`. Builders (`status/platform.rs`):
`dirwalk_options(..)`, `untracked_files(UntrackedFiles::{None, Collapsed, Files})`,
`should_interrupt_shared(&'static AtomicBool)`, `should_interrupt_owned(Arc<AtomicBool>)`,
`index_worktree_submodules(..)`, `index(IndexPersistedOrInMemory)`,
`index_worktree_rewrites(Option<Rewrites>)`, `index_worktree_options_mut(..)`,
`head_tree(ObjectId)`, `tree_index_track_renames(TrackRenames::{AsConfigured, Given(Rewrites), Disabled})`.
`status.showUntrackedFiles` is read for the default.

Two iterators:
- `Platform::into_iter(patterns) -> Result<gix::status::Iter, into_iter::Error>` —
  BOTH tree-vs-index and index-vs-worktree (`status/iter/mod.rs`). Items are
  `gix::status::Item::{IndexWorktree(index_worktree::Item), TreeIndex(gix_diff::index::Change)}`
  (`status/iter/types.rs`). Documented **undefined ordering**: with `parallel`, three
  producers (dirwalk, index-vs-worktree entry checks, tree-vs-index) run at once on
  two spawned threads (`gix::status::tree_index::producer`, `gix::status::index_worktree::producer`)
  and the consumer receives over an `mpsc` channel. `Iter::next` uses
  `rx.recv_timeout(25ms)` and polls `should_interrupt` on each timeout.
- `Platform::into_index_worktree_iter(patterns) -> index_worktree::Iter` — worktree only
  (`head_tree = None`), items `index_worktree::Item`.

The iterator's `Outcome` (`into_outcome()`) has `write_changes()` to persist refreshed
stat data back to `.git/index` — a WRITE, so under Cairn's rules it can only be
called from `cairn-git::ops`, and probably never (git itself does this on `status`;
skipping it just costs repeated hashing of racily-clean entries).

Lower-level, no threads spawned by gix: `Repository::tree_index_status(tree_id, &worktree_index, pathspec, TrackRenames, cb)`
(`status/tree_index.rs`) and `Repository::index_worktree_status(&index, patterns, delegate, compare, submodule, progress, should_interrupt, options)`
(`status/index_worktree.rs`).

### Staged: HEAD tree vs index — `gix_diff::index::Change`

`tree_index_status` converts the tree to an in-memory index (`index_from_tree`) and
runs `gix_diff::index(..)`. `gix_diff::index::ChangeRef<'lhs, 'rhs>` (`gix-diff-0.67.1/src/index/mod.rs`):
`Addition { location: Cow<BStr>, index: usize, entry_mode: gix_index::entry::Mode, id: Cow<oid> }`,
`Deletion { .. }`, `Modification { location, previous_index, previous_entry_mode, previous_id, index, entry_mode, id }`,
`Rewrite { source_location, source_index, source_entry_mode, source_id, location, index, entry_mode, id, copy }`.
`Change = ChangeRef<'static, 'static>`. Errors: `IsSparse` ("Cannot diff indices that
contain sparse entries") and `LhsHasUnmerged`. Rename tracking follows
`status.renames` then `diff.renames`, defaulting ON.

Modes here are `gix_index::entry::Mode` bitflags (`FILE`, `FILE_EXECUTABLE`, `SYMLINK`, `COMMIT`, `DIR`),
not `gix_object::tree::EntryMode`.

Content for a staged diff: both sides are blob ids (the index entry's `id` and the
HEAD tree entry's id), so the resource cache in `Mode::ToGit` with no worktree roots
reads both from the ODB unfiltered — exactly what `git diff --cached` compares.

### Unstaged: index vs worktree — `index_worktree::Item`

(`status/index_worktree.rs`)
- `Item::Modification { entry: gix_index::Entry, entry_index, rela_path: BString, status: EntryStatus<(), submodule::Status> }`
- `Item::DirectoryContents { entry: gix_dir::Entry, collapsed_directory_status }` — untracked/ignored
- `Item::Rewrite { source: RewriteSource, dirwalk_entry, dirwalk_entry_id, diff: Option<DiffLineStats>, copy, .. }`

`gix_status::index_as_worktree::EntryStatus<T, U>` (`gix-status-0.34.1/src/index_as_worktree/types.rs`):
`Conflict { summary: Conflict, entries: Box<[Option<ConflictIndexEntry>; 3]> }`,
`Change(Change<T, U>)`, `NeedsUpdate(Stat)`, `IntentToAdd`.
`gix_status::index_as_worktree::Change<T, U>`:
`Removed`, `Type { worktree_mode }` (deviation: git calls a type change a modification),
`Modification { executable_bit_changed: bool, content_change: Option<T>, set_entry_stat_size_zero: bool }`,
`SubmoduleModification(U)`. With the `FastEq` comparer the platform uses, `T = ()`:
you learn THAT content changed, not what.

`Item::summary()` → `Summary::{Removed, TypeChange, Modified, Conflict, IntentToAdd, Added, Copied, Renamed}` or `None` for `NeedsUpdate`.

### Getting the CONTENT of a worktree file, filtered to-git

Two verified routes; the first is the one the diff wants.

**(a) The resource cache with a worktree root**, which is what `index_worktree_status`
itself builds for rename similarity (`status/index_worktree.rs`):

```rust
let resource_cache = crate::diff::resource_cache(
    self,
    gix_diff::blob::pipeline::Mode::ToGit,
    attrs_and_excludes.inner,
    gix_diff::blob::pipeline::WorktreeRoots { old_root: None, new_root: Some(workdir.to_owned()) },
)?;
```

Public equivalent: `Repository::diff_resource_cache(Mode::ToGit, WorktreeRoots { old_root: None, new_root: Some(workdir) })`
(`repository/diff.rs`). Then per file:
`platform.set_resource(index_entry.id, EntryKind::Blob, rela_path, ResourceKind::OldOrSource, &repo.objects)`
(read from ODB) and
`platform.set_resource(ObjectId::null(kind), EntryKind::{Blob|BlobExecutable|Link}, rela_path, ResourceKind::NewOrDestination, &repo.objects)`
(read from `new_root/rela_path`). `Pipeline::convert_to_diffable` (`blob/pipeline.rs`)
for a root-backed resource in a `to_git()` mode opens the file and runs
`self.worktree_filter.convert_to_git(file, rela_path, attributes, ..)` — the full
gix-filter clean pipeline (eol/autocrlf, ident, working-tree-encoding, `filter.<driver>.clean`
/ long-running `process`), driven by `.gitattributes` through the attribute stack
gix built with `Source::WorktreeThenIdMapping` because a root is set. That is the
to-git form: what `git diff` shows and what `git apply --cached` expects the
preimage to be. A symlink resource gets its link target as the buffer. Mode is
trusted (`convert_to_diffable` doc: "we will not re-validate that the entry in the worktree actually is of that mode") — take it from the status item.
`Data::Missing` if the file is gone; `Data::Binary { size }` per section 6.

**(b) `gix::filter::Pipeline`** — `Repository::filter_pipeline(tree_if_bare) -> (filter::Pipeline<'repo>, IndexPersistedOrInMemory)`
(`repository/filter.rs`, `filter.rs`) with `convert_to_git(src: impl Read, rela_path: &Path, index: &gix_index::State) -> ToGitOutcome`
(`Unchanged(R)` / `Buffer(&[u8])` / `Process(Read)`), `convert_to_worktree(..)`,
and `worktree_file_to_object(rela_path, index)` — the latter WRITES a blob (`repo.write_blob`),
so it is `ops`-only.

`Pipeline::options(repo)` reads `core.autocrlf`, `core.eol`, `core.safecrlf`,
`core.checkRoundtripEncoding` and every `filter.<name>.{clean,smudge,process,required}`.
A `process`/`clean` filter is a subprocess gix spawns — a READ path that spawns.
Cairn's D1 note "reads never spawn a process" is therefore not strictly true for a
repository using LFS or another filter driver (gix runs the user's clean filter to
compute the to-git form, exactly as `git diff` would). Flag for the planner.

### What gix-status honours and what it does not (program O2 overlaps; only the content path is this packet's)

Read from `gix-status-0.34.1/src/index_as_worktree/function.rs`:
- Entries flagged `UPTODATE | SKIP_WORKTREE | ASSUME_VALID | FSMONITOR_VALID` are skipped
  outright (line ~280). So **sparse checkout** (skip-worktree) entries are respected by
  being skipped, and the `FSMONITOR_VALID` bit, if a previous `git status` set it, is
  honoured. gix **never invokes `core.fsmonitor`** (no `fsmonitor` mention in gix-status
  or gix outside the init hook template; crate-status: `[ ] support for fs-monitor for modification checks`),
  so the bit is only as fresh as git last left it.
- `.gitattributes` are read (`attr_stack` with `WorktreeThenIdMapping`); the
  `stream_worktree_file` path runs `filter.convert_to_git` before hashing, so a CRLF
  file with `text=auto` is NOT reported modified — same as git.
- `core.symlinks`, `core.fsCache` (Windows), `core.precomposeUnicode`/`ignore_case`
  via `fs_capabilities`, `index.skipHash`, `status.showUntrackedFiles`, `diff.ignoreSubmodules`
  and per-submodule `ignore` are read. Racy-git handling: `FastEq` re-hashes when
  `stat.size == 0`, `set_entry_stat_size_zero` requests the fix-up.
- Sparse INDEX (not sparse checkout) is unsupported: `gix_diff::index::Error::IsSparse`.
  Split index: crate-status `[ ] sparse-index and split-index aware status acceleration`;
  whether gix-index 0.55 loads a split index at all is **OPEN** (not needed by this packet).
- Untracked cache extension: **OPEN**, not checked.

---

## 6. Binary detection and large files

`gix_diff::blob::pipeline::Options { large_file_threshold_bytes: u64, fs: gix_fs::Capabilities }`,
filled by `config.diff_pipeline_options()` from `core.bigFileThreshold` with default
`512 * 1024 * 1024` (`config/cache/access.rs::big_file_threshold`). `0` disables.

Decision in `Pipeline::convert_to_diffable` (`blob/pipeline.rs`), in this order:
1. The `diff` attribute at the path: unset (`-diff`) → binary; set to a driver name →
   that `Driver`'s `is_binary` (`diff.<driver>.binary` = true/false/`auto`→`None`), but
   a driver WITH `textconv` is never treated as binary by flag.
2. ODB resource: `objects.try_header(id)` gives the size without decompressing; if
   `is_binary` is still undecided and `size > threshold` → `Data::Binary { size }` and the
   object is never read. Worktree resource: `metadata().len()` likewise.
3. Otherwise the buffer is loaded and, unless the driver/attribute decided,
   `is_binary_buf`: a NUL byte within the first 8000 bytes (`buf[..len.min(8000)].contains(&0)`) — git's heuristic.

Result type `gix_diff::blob::platform::resource::Data<'a>::{Missing, Buffer { buf, is_derived }, Binary { size }}`
on `platform::Resource { driver_index, data, mode: EntryKind, rela_path, id }`.
`Platform::prepare_diff()` returns `Operation::SourceOrDestinationIsBinary` when either
side is `Binary` and no diff is run; for `Data::Binary` the buffer is cleared, so the
bytes are not even available to render "Binary files differ" with sizes — only `size` is.

Drivers: `gix_diff::blob::Driver { name, command: Option<BString>, algorithm: Option<Algorithm>, binary_to_text_command: Option<BString>, is_binary: Option<bool> }`,
read by `config.diff_drivers()` from `diff.<driver>.{command, textconv, algorithm, binary}`
(`config/cache/access.rs`). So gix READS `diff.<driver>.command` (external diff) and
`textconv`; it IGNORES `xfuncname`/`funcname`, `cachetextconv`, `wordRegex`. `diff.external`
is a known key but the platform never runs it — `prepare_diff_command()` prepares a
`std::process::Command` for the caller (with two tempfiles); `Platform::lines()` forces
`skip_internal_diff_if_external_is_configured = false` and never spawns.

**textconv runs a subprocess and is applied automatically** in `Mode::ToWorktreeAndBinaryToText`
and `ToGitUnlessBinaryToTextIsPresent` (`run_cmd(..)` in `pipeline.rs`, via a shell,
`gix_command::prepare(..).with_shell()`), never in `Mode::ToGit`. For Cairn's "reads
never spawn" rule, `Mode::ToGit` is the mode that guarantees no textconv process; it
also matches git's rename detection. `Data::Buffer { is_derived: true }` flags textconv
output so a UI never offers to stage it.

`gix::Repository::diff_resource_cache(mode, worktree_roots)` — exists, signature in
section 1. `gix_diff::blob::pipeline::Mode` (`blob/pipeline.rs`):

```rust
pub enum Mode { #[default] ToWorktreeAndBinaryToText, ToGitUnlessBinaryToTextIsPresent, ToGit }
```

Note the DEFAULT is `ToWorktreeAndBinaryToText`, which for an ODB blob runs the
smudge side (`convert_to_worktree`) — that is NOT what `git diff` shows. Use `ToGit`.
`Repository::diff_resource_cache_for_tree_diff()` is the `ToGit` + no-roots shorthand.

Cache limits: there are none. `Platform.diff_cache: HashMap<CacheKey, CacheValue>` grows
with every `set_resource` until `clear_resource_cache()` or
`clear_resource_cache_keep_allocation()` (the latter keeps buffers on a free-list;
documented sizing rule in `platform.rs`). Keys are by id (+ is-link) for ODB resources
and by `rela_path` for worktree resources, so **a worktree file that changes on disk is
NOT re-read while cached** ("this also has to be called if the same resource is going
to be diffed in different states"). Every worktree diff query should therefore start
by clearing.

`Platform::set_resource` refuses `EntryKind::{Tree, Commit}` (`set_resource::Error::InvalidMode`).

---

## 7. Submodules and symlinks in a tree diff

`gix_object::tree::EntryKind` (`gix-object-0.64.1/src/tree/mod.rs`, `#[repr(u16)]`):
`Tree = 0o040000, Blob = 0o100644, BlobExecutable = 0o100755, Link = 0o120000, Commit = 0o160000`.
`EntryMode { internal: u16 }` is the raw mode; `EntryMode::kind() -> EntryKind`,
`is_tree/is_commit/is_link/is_blob/is_executable/is_blob_or_symlink/is_no_tree`,
`as_octal_str() -> &'static BStr` (the six-digit form for patch headers),
`TryFrom<u32>`. `From<EntryKind> for EntryMode` and back.

In a tree diff a submodule appears as an `Addition`/`Deletion`/`Modification` whose
`entry_mode.is_commit()`; a symlink as `is_link()`. A symlink CAN be handed to the
blob platform (its target string becomes the buffer, both from ODB and from disk), a
gitlink cannot (`InvalidMode`), and `Tracker` matches links and gitlinks by identity
only. Type changes (file → symlink) come as a `Modification` with differing modes; git
renders those as a delete + add pair in patch text, which Cairn's model must do too.
Index-side modes for the staged/unstaged diffs are `gix_index::entry::Mode` bitflags
(section 5).

---

## 8. Threading

Compile-checked in a scratch crate against the locked versions (`cargo check` passed
with `fn assert_send<T: Send>()` instantiated for each): **`Send`**:
`gix::diff::blob::Platform`, `gix::diff::blob::Pipeline`, `gix::diff::tree::State`,
`gix::diff::blob::Diff`, `gix::diff::blob::InternedInput<&[u8]>`, `gix::diff::blob::Interner<&[u8]>`,
`gix::diff::tree_with_rewrites::Change`, `gix::diff::Rewrites`, `gix::status::Item`,
`gix::status::Iter`, `gix::status::index_worktree::Iter`, `gix::Repository`,
`gix::ThreadSafeRepository` (also `Sync`), `gix::worktree::IndexPersistedOrInMemory`,
`gix::filter::plumbing::Pipeline`, `gix::worktree::Stack`. `Repository` is `!Sync`
(D3's one-handle-per-worker already assumes this).

So a `gix_diff::blob::Platform` (resource cache) can be created once by the worker that
owns the thread-local `Repository` handle and reused across queries — it holds no
repository borrow (it takes `&repo.objects` per `set_resource` call), only a
`gix_worktree::Stack` snapshot of attributes and a `gix_filter::Pipeline` (which may
hold running long-lived filter processes: `driver::State.running: HashMap<BString, process::Client>`).
Two caveats: it is keyed by path for worktree resources (section 6: clear per worktree
query), and it was built from the index/attributes at construction time
(`diff_resource_cache` doc: "attributes will always be obtained from the current `HEAD`
index"), so a `.gitattributes` edit is not seen until it is rebuilt. `gix::Tree<'repo>`
and the fluent `object::tree::diff::Platform<'a, 'repo>` borrow the repository and are
not for crossing threads; detach (`Change::detach()` / `into_owned()`) before sending.

**Cancellation.**
- Tree diff: cooperative, through the callback only. Returning `ControlFlow::Break(())`
  from `for_each` makes `gix_diff::tree` return `tree::Error::Cancelled`, surfaced from
  `tree_with_rewrites` as `Error::Diff(tree::Error::Cancelled)` (`tree/mod.rs`,
  `tree_with_rewrites/function.rs`). No `should_interrupt` flag is polled anywhere in
  `gix-diff` (grep: none). The callback runs once per change, so the epoch check Cairn
  already does for the history walk maps directly: `if superseded { Break }`. The gap
  is the rename tracker's similarity phase, which runs N×M blob diffs BETWEEN callbacks
  (`Tracker::emit` → `match_pairs`), bounded only by `Rewrites::limit`.
- Blob diff: `Diff::compute` is a pure CPU call with no interruption point. Bound is
  the input (`core.bigFileThreshold` and Cairn's own line cap) — a superseded blob diff
  is discarded, not stopped.
- Status: `Platform::should_interrupt_owned(Arc<AtomicBool>)`; `index_as_worktree`
  passes it into its parallel chunks and the tree-index producer checks it per change;
  `Iter::next` polls it every 25 ms while waiting. `FastEq` notes
  "TODO: make all streaming IOPs interruptible" — a single large file hash cannot be
  interrupted. `gix::interrupt` (`gix-0.87.1/src/interrupt.rs`) is the process-global
  SIGINT variant (`interrupt` feature, brings `signal-hook` + `parking_lot`); not
  needed — the `Arc<AtomicBool>` route is the one to use, and it composes with the
  worker's epoch by flipping the flag when a newer query arrives.

Parallelism inside the library: tree diff and rename tracking are single-threaded
(no `in_parallel`/`thread` in gix-diff). `index_as_worktree` uses
`gix_features::parallel::in_parallel_if` with `Options::thread_limit` (0/None = all
cores). `status().into_iter()` spawns two producer threads of its own — inside Cairn's
worker that is fine (the worker blocks on `next()`), but `Iter` must not be handed to
the UI thread (`crates/cairn-app` guard: `Receiver`/`JoinHandle` are inside it).

---

## 9. Performance knobs

- `max-performance` in `gix 0.87.1` is `max-performance = ["max-performance-safe"]`,
  `max-performance-safe = ["max-control"]`, `max-control = ["parallel", "pack-cache-lru-static", "pack-cache-lru-dynamic"]`
  (`gix-0.87.1/Cargo.toml`). It no longer selects a zlib backend: inflate goes through
  `gix-zlib 0.1.0`, whose only feature is `serde`, and `Cargo.lock` resolves `flate2`
  with the pure-Rust `zlib-rs` backend. No zlib-ng, no C. So there is nothing more to
  turn on for decompression, and nothing to lose by keeping the feature set as is.
- `parallel` is what gives the threaded status and pack access; already on.
- Object cache: `Repository::object_cache_size(bytes)`, `object_cache_size_if_unset(bytes)`,
  and `compute_object_cache_size_for_tree_diffs(&index) -> usize` ("about 10MB for every
  10k files in `index`, and a minimum of 4KB") (`repository/cache.rs`). `Tree::changes()`
  doc: "It's highly recommended to set an object cache". The cache is per `Repository`
  handle (thread-local), so the worker sets it once on its handle. BUT `Tracker::emit`
  doc: "object-caching *should not* be enabled as caching is implemented by
  `diff_cache`" — the rename tracker reads blobs through the resource cache, so the
  object cache pays for trees (many small reads during the walk) and double-buffers
  blobs. Unset by default. Tree diff is where it pays; sizing per that helper.
- Blob-level caching: the `Platform` (section 6); `clear_resource_cache_keep_allocation()`
  between files of one query keeps the buffers, `clear_resource_cache()` between queries.
  `Tree::changes().stats()` shows the intended loop (`for_each_to_obtain_tree` +
  `change.diff(&mut cache)` + `line_counts()` + `clear_resource_cache_keep_allocation()`).
- `tree::State` is reusable (`Default + Clone`, buffers kept).
- `InternedInput::clear()` keeps allocations; `Interner::erase_tokens_after(Token)`
  rolls back a per-hunk word diff's tokens.
- Tree diff is not parallel (section 8).

---

## 10. Patch application compatibility (`git apply --cached` for a partial stage)

Sources: `man git-apply`, `man git-diff` ("GENERATING PATCH TEXT WITH -P"), `man git-add`
("EDITING PATCHES") from git 2.55.0; `add-patch.c` and `apply.c` from
`raw.githubusercontent.com/git/git/master` (fetched 2026-09-17).

**What `git add -p` actually sends.** `patch_mode_add` (`add-patch.c`):
`.diff_cmd = { "diff-files", NULL }`, `.apply_args = { "--cached", NULL }`,
`.apply_check_args = { "--cached", NULL }`. The reassembled patch is first validated
with `apply --check --cached` (`run_apply_check`) and then piped to `apply --cached`
(`apply_patch`: `setup_child_process(s, &cp, "apply", NULL); strvec_pushv(&cp.args, s->mode->apply_args);`).
**No `--recount`, no `--allow-overlap`, no `-p0`, no `--unidiff-zero`** are passed
(grep of `add-patch.c`: none of those strings appear). The unstage mode
`patch_mode_reset_head` uses `{ "-R", "--cached" }`; discard-in-worktree modes use
`apply -R` without `--cached`; `checkout -p` runs `apply --cached --check`, then
`apply --cached` and `apply` (worktree) in sequence.

**Hunk selection.** `reassemble_patch()`: a skipped hunk contributes
`delta += hunk->header.old_count - hunk->header.new_count`; a kept hunk is rendered
with `render_hunk(s, hunk, delta, ..)`, which emits

```c
strbuf_addf(out, "@@ -%lu", old_offset);
if (header->old_count != 1) strbuf_addf(out, ",%lu", header->old_count);
strbuf_addf(out, " +%lu", new_offset);
if (header->new_count != 1) strbuf_addf(out, ",%lu", header->new_count);
strbuf_addstr(out, " @@");
```

with `new_offset += delta` in the forward direction (`old_offset -= delta` when
reversing). So the OLD offsets are the original ones and only the NEW offsets shift by
the net lines of every earlier skipped hunk. Adjacent selected hunks that touch are
merged (`merge_hunks`: `header->old_count = next->old_offset + next->old_count - header->old_offset;`
`header->new_count = next->new_offset + delta + next->new_count - header->new_offset;`).
The trailing text after `@@` (function context) is copied through if present and is
not required.

**Line-level selection (the `e` editor rules, `man git-add` EDITING PATCHES).** To not
stage an added line, DELETE the `+` line. To not stage a removal, turn the `-` into a
`' '` context line. After editing, `recount_edited_hunk()` recomputes counts from the
markers: `'-'` → `old_count++`; `'+'` → `new_count++`; `' '` → both. `normalize_marker`
treats an empty line (`\n` or `\r\n` alone) as a context line. This is exactly the
"recount rules" Cairn must implement when it builds a patch from a subset of lines:
counts derived from the emitted lines, `+N`-offset = original new offset + delta of
earlier omissions, `-N` offset untouched.

**`\ No newline at end of file`.** In `add-patch.c`, `split_hunk`: `if (ch == '\\') ch = marker ? marker : ' ';`
— "Comment lines are attached to the previous line", i.e. the marker line belongs to
whichever `+`/`-`/` ` line precedes it and travels with it. In `apply.c`,
`adjust_incomplete()`: a `' '`, `'+'`, `'-'` (or bare `\n`) line followed by a line
beginning with `"\\ "` of at least 12 bytes has its trailing `\n` dropped and the marker
line is eaten; the l10n text after `\ ` is not inspected. So Cairn must emit the marker
after the last line of whichever side lacks a terminator, and when a partial selection
turns the final `-` line into context, the marker must follow it as a context line's
marker (still `\ No newline...`), and if the final `+` line is dropped the `\` line that
followed it must be dropped too. `recount_diff` (only with `--recount`) also skips `\`
lines. `--inaccurate-eof` is for broken external diffs, not for this.

**Headers.** `man git-diff` §"GENERATING PATCH TEXT": `diff --git a/<path> b/<path>`
(never `/dev/null` in that line, even for add/delete), then any of `old mode`, `new mode`,
`deleted file mode`, `new file mode`, `copy from/to`, `rename from/to`,
`similarity index N%`, `dissimilarity index N%`, `index <hash>..<hash> <mode>` ("The
<mode> is included if the file mode does not change; otherwise, separate lines
indicate the old and the new mode"), then the `--- a/<path>` / `+++ b/<path>` pair
(`/dev/null` there for add/delete). Modes are 6-digit octal (`EntryMode::as_octal_str`).
`apply.c` parses `index ` through the optional-header table (`{ "index ", gitdiff_index }`
in `parse_git_diff_header`); the parsed `old_oid_prefix` is consulted only for `--3way`
(`build_fake_ancestor`, `try_threeway`), for gitlink patches and for binary patches
(grep of `old_oid_prefix` use sites) — a text `--cached` apply reads the preimage from
the index entry by path (`load_patch_target` under `state->cached`). So the `index`
line is OPTIONAL for what Cairn does, but cheap and honest to emit (Cairn has both ids
for a staged diff; for a worktree side the new id is unknown — git writes the
worktree-side hash it computes; emitting `0000000` there is what `git diff` does for a
dirty worktree file with `diff-files`), and it is what makes a mode-only change
applyable. `add-patch.c` treats a `mode change` as its own selectable pseudo-hunk
(`file_diff->mode_change`), and rejects a diff that is "delete *or* add *or* a mode
change" combined (`BUG(...)`), so one file → one of those.

**Context.** `git apply` "expects that the patch being applied is a unified diff with
at least one line of context" (`--unidiff-zero` doc); `add-patch.c` keeps whatever
`diff-files` produced (`diff.context`, default 3). Cairn should emit 3 lines of
context computed from the ORIGINAL (to-git-form) file bytes, not from a
whitespace-normalised view — `--ignore-whitespace` on apply only relaxes CONTEXT
matching ("ignore changes in whitespace in context lines if necessary"), and is not
something `add -p` passes. CRLF: `check_old_for_crlf` in `parse_fragment` lets apply
tolerate a CRLF preimage, but since Cairn's preimage IS the to-git form (filters
applied, section 5), the patch lines must be the to-git bytes verbatim; there is no
`--ignore-whitespace` needed if that holds.

**Verification loop to copy.** `add -p` never trusts its own arithmetic: it runs
`apply --check --cached` on the reassembled patch before `apply --cached`. Cairn's
stage-hunk op should do the same (both are `git` subprocesses through `ops`, per D1),
which also means the exact recount rules above are safety-checked by git itself on every
stage.

---

## Open questions for the planner

1. **Reads that spawn.** `Mode::ToGit` never runs textconv, but the to-git conversion of a
   WORKTREE file runs the user's `filter.<driver>.clean`/`process` (gix-filter) — so a
   repository with LFS makes "read the unstaged diff" spawn a process. D1's "reads never
   spawn a process" needs either an exception stated (it is git's own clean filter, the
   same one `git diff` runs) or a `GitEnvironment`-style rule for the filter's
   environment (`gix_filter::Pipeline::new(repo.command_context()?, ..)` — what
   environment does `gix_command::Context` give the filter? **OPEN**, not read).
2. **Ordering.** gix emits tree-diff changes in traversal/tracker order, not path order;
   the model must sort. Decide the sort key (git's tree-entry compare, or plain bytes).
3. **`\ No newline`** is invisible through `prepare_diff::Outcome::interned_input()`
   (terminators stripped). Cairn must tokenize with `byte_lines` (terminators kept) or
   keep a per-line terminator bit to emit the marker and to match git's own
   "changed the last line's newline" behaviour.
4. **Whitespace modes are Cairn's** (section 3): a normalising tokenizer with a
   one-to-one map back, and `ignore-blank-lines` needs a separate design.
5. **Word diff is Cairn's** beyond `Hunk::latin_word_diff` (str-only, Myers,
   alphanumeric words).
6. **Cancellation of rename tracking**: the similarity phase runs between callbacks;
   a superseded tree diff on a large rename-heavy commit finishes that phase before
   the `Break` is seen. Either accept, or cap `Rewrites::limit` below git's 1000 for
   interactive queries, or run tree diff twice (fast, no rewrites; then rewrites).
7. **Cache invalidation**: the `Platform` is path-keyed for worktree files and holds
   attributes from construction time. Policy needed: clear per worktree query; rebuild
   on `.gitattributes` change (how is that detected? **OPEN**).
8. **Which `Item` order does `status().into_iter()` produce?** Undefined by contract;
   the model needs its own merge of `TreeIndex` and `IndexWorktree` items into one
   per-path row (a file can be both staged and modified). Also whether to call
   `Outcome::write_changes()` (an index write, hence `ops`) to keep status fast — **OPEN**
   policy question, overlaps program O2.
9. **`index <old>..<new>` for a dirty worktree file**: emit `0000000..0000000`, the
   index entry's id on the left with zeros on the right, or hash the to-git bytes
   (`gix_object::compute_hash`, no ODB write)? Git's `diff-files` prints the index
   blob on the left and zeros on the right for a stat-dirty file. **OPEN**: verify
   against `git diff-files -p` output before pinning a test.
10. **Split index / untracked cache** support in gix-index 0.55 — **OPEN**, belongs to O2.
11. **Function context (`xfuncname`)** for hunk headers: gix has none; git ignores it on
    apply. Cosmetic-only; decide if the header shows it (would need a Cairn regex table).
