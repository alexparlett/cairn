//! The diff engine against real `git`: what changed, what one file's change is, and
//! whether the patches the model emits mean what they say.
//!
//! One binary, because the fixtures and the scratch index are shared by all of it.
#![cfg(unix)]

mod diff;
