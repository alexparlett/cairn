# Feature inventory

Intent, not as-built. Nothing here exists unless `docs/systems/` says so; the
root `CLAUDE.md` status paragraph is the authority on what is built today.

Assembled 2026-09-14 against Fork's own published feature list (the reference
client, per `cairn.md`) plus what a git client needs that Fork does not
advertise. ~69 items. The count is the least interesting thing about it — the
useful structure is which few things everything else depends on, and which few
can lose someone's work.

## How this is organised, and why not by menu

Grouped by **risk**, because that is what Cairn's architecture is organised
around: the `Confirmed` seal, `destructive-ops-reviewer`, and the `ops/`
confinement all exist to serve the destructive tier. A menu-shaped list would
scatter those fourteen items across every group and hide the thing that matters.

**[D]** marks an operation that can lose work a user has not got another copy of.

## Tier 0 — Foundations

Everything downstream consumes these. Getting one wrong is a rewrite, not a fix.

| Feature | Note |
| --- | --- |
| Repository open / discovery | Minimal version is `history-graph` R5: a command-line argument. The manager shape is deliberately parked. |
| History graph | `history-graph` packet. Incremental lane assignment (D4). |
| Refs enumeration | Branches, remotes, tags, stashes. Feeds graph decoration, branch UI, and every ref operation. |
| Working-tree status | Changed, staged, untracked, ignored, conflicted. The other half of what staging needs. |
| **Diff model and rendering** | The single most load-bearing item after the graph — see below. |

### Why diff is the one to get right

Nine consumers: commit details, working-tree changes, hunk staging, line staging,
compare revisions, conflict resolution, image diffs, stash contents, and
interactive-rebase preview.

And the trap: **a diff model built for display makes line-level staging
impossible without a rewrite.** Staging one line means *constructing a patch* and
handing it to `git apply --cached` — which is what `git add -p` does internally.
The model must be able to emit a valid patch for an arbitrary subset of hunks and
lines, including correct hunk headers and context, from the moment it exists.
Fork headlines line-by-line staging; it is the feature that most demands the
foundation be right the first time.

## Tier 1 — Safe reads

No risk, and most of the perceived quality of a git client lives here.

| Feature | Note |
| --- | --- |
| Commit details | Message, author, committer, parents, refs pointing at it. |
| File tree at a revision | Browse the repo as it was at any commit. |
| Blame | Per-line last-change attribution. `gix-blame` is available. |
| File / path history | All commits touching a path. |
| Commit search | `--grep`, author, date, and `-S`/`-G` pickaxe. Fork under-serves this; at scale it is badly missed. A differentiator. |
| Compare arbitrary revisions | Two commits, or a branch against its upstream. |
| Reflog view | Fork sells this as "restore lost commits". It is the recovery story for commit-level destruction — see Recovery below. |
| Stash contents | Fork shows stashes inline in the commit list rather than in a side panel; worth copying. |
| Submodule status | Which submodules exist, which are dirty, which are behind. |
| Image diffs | Fork advertises this. Common, concrete, moderate cost once the diff foundation exists. |
| Diff options | Whitespace handling, word-level intra-line diff, context lines, rename detection. |
| Repository summary / statistics | Fork has it. Cheap, and nobody opens it twice. Late or never. |

## Tier 2 — Working-tree mutations

| Feature | Note |
| --- | --- |
| Stage / unstage file | |
| Stage / unstage hunk | |
| Stage / unstage line | Fork headline. Requires the patch-capable diff model. |
| Discard file changes **[D]** | |
| Discard hunk / line **[D]** | |
| Clean untracked files **[D]** | |
| Commit | Must run hooks (D1) — that is the whole reason writes go through `git`. |
| Amend **[D]** | Rewrites a commit; the old one survives only in the reflog. |
| `.gitignore` editing | |
| Stash create / apply / pop | |
| Stash drop **[D]** | |

## Tier 3 — Ref mutations

| Feature | Note |
| --- | --- |
| Create branch | |
| Rename branch | |
| Delete branch **[D]** | Unmerged commits become unreachable. |
| Checkout / switch **[D]** | Destructive only when the working tree is dirty — which is exactly when a user does not expect it to be. |
| Set upstream | |
| Create tag | |
| Delete tag **[D]** | |

## Tier 4 — History rewrites

The tier the architecture exists for. Every one of these needs `Confirmed` and
`destructive-ops-reviewer`, and every one can leave the repository mid-operation.

| Feature | Note |
| --- | --- |
| Merge | Plus conflict resolution (D6). |
| Rebase | Abort / continue / skip state machine. |
| Interactive rebase | Not a feature — a subsystem. See below. |
| Cherry-pick | |
| Revert | The one non-destructive member: it adds a commit. |
| Reset soft / mixed | Recoverable via reflog. |
| Reset hard **[D]** | Loses uncommitted work with no recovery at all. |
| Squash / fixup **[D]** | |

### Interactive rebase is a program, not a packet

It needs the graph, commit details, diff, conflict resolution, a todo-list editor
UI, an abort/continue/skip state machine, and reflog for recovery. It is the last
thing Cairn builds, and it gets its own program. Fork leads its marketing with it
because it is the hardest thing in this space to do well — which is the same
reason it comes last here.

## Tier 5 — Remote

| Feature | Note |
| --- | --- |
| Fetch | `credential-prompts` packet. |
| Pull | Merge or rebase; the choice must be visible, not buried in config. |
| Push | |
| Force push **[D]** | `--force-with-lease` is the default; plain `--force` should be hard to reach. |
| Clone | |
| Remote add / edit / remove | Distinct from *creating* a repository on a platform, which is out of scope. |
| Prune remote refs **[D]** | Deletes local tracking refs. |
| Push / delete tags **[D]** | |

## Tier 6 — Repository management

| Feature | Note |
| --- | --- |
| Recent repositories | |
| Multi-repository UI | Tabs, sidebar, or windows — parked in `cairn.md`, "Still open". |
| Init | |
| **Worktrees** | First-class (D8): list, create, switch, remove, and show which worktree holds a branch. A differentiator; Fork does not advertise it. |
| Submodule init / update / sync | |
| Git LFS | Comes free from D1 — `git` applies the filters. Needs verifying, not building. |
| `git config` editing | |

## Tier 6½ — Forge links

One mechanism, several commands, and the whole group needs no API token and no
network call from Cairn: read the branch and its upstream, read the remote URL,
identify the forge, construct a URL, hand it to the system opener. Decision D9
draws the line — links in, APIs out.

| Feature | Note |
| --- | --- |
| **Create pull request for the current branch** | The command that caused D9. One of the most-used context-menu items in Fork, and the thing a user wants *immediately after pushing* — so it wants designing together with push, not bolted on beside it. |
| Open commit / branch / tag in browser | |
| Open file at a revision in browser | |
| Copy permalink to a selected line | Same machinery; heavily used for sharing code with someone. |
| Open compare view between two refs | |

**The wrinkle:** github.com and gitlab.com are identifiable from the hostname;
self-hosted GitLab, Gitea and Forgejo are not. The forge table is DATA, so adding
one is an entry rather than code, and an unrecognised remote gets **no menu item**
rather than a guessed URL and a 404.

## Tier 7 — Application shell

| Feature | Note |
| --- | --- |
| Preferences | |
| Light / dark theme | Partly present. |
| Accelerator table | One logical-action-to-chord map (D5). Not per-component literals. |
| Command palette | |
| Open in terminal / editor | |
| External diff / merge tool | The escape hatch D6 depends on. |
| Update mechanism | |
| Accessibility | Keyboard reachability throughout; lane identity legible without colour. |
| Error and notification surface | |
| Operation log | Already implied by `ops::Performed` — it quotes the prompt the user accepted. Half the recovery story. |

## Recovery: two classes, two stories

The most important structural point in this document, and the one easiest to get
wrong by assuming reflog covers everything.

**Committed work** — reset `--hard`, branch delete, rebase, amend, squash. The
reflog holds it, so recovery means *making the reflog visible and usable*. That
is why the reflog view ships alongside the first commit-level destructive
operation and not later.

**Uncommitted work** — discard file, discard hunk, discard line, clean untracked,
reset `--hard` over a dirty tree. **The reflog does not help at all. There is no
recovery.** A confirmation dialog is the only barrier, which makes it a thin one.

The option worth considering for that second class: Cairn **auto-stashes before a
destructive working-tree operation**, giving the one thing git itself does not — a
way back from a discarded edit. It would be a genuine differentiator and it fits
the `Confirmed` design rather than fighting it. Not decided; raised here so the
staging packet meets it deliberately.

## Explicitly out of scope

Each with the reason, so the next person does not relitigate it.

**Reviewed against real usage on 2026-09-14** and confirmed, with one change: the
pull-request entry moved into scope as D9, because creating a PR turned out to be
URL construction rather than API work, and one of the most-used commands in the
reference client. Everything below survived that review. Treat the list as
validated rather than asserted — reopening an entry wants new information, not a
fresh opinion.

| Not doing | Why |
| --- | --- |
| *Reviewing* pull requests, reading or filing issues | D9: panels need an API token and go stale. Note that *creating* a PR does not — it is a URL, and it is in scope (Tier 6½). |
| CI status | D9: it needs a per-forge API token, which would make Cairn a credential holder. D2's premise is that it never is, so this is inconsistent with a decision already taken — not merely expensive. |
| Creating / deleting repositories on a platform | Fork does this. Genuinely API work, so D9 puts it out. |
| Being a text editor | `cairn.md`. D6 keeps conflict resolution *structured* precisely to stay on this side of the line. |
| Git-flow | Fork has it. A lot of UI for a convention that has fallen out of fashion. |
| A git tutorial | `cairn.md`: Cairn assumes the user knows what a rebase is. |
| Windows | `cairn.md` D5: Linux first, macOS where it is cheap. |
| `git bisect`, `filter-branch` / `filter-repo` | Rare, and better in a terminal where the output is the point. |

## Speed is a feature, not a consequence

Fork's tagline is "fast and friendly", and Cairn's competitive claim rests on the
same ground. Choosing gitoxide (D1) does not deliver it; it only makes it
possible. Every surface in Tier 0 and Tier 1 needs a stated, measured bar the way
`history-graph`'s A7 does — against a named real repository, with the numbers
recorded. An unmeasured performance claim decays silently, and the decay is
invisible until a user with a big repository finds it.
