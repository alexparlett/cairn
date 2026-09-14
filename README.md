# Cairn

A native git client for Linux, built in Rust with [Freya](https://freyaui.dev)
and [gitoxide](https://github.com/GitoxideLabs/gitoxide). It aims at what Fork
and Sourcetree do well — a readable history graph, hunk-level staging, and
destructive operations that say what they will cost before they cost it —
without shipping a browser to do it.

**Status: early.** The workspace, the engine/UI seam, and the enforcement layer
exist and are green. The window opens and renders an empty history list. No git
query or command is implemented yet. Design intent: [`docs/design/cairn.md`](docs/design/cairn.md).

## Build and run

Needs a Rust toolchain (pinned in `rust-toolchain.toml`; rustup installs it
automatically) and the system graphics/windowing development packages Freya links
against — on a Debian-derived distro that is `build-essential cmake pkg-config
libgtk-3-dev libxdo-dev libssl-dev libglib2.0-dev libwayland-dev
libxkbcommon-dev`; see `.github/workflows/ci.yml` for the list CI installs.

```bash
cargo run -p cairn-app
```

## Layout

| Crate | What it owns |
| --- | --- |
| `cairn-model` | The vocabulary crossing the engine/UI seam. Plain data; depends on nothing. |
| `cairn-git` | The repository engine: gitoxide-backed reads, and every write under `src/ops/`. |
| `cairn-ui` | Freya components. Renders model values; cannot reach a repository. |
| `cairn-app` | The binary — window, worker threads, and the only place the two layers meet. |
| `cairn-guards` | Test-only. The mechanical enforcement twins for the invariants in `CLAUDE.md`. |

The seam is the point: `cairn-ui` compiles with neither `gix` nor `cairn-git` in
its dependency graph, and nothing outside `cairn-git::ops` can mutate a
repository. Both are enforced by tests, not by convention — see
[`CLAUDE.md`](CLAUDE.md) for the full set.

## Checks

```bash
scripts/gate.sh
```

That is the merge bar: format, lint, typecheck, invariant guards, dependency
policy, full test suite. `scripts/gate.sh --fast` is the day-loop subset. CI runs
the same steps through the same script. The layered contract — what runs at which
boundary, and which reviewer covers what no check can — is
[`docs/qa-gate.md`](docs/qa-gate.md).

Contributions run through that gate and are merged by a human, never by an agent.

## Development harness

This repository uses the [agentic-starter](https://github.com/alexparlett/agentic-starter)
harness: contract files agents load, deterministic gates at the cheapest
boundary, fresh-context reviewers for what grep cannot judge, and a packet system
for multi-session work. `CLAUDE.md` is the entry point.

## License

MIT ([LICENSE-MIT](LICENSE-MIT)) or Apache-2.0
([LICENSE-APACHE](LICENSE-APACHE)), at your option.
