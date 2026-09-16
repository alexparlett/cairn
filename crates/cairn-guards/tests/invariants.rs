//! The enforcement twins for the invariants in `CLAUDE.md`.
//!
//! One test per invariant, named for it. When you add an invariant to any
//! CLAUDE.md, add its twin here in the SAME change; when you delete one, delete
//! the twin. A failure message says what rule was broken and where, because the
//! agent that trips a guard has to be able to fix it without reading the guard.

use std::collections::BTreeSet;
use std::path::Path;

use cairn_guards::{
    code_only, code_without_strings, mentions_crate, repo_root, rust_sources, spawns_git,
    waits_on_work,
};

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

/// Everything that renders. Each of these is a render path end to end, and none
/// of it may reach a repository or wait for one.
const RENDER_SOURCE_DIRS: &[&str] = &["crates/cairn-ui/src", "crates/cairn-app/src"];

/// The one exception inside those directories: where repository work runs, and
/// therefore the only place waiting is allowed. It renders nothing, which is
/// checked in the same guard — otherwise the two sets could overlap and the
/// partition would say nothing.
const WORKER_DIR: &str = "crates/cairn-app/src/worker";

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
fn the_ui_thread_never_waits_on_repository_work() {
    let worker = Path::new(WORKER_DIR);
    let mut working = 0usize;

    for dir in RENDER_SOURCE_DIRS {
        let mut rendering = 0usize;
        for (path, source) in rust_sources(dir) {
            if path.starts_with(worker) {
                working += 1;
                // The worker side may wait, because it is not the UI thread.
                // What it may not do is render: if a file could do both, the
                // partition would stop meaning anything.
                for ident in ["freya", "dioxus"] {
                    let hits = mentions_crate(&source, ident);
                    assert!(
                        hits.is_empty(),
                        "{}:{} names `{ident}` inside {WORKER_DIR}. That module is where \
                         repository work blocks; a render path inside it would be a UI thread \
                         waiting on a repository (CLAUDE.md, Invariants).",
                        path.display(),
                        hits[0]
                    );
                }
                continue;
            }

            rendering += 1;
            let hits = waits_on_work(&source);
            assert!(
                hits.is_empty(),
                "{}:{} waits for something, and it is on a render path. Repository work goes \
                 through {WORKER_DIR} and comes back as values; nothing outside it may block, \
                 join, lock, receive or build a channel (CLAUDE.md, Invariants).",
                path.display(),
                hits[0]
            );
            // `cairn-ui` is sealed from the engine by its own roster in
            // FORBIDDEN_IDENTS, which is one authority; this is the other half
            // of the partition, and it is `cairn-app`'s alone.
            if dir.starts_with("crates/cairn-app/") {
                for ident in ["cairn_git", "gix"] {
                    let hits = mentions_crate(&source, ident);
                    assert!(
                        hits.is_empty(),
                        "{}:{} names `{ident}` on a render path. An engine call reachable from \
                         a render is exactly what the worker boundary exists to prevent: ask \
                         for it through a `worker::Request` instead (CLAUDE.md, Invariants; \
                         PRD A6).",
                        path.display(),
                        hits[0]
                    );
                }
            }
        }
        // Per directory, not in aggregate: a new render crate added to the
        // roster but pointed at the wrong path would otherwise be covered by
        // whichever directory still had files in it.
        assert!(
            rendering > 0,
            "the responsiveness guard found no render files under {dir}. Every directory in \
             RENDER_SOURCE_DIRS must contribute, or the guard is scanning less than it claims."
        );
    }

    assert!(
        working > 0,
        "the responsiveness guard found no files under {WORKER_DIR}. Either the worker moved, \
         in which case move this guard with it, or it is gone — and then every engine call in \
         cairn-app is on a render path."
    );
}

#[test]
fn a_history_sized_list_renders_through_a_virtualizing_view() {
    let worker = Path::new(WORKER_DIR);
    let mut virtualizes_the_history = Vec::new();

    for dir in RENDER_SOURCE_DIRS {
        let mut rendering = 0usize;
        for (path, source) in rust_sources(dir) {
            if path.starts_with(worker) {
                continue;
            }
            rendering += 1;
            // Strings blanked as well as comments: a file naming a view only
            // inside an error message has not used one, in either direction.
            let code = code_without_strings(&source);

            if let Some(line) = unbounded_view(&code) {
                let excused = UNBOUNDED_VIEW_EXCEPTIONS
                    .iter()
                    .any(|(excused, _)| Path::new(excused) == path);
                assert!(
                    excused,
                    "{}:{line} names `{UNBOUNDED_VIEW}`, which lays out every child whether it \
                     is on screen or not. A history is however long somebody's repository is, so \
                     Cairn's lists use `{VIRTUALIZING_VIEW}` (CLAUDE.md, Invariants; PRD R4.1). \
                     A BOUNDED panel may legitimately want the plain one — if this is that, add \
                     the file and the reason to UNBOUNDED_VIEW_EXCEPTIONS in this file, which is \
                     the review the rule exists to force.",
                    path.display(),
                );
            }

            if !mentions_crate(&code, VIRTUALIZING_VIEW).is_empty()
                && !mentions_crate(&code, "HistoryRow").is_empty()
            {
                virtualizes_the_history.push(path);
            }
        }
        // Per directory, not in aggregate, for the reason the responsiveness
        // guard gives above: a roster pointed at a renamed directory would
        // otherwise be covered by whichever one still had files in it.
        assert!(
            rendering > 0,
            "the virtualization guard found no render files under {dir}. Every directory in \
             RENDER_SOURCE_DIRS must contribute, or the guard is scanning less than it claims."
        );
    }

    assert!(
        !virtualizes_the_history.is_empty(),
        "no file under {RENDER_SOURCE_DIRS:?} uses `{VIRTUALIZING_VIEW}` over `HistoryRow`s. \
         The history list is the one unbounded list Cairn renders and it is virtualized \
         (`crates/cairn-ui/src/history_list.rs`); restore that call site, or — if the list \
         genuinely moved — move this guard with it."
    );

    for (excused, _) in UNBOUNDED_VIEW_EXCEPTIONS {
        assert!(
            RENDER_SOURCE_DIRS
                .iter()
                .flat_map(rust_sources)
                .any(|(path, _)| path == Path::new(excused)),
            "UNBOUNDED_VIEW_EXCEPTIONS excuses `{excused}`, which does not exist. A dead \
             exception is a hole nobody can see: delete the row or fix the path."
        );
    }
}

/// The scroll view that lays out every child it is given.
const UNBOUNDED_VIEW: &str = "ScrollView";

/// The one that builds only what its viewport shows.
///
/// `ScrollView` is a prefix of neither: [`mentions_crate`] matches on word
/// boundaries, so `VirtualScrollView` does not count as naming `ScrollView`.
const VIRTUALIZING_VIEW: &str = "VirtualScrollView";

/// Render files allowed to name [`UNBOUNDED_VIEW`] anyway, and why.
///
/// **Empty on purpose.** A bounded panel — a commit message, a settings pane —
/// can legitimately use the plain scroll view, and when one does, adding its row
/// here is the review this rule exists to force. Leaving the roster empty is
/// what makes that a decision rather than a default.
const UNBOUNDED_VIEW_EXCEPTIONS: &[(&str, &str)] = &[];

/// The 1-based line where `code` names the unbounded scroll view, if it does.
///
/// Deliberately NOT a check for "renders a collection of rows": whether an
/// iteration is over a history or over three tabs is not decidable from tokens,
/// and a matcher that pretended otherwise would report a rule it had not
/// checked. What IS decidable is which VIEW a file reaches for, and that is the
/// regression worth catching — a list swapped to the unbounded view to dodge a
/// layout problem.
fn unbounded_view(code: &str) -> Option<usize> {
    mentions_crate(code, UNBOUNDED_VIEW).first().copied()
}

/// The roster's own self-test, in the shape `every_waiting_spelling_in_the_roster_is_matched`
/// established: a matcher that has quietly stopped matching reports green while
/// the coverage it names is gone.
#[test]
fn the_unbounded_view_matcher_catches_the_shapes_it_claims() {
    let caught = [
        "ScrollView::new()",
        "ScrollView::new_controlled(controller)",
        "use freya::prelude::ScrollView;",
        "use freya::prelude::ScrollView as Plain;",
        "freya::components::scrollviews::ScrollView::new()",
        "let view:\n    ScrollView = todo();",
    ];
    for source in caught {
        assert!(
            unbounded_view(&code_without_strings(source)).is_some(),
            "the unbounded-view matcher missed {source:?}"
        );
    }

    let ignored = [
        "VirtualScrollView::new_with_data_controlled(data, build_row, controller)",
        "use freya::prelude::VirtualScrollView;",
        "let hint = \"ScrollView\";",
        "// a plain ScrollView would lay out every child",
        "MyScrollViewThing::new()",
        "scroll_view()",
    ];
    for source in ignored {
        assert_eq!(
            unbounded_view(&code_without_strings(source)),
            None,
            "the unbounded-view matcher fired on {source:?}"
        );
    }
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
