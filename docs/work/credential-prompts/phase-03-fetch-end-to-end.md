# Phase 03 — Fetch, end to end

```
STEP 0  Pre-flight: read docs/work/credential-prompts/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/credential-prompts before
        editing. Verify phases 01 and 02 are present at the integration tip.
STEP 1  Load context via an Explore agent over crates/cairn-git/src/ops/,
        crates/cairn-ui/src/, crates/cairn-app/src/, and
        docs/prd/credential-prompts.md (requirement R4 and the product rules).
        REQUIRED: verify Freya's dialog/modal and focus APIs against the vendored
        source at ~/.cargo/registry/src/index.crates.io-*/freya-0.5.0-rc.6/ and
        freya-components-0.5.0-rc.6/. Do not read the other planning docs
        directly.
STEP 2  Settle O4 and O5 BY TEST, then implement.

        SETTLE — O4: the precedence between GIT_ASKPASS, core.askPass and a
        configured credential.helper, and whether setting the askpass variables
        can suppress a helper the user configured. This is unverified in the
        evidence record and it can invalidate L7, the packet's central product
        rule. Settle it with a test against a stub helper, not by reading docs.
        Record the finding in the evidence record as an addendum — evidence is
        never retro-edited, so append rather than revise.

        SETTLE — O5: the OpenSSH version that introduced SSH_ASKPASS_REQUIRE and
        the behaviour when it is absent. Record it the same way.

        Deliverables:
        1. `fetch` in cairn-git/src/ops/, using phase 01's backend. It is not
           destructive and takes no Confirmed — say so in its doc comment, so the
           next person does not assume the omission was an oversight.
        2. A credential prompt dialog in cairn-ui that states WHICH remote and
           WHICH URL is asking. A prompt that does not say who wants the password
           trains people to type it into anything.
        3. The app side of phase 02's channel: receive a prompt request, show the
           dialog, return the secret, one use only.
        4. A local git remote fixture — HTTP with a fixed credential, and an SSH
           remote with a passphrase-protected key. Fixtures GENERATE their
           credentials; they never contain them.
        5. Fetch runs off the UI thread and its progress is visible. If the
           history-graph packet's worker boundary has landed, use it; if not,
           keep the mechanism local and say so in state.md rather than inventing
           a second competing abstraction.
        6. Tests for B3, B4 and B5. B4 — no prompt when a credential helper
           already answers — is the regression that protects L7 and is the single
           most important test in this packet.

        Invariants in play: no secret in argv, in a log, or in application state;
        the UI thread never waits on repository work; no secret committed, ever.

        Out of scope: push, clone, submodules, proxies, GPG signing, remembering
        credentials (D2 rules that out permanently, not just here).
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: run /qa over the phase diff with responsiveness-reviewer,
        destructive-ops-reviewer and test-coverage-auditor spawned fresh, plus
        the qa-checklist.md items this phase covers and the QA brief below.
        Adjudication goes to the qa-confirm agent (fresh), never this session
        inline; log dismissed findings with reasons in progress.md; fix confirmed
        findings in focused fixes; disputed findings go to the user.
STEP 4  Acceptance: PRD criteria B3, B4 and B5 pass against their tests.
STEP 5  Update state.md (symbol table, O4 and O5 settled) and progress.md. Save
        memory-worthy decisions.
STEP 6  Branch authority follows the declared mode, as phase 01.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if O4 turns out badly — if setting
GIT_ASKPASS does suppress a configured credential.helper, L7 cannot be satisfied
as designed and the packet needs a different mechanism, which is the user's call.
Otherwise do not stop for permission.
```

## QA brief

- B4 is the criterion to attack hardest. Confirm the test genuinely exercises a
  working credential helper and would FAIL if the implementation prompted anyway.
  A test that passes because the helper is never consulted proves the opposite of
  what it claims.
- Check the SSH path separately from the HTTPS path. They share no code in git
  and sharing a test between them hides a break in one.
- Confirm no secret reaches the operation log, including on the failure paths.
  Authentication failures are where logging gets added under pressure.
- The dialog must name the remote and URL (product rule). Verify it renders the
  actual URL being authenticated against, not a stored nickname that could
  disagree with where the request is going.
- Check for an orphaned helper or a `git` left waiting on a pipe after a cancel.
  A test that passes while leaking a process passes on a fresh machine and hangs
  on a busy one.
