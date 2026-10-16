# Fork for Windows 2.21.1: discarding a nested repository, observed first-hand (2026-10-10)

Evidence, saved verbatim and never edited after: the "nested repository" section of the
coordinator's first-hand Fork notes of 2026-10-10, copied here because
`fork-observed-2026-10-10.md` beside it is never edited. It is the evidence for the user's
decision of 2026-10-10 on phase 15's Q6 (PRD R8.4): a nested repository is left as it is.

Driven in a Windows 10 22H2 VM (QEMU/KVM), git 2.56.0.windows.2, a test repository with a local bare
remote. Screenshots: ~/VMs/fork-win10/shots/ (lc*, ln*, dc*, cb*, am*, lk*, amd*). VERIFIED = seen on screen.

## Discarding a selection that holds an untracked nested repository (observed 19:17)
- Repo `n`: `f.txt` modified, `lib/` an untracked repository with its own commit ("inner"). Fork lists `lib` as a
  folder holding one nameless added row. (n5)
- Select the nameless `lib` row and `f.txt`, right-click: **Discard changes…** offered, no warning. (n6)
- Dialog: **"Discard changes"** / **"Do you want to discard all your changes in the selected files?"** /
  **"Discard Changes in 2 Files"** (focused) / **"Cancel"** — the nested repository is not mentioned. (n7)
- Confirm: **Git Error** with an internal .NET exception: `[Internal]: System.UnauthorizedAccessException: Access
  to the path '9efe8e…' is denied … at Fork.Git.Commands.DiscardFileChangesGitCommand.DeleteFile`. Fork tried to
  delete `lib/` recursively itself (not through git). (n8)
- After: `lib/` holds only a partial `.git` (`x.txt` gone); `git -C lib log` shows the OUTER repo's commit — the
  nested repository is broken and its commit lost. `f.txt` is **still modified**: the discard of the other file did
  not happen. (n9)
- So Fork treats a nested repository as an untracked folder to delete, says nothing, and on Windows half-deletes it
  and fails; on a filesystem that allows the delete it would remove it and its history entirely.
