# Phase 02 — The askpass helper

```
STEP 0  Pre-flight: read docs/work/credential-prompts/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/credential-prompts before
        editing. Verify phase 01 is present at the integration tip.
STEP 1  Load context via an Explore agent over crates/, the evidence record
        docs/research/credential-prompts/git-credential-delegation.md, and
        docs/prd/credential-prompts.md (requirements R2 and R3). Do not read the
        other planning docs directly.
STEP 2  Decide O2 and O3, then implement.

        DECIDE — O2: the helper/app channel. The candidate is a unix domain
        socket in the user's runtime directory with a per-operation single-use
        token. Justify whatever you choose in brainstorm.md: this channel carries
        plaintext secrets between two processes and its permissions are the
        entire security story. State explicitly what a hostile local process can
        and cannot do.

        DECIDE — O3: whether the secret type can zero on drop without a new
        dependency. A dependency is a user decision; a hand-rolled zeroing the
        optimiser removes is worse than none, because it looks safe. Decide with
        evidence — read the generated code or use a crate that documents the
        guarantee.

        Deliverables:
        1. A `cairn-askpass` binary: invoked by git with the prompt as argv[1],
           obtains the secret from the running Cairn instance, writes it to
           stdout with a trailing newline, exits. It does nothing else, links as
           little as possible, and never writes to a log.
        2. The channel from O2, with its access control implemented and its
           threat model written down in the module docs — not in a commit
           message, where nobody will find it.
        3. The secret type: no Debug, no Display, no Serialize, zeroes on drop
           (O3). A containing type that derives Debug defeats it, so the type
           must make that hard.
        4. The askpass environment variables added to phase 01's builder:
           GIT_ASKPASS, SSH_ASKPASS, SSH_ASKPASS_REQUIRE=force.
        5. THE GUARD for "no credential value is logged, Debug-printed,
           serialised, or stored in application state": a guard test over the
           type's impls plus a forbidden-token check, with matcher self-tests
           proving it fires on the disguised forms. Add the invariant to
           CLAUDE.md in the same commit.

        Invariants in play: a secret never travels on argv (L6) — /proc makes it
        world-readable; never commit secrets; every invariant gets its twin in
        the same change; a new dependency is a user decision.

        Out of scope: the UI dialog (phase 03), fetch (phase 03), push, clone,
        proxy configuration, GPG signing.
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: run /qa over the phase diff with destructive-ops-reviewer,
        gate-integrity-reviewer (this phase changes the enforcement layer) and
        test-coverage-auditor spawned fresh, plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to the qa-confirm
        agent (fresh), never this session inline; log dismissed findings with
        reasons in progress.md; fix confirmed findings in focused fixes;
        disputed findings go to the user.
STEP 4  Acceptance: PRD criteria B6 and B7 pass against their tests.
STEP 5  Update state.md (symbol table, O2 and O3 resolved) and progress.md. Save
        memory-worthy decisions.
STEP 6  Branch authority follows the declared mode, as phase 01.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if O2 has no option whose threat model you
can state honestly, or if O3 needs a dependency. Both are user decisions: one is
a security posture, the other is the dependency rule. Otherwise do not stop for
permission.
```

## QA brief

This phase's defects are the kind that pass every test and leak anyway.

- Read the helper's linked surface. Every crate it pulls in is code running in a
  process that holds a plaintext secret; a logging framework linked in "for
  convenience" is a finding.
- Check for a secret in a panic path. `unwrap` on a channel carrying the secret
  can print it in the panic message — the clippy floor denies unwrap outside
  tests, so confirm nothing was allowed through with an attribute.
- Confirm the forbidden-token guard fires on the disguised forms: a secret moved
  into a struct that derives Debug, a `format!("{:?}")` on a container, a
  `tracing` field. A guard that only catches `println!("{secret}")` is theatre.
- Verify the zeroing (O3) actually happens rather than being optimised out. If
  you cannot verify it, say so in progress.md rather than claiming it.
- Test the failure paths, not just the success path: helper cannot reach the app,
  app has no UI, user cancels, token already used, socket has wrong permissions.
  Each must fail closed.
