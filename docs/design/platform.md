# Platform

Intent, not as-built. Spine: `docs/design/cairn.md`, where this is decision
**D5**.

## Linux first, macOS where it is cheap

Linux is the target. macOS follows where Freya makes it cheap, and no Linux
design is compromised for it. Windows is not a goal.

Two disciplines keep macOS reachable at near-zero cost:

1. **Platform surface stays in `cairn-app`.** `cairn-model` and `cairn-git` are
   portable; they stay that way.
2. **Keyboard shortcuts resolve through one accelerator table** mapping a logical
   action to a per-platform chord. No component names a literal `Ctrl`.

Credentials raise no platform question, because git's helpers hold them
(`credentials.md`).

## The toolkit

The interface is Freya: a Rust-native, Skia-backed, declarative toolkit, with no
web runtime and no FFI layer between the view and the data. The cost is maturity
— Freya 0.5 is a release candidate whose API replaced the previous one wholesale.
Accepted deliberately: the alternatives either bring a browser or bring C++.

On Linux the chrome is native: no traffic lights, client-side decorations with
the controls at the right, keyboard-first throughout (`ui.md`).

## Open

- The menu bar on both platforms — a Freya capability question to answer before
  promising anything.
