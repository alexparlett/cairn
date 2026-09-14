//! The enforcement twins for the invariants in `CLAUDE.md`.
//!
//! One test per invariant, named for it. When you add an invariant to any
//! CLAUDE.md, add its twin here in the SAME change; when you delete one, delete
//! the twin. A failure message says what rule was broken and where, because the
//! agent that trips a guard has to be able to fix it without reading the guard.

use std::collections::BTreeSet;
use std::path::Path;

use cairn_guards::{code_only, mentions_crate, repo_root, rust_sources, spawns_git};

/// Crates whose dependency list is pinned, and what each is allowed to name.
/// A new crate with no row here fails `layer_dependencies_are_allowlisted`,
/// which is the point: adding a layer is a decision, not a default.
const DEPENDENCY_ALLOWLIST: &[(&str, &[&str])] = &[
    ("cairn-model", &[]),
    ("cairn-git", &["cairn-model", "gix", "thiserror"]),
    ("cairn-ui", &["cairn-model", "freya"]),
    (
        "cairn-app",
        &["cairn-git", "cairn-model", "cairn-ui", "freya"],
    ),
    ("cairn-guards", &["toml"]),
];

/// Crate source directory → crate identifiers it may never name in code.
const FORBIDDEN_IDENTS: &[(&str, &[&str])] = &[
    (
        "crates/cairn-model/src",
        &["gix", "freya", "cairn_git", "cairn_ui"],
    ),
    ("crates/cairn-ui/src", &["gix", "cairn_git"]),
    ("crates/cairn-git/src", &["freya", "dioxus", "cairn_ui"]),
];

/// The product crates, for guards that must not scan the guard suite's own
/// fixtures (which contain the very spellings they forbid).
const PRODUCT_SOURCE_DIRS: &[&str] = &[
    "crates/cairn-model/src",
    "crates/cairn-git/src",
    "crates/cairn-ui/src",
    "crates/cairn-app/src",
];

#[test]
fn layer_dependencies_are_allowlisted() {
    let crates_dir = repo_root().join("crates");
    let mut seen = BTreeSet::new();

    let entries = std::fs::read_dir(&crates_dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", crates_dir.display()));
    for entry in entries.filter_map(Result::ok) {
        let manifest = entry.path().join("Cargo.toml");
        if !manifest.is_file() {
            continue;
        }
        let text = std::fs::read_to_string(&manifest)
            .unwrap_or_else(|e| panic!("reading {}: {e}", manifest.display()));
        let parsed: toml::Table = text
            .parse()
            .unwrap_or_else(|e| panic!("parsing {}: {e}", manifest.display()));
        let name = parsed["package"]["name"]
            .as_str()
            .unwrap_or_default()
            .to_owned();

        let allowed: BTreeSet<&str> = DEPENDENCY_ALLOWLIST
            .iter()
            .find(|(krate, _)| *krate == name)
            .unwrap_or_else(|| {
                panic!(
                    "crate `{name}` has no row in DEPENDENCY_ALLOWLIST. Adding a layer is a \
                     decision to surface to the user: add the row with its allowed deps."
                )
            })
            .1
            .iter()
            .copied()
            .collect();

        let declared: BTreeSet<String> = parsed
            .get("dependencies")
            .and_then(toml::Value::as_table)
            .map(|t| t.keys().cloned().collect())
            .unwrap_or_default();

        for dep in &declared {
            assert!(
                allowed.contains(dep.as_str()),
                "{name} declares `{dep}`, which its allowlist in \
                 crates/cairn-guards/tests/invariants.rs does not permit. Either the layering \
                 changed (update CLAUDE.md and this list together) or the dependency is wrong."
            );
        }
        seen.insert(name);
    }

    for (krate, _) in DEPENDENCY_ALLOWLIST {
        assert!(
            seen.contains(*krate),
            "DEPENDENCY_ALLOWLIST pins `{krate}`, but no such crate exists under crates/. \
             A dead roster entry hides a moved crate: remove the row or fix the path."
        );
    }
    assert!(!seen.is_empty(), "no crates found under crates/");
}

#[test]
fn layers_never_name_the_crates_they_are_sealed_from() {
    for (dir, forbidden) in FORBIDDEN_IDENTS {
        let sources = rust_sources(dir);
        for (path, source) in &sources {
            for ident in *forbidden {
                let hits = mentions_crate(source, ident);
                assert!(
                    hits.is_empty(),
                    "{}:{} names `{ident}`, which {dir} is sealed from (CLAUDE.md, Invariants). \
                     If the boundary really moved, move it in CLAUDE.md and here first.",
                    path.display(),
                    hits[0]
                );
            }
        }
    }
}

#[test]
fn only_the_ops_module_mutates_a_repository() {
    let ops = Path::new("crates/cairn-git/src/ops");
    let mut scanned = 0usize;

    for dir in PRODUCT_SOURCE_DIRS {
        for (path, source) in rust_sources(dir) {
            if path.starts_with(ops) {
                continue;
            }
            scanned += 1;
            let hits = spawns_git(&source);
            assert!(
                hits.is_empty(),
                "{}:{} spawns a `git` subprocess outside crates/cairn-git/src/ops. Every \
                 repository mutation lives in that module so the confirmation seal cannot be \
                 routed around.",
                path.display(),
                hits[0]
            );
        }
    }
    assert!(
        scanned > 0,
        "the mutation guard scanned nothing; did the crates move?"
    );
}

#[test]
fn destructive_operations_are_sealed_behind_the_confirmation_token() {
    let (_, confirm) = rust_sources("crates/cairn-model/src")
        .into_iter()
        .find(|(p, _)| p.ends_with("confirm.rs"))
        .unwrap_or_else(|| {
            panic!("cairn-model/src/confirm.rs is gone; the seal it defines is an invariant")
        });
    let code = code_only(&confirm);

    assert!(
        code.contains("acknowledged: String"),
        "Confirmed's field stopped being private data: the token is only proof because it \
         cannot be built without the prompt text the user saw."
    );
    let constructors = code.matches("pub fn ").count();
    assert_eq!(
        constructors, 2,
        "Confirmed should expose exactly two public functions (`by_user` and `acknowledged`); \
         found {constructors}. A second way to build the token is a second way to reach a \
         destructive operation without a prompt."
    );

    let ops = rust_sources("crates/cairn-git/src/ops");
    assert!(
        ops.iter().any(|(_, s)| code_only(s).contains("Confirmed")),
        "no operation in cairn-git/src/ops takes a Confirmed token; either the module is empty \
         of destructive work (delete this guard's expectation) or the seal was dropped."
    );
}

#[test]
fn ci_runs_every_merge_bar_gate_step() {
    let root = repo_root();
    let gate = std::fs::read_to_string(root.join("scripts/gate.sh"))
        .unwrap_or_else(|e| panic!("reading scripts/gate.sh: {e}"));
    let ci = std::fs::read_to_string(root.join(".github/workflows/ci.yml"))
        .unwrap_or_else(|e| panic!("reading .github/workflows/ci.yml: {e}"));

    // The steps gate.sh knows about, read from its --step dispatch arms.
    let gate_steps: BTreeSet<&str> = gate
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let name = line.strip_suffix(";;")?.split(')').next()?.trim();
            (line.contains("run_") && !name.is_empty() && !name.contains(' ')).then_some(name)
        })
        .collect();

    let ci_steps: BTreeSet<&str> = ci
        .lines()
        .filter_map(|line| line.split("gate.sh --step ").nth(1))
        .map(|rest| rest.split_whitespace().next().unwrap_or_default())
        .filter(|s| !s.is_empty())
        .collect();

    assert!(
        !gate_steps.is_empty(),
        "parsed no steps out of scripts/gate.sh"
    );
    assert!(!ci_steps.is_empty(), "parsed no gate steps out of ci.yml");

    for step in &ci_steps {
        assert!(
            gate_steps.contains(step),
            "ci.yml runs `gate.sh --step {step}`, which gate.sh does not define. CI would fail \
             with exit 2 rather than running the check."
        );
    }

    // `test-fast` is the day-loop subset; CI runs `test-full` instead.
    for step in gate_steps.iter().filter(|s| **s != "test-fast") {
        assert!(
            ci_steps.contains(step),
            "gate.sh defines the merge-bar step `{step}` but no CI job runs it, so a change \
             could merge without it. Add the step to .github/workflows/ci.yml."
        );
    }
}
