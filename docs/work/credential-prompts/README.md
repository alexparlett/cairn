# Packet: credential-prompts

Runs an authenticated `git` operation from Cairn without Cairn ever holding a
credential. Lands the `git` subprocess backend that decision D1 requires, the
askpass helper binary that lets git ask a GUI for a secret, and `fetch` wired end
to end to prove the path.

Spec: `docs/prd/credential-prompts.md`. Design frame: `docs/design/cairn.md`
decisions D1 and D2. Evidence:
`docs/research/credential-prompts/git-credential-delegation.md`.

Integration branch: `feature/credential-prompts`, off `main`.

## Phases

| Phase | What it lands |
| --- | --- |
| [01](phase-01-git-backend.md) | the typed `git` subprocess backend in `cairn-git/src/ops/cli.rs` |
| [02](phase-02-askpass-helper.md) | the askpass helper binary, its channel, and the secret type |
| [03](phase-03-fetch-end-to-end.md) | fetch wired end to end, with the prompt UI and the fixtures that test it |
| [04](phase-04-qa.md) | merge-bar QA over the whole packet, then teardown |

Strictly sequential: 02 has nothing to serve without 01, and 03 proves both.
