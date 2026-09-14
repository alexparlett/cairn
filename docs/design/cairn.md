# Cairn — design spine

Intent, not as-built. What exists today is in the root `CLAUDE.md` status
paragraph; what each system actually does, once it does anything, goes in
`docs/systems/`.

## What Cairn is

A native git client for people who already know git and want to *see* it. The
audience is the developer who reaches for Fork or Sourcetree not to avoid the
command line but because a graph, a diff, and a staging area are genuinely better
rendered than printed.

Three things that has to mean in practice:

1. **The history graph is readable at scale.** Not a list of commits with a
   decorative gutter — an actual graph that stays legible across a repository with
   many long-lived branches, and stays interactive while it loads.
2. **Staging is hunk-level and fast.** Selecting, splitting and discarding hunks
   is the operation a GUI wins at. It must be faster than `git add -p`, not just
   prettier.
3. **Destructive operations tell the truth.** Before a force push, a hard reset,
   a branch delete or a rebase, Cairn says what will be lost, how much, and
   whether it can be recovered — in that order, in words, before the button.

## What Cairn is not

- Not a git tutorial. It assumes the user knows what a rebase is.
- Not a platform client. Pull requests, issues and CI live in a browser; a
  half-implemented GitHub panel is worse than a link.
- Not cross-platform-first. Linux is the target; macOS follows where Freya makes
  it free. Windows is not a goal and no design should be compromised for it.
- Not an editor. Diffs are read-only; conflict resolution opens the user's editor.

## The bets

**Freya for the UI.** A Rust-native, Skia-backed, declarative toolkit: no web
runtime, no FFI layer between the view and the data, and a component model that
holds up as the UI grows. The cost is maturity — Freya 0.5 is a release candidate
whose API replaced the previous one wholesale. Accepted deliberately: the
alternative toolkits either bring a browser or bring C++.

**gitoxide for the engine.** Pure Rust, no libgit2, and performance work that is
aimed squarely at large repositories — the case where a GUI lives or dies. The
cost is coverage: gitoxide's read paths are strong while some mutating operations
are still maturing. The mitigation is structural rather than hopeful — the whole
engine sits behind `cairn-model` types, so if a specific operation needs the
`git` binary or a different backend, that is a change inside
`crates/cairn-git/`, invisible to every component. The seam is what makes this
bet cheap to be wrong about, which is why the guard suite defends the seam and
not the backend choice.

**The confirmation token as a type, not a convention.** `cairn_model::Confirmed`
exists because "remember to ask first" is exactly the kind of rule that holds for
a year and then quietly does not. Making it a value a destructive function
demands moves the rule from review to the compiler. Its limit is honest: the type
guarantees a prompt happened, not that the prompt was true — which is why
`destructive-ops-reviewer` exists and what it spends itself on.

## Open questions

Real ones, not placeholders. Each should end as a `/feature-plan` packet or a
decision recorded here with its evidence in `docs/research/`.

- **Graph layout algorithm.** Which lane-assignment approach stays both readable
  and incremental as commits stream in? Incremental matters more than optimal: a
  layout that must see the whole history before drawing anything cannot render a
  large repository at all.
- **Where the worker boundary sits.** One engine thread per repository, a pool, or
  per-query tasks? This decides what cancellation and staleness look like
  everywhere else, so it wants deciding before much UI exists.
- **How far gitoxide carries the write path.** Which mutating operations does it
  cover today at the quality a user's repository deserves, and which need the
  `git` binary in the interim? Worth an evidence-gathering spike rather than a
  guess, since the answer moves as gitoxide releases.
- **Credential handling.** Fetch and push need credentials, and a git client that
  handles them carelessly is a security problem, not a UX one. The likely answer
  is delegating entirely to the user's existing git credential helper and SSH
  agent, but "likely" is not a decision.
- **macOS reach.** What actually differs — window chrome, the menu bar, keyboard
  conventions, the credential store — and how much of it can be absorbed without
  bending the Linux design.
