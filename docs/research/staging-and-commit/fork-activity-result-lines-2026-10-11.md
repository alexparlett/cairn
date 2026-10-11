# Fork 2.21.1 (Windows 10 VM) — Activity Manager rows, observed 2026-10-11

Test repo C:\t2\a (clone of bare C:\t2\r.git; second clone C:\t2\b). Screenshots in ~/VMs/fork-win10/shots/.
Popover layout: left list, tabs All / User / Background; each row = bold name (left), time HH:MM (right), result line under it.
Right pane: header = row name (left), "succeeded <d Mon yyyy HH:MM:SS>" (right, grey; cut at the screen edge in these shots), then `$ git ...` in bold mono, then output.

## 1. Unstage
| Row name | Result line | Icon | Right-pane header | Git command | Status |
|---|---|---|---|---|---|
| "Unstage 1 File" | "unstaged" | none | "Unstage 1 File" / "succeeded 11 Oct 2026 09:47:…" | `$ git reset -q --no-refresh HEAD --pathspec-from-file=- --pathspec-file-nul --` | VERIFIED ar1.png |
| "Unstage 2 Files" | "unstaged" | none | "Unstage 2 Files" / "succeeded 11 Oct 2026 09:48:…" | same `git reset -q --no-refresh HEAD --pathspec-from-file=- --pathspec-file-nul --` | VERIFIED ar2.png |

Body below the command for unstage is Fork's own index-refresh trace, not git output: blocks "Old:", "Requested:", "Response:", "Refreshed:", "Result:" listing porcelain status lines (e.g. ` M f2.txt`, `- M f2.txt`) (ar1.png, ar2.png).

## 7a. Fetch, nothing new (Fork's automatic background fetch)
| Row name | Result line | Icon | Right-pane header | Git command | Status |
|---|---|---|---|---|---|
| "Fetch 'origin'" | "Already up to date" | none | "Fetch 'origin'" / "succeeded 11 Oct 2026 09:47:…" | `$ git fetch --prune origin`; body (grey) "Already up to date" | VERIFIED ar3.png; listed under Background tab (and All, not User) ar3b.png |

## 2. Discard
Confirmation dialog first: title "Discard changes", text "Do you want to discard all your changes in the selected files?", buttons "Discard" / "Cancel".
| Row name | Result line | Icon | Right-pane header | Git command | Status |
|---|---|---|---|---|---|
| "Discard Changes" (one file, f1.txt) | "discarded" | none | "Discard Changes" / "succeeded 11 Oct 2026 09:48:…" | `$ git checkout-index --index --force --stdin -z`; body is Fork's Old/Requested/Response/Refreshed/Result trace | VERIFIED ar4.png |
| (discard one selected line in the diff: line-hover "Discard..." -> dialog "Discard changes" / "Do you want to discard all your changes in the selected lines?" / "Discard Line") | — NO ROW ADDED. The line was discarded (f2.txt left the list, ar5b.png) but the Activity Manager's newest row stayed "Discard Changes 09:48" | — | — | — | VERIFIED absence, ar5.png (taken 09:49, after the line discard) |

## Stage (seen in passing)
| Row name | Result line | Icon | Right-pane header | Git command | Status |
|---|---|---|---|---|---|
| "Stage 2 Files" | "staged" | none | "Stage 2 Files" / "succeeded 11 Oct 2026 09:49:…" | `$ git add --force --verbose --pathspec-from-file=- --pathspec-file-nul --`; body starts with git's own `add 'f3.txt'` / `add 'f4.txt'`, then Fork's Old/Requested/Response/Refreshed/Result trace | VERIFIED ar7.png |

## 3. Commit
| Row name | Result line | Icon | Right-pane header | Git command | Status |
|---|---|---|---|---|---|
| "Commit 2 files" (lower-case "files", unlike "Stage 2 Files"; button read "Commit 2 Files") | NONE — the row has an empty second line (row keeps its two-line height) | none | "Commit 2 files" / "succeeded 11 Oct 2026 09:49:…" | `$ git commit --file "C:\t2\a\.git\COMMITMESSAGE"`; body: `[master 1a479bb] third commit` (grey) then ` 2 files changed, 2 insertions(+)` (black) | VERIFIED ar6.png, zoom ar6z.png |

## 4. Amend
| Row name | Result line | Icon | Right-pane header | Git command | Status |
|---|---|---|---|---|---|
| "Amend" | NONE (empty second line) | none | "Amend" / "succeeded 11 Oct 2026 09:50:…" | `$ git commit --amend --file "C:\t2\a\.git\COMMITMESSAGE"`; body: `[master 3680836] third commit amended` (grey), ` Date: Sun Oct 11 09:49:38 2026 +0100`, ` 2 files changed, 2 insertions(+)` | VERIFIED ar8.png |

## 5. Create Branch without "Check out after create"
(Dialog button changes from "Create and Checkout" to "Create" when the box is unticked.)
| Row name | Result line | Icon | Right-pane header | Git command | Status |
|---|---|---|---|---|---|
| "Create branch 'side'" | NONE (empty second line) | none | "Create branch 'side'" / "succeeded 11 Oct 2026 09:50:…" | `$ git branch --no-track side refs/heads/master`; no output below | VERIFIED ar9.png |
| (for contrast, earlier session, WITH checkout) "Create branch 'topic'" | "Switched to branch'topic'" (sic, no space) | none | "Create branch 'topic'" / "succeeded 10 Oct 2026 09:40:…" | `$ git checkout --no-track -b topic refs/heads/main --force`; body "Switched to a new branch 'topic'" | VERIFIED shots/am1.png (earlier session) |

## 6. index.lock: failed stage, then "Remove .git/index.lock"
Dialog (ar10.png): title "Git Error", subtitle "An unexpected error occured while performing the git request" (sic), Fork icon with a yellow ⚠ badge, a scrolling mono box with git's stderr repeated per attempt (bold `fatal: Unable to create 'C:/t2/a/.git/index.lock': File exists.`, then git's "Another git process seems to be running..." advice), buttons "Remove .git/index.lock" (left) and "Close" (right).
| Row name | Result line | Icon | Right-pane header | Git command | Status |
|---|---|---|---|---|---|
| "Stage 1 File" | "stage failed" | yellow ⚠ triangle left of the time ("⚠ 09:51") | "Stage 1 File" / "failed 11 Oct 2026 09:51:…" | `$ git add --force --verbose --pathspec-from-file=- --pathspec-file-nul --`, repeated per attempt; each attempt: bold `fatal: Unable to create 'C:/t2/a/.git/index.lock': File exists.`, grey advice paragraph, then grey "Repository is locked. Retrying..." between attempts; the last attempt ends with the advice (no "Retrying...") | VERIFIED ar11.png (top), ar11z.png / ar11h.png (zoom), ar12.png (end) |
| "Remove .git/index.lock" | — NO ROW ADDED. After pressing the button the newest row stays the failed "Stage 1 File"; the stage was not retried (f5.txt still unstaged) | — | — | — | VERIFIED absence, ar11.png |

## 7. Fetch (toolbar Fetch -> dialog "Fetch" / "Fetch latest changes from remote repository", Remote: origin, "Fetch all remotes" unticked)
| Row name | Result line | Icon | Right-pane header | Git command | Status |
|---|---|---|---|---|---|
| "Fetch 'origin'" (remote had one new commit, pushed from second clone) | "Fetched 1 branch" | none | "Fetch 'origin'" / "succeeded 11 Oct 2026 09:52:…" | `$ git fetch --prune origin`; body is git's stderr verbatim: `remote: Enumerating objects: 4, done.` / `remote: Total 3 (delta 1), reused 0 (delta 0), pack-reused 0 (from 0)` / `From C:/t2/r` / `   12c8bf0..e94e4f0  master     -> origin/master` | VERIFIED ar13.png |
| "Fetch 'origin'" (user fetch, nothing new) | "Already up to date" | none | "Fetch 'origin'" / "succeeded 11 Oct 2026 09:52:…" | `$ git fetch --prune origin`; body (grey) "Already up to date" | VERIFIED ar14.png |
User tab lists the two user fetches, the failed stage, branch, amend, commit, stage, discard, unstages — and NOT the 09:47 automatic fetch, which is Background only (ar15.png vs ar3b.png). The row list gets a scrollbar once it overflows (ar14.png).

## 9. Failed commit (pre-commit hook: `echo hook says no; exit 1`)
Dialog (ar16.png): "Git Error" / "An unexpected error occured while performing the git request", box showing bold `$ git commit --file "C:\t2\a\.git\COMMITMESSAGE"` then "hook says no"; buttons "Skip pre-commit hooks and commit" (left) and "Close".
| Row name | Result line | Icon | Right-pane header | Git command | Status |
|---|---|---|---|---|---|
| "Commit 1 file" | "1 file..." (sic, with ellipsis — apparently a leftover progress line) | NONE — no ⚠ | "Commit 1 file" / "succeeded 11 Oct 2026 09:53:…" (sic: says succeeded although the hook failed and nothing was committed) | `$ git commit --file "C:\t2\a\.git\COMMITMESSAGE"`; body (grey) "hook says no" | VERIFIED ar17.png, zoom ar17z.png / ar17h.png; unchanged 3+ s later, ar18.png |
| "Stage 1 File" (staging f5 before it) | "staged" | none | — | — | VERIFIED ar17.png (row only) |

## Long output (coordinator's extra): pre-commit hook printing `line 1`..`line 500`, sleeping 0.3 s every tenth line (~15 s), exit 0
| What | Observed | Status |
|---|---|---|
| Running row | "Commit 1 file" / result line "1 file..." with a spinner and an × (cancel) button in place of the time | VERIFIED ar19.png, zoom ar22z.png |
| Status box while running | spinner + "Commit 1 file" over "1 file..." and an × at its right edge | VERIFIED ar22z.png |
| Right-pane header while running | "Commit 1 file" / "runnin…" (cut at screen edge) | VERIFIED ar22z.png |
| Scroll position while running | Opened at the TOP (command line, then `line 1`…`line 24` visible) and STAYED at the top in shots ~1 s, ~5 s and ~10 s after the AM opened, while the row still said running — it does not follow new lines (that more lines had arrived by then is inferred from the hook's timing, not seen) | VERIFIED ar19.png, ar20.png, ar21.png |
| Finished row | "Commit 1 file" / NO result line; header "succeeded 11 Oct 2026 09:54:…" | VERIFIED ar23.png |
| Scroll position after completion, and when the row is selected afresh | Still at the TOP (`line 1`) | VERIFIED ar23.png, ar24.png (popover closed, reopened, another row then this one clicked) |
| End of the output (scrolled by hand) | `line 500`, then `[master 501e5f5] fail test`, bold ` 1 file changed, 1 insertion(+)`, ` create mode 100644 f5.txt` — the whole 500 lines kept, not truncated | VERIFIED ar25.png |
Also: the failed commit's row (09:53) still reads "1 file..." — so the result line of a failed commit is the stuck progress line ("1 file..." is the running progress text). VERIFIED ar19.png vs ar23.png.

## 8. Cancelled operation (× on the running row, during the 500-line hook commit)
| What | Observed | Status |
|---|---|---|
| Immediately after pressing × | Row "Commit 1 file" / result line changes to "canceled" (no time, no icon yet); status box: spinner + "Commit 1 file" over "Canceling..."; right-pane header "Commit 1 file" / "canceling…" (cut) | VERIFIED ar27.png |
| It stayed "Canceling..." for >8 s — the hook kept running to the end (output reaches `line 500`); Windows did not end the hook's sh | VERIFIED ar28.png, ar31.png |
| Then a modal | "Git Error" / "An unexpected error occured while performing the git request", box text `[internal] Cancelled`, only a "Close" button | VERIFIED ar29.png |
| Final row | "Commit 1 file" / "canceled", yellow ⚠ before the time ("⚠ 09:55") | VERIFIED ar30.png, zoom ar30z.png |
| Final right-pane header | "Commit 1 file" / "canceled 11 Oct 2026 09:55:…" | VERIFIED ar30h.png |
| Body | `$ git commit ...` then hook output to `line 500`, and nothing after it (no git summary, no "cancelled" text in the pane) | VERIFIED ar31.png |
| Effect | nothing committed: `git log` HEAD still 501e5f5 "fail test", f5.txt still staged (`M  f5.txt`) | VERIFIED ar32.png (PowerShell) |
Scroll note: this time the AM was opened ~3 s into the run and the pane showed `line 35`…`line 61` with 61 at the bottom edge (ar26.png) — i.e. NOT at the top — and it did not move in ar27/ar28 (3 s and 8 s later). See the follow-up experiment below.

## Long output, follow-up experiment (settles the scroll question)
Hook: print `line 1`..`line 100` at once, `sleep 8`, print `line 101`..`line 500` at once, `sleep 8`, `exit 0`. AM opened ~3 s into the run (during the first sleep).
| What | Observed | Status |
|---|---|---|
| On opening a RUNNING entry | Pane scrolled to the END of the output so far: `line 75`…`line 100`, with `line 100` the last line at the bottom | VERIFIED ar33.png |
| While running, after 400 more lines arrived | Pane did NOT follow: still `line 75`…`line 100`, with only `line 101` now peeking under the bottom edge | VERIFIED ar34.png (9 s later, row still running with spinner/×) |
| On completion | Still not moved (`line 75`…`line 101`); header "succeeded 11 Oct 2026 09:57:…"; row "Commit 1 file" with no result line | VERIFIED ar35.png |
| Selecting a FINISHED entry afresh | Opens at the TOP (`$ git commit ...`, `line 1`…) | VERIFIED ar24.png (first run) |
Summary: a running entry opens at the end of what has arrived so far and then stays put (no follow); a finished entry opens at the top. In the very first run (ar19-21) the AM was opened when about 24 lines existed, so "the end" was the top, and it then stayed at the top as lines came in — consistent with this.

## Rows not produced
| Operation | Status |
|---|---|
| Discard of a selected line in the diff | Produces NO row (see section 2) |
| "Remove .git/index.lock" | Produces NO row (see section 6) |
