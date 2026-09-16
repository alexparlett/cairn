//! Invariant guards.

use std::collections::BTreeSet;
use std::path::Path;

use cairn_guards::{
    code_only, code_without_strings, code_without_test_modules, declared_dependencies,
    mentions_crate, repo_root, rust_sources, spawns_git, waits_on_work,
};

/// Crates whose dependency list is pinned; a crate with no row here fails.
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

/// What a crate may take as a dev-dependency beyond its [`DEPENDENCY_ALLOWLIST`] row.
const TEST_ONLY_ALLOWLIST: &[(&str, &[&str])] = &[
    ("cairn-ui", &["freya-testing"]),
    ("cairn-app", &["freya-testing"]),
];

/// Crate directory → crate identifiers it may never name in code, in `src/`, `tests/` or anywhere
/// else under it.
const FORBIDDEN_IDENTS: &[(&str, &[&str])] = &[
    (
        "crates/cairn-model",
        &["gix", "freya", "cairn_git", "cairn_ui"],
    ),
    ("crates/cairn-ui", &["gix", "cairn_git"]),
    ("crates/cairn-git", &["freya", "dioxus", "cairn_ui"]),
];

/// The product crates: the guard suite's own fixtures contain the spellings they forbid.
const PRODUCT_SOURCE_DIRS: &[&str] = &[
    "crates/cairn-model/src",
    "crates/cairn-git/src",
    "crates/cairn-ui/src",
    "crates/cairn-app/src",
];

const RENDER_SOURCE_DIRS: &[&str] = &["crates/cairn-ui/src", "crates/cairn-app/src"];

/// Where repository work runs, and so the only place waiting is allowed.
const WORKER_DIR: &str = "crates/cairn-app/src/worker";

/// What a file must not name to count as rendering nothing. `cairn_ui` is here because
/// `cairn-app` may depend on it.
const RENDERING_IDENTS: &[&str] = &["freya", "dioxus", "cairn_ui"];

/// Closes `RENDER_SOURCE_DIRS` against the manifests: a crate that declares `freya` must be on it.
#[test]
fn every_crate_that_renders_is_on_the_render_roster() {
    let crates_dir = repo_root().join("crates");
    let mut draws = BTreeSet::new();

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
        let declares_freya = declared_dependencies(&parsed).shipped.contains("freya");
        if declares_freya {
            let dir = entry.file_name().to_string_lossy().into_owned();
            draws.insert(format!("crates/{dir}/src"));
        }
    }

    assert!(
        !draws.is_empty(),
        "no crate under crates/ declares `freya`, so this check compared nothing. Either the \
         toolkit changed — in which case change the rule here with it — or the manifest walk \
         is looking in the wrong place."
    );

    for dir in &draws {
        assert!(
            RENDER_SOURCE_DIRS.contains(&dir.as_str()),
            "`{dir}` belongs to a crate that declares `freya`, so it renders, but it is not in \
             RENDER_SOURCE_DIRS. Both the waiting guard and the virtualization guard scan only \
             what that roster names: a render crate missing from it is unguarded, silently. Add \
             the row (CLAUDE.md, Invariants)."
        );
    }

    for dir in RENDER_SOURCE_DIRS {
        assert!(
            draws.contains(*dir),
            "RENDER_SOURCE_DIRS names `{dir}`, whose crate does not declare `freya`. A roster \
             row that points at something which no longer renders is a rule nobody is keeping: \
             remove it, or fix the path."
        );
    }
}

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

        assert!(
            DEPENDENCY_ALLOWLIST.iter().any(|(krate, _)| *krate == name),
            "crate `{name}` has no row in DEPENDENCY_ALLOWLIST. Adding a layer is a decision to \
             surface to the user: add the row with its allowed deps."
        );
        let (shipped, test_only) = unpermitted_dependencies(&name, &parsed);
        assert!(
            shipped.is_empty(),
            "{name} declares {shipped:?}, which its allowlist in \
             crates/cairn-guards/tests/invariants.rs does not permit. Either the layering \
             changed (update CLAUDE.md and this list together) or the dependency is wrong."
        );
        assert!(
            test_only.is_empty(),
            "{name} declares {test_only:?} as dev-dependencies, which neither its \
             DEPENDENCY_ALLOWLIST nor its TEST_ONLY_ALLOWLIST row permits. A test-only \
             dependency on a sealed crate is still a crossed seal."
        );
        seen.insert(name);
    }

    for (krate, _) in TEST_ONLY_ALLOWLIST {
        assert!(
            seen.contains(*krate),
            "TEST_ONLY_ALLOWLIST pins `{krate}`, but no such crate exists under crates/: remove \
             the row or fix the path."
        );
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

/// (shipped, test-only) dependencies of crate `name` that its allowlist rows do not permit.
fn unpermitted_dependencies(name: &str, manifest: &toml::Table) -> (Vec<String>, Vec<String>) {
    let row = |list: &[(&str, &'static [&'static str])]| -> BTreeSet<&'static str> {
        list.iter()
            .filter(|(krate, _)| *krate == name)
            .flat_map(|(_, deps)| deps.iter().copied())
            .collect()
    };
    let shipped_allowed = row(DEPENDENCY_ALLOWLIST);
    let test_only_allowed: BTreeSet<&str> = row(TEST_ONLY_ALLOWLIST)
        .union(&shipped_allowed)
        .copied()
        .collect();

    let declared = declared_dependencies(manifest);
    let shipped = declared
        .shipped
        .into_iter()
        .filter(|dep| !shipped_allowed.contains(dep.as_str()))
        .collect();
    let test_only = declared
        .test_only
        .into_iter()
        .filter(|dep| !test_only_allowed.contains(dep.as_str()))
        .collect();
    (shipped, test_only)
}

#[test]
fn the_allowlist_check_rejects_a_sealed_crate_in_every_dependency_table() {
    let manifest = |table: &str, dep: &str| -> toml::Table {
        format!("[package]\nname = \"cairn-ui\"\n[{table}]\n{dep} = \"1\"\n")
            .parse()
            .unwrap()
    };
    let none: Vec<String> = Vec::new();
    let gix = vec!["gix".to_owned()];

    assert_eq!(
        unpermitted_dependencies("cairn-ui", &manifest("dependencies", "gix")),
        (gix.clone(), none.clone())
    );
    assert_eq!(
        unpermitted_dependencies("cairn-ui", &manifest("build-dependencies", "gix")),
        (gix.clone(), none.clone())
    );
    assert_eq!(
        unpermitted_dependencies("cairn-ui", &manifest("dev-dependencies", "gix")),
        (none.clone(), gix.clone()),
        "a test-only dependency on a sealed crate passed"
    );
    assert_eq!(
        unpermitted_dependencies(
            "cairn-ui",
            &manifest("target.'cfg(unix)'.dev-dependencies", "gix")
        ),
        (none.clone(), gix),
        "a target-scoped dev-dependency on a sealed crate passed"
    );

    assert_eq!(
        unpermitted_dependencies("cairn-ui", &manifest("dev-dependencies", "freya-testing")),
        (none.clone(), none.clone())
    );
    assert_eq!(
        unpermitted_dependencies("cairn-ui", &manifest("dependencies", "freya-testing")),
        (vec!["freya-testing".to_owned()], none),
        "a test-only allowance let the crate ship the dependency"
    );
}

#[test]
fn the_seal_scan_reads_tests_as_well_as_src() {
    for (dir, _) in FORBIDDEN_IDENTS {
        let scanned = rust_sources(dir);
        for part in ["src", "tests"] {
            let under = Path::new(dir).join(part);
            if part == "src" || repo_root().join(&under).is_dir() {
                assert!(
                    scanned.iter().any(|(path, _)| path.starts_with(&under)),
                    "the seal scan of {dir} read nothing under {}",
                    under.display()
                );
            }
        }
    }
    assert!(
        FORBIDDEN_IDENTS
            .iter()
            .any(|(dir, _)| repo_root().join(dir).join("tests").is_dir()),
        "no sealed crate has a tests/ directory, so nothing here proves tests/ is read"
    );
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
                // The worker side may wait, but must not render.
                for ident in RENDERING_IDENTS {
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
            // `cairn-ui` is covered by `FORBIDDEN_IDENTS`; this half is `cairn-app`'s alone.
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
        // Per directory, not in aggregate: a wrong roster path would otherwise pass.
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
            // Strings blanked too: naming a view inside a message is not using one.
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

            // Test modules blanked for this half only.
            let production = code_without_test_modules(&code);
            if !mentions_crate(&production, VIRTUALIZING_VIEW).is_empty()
                && !mentions_crate(&production, "HistoryRow").is_empty()
            {
                virtualizes_the_history.push(path);
            }
        }
        // Per directory, as above.
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

/// The one that builds only what its viewport shows. [`mentions_crate`] matches
/// on word boundaries, so this does not count as naming `ScrollView`.
const VIRTUALIZING_VIEW: &str = "VirtualScrollView";

/// Render files allowed to name [`UNBOUNDED_VIEW`] anyway, and why. Empty on purpose.
const UNBOUNDED_VIEW_EXCEPTIONS: &[(&str, &str)] = &[];

/// The 1-based line where `code` names the unbounded scroll view, if it does.
fn unbounded_view(code: &str) -> Option<usize> {
    mentions_crate(code, UNBOUNDED_VIEW).first().copied()
}

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
