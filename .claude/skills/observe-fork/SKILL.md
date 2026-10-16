---
name: observe-fork
description: Check what Fork for Windows actually does — its wording, dialogs, buttons, selection and the git it runs — by booting the local Windows VM and driving Fork through the fork-observer agent. Use before a UI decision rests on "as Fork does" and docs/research/ holds no first-hand observation of it, or when the user asks "what does Fork do?". Trigger on "/observe-fork <questions>", "check Fork in the VM", "what does Fork show for …".
user-invocable: true
---

# /observe-fork

Answers a "what does Fork do?" question from Fork itself. Fork is Cairn's UI
standard, and a claim about it reaches a PRD only once it has been seen
(`docs/design/ui.md`); a tracker thread, a release note or memory is a lead,
not evidence.

## Step 1: Is it already observed?

Search `docs/research/*/fork-observed-*.md` and `fork-*-20*.md` for the
behaviour. If a VERIFIED row answers it, cite that row and stop: the VM costs
tens of minutes.

## Step 2: Write the questions

List each behaviour to observe as a question Fork's UI can answer by being
driven — "what does the row under Unstage say", "does the output pane open at
the top or the end" — with the repository state it needs. One VM run should
carry every question the current decision needs; batch them.

## Step 3: Dispatch the agent

Spawn the `fork-observer` agent in the background, with:

- the questions, in priority order;
- the observation file to write: `docs/research/<topic>/fork-<subject>-<YYYY-MM-DD>.md`
  in the working tree, or the session scratchpad when nothing will be decided
  from it;
- the screenshot prefix (two or three letters not yet used in `~/VMs/fork-win10/shots/`);
- a hard time limit: 50 minutes, VM included, reporting whatever it has when the
  limit comes.

Set a background check-in timer for the same limit and stop it when the agent
reports; never poll the VM yourself while the agent drives it.

## Step 4: Use the answer

- Confirm the agent reported the VM off (`pgrep -af 'name fork-win10'` prints
  nothing but itself).
- Relay the findings to the user plainly: what Fork showed, and where Cairn
  would follow it or deviate. Fork's bugs (for example a failed commit Fork
  marks "succeeded") are named as bugs, and copying one is the user's call.
- A decision taken from the observation cites the observation file, and the
  file lands in `docs/research/` with the change that relies on it.
