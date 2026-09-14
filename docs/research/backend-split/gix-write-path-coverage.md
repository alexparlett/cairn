# What gix 0.87.1 can and cannot write

Evidence record behind design decision **D1** (`docs/design/cairn.md`). Gathered
2026-09-14. Method: reading the linked source under
`~/.cargo/registry/src/index.crates.io-*/gix-0.87.1/` — gix is pre-1.0, and the
registry copy is what actually compiles into Cairn, so published docs and release
notes were not treated as authoritative.

Queries run: `grep` over `src/repository/*.rs` for public methods matching
commit/merge/rebase/cherry/reset/checkout/index/stash/push/fetch/branch/tag/
worktree; `grep` for `pub mod`/`pub fn` matching push, rebase, cherry, stash,
reset; inspection of `src/push.rs`, `src/remote/connection/`,
`src/repository/merge.rs`, `src/repository/index.rs` and `src/clone/checkout.rs`.

## Present

| Capability | Where |
| --- | --- |
| Commit creation | `Repository::commit`, `commit_as`, `new_commit`, `new_commit_as` (src/repository/object.rs) |
| Three-way tree and commit merge | `merge_trees`, `merge_commits`, `virtual_merge_base`, `virtual_merge_base_with_graph` (src/repository/merge.rs) |
| Merge bases, including octopus | `merge_base`, `merge_bases_many`, `merge_base_octopus` and `_with_graph` variants (src/repository/revision.rs) |
| Tag creation | `Repository::tag`, `tag_reference` |
| Local branch deletion | `delete_local_branches` (src/repository/branch.rs) |
| Index read and write | `open_index`, `index`, `index_or_empty`, `index_from_tree` (src/repository/index.rs); `gix_index::File::write` |
| Fetch | `src/remote/connection/fetch/` |
| Blame, status, dirwalk | `gix-blame`, `gix-status`, `gix-dir`, re-exported |

## Absent

| Capability | Evidence of absence |
| --- | --- |
| **Push** | `gix::push` (src/push.rs) contains only the `push.default` config enum — `Nothing`, `Current`, `Upstream`, `Simple`, `Matching`. No transport, no ref update, no negotiation. `src/remote/connection/` has a `fetch/` subdirectory and no push counterpart. |
| **Branch-switch checkout** | `gix_worktree_state::checkout` exists, but the only call site in gix is `src/clone/checkout.rs` — populating a fresh worktree after a clone. Nothing computes a delta against a dirty working tree. |
| Reset | No `pub fn` matching `reset` outside `interrupt.rs`, which is about interrupt handling. |
| Rebase, cherry-pick, revert | No matches at all. |
| Stash | No matches at all. |
| Hunk-level staging | The index can be written, but nothing constructs a partial blob from selected hunks. |

## The argument this evidence does NOT make

Coverage is the second reason for D1, not the first. Even if gix covered every
operation, mutations would still go through `git`, because gix runs no hooks. A
client that does not run `pre-commit` and `commit-msg` is broken for a large
share of users, and the same reasoning covers clean/smudge filters (Git LFS),
`core.fsmonitor`, sparse checkout, submodule recursion, and index locking
semantics — every one a place where a reimplementation corrupts a repository
quietly rather than loudly.

That matters for how this record ages: gix filling in push and rebase would NOT
reopen D1. What would reopen it is gix growing a hook-running, filter-applying
execution layer, which is a different and much larger thing.

## Not established

- Whether gix's `merge_trees` matches git's merge result on the cases where
  git's own strategies differ (rename detection thresholds, `diff3` conflict
  markers, submodule conflicts). Cairn does not currently depend on this, since
  merges go through `git`, but a future conflict-preview feature would.
- The cost of a process spawn per mutation on Windows, which is far higher than
  on Linux. Not currently relevant: Windows is not a target (D5).
