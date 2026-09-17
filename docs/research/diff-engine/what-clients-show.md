# What mature git clients actually SHOW for a diff, and how they build a partial patch

> **Corrections.** The Fork claims in this record were re-verified afterwards in
> `fork-detail-and-diff-ui.md`, which lists thirteen corrections to them. Where
> the two disagree, that record is right and this one is the earlier reading.
> Neither is retro-edited: both are evidence, dated where they were written.

Evidence record. Gathered 2026-09-17 for the `diff-engine` packet, to answer two
questions the packet's brief asks and its open question O1 depends on: **what is
on screen when a commit or a working-tree change is selected**, and **how the
clients that let a user stage a subset of lines turn that subset into something
`git` will apply**.

Its companion, `docs/research/history-graph/what-clients-show.md`, covered the
history graph — default ref scope, lane counts, what a commit *row* carries, and
each client's commit-detail pane at the level of "which fields". This record does
not repeat that; where it needs a detail-pane fact already established there
(Fork's `Commit` / `Changes` / `File Tree` tabs, Sourcetree's multi-select
cumulative diff), it cites that record's finding number. The engine-side
companions in this directory — `engine-and-worker-as-built.md`, `gix-diff-api.md`
— describe what Cairn already has; this record describes what the field does.

## Why this record exists

The packet must ship a diff model that is **patch-capable**: able to emit a
unified patch for an arbitrary subset of hunks and lines, for a later packet to
hand to `git apply --cached`. O1 asks whether the view should be side-by-side,
unified, or both, and — if both — whether the patch model can be independent of
the presentation. Both questions have been answered, in code, by the open-source
clients, and answered by precedent in the closed ones; the brief's suspicion was
that line selection might be a unified-view-only affordance everywhere, which
would settle O1 by itself. **It is not** (Part C), and the four readable
implementations of "selected lines → index" split evenly between building a patch
and never building one at all (Part D).

## Method, and what each class of evidence is worth

Fork, Sourcetree, GitKraken, Sublime Merge and Tower are closed source. Nothing
below claims to describe their implementation, and no inference is written as a
fact. Every finding carries one of the labels the companion record defined:

- **DOCUMENTED BY VENDOR** — a KB page, manual, help article or release note.
- **VENDOR STATEMENT IN A PUBLIC TRACKER** — a maintainer's comment.
- **VENDOR TRACKER STATE** — an issue's existence, state and dates on the
  vendor's own tracker, without a maintainer comment. Weaker than a statement:
  it proves the request was made and not closed as invalid, not that the
  described behaviour is exact.
- **CONSISTENTLY REPORTED BY USERS** / **SINGLE USER REPORT**.
- **READ THE SOURCE** — open-source clients, at a pinned commit.
- **INFERRED** — reasoning over the above, never stated as observation.
- **OPEN** — looked for, not found.

Open-source clients were cloned and read at these commits; every path below is
relative to the repository root and every permalink is
`https://github.com/<owner>/<repo>/blob/<sha>/<path>`:

| Client | Repository | Commit read |
| --- | --- | --- |
| lazygit | `jesseduffield/lazygit` | `71d3e7dfa5f9278172013bfa1bb83d60d155436a` |
| Magit | `magit/magit` | `83ba66c8ab6fcdbd809077ae4db1f7e2ed832655` |
| gitui | `gitui-org/gitui` | `2fa693cb6ed431b21ebc300dd02e83c2476699ce` |
| VS Code git extension | `microsoft/vscode` | `94dbeaa3754378b9119b88f205d06c44897495c6` |
| GitHub Desktop | `desktop/desktop` | `9dfe6e60dbcf74961d58c9e38b8c98f0a6120028` |
| GitButler | `gitbutlerapp/gitbutler` | `6d0e7c0d345edaacbc520966ffb3feebec6cc3c2` |
| tig | `jonas/tig` | `1b86f070a1f6d4c686a09b997fd4249d52a2a272` |
| git (documentation only) | `git/git` | `12cb6293d6288865c1a133cf22accbaf99d13eb6` |

Code is quoted in fragments short enough to be checked against the permalink;
the licences are MIT (lazygit, gitui, VS Code, GitHub Desktop, GitButler), GPL
(Magit, tig, git).

## Part A — what each client shows for a selected commit

### Finding 1 — the `git` baseline: `show` is header, then diffstat, then patch; a merge gets a dense combined diff

`git show` prints the `medium` format — `commit <hash>`, `Author:`, `Date:`, the
message — followed by the patch; `--pretty=fuller` adds `Commit:` and
`CommitDate:` so author and committer are distinguished. For merge commits its
diff format defaults to **`dense-combined`** — "Default is `dense-combined`
unless `--first-parent` is in use, in which case `first-parent` is the default"
— which is the `--cc` shortcut; `git log -p` shows no diff for merges at all
unless `-m` / `--diff-merges` is given
(<https://git-scm.com/docs/git-show>, <https://git-scm.com/docs/diff-options>).
A combined diff has a different shape from a two-way patch: `diff --cc`, an
`@@@ … @@@` header with one range per parent, and one leading column per parent
on every line. It cannot be handed to `git apply`.

**Confidence:** DOCUMENTED BY VENDOR (the git manual).
**Implies:** "what a merge commit shows" has a git-native default that no GUI in
this record actually uses (Finding 2, Finding 4, Finding 8). Whatever Cairn
picks, the combined format is a *third* shape of diff, not a variant of the
patch model.

### Finding 2 — Fork: separate author/committer, parents as links, a `File Tree` tab, and a first-parent diff for merges

The detail pane is tabbed `Commit` / `Changes` / `File Tree`, shows Author and
Committer separately, the full SHA, and both parents as clickable links
(companion record, Finding 9). The file list has had a tree mode since 1.0.29
("Show changed files as a tree") and the commit-details tree since 2.0 (13 Nov
2020, "Display File Tree in Commit Details view"); renamed files show both names
since 2.28 (14 Apr 2023, "Show old and new filenames for renamed files"); a
submodule entry gets its own icon since 2.69 ("Show submodule icon instead of
blank file icon in file views") (<https://git-fork.com/releasenotes>).

For a merge commit Fork shows the diff **against the first parent**. A request
opened 30 Apr 2026 states it as the premise — "When viewing a merge commit, Fork
shows the diff against the first parent, which includes everything brought in by
the merged branch" — and asks for a `--cc` / `--remerge-diff` toggle so the
merger's manual resolution can be told apart from what merged cleanly
(<https://github.com/fork-dev/TrackerWin/issues/2774>, open, no vendor reply).
An older request for a `--first-parent` *history* view is separate
(<https://github.com/fork-dev/Tracker/issues/220>).

The diff renders in the same pane below the file list, not in a separate window;
pressing space opens "large side by side diff view" as a quick look (Windows
1.36, 12 Jul 2019, <https://git-fork.com/releasenoteswin>).

**Confidence:** DOCUMENTED BY VENDOR for the release notes; VENDOR TRACKER STATE
for the first-parent behaviour (the reporter's premise, uncontradicted, in a
tracker the vendor reads).
**Implies:** the client Cairn is modelled on made the first-parent choice for
merges and has lived with it since at least 2017; the combined diff is a
five-month-old open request there.

### Finding 3 — Sourcetree: hash, parents, author, date, labels, files, and a diff "for most files"; no internal side-by-side text diff was found

Atlassian's KB: clicking a commit expands it to show "Commit's full hash,
commit's parents, commit author, commit date, and commit labels/tags, as well as
the files involved in the commit. For most files, SourceTree will even show the
diff of the file"
(<https://support.atlassian.com/sourcetree/kb/viewing-log-history-of-a-repository/>).
Selecting several commits shows their cumulative diff (companion record,
Finding 15).

An internal side-by-side **text** diff was not found on either platform. The
Windows tracker has carried "Internal diff side by side view" since December
2013, status `In Progress`, no fix version
(<https://jira.atlassian.com/browse/SRCTREEWIN-1296>); community answers and a
how-to post both say the only route is an external tool ("Unfortunately, there is
no built-in solution. The only way to view changes side-by-side is to use an
external diff tool", <https://blog.tomasbouda.cz/quick-tip-side-by-side-diff-in-sourcetree/>).
The only "side-by-side" in Sourcetree's Mac release notes is the *image* diff's
Before / After / Side-by-side modes (SRCTREE-2283, cited in
<https://product-downloads.atlassian.com/software/sourcetree/ReleaseNotes/Sourcetree_4.2.11.html>).
Intra-line highlighting is likewise absent on Mac: "Offer word diff" is an open
request (<https://jira.atlassian.com/browse/SRCTREE-888>). What the pane shows
for a merge commit is **OPEN** — no KB page or tracker item found states it.

**Confidence:** DOCUMENTED BY VENDOR for the pane's fields; VENDOR TRACKER STATE
plus CONSISTENTLY REPORTED BY USERS for the absence of side-by-side and word
diff.
**Implies:** of the two clients Cairn is positioned against, one has never
shipped a side-by-side text view in twelve years of requests. Side-by-side is a
differentiator in this pair, not table stakes.

### Finding 4 — GitKraken: a file list with list / tree / auto layout, three diff modes with Hunk as default, and no documented merge behaviour

Clicking a commit lists its changed files in the right-hand Commit Panel; a
toggle switches the file layout between "list, tree, or auto"; clicking a file
opens its diff (<https://support.gitkraken.com/working-with-commits/diff/>,
<https://help.gitkraken.com/gitkraken-desktop/commits/>). The diff has three
toggles — **Hunk View** ("Displays only the changed blocks of a file", the
default), **Inline View** ("Displays changes within the full context of the
file") and **Split View** ("Displays changes side-by-side, with the original file
on the left and the updated version on the right") — plus word diffing, syntax
highlighting, a word-wrap toggle, a minimap, next/previous change arrows, and a
per-hunk Revert button in Hunk View
(<https://help.gitkraken.com/gitkraken-desktop/diff/>). Whether any view shows
merges against the first parent, combined, or via a picker is **OPEN**; the
support page mentions only that the parent hash in the details pane is
clickable. Two commits can be compared with shift-click.

**Confidence:** DOCUMENTED BY VENDOR throughout; OPEN for merges.
**Implies:** the one client with three presentation modes made the *hunk-only*
unified view the default, not the split view — and its users then asked for a
hunk-mode split view with expand-up/down (Finding 14).

### Finding 5 — Sublime Merge: metadata, then every changed file with its diff inline; condensed like `git diff`, context by dragging

"The details of the selected commit are shown in the details section. Commit
metadata such as the commit message and author is displayed at the top of this
section. Below the metadata is a list of all changed files and their associated
diffs (changes)" (<https://www.sublimemerge.com/docs/getting_started>). So the
file list and the diffs are one scrolling document, not a list-plus-pane.
"Changes are shown in a condensed view by default, similar to git diff";
context is added by dragging a hunk's top or bottom edge, double-clicking the
edge adds "a few lines", and a full-file toggle sits in the hunk header (build
2032, 25 Aug 2020: "Added full file diffs - click the toggle button in the hunk
header") (<https://www.sublimemerge.com/docs/diff_context>,
<https://www.sublimemerge.com/download>). Side-by-side exists and is chosen by
window width — "The default diff display changes from over-under to side-by-side
based on window width" (<https://forum.sublimetext.com/t/side-by-side-diff/41760>,
user report). Merge-commit diff behaviour: **OPEN** (the release notes mention
only that merge commits are folded in the graph by default, build 1070).

**Confidence:** DOCUMENTED BY VENDOR, except the width-driven layout switch,
which is a SINGLE USER REPORT on the vendor forum.
**Implies:** the one client that renders the whole commit as a single document
also made "expand context" a drag gesture rather than a button — the closest
thing in this record to the terminal's `git show` shape with a GUI on it.

### Finding 6 — Tower: a Changeset view with a documented three-way merge setting, a whole-tree Tree view, and diffs collapsed by default for performance

Tower's commit details are two views: Changeset, with "meta information about the
commit (including author, message, date, commit hash, etc.) as well as the
detailed changes that happened in this revision", and Tree, browsing "the
complete file tree of your project at that point in time". "By default,
changesets are collapsed to ensure optimum performance"; expand all with
⌘⌃→. Double-clicking a commit opens "a detailed view showing only this commit's
changeset". Merge commits have a **setting**, "Show diffs for merge commits",
with three values: "None" (no diffs, the git default), "First parent" ("shows a
diff for the first parent, showing what changes were introduced by the merge")
and "Merged file paths" ("shows a diff only for files changed from all parents")
(<https://www.git-tower.com/help/guides/commit-history/commit-details/mac>).

**Confidence:** DOCUMENTED BY VENDOR.
**Implies:** Tower is the only client in this record that exposes the merge
question as a user choice, and its "None" default matches `git log -p`, not
`git show`. Its "collapsed by default" is also the only vendor-documented
*rendering* budget for a large commit (Part E).

### Finding 7 — GitButler: a working-tree-first client whose commit view is a unified diff and whose model has no patch text

GitButler's commit and change views render `UnifiedDiffView.svelte` over
`packages/ui/src/lib/components/hunkDiff/`; no side-by-side component was found
in `apps/desktop/src` or `packages/ui/src` (READ THE SOURCE — an absence). The
selected-commit surface is secondary to its virtual-branch workspace, and what it
shows for a merge commit is **OPEN**. Its patch model is the subject of
Finding 20.

### Finding 8 — lazygit, gitui, Magit, tig: the readable clients delegate the header to git and show a tree or a list beside it

- **lazygit** renders the selected commit with `git show --stat --decorate -p`
  (plus `--submodule`, colour, the user's context size and whitespace flags —
  `CommitCommands.ShowCmdObj`, `pkg/commands/git_commands/commit.go`), so the
  header is git's `medium` format and a merge gets git's dense-combined diff by
  default (Finding 1). Its file panel is `git diff --name-status -z
  --find-renames=N% <from> <to>` (`pkg/commands/git_commands/commit_file_loader.go`)
  and shows a tree by default (`ShowFileTree: true`, `pkg/config/user_config.go`).
- **gitui** draws Author, Date, Committer (only when it differs), Sha and the
  message (`src/components/commit_details/details.rs`), a file tree beside them,
  and diffs a commit against **`parent_id(0)`** — the first parent
  (`asyncgit/src/sync/commit_files.rs`).
- **Magit**'s revision buffer inserts `Author:` / `AuthorDate:` / `Commit:` /
  `CommitDate:` from `magit-revision-headers-format`, then one `Parent:` line per
  parent and, optionally, `Merged:` / `Contained:` / `Follows:` / `Precedes:`
  (`magit-insert-revision-headers`, `lisp/magit-diff.el`); the default diff
  arguments are `("--stat" "--no-ext-diff")`.
- **tig**'s diff view is `git show --pretty=fuller --root --patch-with-stat …
  --no-color` (`src/diff.c`, `diff_open`), i.e. fuller headers, diffstat, patch,
  and git's combined diff for merges — tig parses `diff --cc` / `diff --combined`
  headers explicitly (`src/diff.c`).

**Confidence:** READ THE SOURCE, pinned.
**Implies:** four independent implementations, two of which draw a header
themselves, all separate author from committer and all show a diffstat or
per-file stats. Where a client shells out for the *patch* (lazygit, tig, Magit)
it inherits git's combined merge diff for free; where it computes the diff itself
(gitui) it picks the first parent. That is the whole field's split on merges:
**first parent when you compute it, combined when git does** (Findings 2, 8),
with Tower alone offering a choice (Finding 6).

## Part B — diff presentation

### Finding 9 — unified is every client's default; side-by-side is a toggle in Fork, GitKraken, Sublime Merge (by width) and GitHub Desktop, and absent from Sourcetree, Tower, GitButler and the terminals

| Client | Default | Side-by-side | Since / source |
| --- | --- | --- | --- |
| Fork | unified | toggle in commit view and (later) Local Changes | Mac 1.0.50 (30 Jun 2017) "Introduced side-by-side diff view"; 1.0.88 (13 Dec 2019) "Side by side diff in commit changes"; 2.21 (19 Aug 2022) "Side-by-side mode in the Local Changes view!"; Windows 1.33 (17 May 2019) "Side by side diff!" (<https://git-fork.com/releasenotes>, <https://git-fork.com/releasenoteswin>) |
| Sourcetree | unified | not found (Finding 3) | SRCTREEWIN-1296 `In Progress` since 2013 |
| GitKraken | Hunk (unified, changed blocks only) | Split View toggle | <https://help.gitkraken.com/gitkraken-desktop/diff/> |
| Sublime Merge | over-under, switching to side-by-side by window width | automatic | forum, SINGLE USER REPORT |
| Tower | unified | not found; the feature list offers external diff apps for it | <https://www.git-tower.com/features/all-features> (INFERRED from absence) |
| GitButler | unified | not found | READ THE SOURCE (absence) |
| GitHub Desktop | unified | "Diff display: Unified / Split" | 2.6, 17 Nov 2020 (<https://github.blog/news-insights/product-news/introducing-split-diffs-in-github-desktop/>) |
| VS Code | the editor's diff editor, side-by-side by default with an inline toggle | both | editor setting, not the git extension's |
| lazygit, gitui, Magit, tig | unified | none | READ THE SOURCE |

The one dated data point on *demand*: Fork's Mac users asked why the staging area
lacked the side-by-side option the commit view had (10 Dec 2021,
<https://github.com/fork-dev/Tracker/issues/1526>, closed) and got it eight
months later in 2.21.

**Confidence:** as per row.
**Implies:** no client in this record defaults to side-by-side. It is universally
a toggle, and half the field does not have it at all.

### Finding 10 — intra-line highlighting is on by default where it exists, and its granularity is "token" or "character"

- Fork Windows 2.20 (5 Jun 2026): "Improved Token-based inline diff
  highlighting"; an earlier note says highlighting was made "less aggressive,
  with less random code being highlighted" (<https://git-fork.com/releasenoteswin>,
  <https://fork.dev/blog/tags/release-notes/>). Nothing found describing an
  off switch.
- GitKraken: "Word diffing" is listed as a built-in feature with no toggle
  described (<https://help.gitkraken.com/gitkraken-desktop/diff/>).
- Sublime Merge: "character diffs" on the product page; builds 2096 (22 Apr 2024)
  and 2102 (28 Oct 2024) each "Enhanced the character diffing algorithm for
  greater precision" (<https://www.sublimemerge.com/download>).
- Sourcetree Mac: none — SRCTREE-888 "Offer word diff" is open (Finding 3).
- GitHub Desktop: none in the diff itself.
- tig: `diff-highlight` is an option that pipes through git's contrib script
  (`src/options.c`); Magit exposes `--word-diff` through its transient; lazygit
  and gitui have none.

**Confidence:** DOCUMENTED BY VENDOR for the release notes; the *default-on*
claim is INFERRED from the absence of any documented toggle.
**Implies:** three vendors ship intra-line highlighting with no off switch and
iterate on its *precision* in release notes, which is a signal that the naive
version (highlight everything that differs) reads badly and that the algorithm
is product surface, not plumbing.

### Finding 11 — whitespace ignore, context lines and expand-context: where each client puts them

- **Fork:** context lines via the diff control's context menu since 1.0.32 (2 Dec
  2016); "Allow to ignore whitespaces in commit view" 1.0.65 (15 Mar 2018);
  Windows 1.28 "Option to show whitespace characters in diff"; 1.16 "Diff mode
  controls above text editors"; Fork's own description of the controls above a
  diff: ignore whitespace, word wrap, text size, "show the entire file"
  (<https://fork.dev/blog/tags/release-notes/>). A bug shows the whitespace
  toggle interacts with staging: "when 'ignore whitespace' option is enabled, the
  floating 'Stage' button doesn't work when selecting lines"
  (<https://github.com/fork-dev/Tracker/issues/360>).
- **Sourcetree:** "Ignore whitespace" in the diff view; Windows requests
  SRCTREEWIN-2514 / SRCTREEWIN-1025 record it arriving there later than on Mac.
- **GitKraken:** no whitespace or context control documented on the diff page
  (OPEN); a feedback item asks for split-plus-hunk with expand up/down
  (<https://feedback.gitkraken.com/suggestions/298953/combined-split-and-hunk>).
- **Sublime Merge:** "Can now ignore whitespace changes in diffs" (build 1084,
  29 Oct 2018), narrowed in 1092 to "only ignores space and tab changes, not
  newline changes"; context by drag / double-click / full-file toggle
  (Finding 5).
- **Tower:** a diff footer with context lines, a whitespace-only toggle and a
  "Complete File" view (search-result summary of a Tower page; not verified
  against the page itself — treat as SINGLE USER REPORT class).
- **GitHub Desktop:** "Hide Whitespace Changes" (passes `-w` to `git diff`,
  `app/src/lib/git/diff.ts`); expand context by clicking an arrow above or below
  the line numbers in 20-line steps (`DefaultDiffExpansionStep = 20`,
  `app/src/ui/diff/text-diff-expansion.ts`) or "Expand Whole File" from the
  context menu (<https://docs.github.com/en/desktop/making-changes-in-a-branch/committing-and-reviewing-changes-to-your-project-in-github-desktop>).
- **lazygit:** `git.diffContextSize` (default 3, `--unified=N`),
  `git.ignoreWhitespaceInDiffView` (`--ignore-all-space`),
  `git.renameSimilarityThreshold` (default 50, `--find-renames=N%`), all in
  `pkg/config/user_config.go` and applied in `git_command_builder.go`.
- **gitui:** context, inter-hunk lines and ignore-whitespace are runtime toggles
  fed to libgit2's `DiffOptions` (`asyncgit/src/sync/diff.rs`, `src/options.rs`).
- **Magit:** a transient with `-b` / `-w` / `--ignore-space-at-eol`, `-U<n>`
  (`magit-diff-more-context` / `less`), `-W` function context and
  `--diff-algorithm=` (`lisp/magit-diff.el`). Note Magit *refuses to apply* when
  context is zero: "Not enough context to apply patch.  Increase the context"
  (`magit-apply-patch`, `lisp/magit-apply.el`).
- **tig:** `diff-context` (keys `[` / `]`), `show-ignore-space` toggle,
  `word-diff`, and a user-supplied `diff-options` string passed through
  (`src/options.c`, `include/tig/git.h`).

**Confidence:** DOCUMENTED BY VENDOR or READ THE SOURCE per row; Tower's footer
is the one unverified item.
**Implies:** every client puts whitespace-ignore and context on the diff pane
itself, not in preferences; and two implementations (Magit, Fork's bug) show the
same coupling: a whitespace-ignored *view* is not the patch `git apply` needs.

### Finding 12 — syntax highlighting, images, binaries, LFS, renames, mode changes, submodules

- **Syntax highlighting:** Fork Mac 1.0.59 (17 Nov 2017), Fork Windows 2.1 (27
  Sep 2024, "Syntax highlighting!" — seven years after Mac); GitKraken, Sublime
  Merge, GitHub Desktop all have it. Terminal clients rely on git's colour.
- **Images:** Fork 1.0.26 "Render images and show diffs for the common image
  formats", Windows 1.37 "Swipe and onion views for image diff", 2.42 / Windows
  1.97 "Highlight exact pixel diff for images"; Sublime Merge build 2025 (22 Jul
  2020) "Added image diffs", 2032 "Git LFS image diff support", 2047 PSD/TGA/PPM/PGM;
  Sourcetree image side-by-side/Before/After (Finding 3); GitHub Desktop has an
  `image-diffs` component. GitKraken: not on the diff page (OPEN).
- **Binary and LFS:** Fork 1.0.70 "Custom diff view for binary and LFS files",
  2.41 "Show a proper diff for binary to LFS changes and vice versa"; Sublime
  Merge 2125 "Improved metadata display in changed binary files"; GitKraken
  "does not currently generate patches from binary files" and detects binary by
  a 10 MB size limit users have asked to make configurable
  (<https://feedback.gitkraken.com/suggestions/365949/add-user-setting-for-binary-detection-file-size-limit>);
  GitHub Desktop refuses a partial commit on binary, image or submodule diffs
  ("Can't create partial commit in binary file", `app/src/lib/git/apply.ts`).
- **Renames:** Fork 2.28 shows old and new names; lazygit and VS Code pass an
  explicit `--find-renames=N%` (50 % default in lazygit; VS Code's
  `git.similarityThreshold`); GitHub Desktop passes `-M` for commit diffs
  (`app/src/lib/git/diff.ts`); tig passes `-C` (copies too) in its staged and
  unstaged views (`include/tig/git.h`). Similarity percentages in the UI: not
  found in any vendor doc (OPEN).
- **Mode changes:** no client documents a mode-change presentation (OPEN).
- **Submodules:** Fork 2.69 icon, Windows 2.21 "Submodule diff when commit is
  missing (falls back to text diff)"; lazygit passes `--submodule` everywhere;
  GitHub Desktop has a `submodule-diff.tsx`; Sublime Merge lists submodule
  operations but not a diff shape.

**Confidence:** DOCUMENTED BY VENDOR / READ THE SOURCE.
**Implies:** the table of "kinds of file change" a diff pane must draw is at
least: text, image, binary, LFS pointer, submodule, rename (with both names),
and — for the terminals — whatever git prints. Mode-change and similarity-%
display are absent from every vendor document found, so they are a Cairn
decision without precedent either way.

### Finding 13 — what happens with a commit that touches thousands of files

The only vendor statements are Tower's and Sourcetree's, and they disagree in
mechanism. Tower **collapses** every file's diff by default "to ensure optimum
performance" and lets the user expand all (Finding 6). Sourcetree **loads every
diff** when a commit is clicked — "When clicking on a commit with numerous file
changes … Sourcetree attempts to load all diffs simultaneously … causing the
application to become unresponsive for an extended period", 37 votes, closed
*Cannot Reproduce* (<https://jira.atlassian.com/browse/SRCTREE-4972>, 2017);
"When commits contain more than 100 hunks, SourceTree becomes unresponsive,
regardless of file sizes" (<https://jira.atlassian.com/browse/SRCTREE-4424>,
2016); and a 3,500-file working tree hung its **tree view** until 1.6.13, with
Atlassian's workaround being "flat view's should be used in preference to the
treeview" (<https://jira.atlassian.com/browse/SRCTREEWIN-2572>, 2014). Fork's
only related note is 2.38 (12 Jan 2024) "Do not load diff for large untracked
files by default", and an early freeze report on "more than 10k files changed per
commit" (companion record, Finding 10). Nothing found for GitKraken or Sublime
Merge on file *count*; Sublime Merge's "Large files are now only diffed when
clicked on" (build 1116, 3 Jun 2019) is per-file size, not count. No vendor
documents file-list virtualization.

**Confidence:** DOCUMENTED BY VENDOR (Tower), VENDOR TRACKER STATE (Sourcetree).
**Implies:** the failure mode users report for a huge commit is eager diff
loading, not the file list — and the one vendor that documents a remedy chose
laziness (collapsed by default) over a cap. The tree-versus-flat hang is the one
piece of evidence about the file list itself, and it says the *tree* is the
expensive one.

## Part C — line-staging UX, and whether side-by-side offers it

This is the part that bears on O1 directly.

### Finding 14 — how lines are selected in the closed-source clients

- **Fork:** select lines in the diff and a floating **Stage** button appears
  (Tracker #360 describes "the floating 'Stage' button … when selecting lines").
  Windows 1.76 (29 Jul 2022) "Rework partial staging. Make chunk staging more
  precise". A discard exists for staged and unstaged ("Partial reset is supported
  too", Mac 1.0.1).
- **Sourcetree:** hovering a hunk shows `Stage hunk`; selecting lines shows
  `Stage selected lines` (SRCTREEWIN-2534 "'stage hunk' and 'stage lines' buttons
  missing in the live diff display", SRCTREEWIN-1688 "Leave staging buttons (Stage
  hunk, Stage selected lines …"); the KB describes only the hunk button
  (<https://support.atlassian.com/sourcetree/kb/viewing-file-status-of-a-repository/>).
- **GitKraken:** "click a file to open its diff, highlight the lines, right-click,
  and select **Stage selected lines**"; the same route unstages and discards
  (<https://help.gitkraken.com/gitkraken-desktop/staging/>). Two feedback items
  describe the model's edges: added and removed lines are **paired** and cannot
  be staged separately
  (<https://feedback.gitkraken.com/suggestions/201785/ability-to-stage-added-and-removed-lines-separately>),
  and multi-line selection by drag or shift-click was a request
  (<https://feedback.gitkraken.com/suggestions/195160/select-mutiple-lines-at-once-in-diff-view>).
- **Sublime Merge:** "select the individual lines you wish to stage and select
  **Stage lines**"; "select one or more lines to split hunks into multiple
  changes" (<https://www.sublimemerge.com/docs/getting_started>,
  <https://www.sublimemerge.com/>).
- **Tower:** "select individual lines by clicking on their line numbers, and the
  'Stage Chunk' button then turns into the **Stage Lines** button"
  (<https://www.git-tower.com/help/guides/working-copy/stage-changes/mac>).
- **GitHub Desktop:** "click one or more changed lines so the blue disappears";
  drag "the vertical bar to the right of the line numbers" for a range; discard
  by right-clicking a line number (docs.github.com, Finding 11's link).
- **GitButler:** a gutter checkbox per line and per hunk; pressing on a line and
  dragging paints every line crossed (`onStart` requires `ev.buttons === 1`,
  `onMoveOver` extends while the button is held, shift and ctrl/meta are passed
  through — `packages/ui/src/lib/components/hunkDiff/lineSelection.svelte.ts`).

**Confidence:** DOCUMENTED BY VENDOR / VENDOR TRACKER STATE / READ THE SOURCE
per row.
**Implies:** three interaction idioms coexist — text-style selection plus a
button (Fork, Sourcetree, GitKraken, Sublime Merge, Tower), gutter click/drag on
line numbers (Tower's line numbers, GitHub Desktop's bar, GitButler's
checkboxes), and a checkbox model where the selection *is* the commit contents
(GitHub Desktop, GitButler). GitKraken's pairing complaint is the one documented
model bug: a client that treats a `-`/`+` pair as one unit cannot express "keep
the deletion, drop the addition".

### Finding 15 — line staging in side-by-side mode exists in at least four clients, so line selection is NOT a unified-only affordance

This is the finding the brief asked for, and the answer is negative for the
hypothesis.

- **Fork** offers it, with a documented limitation: "the ability to stage/discard
  hunks in the side-by-side diff is available, but selecting and staging lines is
  only possible on one side of the diff at a time", and the reporter contrasts it
  with unified, where selecting across both is "already possible"
  (<https://github.com/fork-dev/Tracker/issues/1985>, 13 Oct 2023, open, no
  vendor reply).
- **Sublime Merge** offers it: "Can't discard single, isolated line in Merge's
  side by side diff view" describes selecting one modified line in the
  side-by-side view and pressing **Discard Lines**, failing only for an isolated
  single line (<https://github.com/sublimehq/sublime_merge/issues/866>, 14 Aug
  2020, closed).
- **GitHub Desktop** offers it, and the source shows *how*: the split view's
  `onStartSelection` / `onUpdateSelection` / `onEndSelection` resolve a row and a
  column to a **unified-diff line index** — `getDiffRowLineNumber` returns
  `row.data.diffLineNumber` for added/deleted rows and, for a modified row,
  `column === DiffColumn.After ? row.afterData.diffLineNumber :
  row.beforeData.diffLineNumber` — and hand the result to the same
  `onIncludeChanged(DiffSelection)` the unified view uses
  (`app/src/ui/diff/side-by-side-diff.tsx`). `DiffSelection` is "an immutable,
  efficient, storage object for tracking selections of indexable lines"
  (`app/src/models/diff/diff-selection.ts`) — a set of indices into the unified
  diff, with no knowledge of which view produced it.
- **VS Code** offers it in effect: `git.stageSelectedRanges` reads
  `textEditor.selections` on the *modified document* and intersects them with the
  editor's line changes (`stageSelectedChanges`, `extensions/git/src/commands.ts`);
  the diff editor is side-by-side by default and inline by toggle, and the
  command does not distinguish.
- **GitKraken:** the docs describe "highlight the lines, right-click" in "the
  diff view" without naming a mode; whether Split View permits it is **OPEN**.
- **Sourcetree, Tower, GitButler, terminals:** no side-by-side view, so nothing
  to test.

**Confidence:** VENDOR TRACKER STATE for Fork and Sublime Merge (both issues
presuppose the feature and were not closed as invalid); READ THE SOURCE for
Desktop and VS Code.
**Implies for O1:** the evidence does not support "line selection is universally
a unified-view affordance". It supports something more useful: in the one
readable implementation that has both views, **the selection model is keyed to
the unified diff and the split view is a projection that maps back to it**, and
the one closed client with both views has an open bug precisely where its split
view *failed* to be such a projection (one side at a time).

## Part D — how the open-source clients construct the partial patch

Four of the six readable implementations were studied to the level of the
algorithm; two (lazygit, GitHub Desktop) build a unified patch and hand it to
`git apply`; two (VS Code, GitButler) never build a patch at all — they build the
*new blob* and write it to the index. gitui and tig round out the set (Finding
21).

### Finding 16 — GitHub Desktop's `formatPatch`: the cleanest "selected lines → unified patch" reference, applied at commit time with `--cached --unidiff-zero`

`app/src/lib/patch-formatter.ts`, `formatPatch(file, diff)`, per hunk:

```ts
if (line.type === DiffLineType.Context) {
  hunkBuf += `${line.text}\n`; oldCount++; newCount++
} else if (file.selection.isSelected(absoluteIndex)) {
  hunkBuf += `${line.text}\n`
  if (line.type === DiffLineType.Add) { newCount++ }
  if (line.type === DiffLineType.Delete) { oldCount++ }
  anyAdditionsOrDeletions = true
} else {
  if (file.status.kind === AppFileStatusKind.New ||
      file.status.kind === AppFileStatusKind.Untracked) { return }
  if (line.type === DiffLineType.Add) { return }
  if (line.type === DiffLineType.Delete) {
    hunkBuf += ` ${line.text.substring(1)}\n`; oldCount++; newCount++
  }
}
if (line.noTrailingNewLine) { hunkBuf += '\\ No newline at end of file\n' }
```

Then, only if the hunk still contains a change, a header is written by
`formatHunkHeader(hunk.header.oldStartLine, oldCount, hunk.header.newStartLine,
newCount)` — the **old** start lines are kept and only the counts are recomputed
(git tolerates the drifted new-start because Desktop applies with
`--unidiff-zero`, which disables context matching). The patch header is
`--- a/path` / `+++ b/path`, or `--- /dev/null` for new files, where unselected
lines are dropped rather than turned into context. `absoluteIndex` is
`hunk.unifiedDiffStart + lineIndex`: the same unified index the split view maps
into (Finding 15). The `\ No newline` marker is emitted whenever the *emitted*
line carried the flag, for context and change lines alike.

Application (`app/src/lib/git/apply.ts`, `applyPatchToIndex`):

```ts
const applyArgs = ['apply', '--cached', '--unidiff-zero', '--whitespace=nowarn', '-']
```

fed on stdin, after — for a renamed file — recreating the rename in the index by
hand (`git add --update -- oldPath`, `git ls-tree HEAD -- oldPath`, `git
update-index --add --cacheinfo <mode> <oid> newPath`, because "we've just blown
away the index"). Desktop has no unstage: the index is reset and rebuilt from the
selection when the commit button is pressed ("prior to stageFiles the index has
been completely reset", `app/src/lib/git/update-index.ts`). A partial commit is
refused for `Binary`, `Submodule`, `Image` and `Unrenderable` ("File diff is too
large to generate a partial commit").

**Discard** does not use `--reverse`; it inverts the patch itself.
`formatPatchToDiscardChanges`: a selected `+` becomes `-`, a selected `-` becomes
`+`, an unselected `+` becomes context ("will stay in the file after discarding"),
an unselected `-` is dropped ("not found on the current working copy"), the
header's old/new ranges are swapped, and the result goes to `git apply
--unidiff-zero --whitespace=nowarn -` against the working tree.

**Confidence:** READ THE SOURCE, pinned
(<https://github.com/desktop/desktop/blob/9dfe6e60dbcf74961d58c9e38b8c98f0a6120028/app/src/lib/patch-formatter.ts>,
<https://github.com/desktop/desktop/blob/9dfe6e60dbcf74961d58c9e38b8c98f0a6120028/app/src/lib/git/apply.ts>).
**Implies:** the three rules the brief guessed at are exactly Desktop's —
unselected `+` dropped, unselected `-` turned into context, header recount — with
two refinements worth copying: **new files drop unselected lines entirely**
(there is no old side to keep them as context in), and **discard is a separate
inverted formatter**, not `--reverse` on the stage patch.

### Finding 17 — lazygit's `Transform`: the same rules with `Reverse` flipping which side is "old", a pending-context buffer for ordering, and a running start-line offset

`pkg/commands/patch/transform.go`. `TransformOpts.Reverse` is documented in the
source:

```go
// Create a patch that will applied in reverse with `git apply --reverse`.
// This affects how unselected lines are treated when only parts of a hunk
// are selected: usually, for unselected lines we change '-' lines to
// context lines and remove '+' lines, but when Reverse is true we need to
// turn '+' lines into context lines and remove '-' lines.
Reverse bool
```

`transformHunkLines` defines `isOldFileLine := (DELETION && !Reverse) ||
(ADDITION && Reverse)`; a selected line is kept; an unselected old-file line
becomes `" " + line.Content[1:]` and is **buffered** (`pendingContext`) so it is
emitted after the selected additions of the same change block — "giving the
correct output ordering: [selected deletions] [selected additions] [context from
unselected deletions]"; an unselected new-file line is dropped. The
`\ No newline at end of file` line is kept **unless it immediately follows a
dropped addition** (`skippedNewlineMessageIndex = lineIdx + 1`). Headers are
recounted by `transformHunkHeader`, which also carries a running `startOffset`
across hunks and applies a ±1 correction when a hunk's old or new length becomes
zero. A `FileNameOverride` replaces the original header with a bare `--- a/` /
`+++ b/` pair "because it makes git confused e.g. when dealing with deleted/added
files", and `StripRename` rewrites a rename header into a plain modification of
the new path for partial selections so the rename survives.

Application (`pkg/commands/git_commands/patch.go`):

```go
cmdArgs := NewGitCmd("apply").
    ArgIf(opts.ThreeWay, "--3way").
    ArgIf(opts.Cached, "--cached").
    ArgIf(opts.Index, "--index").
    ArgIf(opts.Reverse, "--reverse").
    Arg(filepath).
```

via a temporary file. The staging controller
(`pkg/gui/controllers/staging_controller.go`) calls it with `Reverse: reverse,
Cached: !reverse || self.staged` — so **stage** is `--cached`, **unstage** is
`--reverse --cached` with the transform's `Reverse` set, and **discard** of an
unstaged selection is `--reverse` against the working tree with the same
transform. Editing a hunk in `$EDITOR` re-parses the text and applies it the same
way (`editHunk`). Range selection is `v`, hunk mode `a`, stage `space`, discard
`d` (`docs/keybindings/Keybindings_en.md`).

**Confidence:** READ THE SOURCE, pinned
(<https://github.com/jesseduffield/lazygit/blob/71d3e7dfa5f9278172013bfa1bb83d60d155436a/pkg/commands/patch/transform.go>,
<https://github.com/jesseduffield/lazygit/blob/71d3e7dfa5f9278172013bfa1bb83d60d155436a/pkg/commands/git_commands/patch.go>).
**Implies:** lazygit is the one implementation that reuses a single transform
for stage, unstage and discard by flipping which side counts as "old", and it
had to invent two things Desktop did not need: the ordering buffer (because its
patch keeps original hunk starts *and* has to apply with context) and the
`\ No newline` suppression rule. It is also the only reader that handles a
**partial selection of a renamed file** deliberately.

### Finding 18 — Magit builds the region patch by a two-line rule and lets Emacs recount the header

`magit-diff-hunk-region-patch` (`lisp/magit-diff.el`), in full:

```elisp
(defun magit-diff-hunk-region-patch (section &optional args)
  (let ((op (if (member "--reverse" args) "+" "-"))
        ...)
    (save-excursion
      (goto-char sbeg)
      (while (< (point) send)
        (looking-at "\\(.\\)\\([^\n]*\n\\)")
        (cond ((or (string-match-p "[@ ]" (match-str 1))
                   (and (>= (point) rbeg)
                        (<= (point) rend)))
               (push (match-str 0) patch))
              ((equal op (match-str 1))
               (push (concat " " (match-str 2)) patch)))
        (forward-line)))
    (let ((buffer-list-update-hook nil)) ; #3759
      (with-temp-buffer
        (insert (string-join (reverse patch)))
        (diff-fixup-modifs (point-min) (point-max))
        (setq patch (buffer-string))))
    patch))
```

Keep the header and every context line; keep every line inside the region;
outside the region turn `op` lines into context (where `op` is `-` normally and
`+` under `--reverse`) and drop the rest — which drops an out-of-region
`\ No newline at end of file` line too, since `\` matches neither branch. Emacs's
own `diff-fixup-modifs` then recounts the `@@` header, and
`magit-apply--adjust-hunk-new-start` fixes the new-side start. The file header is
prepended by `magit-apply-region`, which refuses combined-diff hunks: "Cannot
un-/stage resolution hunks. Stage the whole file".

`magit-apply-patch` runs `git apply <args> -p0 -C<context> --ignore-space-change
-` (or `-C0` when the diff was made with whitespace ignored), after refusing when
context is zero. The direction verbs (`lisp/magit-apply.el`): stage is
`--cached`; unstage is `--reverse --cached`; **discard** picks its flags by
state — an unstaged change gets `--reverse`; a change that has both staged and
unstaged parts gets `--reverse --cached` **then** `--reverse --reject`; otherwise
`--reverse --index`; and `magit-reverse-apply` adds `--reject` unless
`magit-reverse-atomically` or `--3way` is set.

**Confidence:** READ THE SOURCE, pinned
(<https://github.com/magit/magit/blob/83ba66c8ab6fcdbd809077ae4db1f7e2ed832655/lisp/magit-diff.el>,
<https://github.com/magit/magit/blob/83ba66c8ab6fcdbd809077ae4db1f7e2ed832655/lisp/magit-apply.el>).
**Implies:** the algorithm is four lines once the unified text is the model;
everything else is direction flags. Magit's discard is the most careful in the
record about the *staged-and-unstaged* case, which is a state Cairn's model will
have to name.

### Finding 19 — VS Code never builds a patch: it computes the new file content from line ranges and writes the blob to the index with `hash-object` + `update-index --cacheinfo`

`git.stageSelectedRanges` (`extensions/git/src/commands.ts`) takes the editor's
line changes, intersects them with the selection, and calls `_stageChanges`,
which reads the index version of the file (`toGitUri(uri, '~')`), computes
`applyLineChanges(originalDocument, modifiedDocument, changes)`, and passes the
resulting **string** to `repository.stage(resource, result, encoding)`.
`applyLineChanges` (`extensions/git/src/staging.ts`) walks the sorted changes
copying original text up to each change and modified text for it, with an
explicit end-of-document rule referencing
<https://github.com/microsoft/vscode/issues/59670> for "a newline at the end of
the last line which may have been deleted". `Git.stage` (`extensions/git/src/git.ts`)
then runs:

```
git hash-object --stdin -w --path <relativePath>
git update-index [--add] --cacheinfo <mode> <hash> <relativePath>
```

**Unstage** inverts the selected line changes (`invertLineChange`) and applies
them with the *index* document as "original" and HEAD as "modified", then stages
that; **revert** applies the unselected changes over the working file with a
`WorkspaceEdit` and saves. The line changes themselves come from the editor's
diff (`textEditor.diffInformation` via `getWorkingTreeDiffInformation`), not from
`git diff` — so `diff.algorithm`, `core.whitespace` and drivers do not apply to
what is staged (INFERRED from where the changes come from; the extension's `git
diff` calls are used for file lists, with `--find-renames=N%` from
`git.similarityThreshold`).

**Confidence:** READ THE SOURCE, pinned
(<https://github.com/microsoft/vscode/blob/94dbeaa3754378b9119b88f205d06c44897495c6/extensions/git/src/staging.ts>,
<https://github.com/microsoft/vscode/blob/94dbeaa3754378b9119b88f205d06c44897495c6/extensions/git/src/git.ts>).
**Implies:** there is a second, patch-free route to the same result — compute
the blob, write it — that sidesteps every `git apply` failure mode (context
mismatch, whitespace, `\ No newline`, zero-context) at the cost of needing the
full old and new file contents in memory and of bypassing `apply`'s own checks.
It is also the route that makes side-by-side trivially compatible: a selection
over the modified document does not care how the diff was drawn.

### Finding 20 — GitButler's model is a list of `HunkHeader` ranges over two images, applied by splicing; no patch text, no `git apply`

The commit request type (`crates/but-core/src/diff_types.rs`):

```rust
pub struct DiffSpec {
    pub previous_path: Option<BString>,
    pub path: BString,
    /// If one or more hunks are specified, match them with actual changes
    /// currently in the worktree. Failure to match them will lead to the
    /// change being dropped. If empty, the whole file is taken as is …
    pub hunk_headers: Vec<HunkHeader>,
}
pub struct HunkHeader { pub old_start: u32, pub old_lines: u32,
                        pub new_start: u32, pub new_lines: u32 }
```

A line selection becomes headers on the client side: `lineIdsToHunkHeaders`
(`apps/desktop/src/lib/hunks/hunk.ts`) groups selected added lines and selected
removed lines into contiguous runs and emits one `HunkHeader` per run — an
"added" run gets `newStart = firstLine.newLine, newLines = count` and an old
range **anchored to the parent hunk**; a "removed" run the converse. The engine
applies them without ever rendering a patch (`crates/but-core/src/hunks.rs`):

```rust
pub fn apply_hunks(old_image: &BStr, new_image: &BStr, hunks: &[HunkHeader])
    -> anyhow::Result<BString> {
    // To each selected hunk, put the old-lines into a buffer.
    // Skip over the old hunk in old hunk in old lines.
    // Skip all new lines till the beginning of the new hunk.
    // Write the new hunk.
    // Repeat for each hunk, and write all remaining old lines.
```

Discard reuses it with the hunks to *keep*
(`crates/but-workspace/src/tree_manipulation/discard_worktree_changes.rs`). The
diff itself is gitoxide's — `gix::diff::blob::diff_with_slider_heuristics` and
`gix::diff::blob::UnifiedDiff` with the pipeline in
`Mode::ToGitUnlessBinaryToTextIsPresent` (`crates/but-core/src/unified_diff.rs`)
— so `textconv` is honoured, and the UI type carries
`isResultOfBinaryToTextConversion: … hunk-based operations must be disabled`.
Whether GitButler ever exposes a patch to the user for this path: no (READ THE
SOURCE, absence). The older request that motivated line granularity is
<https://github.com/gitbutlerapp/gitbutler/issues/2513>.

**Confidence:** READ THE SOURCE, pinned
(<https://github.com/gitbutlerapp/gitbutler/blob/6d0e7c0d345edaacbc520966ffb3feebec6cc3c2/crates/but-core/src/hunks.rs>,
<https://github.com/gitbutlerapp/gitbutler/blob/6d0e7c0d345edaacbc520966ffb3feebec6cc3c2/apps/desktop/src/lib/hunks/hunk.ts>).
**Implies:** the one Rust-and-gitoxide client in the field chose **ranges over
two images** as its portable model and never produces unified text for staging.
That is directly relevant to Cairn's D1 split: GitButler's route mutates the
index through gitoxide, which Cairn's ops rule forbids — so Cairn *must* produce
patch text for `git apply`, and the question is only whether the model is
patch-text-shaped (Desktop, lazygit, Magit) or range-shaped and rendered to a
patch at the boundary (a hybrid nobody in this record ships).

### Finding 21 — gitui and tig: a libgit2 blob-writer and a one-line header arithmetic

**gitui** stages hunks through libgit2's `Repository::apply` with a
`hunk_callback` selecting one hunk by hash (`ApplyLocation::Index`, or `WorkDir`
for reset — `asyncgit/src/sync/hunks.rs`), but stages **lines** the VS Code way:
`apply_selection` (`asyncgit/src/sync/staging/mod.rs`) rebuilds the file from the
indexed content plus the selected hunk lines, then `repo.blob(new_content)` and
`index.add(&entry)` (`stage_tracked.rs`); `is_stage` false handles unstage by the
same routine over the cached diff, and `discard_lines` writes the rebuilt file to
the working tree (`discard_tracked.rs`). It **stops processing a hunk at an
end-of-file-newline marker** (`DiffLineType::DeleteEOFNL | AddEOFNL => break`) —
a known limitation rather than a rule.

**tig** streams a patch to `git apply --whitespace=nowarn [-p0] [--cached] [-R]
-` (`src/stage.c`, `stage_apply_chunk`) and, for a single line or a partial
chunk, rewrites the hunk header by counting: `if (staged) header.old.lines =
header.new.lines - diff; else header.new.lines = header.old.lines + diff;` where
`diff` is `+1` per added and `-1` per deleted line in the selection
(`stage_apply_line`, `stage_apply_part`), then writes the selected range with
unselected lines converted by `stage_diff_range_write`. Unstage is `--cached -R`;
revert is `-R` alone.

**Confidence:** READ THE SOURCE, pinned
(<https://github.com/gitui-org/gitui/blob/2fa693cb6ed431b21ebc300dd02e83c2476699ce/asyncgit/src/sync/staging/mod.rs>,
<https://github.com/jonas/tig/blob/1b86f070a1f6d4c686a09b997fd4249d52a2a272/src/stage.c>).
**Implies:** the field splits 3–3 on *mechanism* (patch text to `git apply`:
Desktop, lazygit, Magit, tig; blob to index: VS Code, gitui, GitButler) but is
unanimous on the *rules* wherever a patch is written.

### Finding 22 — the shared rule set, stated once

Across Desktop, lazygit, Magit and tig, a partial-stage patch is built from the
unified diff by exactly these rules, and no reader deviates:

1. Context lines are always kept.
2. Selected `+` and `-` lines are kept verbatim.
3. An unselected line on the **side that is being changed away from** (`-` when
   staging; `+` when unstaging or discarding) becomes a context line — it is
   still in the file the patch will be applied to.
4. An unselected line on the other side is dropped.
5. For a **new file**, unselected lines are dropped, not converted (Desktop
   explicitly; lazygit via `TurnAddedFilesIntoDiffAgainstEmptyFile` rewriting
   `--- /dev/null` to `--- a/path` so partial selections apply).
6. The hunk header's counts are recomputed from what was emitted; the start
   lines are either kept from the source hunk (Desktop, tig) or adjusted with a
   running offset (lazygit, Magit via `diff-fixup-modifs` +
   `magit-apply--adjust-hunk-new-start`).
7. A hunk with no change left after 1–4 is omitted.
8. `\ No newline at end of file` is kept with the line it annotates (Desktop),
   dropped when its addition was dropped (lazygit), dropped when outside the
   region (Magit), or a hard stop (gitui).
9. **Unstage** is the same formatter with the sides swapped (lazygit's `Reverse`,
   Magit's `op`), applied with `--reverse --cached` — except Desktop, which has
   no unstage.
10. **Discard** is either the reverse-applied stage patch without `--cached`
    (lazygit, Magit, tig) or a separately inverted patch applied forward
    (Desktop); Magit alone handles the both-staged-and-unstaged case with two
    applies and `--reject`.
11. Every `git apply` caller passes `--whitespace=nowarn` or
    `--ignore-space-change`, and Desktop passes `--unidiff-zero`; Magit refuses
    to apply at zero context; lazygit adds `--3way` for custom patches.

**Confidence:** READ THE SOURCE, four implementations.
**Implies:** rule 8 is the one with no consensus, and it is exactly the case
where a wrong choice yields a patch git rejects or a file whose last newline
silently changes. It needs a test in Cairn's model regardless of which reading
is chosen.

## Part E — engines, options and limits

### Finding 23 — what runs the diff, and what it honours

| Client | Diff engine | `diff.algorithm` | renames | whitespace / context | `textconv` / drivers |
| --- | --- | --- | --- | --- | --- |
| Fork | `git` CLI — Fork "parses git CLI output" and expects English (<https://github.com/fork-dev/Tracker/issues/1339>), asks for a bundled binary (<https://github.com/fork-dev/Tracker/issues/38>) | OPEN | shows old/new names (2.28) | own toggles (Finding 11) | "Fork already respects the textconv option" but ignores a driver's `command` (<https://github.com/fork-dev/TrackerWin/issues/1566>, user claim) |
| Sourcetree | Windows: libgit2 for reads since 2.x ("Sourcetree 2.x switched to using libgit2 for most read actions", <https://blog.sourcetreeapp.com/2018/02/26/making-you-faster-with-sourcetree-2-4-for-windows/>), git.exe otherwise, with a "disable libgit2" option; Mac: embedded or system git (<https://support.atlassian.com/sourcetree/kb/using-embedded-git-or-system-git-in-sourcetree/>) | OPEN | OPEN | Ignore whitespace toggle | OPEN |
| GitKraken | libgit2 through NodeGit (<https://www.gitkraken.com/blog/nodegit-libgit2>); a 2024 talk describes a "shift to native git" (<https://www.linkedin.com/posts/cmgriffing_the-shift-to-native-git-from-libgit2nodegit-activity-7191884930191122433-nd2N>, weak) | OPEN | OPEN | OPEN | OPEN |
| Sublime Merge | "Sublime Merge wraps around the core Git functionality … you can view the exact Git commands" (<https://www.sublimemerge.com/docs/faq>); bundles git 2.30.2 (build 2050) | OPEN | OPEN | own ignore-whitespace (space/tab only) | OPEN |
| Tower | OPEN (no vendor statement found) | OPEN | OPEN | footer toggles | OPEN |
| GitButler | `gix::diff::blob` with `diff_with_slider_heuristics(algorithm)` | passes an algorithm; which one and whether from config: OPEN | via gix rewrites | own context setting (`context_lines`) | yes — pipeline `ToGitUnlessBinaryToTextIsPresent`; hunk ops disabled after conversion |
| GitHub Desktop | `git diff --no-ext-diff --patch-with-raw -z --no-color [-w]`, `-M` for commits | git's (honours config) | `-M` | `-w` toggle; 20-line expansion | `--no-ext-diff` disables drivers' `command`; textconv still applies (git) |
| VS Code | editor's own diff for the gutter/staging; `git diff --name-status … --find-renames=N%` for lists | no (editor) | `git.similarityThreshold` | editor settings | no (editor) |
| lazygit | `git diff` / `git show` with `--no-ext-diff`, `--unified=N`, `--ignore-all-space` opt, `--find-renames=N%` | git's | 50 % default, `(`/`)` adjust | config | `--no-ext-diff`; textconv via git |
| gitui | libgit2 `DiffOptions` (context, interhunk, ignore_whitespace) | libgit2's Myers | `renames_head_to_index` in status | runtime toggles | OPEN |
| Magit | `git diff --stat --no-ext-diff` + transient | `--diff-algorithm=` in the transient | git's | `-b -w`, `-U`, `-W` | `--no-ext-diff` |
| tig | `git diff-index --textconv -C` / `git diff-files --textconv -C` / `git show` | git's | `-C` | `[`/`]`, `W`, `diff-options` | `--textconv` explicit |

**Confidence:** per cell.
**Implies:** three of the four terminal/editor clients that shell out pass
`--no-ext-diff` so a repository's diff *driver* cannot replace the patch they
must parse — and still get `textconv` because git applies it before producing
the patch. That is the same line Fork's users report Fork drawing. Nobody in
the record reads `diff.algorithm` except through git itself.

### Finding 24 — git's own defaults, from the pinned documentation

- `diff.algorithm`: `myers` — "The basic greedy diff algorithm. Currently, this
  is the default." (`Documentation/config/diff.adoc`)
- `diff.renames`: "Defaults to true. Note that this affects only git diff
  Porcelain like git-diff and git-log, and not lower level commands such as
  git-diff-files." — so a plumbing-based engine gets no renames unless it asks.
- `diff.renameLimit`: "If not set, the default value is currently 1000." The
  `-l<num>` option: "prevents the exhaustive portion of rename/copy detection from
  running if the number of source/destination files involved exceeds the
  specified number … a value of 0 is treated as unlimited."
- `merge.renameLimit`: "If neither merge.renameLimit nor diff.renameLimit are
  specified, currently defaults to 7000."
- `-M` similarity: "The default similarity index is 50%."
- `diff.context`: 3; `diff.interHunkContext`: 0 unless set;
  `diff.wsErrorHighlight`: `new`.
- `core.whitespace` defaults: `blank-at-eol,blank-at-eof,space-before-tab`.
- `core.bigFileThreshold`: "The default is 512 MiB." Files above it "Will be
  treated as if they were labeled "binary" (see gitattributes). e.g. git-log and
  git-diff will not compute diffs for files above this limit."
  (`Documentation/config/core.adoc`)

**Confidence:** DOCUMENTED BY VENDOR, at git commit
`12cb6293d6288865c1a133cf22accbaf99d13eb6`.
**Implies:** two of these are traps for a gitoxide-backed engine: `diff.renames`
is a *porcelain* default, so matching `git` means turning rename detection on
deliberately; and `core.bigFileThreshold` is the one size cap git itself
imposes on diffs, and it is 512 MiB — far above every client cap in Finding 25.

### Finding 25 — per-file size caps and "too large" thresholds across the field

| Client | Threshold | Behaviour | Source |
| --- | --- | --- | --- |
| Sourcetree (Windows) | "Size Limit (Text)" default **1024 KB**; also "Max File Count", "Max Diff Line Count", "Size Limit (Binary)" in Options → Diff | shows "No changes in this file have been detected, or it is a binary file" — misleading, per the thread | <https://community.atlassian.com/forums/Sourcetree-questions/SourceTree-incorrectly-shows-a-large-text-file-as-binary-doesn-t/qaq-p/612319> |
| Sourcetree | "Max Diff Line Count" | one thread reports new files truncated at **100 lines** regardless of the setting (3.4.27, 3.4.30) | <https://community.atlassian.com/forums/Sourcetree-questions/Diff-view-only-ever-shows-100-lines-of-text-regardless-of-Max/qaq-p/2436042> |
| Tower | "Show warning for diffs larger than (kB)", default **20 KB** | prompt before displaying | search-result summary of Tower help; not verified against the page |
| Fork | unknown | "Changes are too large to display" with a **Load Diff** button, reported even for a one-character change in a file with 195-character lines; pressing Load Diff on a genuinely large diff "Fork freezes for a while … sometimes Fork crashes" | <https://github.com/fork-dev/TrackerWin/issues/496> (2019), <https://github.com/fork-dev/TrackerWin/issues/1904> (2023) — threshold OPEN |
| GitKraken | **10 MB** binary-detection limit | file treated as binary above it; users with 15–16 MB XML/JSON ask for a setting | <https://feedback.gitkraken.com/suggestions/365949/add-user-setting-for-binary-detection-file-size-limit> |
| Sublime Merge | "Large files are now only diffed when clicked on" | lazy per file | build 1116, 3 Jun 2019 |
| GitHub Desktop | `MaxDiffBufferSize = 70e6` (hard, V8 string limit), `MaxReasonableDiffSize = 70e6 / 16` (~4.4 MB → `LargeText`, "could be displayed but it might cause some slowness"), `MaxCharactersPerLine = 5000` (any longer line → too large) | `LargeText` shows a warning and a button; `Unrenderable` refuses a partial commit | `app/src/lib/git/diff.ts` |
| git | 512 MiB (`core.bigFileThreshold`) | treated as binary | Finding 24 |

**Confidence:** per row; Fork's number is OPEN and Tower's is unverified.
**Implies:** the caps span three orders of magnitude (20 KB to 70 MB) and two
different *units* — bytes, and Desktop's per-line character cap, which is the
only one that targets the actual rendering cost (a 195-character-line generated
file is what tripped Fork's users). Sourcetree's and Fork's misleading messages
are the cautionary tale: whatever the cap, the message must say *what* was
exceeded and offer the load anyway.

## What the evidence does NOT settle

- **Whether GitKraken's Split View allows line staging.** The docs say "from the
  diff view" without a mode; no feedback item found either way.
- **What Sourcetree, GitKraken, Sublime Merge and GitButler show for a merge
  commit.** Only Fork (first parent, tracker premise), Tower (setting), gitui
  (first parent, source) and the git-delegating terminals (combined) are known.
- **Fork's "too large" threshold**, and whether it is bytes, lines or line
  length.
- **Whether Fork or Sublime Merge compute the diff they draw themselves or parse
  `git diff`.** Both are documented to run git for *commands*; the rendering
  engine is not described. Fork's honouring of `textconv` and not of a driver's
  `command` is consistent with parsing `git diff --no-ext-diff`, and that is
  INFERRED.
- **GitButler's diff algorithm** — `diff_with_slider_heuristics(algorithm)`
  takes one, and which, and whether it reads `diff.algorithm`, was not traced.
- **Similarity percentage in any UI.** No vendor doc shows one.
- **Mode-change presentation.** Absent from every vendor document found.
- **Any vendor's file-list virtualization.** The only file-list performance
  evidence is Sourcetree's tree-view hang (Finding 13).
- **Whether the 3–3 split in Finding 21 reflects a considered trade-off.** No
  source in the record explains *why* VS Code and GitButler write blobs instead
  of patches; both are consistent with "we had the file contents anyway".

## Implications for O1 and the model

Evidence only; the design pass chooses.

1. No client defaults to side-by-side; every one that has it offers it as a
   toggle (Finding 9). Sourcetree and Tower have shipped without it for over a
   decade.
2. Line staging in side-by-side exists in Fork, Sublime Merge, GitHub Desktop
   and VS Code (Finding 15), so "line selection is a unified-only affordance" is
   not supported.
3. In the one readable client with both views, the selection is a set of
   **unified-diff line indices** and the split view maps row + column back to
   that index (Finding 15, 16); the model is independent of presentation.
4. Fork's split view is the counter-example: line selection "only on one side at
   a time" is an open complaint, i.e. a split view that does *not* map back to a
   shared model is the bug users notice (Finding 15).
5. Every patch-writing client uses the same eleven rules (Finding 22); the only
   unsettled one is `\ No newline at end of file`.
6. Two of six readers never write a patch and write a blob instead (Findings 19,
   20, 21); Cairn's D1 rules that route out, so patch text is the boundary.
7. GitButler's range-over-two-images model shows the patch can be *rendered* from
   ranges at the boundary rather than being the model (Finding 20).
8. Merge commits: first parent when the client computes the diff, combined when
   git does, a setting in Tower (Finding 8); combined is not `apply`-able.
9. Every `git apply` caller passes a whitespace flag, and two refuse or break at
   zero context (Findings 11, 22).
10. Caps are 20 KB–70 MB by bytes, plus Desktop's 5,000-characters-per-line rule;
    misleading "too large" messages are a documented user complaint (Finding 25).

## Open questions for the planner

1. Does O1 need "both" at all for the packet, given that no client defaults to
   split and two incumbents never shipped it — or is split a later packet over
   the same unified-indexed model?
2. If the model is a set of unified-diff line indices (Desktop) versus
   `HunkHeader` ranges over old/new images (GitButler), which does Cairn's
   `cairn-model` carry across the seam, and which side renders the patch text?
3. Which `\ No newline at end of file` rule (Finding 22, rule 8) does Cairn
   adopt, and which fixture pins it?
4. Should unstage be the stage formatter with sides swapped and `--reverse
   --cached` (lazygit, Magit), and should discard be a separately inverted patch
   (Desktop) or the reverse-applied stage patch (lazygit, Magit, tig)? Magit's
   both-staged-and-unstaged handling is the case to decide explicitly.
5. Merge commits: first parent (Fork, gitui), combined (git, lazygit, tig,
   Magit), or Tower's three-way setting — and is the combined format a separate
   `cairn-model` type since it cannot be applied?
6. Does Cairn turn rename detection on deliberately to match `git`'s porcelain
   default (Finding 24), and at what limit — git's 1000, or a lower one with a
   visible "renames not detected" state?
7. What is the per-file cap, in what unit (bytes, lines, characters-per-line),
   and what does the message say and offer?
8. Given that the field puts whitespace-ignore on the pane and Magit/Fork show
   it breaking the patch, does a whitespace-ignored view disable line staging,
   re-diff for the patch, or apply with `--ignore-space-change`?
9. Is intra-line highlighting default-on with no toggle (Fork, GitKraken,
   Sublime Merge), and at token or character granularity?
10. Which of the residual OPEN items above (GitKraken split staging, Fork's
    threshold, merge display in four clients) is worth a hands-on check on the
    project owner's Fork install before the design locks?
