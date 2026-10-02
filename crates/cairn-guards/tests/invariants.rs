//! Invariant guards.

use std::collections::BTreeSet;
use std::path::Path;

use cairn_guards::{
    GITOXIDE_MUTATION_CALLS, GITOXIDE_MUTATION_IDENTS, GITOXIDE_MUTATION_METHODS,
    GITOXIDE_MUTATION_PATH_ENDS, GITOXIDE_MUTATION_PATHS, calls_associated_function, calls_method,
    calls_nullary_method, code_only, code_without_strings, code_without_test_modules,
    configures_process_environment, constructs_named_struct, constructs_process_command,
    constructs_struct, declared_dependencies, declares_publicly, derives_or_implements,
    implements_type, mentions_crate, names_gitoxide_mutation, reads_row_content_partially,
    renames_type, renders_in_a_macro, repo_root, rust_sources, spawns_git,
    structs_with_a_field_naming, types_containing, waits_on_work,
};

/// Crates whose dependency list is pinned; a crate with no row here fails.
const DEPENDENCY_ALLOWLIST: &[(&str, &[&str])] = &[
    ("cairn-model", &["zeroize"]),
    // `nix`: SIGTERM on cancel, so git can remove its lock files (issue #19).
    ("cairn-git", &["cairn-model", "gix", "nix", "thiserror"]),
    ("cairn-ui", &["cairn-model", "freya"]),
    (
        "cairn-app",
        &[
            "cairn-askpass",
            "cairn-git",
            "cairn-model",
            "cairn-ui",
            "freya",
        ],
    ),
    ("cairn-guards", &["toml"]),
    // Every crate here runs in a process holding a plaintext secret; keep it this short.
    ("cairn-askpass", &["cairn-model", "zeroize"]),
];

/// What a crate may take as a dev-dependency beyond its [`DEPENDENCY_ALLOWLIST`] row.
const TEST_ONLY_ALLOWLIST: &[(&str, &[&str])] = &[
    ("cairn-ui", &["freya-testing"]),
    ("cairn-app", &["freya-testing"]),
    // The fetch tests serve a real askpass channel; the engine never links the helper.
    ("cairn-git", &["cairn-askpass"]),
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
    // The helper holds a plaintext secret: no engine, no toolkit, and no logging framework.
    (
        "crates/cairn-askpass",
        &[
            "gix",
            "freya",
            "dioxus",
            "cairn_git",
            "cairn_ui",
            "tracing",
            "log",
        ],
    ),
];

/// The product crates: the guard suite's own fixtures contain the spellings they forbid.
const PRODUCT_SOURCE_DIRS: &[&str] = &[
    "crates/cairn-model/src",
    "crates/cairn-git/src",
    "crates/cairn-ui/src",
    "crates/cairn-app/src",
    "crates/cairn-askpass/src",
];

const RENDER_SOURCE_DIRS: &[&str] = &["crates/cairn-ui/src", "crates/cairn-app/src"];

/// The one crate deliberately off `PRODUCT_SOURCE_DIRS`: the guard suite's own fixtures
/// are written out of the spellings the guards forbid, so scanning itself would fail it.
const NOT_PRODUCT_SOURCE: &[&str] = &["cairn-guards"];

/// Closes `PRODUCT_SOURCE_DIRS` against the workspace: a crate that ships source must be
/// on it. Without this the roster is the one hand-maintained list in the suite that can
/// shrink by omission — a crate added later is silently outside both
/// `only_the_ops_module_mutates_a_repository` and
/// `every_git_invocation_disables_the_terminal_prompt`, which is the direction nobody
/// notices, since the guards go on passing. Modelled on
/// `every_crate_that_renders_is_on_the_render_roster`, and bidirectional for the same
/// reason: a row pointing at a crate that is gone is a rule nobody is keeping.
#[test]
fn every_product_crate_is_on_the_product_roster() {
    let crates_dir = repo_root().join("crates");
    let mut ships = BTreeSet::new();

    let entries = std::fs::read_dir(&crates_dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", crates_dir.display()));
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().into_owned();
        if NOT_PRODUCT_SOURCE.contains(&name.as_str()) {
            continue;
        }
        if !entry.path().join("Cargo.toml").is_file() || !entry.path().join("src").is_dir() {
            continue;
        }
        ships.insert(format!("crates/{name}/src"));
    }

    assert!(
        !ships.is_empty(),
        "no crate under crates/ ships a src/, so this check compared nothing; the manifest          walk is looking in the wrong place."
    );

    for dir in &ships {
        assert!(
            PRODUCT_SOURCE_DIRS.contains(&dir.as_str()),
            "`{dir}` is a product crate's source but is not in PRODUCT_SOURCE_DIRS, so nothing              checks that it keeps the mutation and terminal-prompt invariants: it could spawn              a `git` subprocess with an inherited environment and every guard would still              pass. Add the row, or name the crate in NOT_PRODUCT_SOURCE with the reason              (CLAUDE.md, Invariants)."
        );
    }

    for dir in PRODUCT_SOURCE_DIRS {
        assert!(
            ships.contains(*dir),
            "PRODUCT_SOURCE_DIRS names `{dir}`, which is not a product crate's source              directory any more. A roster row that points at nothing is a rule nobody is              keeping: remove it, or fix the path."
        );
    }
}

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
        (vec!["freya-testing".to_owned()], none.clone()),
        "a test-only allowance let the crate ship the dependency"
    );
    assert_eq!(
        unpermitted_dependencies("cairn-ui", &manifest("dev-dependencies", "freya")),
        (none.clone(), none),
        "a dependency the crate may ship was refused as a dev-dependency"
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

/// Crates that may read `RowContent` however they like: its owner, and this suite's fixtures.
const ROW_CONTENT_EXEMPT: &[&str] = &["cairn-model", "cairn-guards"];

#[test]
fn every_view_of_a_row_names_every_kind_of_row() {
    let crates_dir = repo_root().join("crates");
    let entries = std::fs::read_dir(&crates_dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", crates_dir.display()));
    let mut crates: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("Cargo.toml").is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    crates.sort();

    for exempt in ROW_CONTENT_EXEMPT {
        assert!(
            crates.iter().any(|krate| krate == exempt),
            "ROW_CONTENT_EXEMPT names `{exempt}`, which is not a crate under crates/: remove the \
             row or fix it."
        );
    }

    let mut readers = 0usize;
    for krate in crates
        .iter()
        .filter(|krate| !ROW_CONTENT_EXEMPT.contains(&krate.as_str()))
    {
        for (path, source) in rust_sources(format!("crates/{krate}")) {
            if !mentions_crate(&source, "RowContent").is_empty() {
                readers += 1;
            }
            let hits = reads_row_content_partially(&source);
            assert!(
                hits.is_empty(),
                "{}:{} reads a `RowContent` through a wildcard arm, a catch-all binding, `if let`, \
                 `let .. else` or `matches!`. Once there is a second kind of row that compiles and \
                 silently draws nothing for it: match every variant by name (CLAUDE.md, \
                 Invariants).",
                path.display(),
                hits[0]
            );
        }
    }
    assert!(
        readers > 0,
        "no file outside {ROW_CONTENT_EXEMPT:?} names `RowContent`, so this guard checked \
         nothing. If rows are read some other way now, move the guard with them."
    );
}

#[test]
fn the_row_content_matcher_catches_the_shapes_it_claims() {
    let caught = [
        (
            "wildcard arm",
            "match row.content {\n    RowContent::Commit(c) => draw(c),\n    _ => {}\n}",
        ),
        (
            "guarded wildcard",
            "match &row.content {\n    RowContent::Commit(c) => a(c),\n    _ if x => b(),\n}",
        ),
        (
            "catch-all binding",
            "match content {\n    RowContent::Commit(c) => a(c),\n    other => b(other),\n}",
        ),
        (
            "bound wildcard",
            "match content {\n    RowContent::Commit(c) => a(c),\n    rest @ _ => b(rest),\n}",
        ),
        (
            "wildcard in an or-pattern",
            "match content {\n    RowContent::Commit(c) | _ => a(),\n}",
        ),
        (
            "braced arm before the wildcard",
            "match content {\n    RowContent::Commit(c) => { a(c); }\n    _ => (),\n}",
        ),
        (
            "if let",
            "if let RowContent::Commit(commit) = &row.content {\n    draw(commit);\n}",
        ),
        (
            "nested if let",
            "if let Some(RowContent::Commit(c)) = rows.first().map(|r| &r.content) {}",
        ),
        ("while let", "while let RowContent::Commit(c) = next() {}"),
        (
            "let else",
            "let RowContent::Commit(commit) = row.content else {\n    return;\n};",
        ),
        (
            "matches!",
            "let is_commit = matches!(row.content, RowContent::Commit(_));",
        ),
        (
            "spaced matches!",
            "assert!(matches! (\n    content,\n    RowContent::Commit(..)\n));",
        ),
        (
            "attributed wildcard",
            "match content {\n    RowContent::Commit(c) => a(c),\n    #[allow(unreachable_patterns)]\n    _ => b(),\n}",
        ),
        ("glob import", "use cairn_model::RowContent::*;"),
        (
            "braced glob import",
            "use cairn_model::RowContent::{self, *};",
        ),
        (
            "variant import",
            "use cairn_model::RowContent::Commit;\nif let Commit(c) = x {}",
        ),
        (
            "grouped variant import",
            "use cairn_model::{RowContent::Commit, RowId};",
        ),
        (
            "aliased import",
            "use cairn_model::RowContent as Row;\nif let Row::Commit(c) = x {}",
        ),
        (
            "let chain",
            "if ready && let RowContent::Commit(c) = &row.content {\n    draw(c);\n}",
        ),
        (
            "ref binding",
            "match c {\n    RowContent::Commit(x) => a(x),\n    ref other => b(other),\n}",
        ),
        (
            "mut binding",
            "match c {\n    RowContent::Commit(x) => a(x),\n    mut other => b(other),\n}",
        ),
        (
            "underscore binding",
            "match c {\n    RowContent::Commit(x) => a(x),\n    _rest => b(),\n}",
        ),
        (
            "reference wildcard",
            "match &row.content {\n    &RowContent::Commit(ref x) => a(x),\n    &_ => b(),\n}",
        ),
        (
            "reference binding",
            "match &row.content {\n    &RowContent::Commit(ref x) => a(x),\n    &other => b(other),\n}",
        ),
        (
            "wrapped wildcard",
            "match first {\n    Some(RowContent::Commit(c)) => a(c),\n    Some(_) => b(),\n    None => c(),\n}",
        ),
    ];
    for (shape, source) in caught {
        assert!(
            !reads_row_content_partially(source).is_empty(),
            "the row-content matcher missed the {shape} shape: {source:?}"
        );
    }
    assert_eq!(
        reads_row_content_partially(
            "let a = 1;\nmatch c {\n    RowContent::Commit(c) => a(c),\n    _ => {}\n}"
        ),
        vec![4],
        "the matcher reported the wrong line"
    );

    let ignored = [
        (
            "exhaustive match",
            "match &row.content {\n    RowContent::Commit(commit) => draw(commit),\n}",
        ),
        (
            "irrefutable let",
            "let RowContent::Commit(commit) = render.row.content;",
        ),
        (
            "building a row",
            "let row = HistoryRow { content: RowContent::Commit(c), graph };",
        ),
        (
            "a wildcard inside a variant",
            "match content {\n    RowContent::Commit(_) => a(),\n}",
        ),
        (
            "a wildcard over something else",
            "match id {\n    Some(x) => a(x),\n    _ => b(),\n}\nlet c = RowContent::Commit(s);",
        ),
        (
            "if let over something else",
            "if let Some(c) = x {}\nlet r = RowContent::Commit(c);",
        ),
        (
            "matches! over something else",
            "matches!(x, Some(_)); let r = RowContent::Commit(c);",
        ),
        (
            "a wildcard inside an arm body",
            "match content {\n    RowContent::Commit(c) => match c.x {\n        Some(y) => y,\n        _ => 0,\n    },\n}",
        ),
        (
            "an if/else value in a typed let",
            "let r: RowContent = if a { x } else { y };",
        ),
        (
            "an if/else value in an irrefutable let",
            "let RowContent::Commit(c) = if a { x } else { y };",
        ),
        (
            "importing the type",
            "use cairn_model::{HistoryRow, RowContent, RowId};\nuse cairn_model::RowContent;",
        ),
        (
            "a wildcard over a wrapper the rows are not in",
            "match read {\n    Ok(RowContent::Commit(c)) => a(c),\n    Err(_) => b(),\n}",
        ),
        (
            "a let chain over something else",
            "if ready && let Some(c) = x {}\nlet r = RowContent::Commit(c);",
        ),
        (
            "prose",
            "// if let RowContent::Commit(c) = x, or `_ =>`\nlet s = \"matches!(c, RowContent::Commit(_))\";",
        ),
    ];
    for (shape, source) in ignored {
        assert_eq!(
            reads_row_content_partially(source),
            Vec::<usize>::new(),
            "the row-content matcher fired on the {shape} shape: {source:?}"
        );
    }
}

/// Where every repository mutation lives, and the only module that may build a write.
const OPS_DIR: &str = "crates/cairn-git/src/ops";

/// Where every `git` process is built, spawned, waited on and read, and nowhere else.
const PROCESS_DIR: &str = "crates/cairn-git/src/process";

/// Where every read that `git` answers lives, one named function each: the runner's other caller.
const READS_DIR: &str = "crates/cairn-git/src/reads";

/// The engine's source: the one crate that links `gix` and the runner.
const ENGINE_SOURCE_DIR: &str = "crates/cairn-git/src";

#[test]
fn only_the_ops_module_mutates_a_repository() {
    let ops = Path::new(OPS_DIR);
    let process = Path::new(PROCESS_DIR);
    let mut scanned = 0usize;

    for dir in PRODUCT_SOURCE_DIRS {
        for (path, source) in rust_sources(dir) {
            // `process/` runs the `git` binary by design; whether what it runs may write is
            // the write seal's (`the_runner_is_named_only_by_ops_and_reads`).
            if path.starts_with(ops) || path.starts_with(process) {
                continue;
            }
            scanned += 1;
            let hits = spawns_git(&source);
            assert!(
                hits.is_empty(),
                "{}:{} spawns a `git` subprocess outside crates/cairn-git/src/ops. Every \
                 repository mutation lives in that module so the confirmation seal cannot \
                 be routed around.",
                path.display(),
                hits[0]
            );
        }
    }
    assert!(
        scanned > 0,
        "the mutation guard scanned nothing; did the crates move?"
    );

    // The gitoxide half, over the one crate that links gix (the dependency allowlist keeps it
    // that way). Test modules are scanned too: a fixture is built by running real git.
    let mut engine_files = 0usize;
    for (path, source) in rust_sources(ENGINE_SOURCE_DIR) {
        if path.starts_with(ops) {
            continue;
        }
        engine_files += 1;
        let found = names_gitoxide_mutation(&source);
        assert!(
            found.is_empty(),
            "{} names a gitoxide mutation API outside {OPS_DIR}, at {found:?}. Every repository \
             mutation lives in that module, and goes through the `git` binary there (D1); a gix \
             write anywhere else runs none of the user's hooks and skips the seal. The roster \
             and its sources are in crates/cairn-guards/src/lib.rs.",
            path.display()
        );
    }
    assert!(
        engine_files > 0,
        "the gitoxide half of the mutation guard scanned nothing outside {OPS_DIR}"
    );
    assert!(
        DEPENDENCY_ALLOWLIST
            .iter()
            .filter(|(_, allowed)| allowed.contains(&"gix"))
            .map(|(name, _)| *name)
            .eq(["cairn-git"]),
        "a crate other than cairn-git may now depend on gix, so the gitoxide half of this guard, \
         which reads {ENGINE_SOURCE_DIR} alone, no longer sees every caller: widen its scope"
    );
}

/// The process half of a file's verdict: every way it builds, spawns, waits on or reads a
/// process, or calls a method that yields one, as `path:line names ..` — empty for a file that
/// does none of them. Test modules are blanked, as in the environment twin: a fixture may spawn
/// what it likes. Applies to files outside [`PROCESS_DIR`].
fn process_violations(path: &Path, source: &str) -> Vec<String> {
    let production = code_without_test_modules(&code_without_strings(source));
    let at = |line: usize| format!("{}:{line}", path.display());
    let mut found = Vec::new();
    for ident in PROCESS_IDENTS {
        for line in mentions_crate(&production, ident) {
            found.push(format!("{} names `{ident}`", at(line)));
        }
    }
    for name in PROCESS_NULLARY_CALLS {
        if PROCESS_CALL_EXCEPTIONS
            .iter()
            .any(|(excused, method, _)| path.starts_with(excused) && method == name)
        {
            continue;
        }
        for line in calls_nullary_method(&production, &[name]) {
            found.push(format!("{} calls `.{name}()`", at(line)));
        }
    }
    for line in calls_method(&production, PROCESS_YIELDING_METHODS) {
        found.push(format!("{} calls `.command(..)`", at(line)));
    }
    for line in calls_associated_function(&production, PROCESS_ENVIRONMENT_TYPE, "command") {
        found.push(format!("{} calls `GitEnvironment::command`", at(line)));
    }
    found
}

/// What a process is, or what starts and signals one, by name: std's pipe and child types,
/// the extension trait that adds `exec` and `process_group`, and `nix`, which `cairn-git` links
/// for signals and whose `process` feature compiles `posix_spawn` and the `exec` family. An
/// alias is caught on its import line, as with `Command` in the environment twin.
const PROCESS_IDENTS: &[&str] = &[
    "Stdio",
    "Child",
    "ChildStdin",
    "ChildStdout",
    "ChildStderr",
    "CommandExt",
    "nix",
];

/// Methods that spawn, wait on or read a process when called with no arguments — `Command`'s
/// `spawn`, `output` and `status`, `Child`'s `wait`, `try_wait` and `wait_with_output`. Each has
/// a namesake that takes arguments (`thread::Builder::spawn(f)`, `Condvar::wait(guard)`), which
/// is not matched.
const PROCESS_NULLARY_CALLS: &[&str] = &[
    "spawn",
    "output",
    "status",
    "wait",
    "try_wait",
    "wait_with_output",
];

/// Methods that hand back a process: `GitEnvironment::command`, which yields a ready
/// `std::process::Command`. Visible to `process/` alone (`pub(super)`), so the compiler refuses
/// it elsewhere in `cairn-git` too; this is the twin against that visibility widening.
const PROCESS_YIELDING_METHODS: &[&str] = &["command"];

/// Where a nullary call on [`PROCESS_NULLARY_CALLS`] is something else, keyed by a source
/// directory and the method, with what the call is instead and why no process can be behind
/// it. A row excuses that method under that directory wholly; a row whose directory no longer
/// makes the call fails the guard, so the roster cannot outlive its reason.
const PROCESS_CALL_EXCEPTIONS: &[(&str, &str, &str)] = &[(
    "crates/cairn-app/src",
    "status",
    "HistoryProgress::status, the history view's load state, read by the window and the status \
     line. No process can be behind a `.status()` in cairn-app: a Command there is caught by its \
     name and by `.command(..)`, cairn-git's public surface yields none, and a dependency that \
     did would need an allowlist row",
)];
/// Nothing outside `process/` builds, spawns, waits on or reads a process, or calls a method that
/// yields one (process-manager R1.3, G2). The environment twin pins that one file builds the
/// `Command`; this pins that nothing else can drive one, so the runner's stdin, kill and error
/// mapping cannot be bypassed with the built environment in hand.
#[test]
fn only_the_process_module_builds_or_runs_a_process() {
    let process = Path::new(PROCESS_DIR);
    let (mut outside, mut inside) = (0usize, 0usize);
    let mut module = String::new();

    for dir in PRODUCT_SOURCE_DIRS {
        for (path, source) in rust_sources(dir) {
            if path.starts_with(process) {
                inside += 1;
                module.push_str(&code_without_test_modules(&code_without_strings(&source)));
                module.push('\n');
                continue;
            }
            outside += 1;
            let found = process_violations(&path, &source);
            assert!(
                found.is_empty(),
                "a process is handled outside {PROCESS_DIR}: {found:?}. Only that module builds, \
                 spawns, waits on or reads a `git` process, so that every one gets the explicit \
                 environment, closed stdin, the kill path and git's diagnostic in its error; \
                 anything else reaches `git` through `ops/` or `reads/`."
            );
        }
    }
    assert!(
        outside > 0,
        "the process guard scanned nothing outside {PROCESS_DIR}; did the crates move?"
    );
    assert!(
        inside > 0,
        "{PROCESS_DIR} holds no Rust source: the process module moved, and this guard must \
         move with it rather than exempt a directory that is not there"
    );
    // The module does what the matchers look for, so a matcher that stopped reading real code
    // fails here rather than passing everything. The shapes are the runner's own. `.output()`
    // and `.wait()` were here while `GitCommand::run` and `stream` existed, and went with them
    // (process-manager phase 03): SWAPPED, not dropped, for the extension trait that puts each
    // process in a group of its own and the stdout pipe a thread reads, beside `.try_wait()`,
    // which is how the runner reaps. Those two stay on PROCESS_IDENTS and
    // PROCESS_NULLARY_CALLS, so a call outside `process/` still fails.
    for (shape, hits) in [
        ("`.spawn()`", calls_nullary_method(&module, &["spawn"])),
        (
            "`.try_wait()`",
            calls_nullary_method(&module, &["try_wait"]),
        ),
        ("`CommandExt`", mentions_crate(&module, "CommandExt")),
        ("`Stdio`", mentions_crate(&module, "Stdio")),
        ("`Child`", mentions_crate(&module, "Child")),
        ("`ChildStdout`", mentions_crate(&module, "ChildStdout")),
        ("`ChildStderr`", mentions_crate(&module, "ChildStderr")),
        ("`nix`", mentions_crate(&module, "nix")),
        (
            "`.command(..)`",
            calls_method(&module, PROCESS_YIELDING_METHODS),
        ),
    ] {
        assert!(
            !hits.is_empty(),
            "{PROCESS_DIR} no longer shows {shape} to the process matchers, though it is where a \
             process is run; either the runner changed shape and the roster must follow, or \
             the matcher stopped matching"
        );
    }
    // The method that hands back a process stays visible to `process/` alone: widened, it is
    // a `Command` any module could drive, and the call matcher above sees only product `src/`.
    let environment = std::fs::read_to_string(repo_root().join(PROCESS_ENVIRONMENT_FILE))
        .unwrap_or_else(|e| panic!("{PROCESS_ENVIRONMENT_FILE}: {e}"));
    let environment = code_without_test_modules(&code_without_strings(&environment));
    assert!(
        environment.contains("pub(super) fn command(")
            && declares_publicly(&environment, "command").is_empty(),
        "{PROCESS_ENVIRONMENT_FILE} no longer declares `pub(super) fn command(`; a wider \
         {PROCESS_ENVIRONMENT_TYPE}::command is a process any module of the crate can run"
    );
    for (dir, method, _) in PROCESS_CALL_EXCEPTIONS {
        let still_called = rust_sources(dir).iter().any(|(_, source)| {
            let production = code_without_test_modules(&code_without_strings(source));
            !calls_nullary_method(&production, &[method]).is_empty()
        });
        assert!(
            still_called,
            "PROCESS_CALL_EXCEPTIONS excuses `.{method}()` under {dir}, which no longer calls it; \
             remove the row"
        );
    }
}

/// The runner, by the names a caller outside `process/` reaches it through: the read builder,
/// a started process and its kill handle — the streamed `Running` and its `ProcessKill`, and
/// the runner's `Invocation` and its `KillHandle` — and the builder type itself (none of which
/// `process/` re-exports, so the compiler refuses them elsewhere; this is the twin against a
/// re-export). Allowed in `process/`, `ops/` and `reads/`.
const RUNNER_NAMES: &[&str] = &[
    "GitCommand",
    "read_invocation",
    "Running",
    "ProcessKill",
    "Invocation",
    "KillHandle",
];

/// The runner the credential-prompts packet built, which process-manager replaced (R3.7, G18):
/// the streamed process and its kill handle, by name. Banned everywhere in `cairn-git`,
/// `process/` included, test modules included: the runner in `runner.rs` is the one way a
/// process runs, and a second would be a second set of rules for ending one. They stay on
/// [`RUNNER_NAMES`] as well, which is the weaker rule.
const RETIRED_RUNNER_NAMES: &[&str] = &["Running", "ProcessKill"];

/// That runner's two entry points, the methods `GitCommand` declared for it: `run` to
/// completion and `stream` with a kill. Banned as a declaration in any `impl` block whose header
/// names `GitCommand`, and `stream` as a method call anywhere (`run` has innocent namesakes —
/// the runner's own `Driver::run` — so its call is not matched; its declaration is).
const RETIRED_RUNNER_METHODS: &[&str] = &["run", "stream"];

/// The type the retired methods were declared on.
const INVOCATION_BUILDER: &str = "GitCommand";

/// Each `impl` block in `code` (strings and comments blanked) whose header names `ty`, as its
/// 1-based header line and the text between its braces. Generic and trait impls alike:
/// `impl<'a, K: Kind> GitCommand<'a, K> {` and `impl Debug for GitCommand<'_, Read> {`.
fn impl_blocks_naming<'a>(code: &'a str, ty: &str) -> Vec<(usize, &'a str)> {
    let bytes = code.as_bytes();
    let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut blocks = Vec::new();
    for (at, _) in code.match_indices("impl") {
        let bounded =
            (at == 0 || !ident(bytes[at - 1])) && bytes.get(at + 4).is_some_and(|b| !ident(*b));
        if !bounded {
            continue;
        }
        let Some(open) = code[at..].find('{').map(|offset| at + offset) else {
            continue;
        };
        let header = &code[at..open];
        if header.contains(';') || mentions_crate(header, ty).is_empty() {
            continue;
        }
        let mut depth = 0usize;
        let mut close = code.len();
        for (offset, byte) in code[open..].bytes().enumerate() {
            match byte {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        close = open + offset;
                        break;
                    }
                }
                _ => {}
            }
        }
        let line = code[..at].bytes().filter(|b| *b == b'\n').count() + 1;
        blocks.push((line, &code[open + 1..close]));
    }
    blocks
}

/// Whether `body` declares a function named `name`: `fn name(` or `fn name<`, however spaced.
fn declares_function(body: &str, name: &str) -> bool {
    let bytes = body.as_bytes();
    let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    body.match_indices(name).any(|(at, _)| {
        let end = at + name.len();
        let bounded =
            (at == 0 || !ident(bytes[at - 1])) && bytes.get(end).is_none_or(|b| !ident(*b));
        let before = body[..at].trim_end();
        let after = body[end..].trim_start();
        bounded
            && before.ends_with("fn")
            && before[..before.len() - 2]
                .bytes()
                .next_back()
                .is_none_or(|b| !ident(b))
            && (after.starts_with('(') || after.starts_with('<'))
    })
}

/// Every trace of the retired runner in one source file, as `path:line ..`.
fn retired_runner_traces(path: &Path, source: &str) -> Vec<String> {
    let code = code_without_strings(source);
    let at = |line: usize| format!("{}:{line}", path.display());
    let mut found = Vec::new();
    for name in RETIRED_RUNNER_NAMES {
        for line in mentions_crate(&code, name) {
            found.push(format!("{} names `{name}`", at(line)));
        }
    }
    for (line, body) in impl_blocks_naming(&code, INVOCATION_BUILDER) {
        for method in RETIRED_RUNNER_METHODS {
            if declares_function(body, method) {
                found.push(format!(
                    "{} declares `{INVOCATION_BUILDER}::{method}` in the impl opened here",
                    at(line)
                ));
            }
        }
    }
    for line in calls_method(&code, &["stream"]) {
        found.push(format!("{} calls `.stream(..)`", at(line)));
    }
    for method in RETIRED_RUNNER_METHODS {
        for line in calls_associated_function(&code, INVOCATION_BUILDER, method) {
            found.push(format!(
                "{} calls `{INVOCATION_BUILDER}::{method}(..)`",
                at(line)
            ));
        }
    }
    found
}

/// The lines of production code that call `.spawn()` with no arguments — a process started.
/// Over `process/` this is the one place a `git` process is started, `GitCommand::start_with`,
/// so a second runner that spawns for itself makes it two. Lines, not calls: two spawns
/// written on one line count once, a gap accepted and stated in the twin's residual.
fn process_spawns(source: &str) -> usize {
    calls_nullary_method(
        &code_without_test_modules(&code_without_strings(source)),
        &["spawn"],
    )
    .len()
}

/// The other ways a `Command` starts a process — `output()`, `status()` and `CommandExt`'s
/// `exec()`, each with no arguments. None belongs in production `process/`, whose one start is
/// `.spawn()`. An innocent namesake there (a `status()` getter of its own) is caught too, and
/// is renamed: in that module those names mean a process.
const OTHER_PROCESS_STARTS: &[&str] = &["output", "status", "exec"];

/// The production lines of `source` calling one of [`OTHER_PROCESS_STARTS`].
fn other_process_starts(source: &str) -> usize {
    calls_nullary_method(
        &code_without_test_modules(&code_without_strings(source)),
        OTHER_PROCESS_STARTS,
    )
    .len()
}

/// The probe and fetch run on the runner, and the paths they ran on before are gone (process-
/// manager R3.7, G18): nothing in `cairn-git` names the streamed `Running` or its
/// `ProcessKill`, declares `GitCommand::run` or `GitCommand::stream`, or calls `.stream(..)` or
/// `GitCommand::run(..)`/`GitCommand::stream(..)`. Over every file of `crates/cairn-git/src`,
/// `process/` and test modules included — a test that keeps an old path alive keeps its rules
/// alive (`tests/` cannot name these crate-private items at all). And production `process/`
/// starts a process in exactly one place — one line calling `.spawn()`, and none calling
/// `.output()`, `.status()` or `.exec()` — so a second runner starting its own process by those
/// method calls, under any name, through an alias, a free function or a macro, fails here even
/// where the name ban does not see it. Method-call syntax only: a path call
/// (`Command::spawn(&mut c)`) or a `nix` start is not counted here; the terminal-prompt twin
/// catches most of them, and what neither sees is stated in the root `CLAUDE.md`. Proven to
/// read real code by finding `GitCommand`'s own impl blocks, and the runner's `start` in one
/// of them.
///
/// Residual review obligation, `qa-checklist`'s (its item 7): the names are read as spelled, so
/// a retired entry point declared through a `type` alias of the builder, as a free function or
/// by a macro is seen only if it starts a process; two spawns written on one line count once;
/// and a second path built ON `start` — a wrapper that
/// drives an `Invocation` by rules of its own — spawns nothing new and is not seen at all.
#[test]
fn the_retired_runner_is_gone() {
    let mut scanned = 0usize;
    let mut builder_impls = 0usize;
    let mut declares_start = false;
    let mut spawns = Vec::new();
    let mut other_starts = Vec::new();
    for (path, source) in rust_sources(ENGINE_SOURCE_DIR) {
        scanned += 1;
        if path.starts_with(PROCESS_DIR) {
            for _ in 0..process_spawns(&source) {
                spawns.push(path.display().to_string());
            }
            for _ in 0..other_process_starts(&source) {
                other_starts.push(path.display().to_string());
            }
        }
        let found = retired_runner_traces(&path, &source);
        assert!(
            found.is_empty(),
            "the runner process-manager replaced is back: {found:?}. Every invocation runs on \
             `GitCommand::start` and the `Invocation` it hands back, which end a process the one \
             way the packet decided (its whole group, SIGTERM then SIGKILL)."
        );
        let code = code_without_strings(&source);
        for (_, body) in impl_blocks_naming(&code, INVOCATION_BUILDER) {
            builder_impls += 1;
            declares_start |= declares_function(body, "start");
        }
    }
    assert!(
        scanned > 0,
        "the retired-runner guard scanned nothing; did cairn-git move?"
    );
    assert!(
        builder_impls > 0 && declares_start,
        "the retired-runner guard found no impl block of {INVOCATION_BUILDER} declaring `start`; \
         the builder moved or was renamed, and this guard must follow it rather than pass on \
         nothing"
    );
    assert_eq!(
        spawns.len(),
        1,
        "production {PROCESS_DIR} starts a process in {} places ({spawns:?}); the runner's \
         `GitCommand::start_with` is the one way a `git` process starts, so a second spawn is a \
         second runner with rules of its own for ending one",
        spawns.len()
    );
    assert!(
        other_starts.is_empty(),
        "production {PROCESS_DIR} calls `.output()`, `.status()` or `.exec()` ({other_starts:?}): \
         each starts a process outside the runner, as the retired `GitCommand::run` did with \
         `.output()`. If it is an innocent namesake — a getter of {PROCESS_DIR}'s own — rename \
         it: in this module those names mean a process"
    );
}

#[test]
fn the_retired_runner_matcher_catches_the_shapes_it_claims() {
    assert_eq!(
        (RETIRED_RUNNER_NAMES, RETIRED_RUNNER_METHODS),
        (
            ["Running", "ProcessKill"].as_slice(),
            ["run", "stream"].as_slice()
        ),
        "a retired-runner roster changed; spell the entry out here too"
    );
    let file = Path::new("crates/cairn-git/src/process/cli.rs");
    for (shape, source) in [
        ("the streamed process", "fn f(r: Running) {}"),
        ("its kill handle", "struct Fetch(ProcessKill);"),
        (
            "run, in a generic impl",
            "impl<'a, K: Kind> GitCommand<'a, K> {\n    pub(crate) fn run(self) -> R {\n        \
             x\n    }\n}",
        ),
        (
            "stream, in an impl of one kind",
            "impl GitCommand<'_, Write> {\n    fn stream (self) -> R { x }\n}",
        ),
        (
            "run, generic over its argument",
            "impl<K> GitCommand<'_, K> { fn run<F>(self, f: F) {} }",
        ),
        ("a stream call", "let r = command.stream()?;"),
        (
            "run, in a trait impl",
            "impl Run for GitCommand<'_, Read> {\n    fn run(self) -> R { x }\n}",
        ),
        (
            "a stream called by its path",
            "let r = GitCommand::stream(command)?;",
        ),
        (
            "a run called by its path",
            "let o = GitCommand :: run(command)?;",
        ),
        (
            "the streamed process, in a test module",
            "#[cfg(test)]\nmod tests {\n    fn f(r: Running) {}\n}",
        ),
        (
            "a wrapped stream call",
            "let r = command\n    .stream()\n    ?;",
        ),
    ] {
        assert!(
            !retired_runner_traces(file, source).is_empty(),
            "the retired-runner matcher missed {shape}: {source:?}"
        );
    }
    for (shape, source) in [
        (
            "the driver's own run",
            "impl Driver {\n    fn run(&mut self) -> Ended { x }\n}",
        ),
        ("a reader's run, called", "Self::Stdout(p) => reader.run(),"),
        (
            "the runner's start",
            "impl<'a, K: Kind> GitCommand<'a, K> {\n    fn start(self) -> R { x }\n}",
        ),
        (
            "a longer name",
            "impl GitCommand<'_, Read> { fn run_to_end(self) {} fn streaming(self) {} }",
        ),
        ("a longer type", "fn f(t: RunningTotal) {}"),
        ("prose", "// GitCommand::stream and Running are gone\n"),
        (
            "another type's run, by its path",
            "let ended = Driver::run(driver);",
        ),
        (
            "a longer method, by its path",
            "let o = GitCommand::run_to_end(command);",
        ),
        ("a string", "let s = \"Running ProcessKill .stream()\";"),
        (
            "run in an impl of another type after one of the builder",
            "impl GitCommand<'_, Read> { fn start(self) {} }\nimpl Driver { fn run(self) {} }",
        ),
    ] {
        assert!(
            retired_runner_traces(file, source).is_empty(),
            "the retired-runner matcher fired on {shape}: {source:?}"
        );
    }
    assert_eq!(
        impl_blocks_naming(
            "impl A {\n}\nimpl<'a> GitCommand<'a, Read> {\n    fn start(self) { inner { } }\n}",
            "GitCommand"
        )
        .iter()
        .map(|(line, body)| (*line, declares_function(body, "start")))
        .collect::<Vec<_>>(),
        [(3, true)],
        "the impl matcher found the wrong blocks, or cut one short at an inner brace"
    );
    // The spawn count reads production calls only: a test module's spawn, a thread builder's
    // spawn with its closure, and prose do not count.
    for (shape, source, expected) in [
        ("one spawn", "let child = command.spawn()?;", 1),
        (
            "two spawns",
            "let a = c.spawn()?;\nfn second() { let b = d\n    .spawn()?; }",
            2,
        ),
        (
            "a thread builder's spawn",
            "std::thread::Builder::new().spawn(move || {})",
            0,
        ),
        (
            "a spawn in a test module",
            "#[cfg(test)]\nmod tests {\n    fn t() { c.spawn().unwrap(); }\n}",
            0,
        ),
        ("prose", "// command.spawn() is the one place\n", 0),
    ] {
        assert_eq!(
            process_spawns(source),
            expected,
            "the spawn count misread {shape}: {source:?}"
        );
    }
    assert_eq!(
        OTHER_PROCESS_STARTS,
        ["output", "status", "exec"],
        "OTHER_PROCESS_STARTS changed; spell the entry out here too"
    );
    for (shape, source, expected) in [
        ("output", "let out = env.command(p, q).output()?;", 1),
        ("status", "let status = command\n    .status()?;", 1),
        ("exec", "let error = command.exec();", 1),
        (
            "a status with an argument",
            "let s = registry.status(id);",
            0,
        ),
        (
            "output in a test module",
            "#[cfg(test)]\nmod tests {\n    fn t() { c.output().unwrap(); }\n}",
            0,
        ),
        ("prose", "// command.output() was the old run\n", 0),
    ] {
        assert_eq!(
            other_process_starts(source),
            expected,
            "the other-start count misread {shape}: {source:?}"
        );
    }
}

/// A write, by the names that build one: the write builder and the authority it consumes.
/// Allowed in `process/`, which declares the builder and names the type in its signature, and
/// `ops/`, which constructs the authority. Not `reads/`: a read cannot build a write.
const WRITE_NAMES: &[&str] = &["write_invocation", "WriteAuthority"];

const WRITE_AUTHORITY: &str = "WriteAuthority";

/// The file that declares [`WRITE_AUTHORITY`], and the one place it may be constructed.
const WRITE_AUTHORITY_FILE: &str = "crates/cairn-git/src/ops/authority.rs";

/// The runner half of a file's verdict, for a file of `cairn-git`: each runner or write name it
/// uses outside the modules allowed them, and each way it constructs a `WriteAuthority` outside
/// `ops/`, as `path:line names ..`. Comments and strings are blanked; test modules are NOT —
/// a test outside `ops/` cannot construct the authority either, and nothing outside the three
/// modules has a reason to name the runner.
fn runner_violations(path: &Path, source: &str) -> Vec<String> {
    let code = code_without_strings(source);
    let at = |line: usize| format!("{}:{line}", path.display());
    let in_ops = path.starts_with(OPS_DIR);
    let in_process = path.starts_with(PROCESS_DIR);
    let in_reads = path.starts_with(READS_DIR);
    let mut found = Vec::new();
    if !(in_ops || in_process || in_reads) {
        for name in RUNNER_NAMES {
            for line in mentions_crate(&code, name) {
                found.push(format!(
                    "{} names `{name}`, the runner, outside process/, ops/ and reads/",
                    at(line)
                ));
            }
        }
    }
    if !(in_ops || in_process) {
        for name in WRITE_NAMES {
            for line in mentions_crate(&code, name) {
                found.push(format!(
                    "{} names `{name}`, which builds a write, outside ops/",
                    at(line)
                ));
            }
        }
    }
    if !in_ops {
        for line in calls_associated_function(&code, WRITE_AUTHORITY, "new") {
            found.push(format!(
                "{} calls `WriteAuthority::new` outside ops/",
                at(line)
            ));
        }
        for line in constructs_named_struct(&code, WRITE_AUTHORITY) {
            found.push(format!(
                "{} builds a `WriteAuthority` literal outside ops/",
                at(line)
            ));
        }
        for line in implements_type(&code, WRITE_AUTHORITY) {
            found.push(format!(
                "{} implements `WriteAuthority` outside ops/, where a constructor could be \
                 written",
                at(line)
            ));
        }
    }
    found
}

/// Only `ops/` and `reads/` name the runner (beside `process/` itself), and only `ops/`
/// constructs a `WriteAuthority` or builds a write (process-manager R1.2, R1.3, G1, G2). The
/// compiler refuses most of this already — the authority's constructor is `pub(in crate::ops)`,
/// the builder type is not re-exported, `process` is a private module — and this twin is what
/// fails if any of that visibility is widened, which would compile quietly. Over `cairn-git`
/// alone, because the runner's names are crate-private: another crate cannot name them, which
/// the visibility pins below keep true.
#[test]
fn the_runner_is_named_only_by_ops_and_reads() {
    let mut scanned = 0usize;
    let mut authority_file = None;
    let mut ops_docs = None;
    let mut library = None;
    for (path, source) in rust_sources(ENGINE_SOURCE_DIR) {
        scanned += 1;
        let found = runner_violations(&path, &source);
        assert!(
            found.is_empty(),
            "the runner or the write seal is reached from outside the modules allowed it: \
             {found:?}. Only ops/ (every mutation) and reads/ (every read git answers) invoke \
             the runner, and only ops/ can build a write."
        );
        for name in RUNNER_NAMES.iter().chain(WRITE_NAMES) {
            let hits = declares_publicly(&source, name);
            assert!(
                hits.is_empty(),
                "{}:{} declares or re-exports `{name}` as `pub`. The runner and the write seal \
                 are crate-private, so no other crate can run a raw verb or build a write.",
                path.display(),
                hits[0]
            );
        }
        if path == Path::new(WRITE_AUTHORITY_FILE) {
            authority_file = Some(source);
        } else if path == Path::new(OPS_DIR).join("mod.rs") {
            ops_docs = Some(source);
        } else if path == Path::new(ENGINE_SOURCE_DIR).join("lib.rs") {
            library = Some(source);
        }
    }
    assert!(
        scanned > 0,
        "the runner guard scanned nothing; did cairn-git move?"
    );
    for dir in [OPS_DIR, PROCESS_DIR, READS_DIR] {
        assert!(
            repo_root().join(dir).is_dir(),
            "{dir} is gone, and this guard exempts it by path: move the guard with the module"
        );
    }

    let library = library.unwrap_or_else(|| panic!("{ENGINE_SOURCE_DIR}/lib.rs is gone"));
    let library = code_without_strings(&library);
    assert!(
        library.contains("\nmod process;") && declares_publicly(&library, "process").is_empty(),
        "{ENGINE_SOURCE_DIR}/lib.rs no longer declares `mod process;` privately; the process \
         module is crate-private, so nothing outside cairn-git reaches the runner"
    );

    let source = authority_file.unwrap_or_else(|| {
        panic!("{WRITE_AUTHORITY_FILE} is gone; the write seal it defines is an invariant")
    });
    let production = code_without_test_modules(&code_without_strings(&source));
    assert!(
        production.contains("pub(crate) struct WriteAuthority {"),
        "{WRITE_AUTHORITY_FILE} no longer declares `pub(crate) struct WriteAuthority {{`; a \
         public type could be named from another crate, and a tuple or unit struct has no \
         private field to seal it"
    );
    let body = production
        .split("pub(crate) struct WriteAuthority {")
        .nth(1)
        .and_then(|rest| rest.split('}').next())
        .unwrap_or_default();
    assert!(
        !body.trim().is_empty() && mentions_crate(body, "pub").is_empty(),
        "WriteAuthority's field is no longer private; a field another module can name is a \
         literal another module can write"
    );
    assert!(
        production.contains("pub(in crate::ops) fn new() -> Self"),
        "{WRITE_AUTHORITY_FILE}'s constructor is no longer `pub(in crate::ops) fn new() -> Self`; \
         a wider one lets a read build a write"
    );
    // One impl block, the inherent one, and one literal in it: the constructor. The file's own
    // test module builds other types with `Self { .. }`, so the count is taken in the block.
    assert_eq!(
        implements_type(&production, WRITE_AUTHORITY).len(),
        1,
        "{WRITE_AUTHORITY_FILE} should open exactly one impl block for WriteAuthority, the \
         inherent one holding the constructor; a trait impl (`From`, `Default`) is a second way in"
    );
    let inherent = production
        .split("impl WriteAuthority {")
        .nth(1)
        .and_then(|rest| rest.split("\n}").next())
        .unwrap_or_else(|| {
            panic!("{WRITE_AUTHORITY_FILE} no longer opens `impl WriteAuthority {{`")
        });
    assert_eq!(
        constructs_struct(inherent, WRITE_AUTHORITY).len(),
        1,
        "{WRITE_AUTHORITY_FILE} should build a WriteAuthority in exactly one place, the \
         constructor; a second is a second way in"
    );
    assert!(
        constructs_named_struct(&production, WRITE_AUTHORITY).is_empty(),
        "{WRITE_AUTHORITY_FILE} builds a `WriteAuthority {{ .. }}` literal outside its constructor"
    );
    assert!(
        derives_or_implements(&source, WRITE_AUTHORITY, &["Clone", "Copy", "Default"]).is_empty(),
        "WriteAuthority gained Clone, Copy or Default: `Default` is a second constructor, and a \
         copy is an authority that outlives the write it was made for"
    );

    // The public half of the seal: the doctests in ops' module docs, each a one-line difference
    // from a passing scaffold (stable rustdoc checks that a block fails, not why).
    let docs = ops_docs.unwrap_or_else(|| panic!("{OPS_DIR}/mod.rs is gone"));
    assert!(
        docs.contains("//! ```\n//! fn scaffold(git: &cairn_git::ops::GitBinary) {"),
        "{OPS_DIR}/mod.rs lost the passing scaffold of its compile-fail doctests; without it the \
         refused blocks could all fail for a reason unrelated to the seal"
    );
    // Each refused block is exactly the scaffold plus its one line, so none can fail on a typo
    // in the part it shares with the block that compiles.
    let refused_blocks: Vec<&str> = docs
        .split("//! ```compile_fail\n")
        .skip(1)
        .filter_map(|block| block.split("//! ```\n").next())
        .collect();
    for refused in [
        "let _: Option<cairn_git::ops::WriteAuthority> = None;",
        "let _ = cairn_git::ops::WriteAuthority::new();",
        "let _ = git.write_invocation(unreachable!());",
        "let _ = git.read_invocation();",
        "let _ = git.environment().command(git.path(), unreachable!());",
    ] {
        let expected = format!(
            "//! fn scaffold(git: &cairn_git::ops::GitBinary) {{\n//!     let _ = git.path();\n\
             //!     {refused}\n//! }}\n"
        );
        assert!(
            refused_blocks.contains(&expected.as_str()),
            "{OPS_DIR}/mod.rs no longer pins `{refused}` in a compile_fail doctest that is the \
             passing scaffold plus that one line; from outside cairn-git, no write and no \
             WriteAuthority can be named or built (process-manager G1)"
        );
    }
}

#[test]
fn the_gitoxide_mutation_matcher_catches_the_shapes_it_claims() {
    // Every roster entry, in the shape it is matched in, so a matcher that stopped reading one
    // kind of entry fails here rather than passing every file.
    for ident in GITOXIDE_MUTATION_IDENTS {
        let source = format!("let id = repo.{ident}(x)?;");
        assert!(
            !names_gitoxide_mutation(&source).is_empty(),
            "the gitoxide matcher missed `{ident}`: {source:?}"
        );
        let imported = format!("use gix::clone::{ident};");
        assert!(
            !names_gitoxide_mutation(&imported).is_empty(),
            "the gitoxide matcher missed `{ident}` imported: {imported:?}"
        );
    }
    for name in GITOXIDE_MUTATION_CALLS {
        for source in [
            format!("repo.{name}(a, b)"),
            format!("Repository::{name}(&repo, a)"),
            format!("repo\n    .{name}\n    (a)"),
        ] {
            assert!(
                !names_gitoxide_mutation(&source).is_empty(),
                "the gitoxide matcher missed the call `{name}`: {source:?}"
            );
        }
    }
    for name in GITOXIDE_MUTATION_METHODS {
        let source = format!("index.{name}(options)?;");
        assert!(
            !names_gitoxide_mutation(&source).is_empty(),
            "the gitoxide matcher missed the method `{name}`: {source:?}"
        );
    }
    for path in GITOXIDE_MUTATION_PATHS
        .iter()
        .chain(GITOXIDE_MUTATION_PATH_ENDS)
    {
        for source in [
            format!("use {};", path.join("::")),
            format!("let x = {} (a);", path.join(" :: ")),
        ] {
            assert!(
                !names_gitoxide_mutation(&source).is_empty(),
                "the gitoxide matcher missed the path `{}`: {source:?}",
                path.join("::")
            );
        }
    }
    for (shape, source) in [
        (
            "an index written back after it was read",
            "repo.open_index()?.write(Default::default())?;",
        ),
        (
            "a tree editor written",
            "let id = repo.edit_tree(tree)?.write()?;",
        ),
        (
            "the Write trait called fully qualified",
            "gix::objs::Write::write_buf(&repo, kind, bytes)",
        ),
        (
            "a ref transaction",
            "repo.refs.transaction().prepare(edits, a, b)?.commit(None)?;",
        ),
        ("a new repository", "let repo = gix::init(path)?;"),
        (
            "an imported init, called bare",
            "use gix::init;\nlet r = init(path)?;",
        ),
        ("a clone kept", "gix::prepare_clone(url, path)?.persist()"),
        (
            "a lock taken",
            "let f = gix::lock::File::acquire_to_update_resource(p, m, b)?;",
        ),
        (
            "a checkout",
            "gix::worktree::state::checkout(&mut index, dir, objects, &p, &s, opts)",
        ),
        ("a deleted ref", "reference.delete()?;"),
        (
            "a note written",
            "repo.notes()?.replace(target, note, message)?;",
        ),
    ] {
        assert!(
            !names_gitoxide_mutation(source).is_empty(),
            "the gitoxide matcher missed {shape}: {source:?}"
        );
    }
    // Reads, and the namesakes the roster's shapes exist to let through: each is a call
    // cairn-git makes today or a std spelling it may.
    for (shape, source) in [
        (
            "the commit-time order path",
            "Sorting::ByCommitTime(gix::traverse::commit::simple::CommitTimeOrder::NewestFirst)",
        ),
        ("a commit looked up", "let commit = repo.find_commit(id)?;"),
        ("a commit kind", "gix::object::Kind::Commit"),
        ("a local named reference", "let reference = reference?;"),
        ("a tag namespace", "use gix::tag;"),
        ("std's file write", "std::fs::write(path, bytes)?;"),
        (
            "an archive written to a writer",
            "gix::worktree::archive::write_stream(t, w, o)",
        ),
        ("the init error", "Err(gix::init::Error::Init(e))"),
        ("the index read", "let index = repo.open_index()?;"),
        ("Cairn's own lock helper", "lock(&self.process).poll(true)"),
        (
            "Cairn's own probe",
            "fn probe(path: &Path) -> Result<V, E> {",
        ),
        (
            "a password set on a URL in memory",
            "url.set_password(Some(p));",
        ),
        ("a buffer written whole", "stdin.write_all(&bytes)?;"),
        (
            "the config snapshot",
            "let mut config = repo.config_snapshot_mut();",
        ),
        ("a definition named like a write", "fn write(&self) {}"),
        ("prose", "// repo.commit(..) belongs in ops\n"),
        ("a string", "let s = \"repo.write_blob(x)\";"),
        ("a longer identifier", "let commits = new_commits_since(x);"),
    ] {
        assert_eq!(
            names_gitoxide_mutation(source),
            Vec::<String>::new(),
            "the gitoxide matcher fired on {shape}: {source:?}"
        );
    }
    assert_eq!(
        names_gitoxide_mutation("let a = 1;\nlet b = 2;\nrepo.write_blob(x)?;\n"),
        vec!["3: write_blob".to_owned()],
        "the gitoxide matcher reports the wrong line"
    );
}

/// A file outside every module allowed a process or the runner, for the self-tests below.
const SCRATCH_FILE: &str = "crates/cairn-git/src/history/scratch.rs";

/// Both halves of the verdict on a file, as the two twins reach them.
fn verdict(path: &str, source: &str) -> Vec<String> {
    let path = Path::new(path);
    let mut found = runner_violations(path, source);
    if !path.starts_with(PROCESS_DIR) {
        found.extend(process_violations(path, source));
    }
    found
}

#[test]
fn the_process_matcher_catches_the_shapes_it_claims() {
    // Every roster entry on its own, spelled out apart from the roster, so an entry dropped
    // from it fails here rather than taking its own case with it.
    let idents = [
        "Stdio",
        "Child",
        "ChildStdin",
        "ChildStdout",
        "ChildStderr",
        "CommandExt",
        "nix",
    ];
    let calls = [
        "spawn",
        "output",
        "status",
        "wait",
        "try_wait",
        "wait_with_output",
    ];
    assert_eq!(
        (PROCESS_IDENTS.len(), PROCESS_NULLARY_CALLS.len()),
        (idents.len(), calls.len()),
        "a process roster changed; spell the entry out here too, so it has a case of its own"
    );
    for ident in idents {
        let source = format!("use std::process::{ident};");
        assert!(
            !process_violations(Path::new(SCRATCH_FILE), &source).is_empty(),
            "the process matcher missed `{ident}`: {source:?}"
        );
    }
    for name in calls {
        let source = format!("let x = built.{name}();");
        assert!(
            !process_violations(Path::new(SCRATCH_FILE), &source).is_empty(),
            "the process matcher missed `.{name}()`: {source:?}"
        );
    }
    for (shape, source) in [
        ("a pipe", "cmd.stdout(Stdio::piped());"),
        ("a child", "fn reap(child: Child) {}"),
        ("a child's stream", "let err: ChildStderr = taken;"),
        ("a renamed import", "use std::process::Child as Kid;"),
        (
            "the extension trait",
            "use std::os::unix::process::CommandExt;",
        ),
        ("nix", "nix::sys::signal::killpg(group, Signal::SIGTERM)"),
        ("a nix import", "use nix::unistd::Pid;"),
        ("spawn", "let child = built.spawn()?;"),
        ("output", "let out = built.output()?;"),
        ("status", "let status = built.status()?;"),
        ("wait", "let status = child.wait()?;"),
        ("try_wait", "if let Ok(Some(s)) = child.try_wait() {}"),
        ("wait_with_output", "child.wait_with_output()"),
        ("a wrapped call", "built\n    .spawn\n    (\n    )"),
        ("a spaced call", "child . wait ( )"),
        (
            "the environment's process",
            "let built = environment.command(program, profile);",
        ),
        (
            "the environment's process, qualified",
            "GitEnvironment::command(&environment, program, profile)",
        ),
    ] {
        assert!(
            !process_violations(Path::new(SCRATCH_FILE), source).is_empty(),
            "the process matcher missed the {shape} shape: {source:?}"
        );
    }
    for (shape, source) in [
        (
            "a thread spawned with a closure",
            "thread::Builder::new().name(n).spawn(move || work())",
        ),
        ("a condvar wait", "let guard = ready.wait(guard)?;"),
        ("a longer name", "progress.status_line()"),
        (
            "a definition",
            "fn status(&self) -> &Status { &self.status }",
        ),
        ("a field", "let s = self.status;"),
        ("prose", "// child.wait() would block here\n"),
        ("a string", "let s = \"Stdio::piped() then .spawn()\";"),
        ("a longer identifier", "let children = ChildrenOf::new();"),
        (
            "a test module",
            "#[cfg(test)]\nmod tests {\n    fn t() { let _ = cmd.spawn(); }\n}\n",
        ),
    ] {
        assert_eq!(
            process_violations(Path::new(SCRATCH_FILE), source),
            Vec::<String>::new(),
            "the process matcher fired on the {shape} shape: {source:?}"
        );
    }
    // An exception covers its own method under its own directory, and nothing else.
    let excused = "let s = view.progress.read().status().clone();";
    assert!(process_violations(Path::new("crates/cairn-app/src/window.rs"), excused).is_empty());
    assert!(
        !process_violations(Path::new(SCRATCH_FILE), excused).is_empty(),
        "the cairn-app exception leaked into cairn-git"
    );
    assert!(
        !process_violations(
            Path::new("crates/cairn-app/src/window.rs"),
            "let out = built.output()?;"
        )
        .is_empty(),
        "the `status` exception excused another method"
    );
}

#[test]
fn the_runner_matcher_catches_the_shapes_it_claims() {
    // Every roster entry on its own, spelled out apart from the rosters, in a file outside every
    // allowed module — and the write names in reads/ too — so an entry dropped from a roster
    // fails here rather than taking its own case with it.
    let runner = [
        "GitCommand",
        "read_invocation",
        "Running",
        "ProcessKill",
        "Invocation",
        "KillHandle",
    ];
    let write = ["write_invocation", "WriteAuthority"];
    assert_eq!(
        (RUNNER_NAMES.len(), WRITE_NAMES.len()),
        (runner.len(), write.len()),
        "a runner roster changed; spell the entry out here too, so it has a case of its own"
    );
    for name in runner.iter().chain(&write) {
        let source = format!("fn f(x: &{name}) {{}}");
        assert!(
            !runner_violations(Path::new(SCRATCH_FILE), &source).is_empty(),
            "the runner matcher missed `{name}`: {source:?}"
        );
    }
    for name in write {
        let source = format!("fn f(x: &{name}) {{}}");
        assert!(
            !runner_violations(Path::new("crates/cairn-git/src/reads/changes.rs"), &source)
                .is_empty(),
            "the runner matcher let reads/ name `{name}`, which builds a write"
        );
    }
    for (shape, path, source) in [
        (
            "a read built outside ops/ and reads/",
            SCRATCH_FILE,
            "let out = git.read_invocation().arg(\"x\").run()?;",
        ),
        (
            "the builder type",
            SCRATCH_FILE,
            "fn f(c: GitCommand<'_, Read>) {}",
        ),
        (
            "a streamed process",
            SCRATCH_FILE,
            "fn f(r: Running) -> ProcessKill { r.killer() }",
        ),
        (
            "a started invocation",
            SCRATCH_FILE,
            "fn f(i: Invocation<Read>) -> KillHandle { i.kill_handle() }",
        ),
        (
            "a write built outside ops/",
            SCRATCH_FILE,
            "git.write_invocation(authority)",
        ),
        (
            "a write built in reads/",
            "crates/cairn-git/src/reads/changes.rs",
            "git.write_invocation(authority)",
        ),
        (
            "an authority constructed in reads/",
            "crates/cairn-git/src/reads/changes.rs",
            "let a = WriteAuthority::new();",
        ),
        (
            "an authority constructed in process/",
            "crates/cairn-git/src/process/cli.rs",
            "let a = WriteAuthority::new();",
        ),
        (
            "an authority literal",
            "crates/cairn-git/src/process/cli.rs",
            "let a = WriteAuthority { _sealed: () };",
        ),
        (
            "an impl block for the authority",
            "crates/cairn-git/src/process/cli.rs",
            "impl Default for WriteAuthority { fn default() -> Self { todo() } }",
        ),
        (
            "the authority's constructor passed as a value",
            "crates/cairn-git/src/reads/changes.rs",
            "let make = crate::ops::WriteAuthority :: new;",
        ),
        (
            "a test module outside ops/",
            SCRATCH_FILE,
            "#[cfg(test)]\nmod tests {\n    fn t() { let _ = WriteAuthority::new(); }\n}\n",
        ),
    ] {
        assert!(
            !runner_violations(Path::new(path), source).is_empty(),
            "the runner matcher missed {shape} in {path}: {source:?}"
        );
    }
    for (shape, path, source) in [
        (
            "a read in reads/",
            "crates/cairn-git/src/reads/changes.rs",
            "let out = git.read_invocation().arg(\"x\").run()?;",
        ),
        (
            "a write in ops/",
            "crates/cairn-git/src/ops/fetch.rs",
            "git.write_invocation(WriteAuthority::new())",
        ),
        (
            "the write builder declared in process/",
            "crates/cairn-git/src/process/binary.rs",
            "pub(crate) fn write_invocation(&self, authority: WriteAuthority) {}",
        ),
        (
            "prose",
            SCRATCH_FILE,
            "// git.read_invocation() is ops' and reads' alone\n",
        ),
        (
            "a string",
            SCRATCH_FILE,
            "let s = \"WriteAuthority::new()\";",
        ),
        (
            "a longer name",
            SCRATCH_FILE,
            "let r = RunningTotal::default();",
        ),
    ] {
        assert_eq!(
            runner_violations(Path::new(path), source),
            Vec::<String>::new(),
            "the runner matcher fired on {shape} in {path}: {source:?}"
        );
    }

    for (shape, name, source) in [
        (
            "a public struct",
            "GitCommand",
            "pub struct GitCommand<'a, K> {",
        ),
        (
            "a public function",
            "read_invocation",
            "    pub fn read_invocation(&self) {}",
        ),
        (
            "a public const fn",
            "read_invocation",
            "pub const fn read_invocation() {}",
        ),
        (
            "a public re-export",
            "GitCommand",
            "pub use cli::GitCommand;",
        ),
        (
            "a grouped public re-export",
            "Running",
            "pub use crate::process::{GitBinary, Running};",
        ),
        (
            "a public kill handle",
            "KillHandle",
            "pub struct KillHandle(pub(super) Arc<Group>);",
        ),
        (
            "a public invocation re-export",
            "Invocation",
            "pub use crate::process::runner::Invocation;",
        ),
        (
            "a nested, wrapped re-export",
            "GitCommand",
            "pub use crate::process::{\n    cli::{GitCommand, Read},\n    Running,\n};",
        ),
        ("a public module", "process", "pub mod process;"),
    ] {
        assert!(
            !declares_publicly(source, name).is_empty(),
            "the visibility matcher missed {shape}: {source:?}"
        );
    }
    for (shape, name, source) in [
        (
            "a crate-private struct",
            "GitCommand",
            "pub(crate) struct GitCommand<'a, K> {",
        ),
        (
            "a module-private function",
            "read_invocation",
            "pub(super) fn read_invocation(&self) {}",
        ),
        (
            "a restricted function",
            "read_invocation",
            "pub(in crate::ops) fn read_invocation() {}",
        ),
        (
            "a crate-private re-export",
            "GitCommand",
            "pub(crate) use cli::GitCommand;",
        ),
        (
            "another name re-exported publicly",
            "Running",
            "pub use crate::process::{Askpass, GitBinary};",
        ),
        ("a private module", "process", "mod process;"),
        ("prose", "GitCommand", "// pub use cli::GitCommand;\n"),
        (
            "a use inside a function",
            "GitCommand",
            "fn f() { use cli::GitCommand; }",
        ),
    ] {
        assert_eq!(
            declares_publicly(source, name),
            Vec::<usize>::new(),
            "the visibility matcher fired on {shape}: {source:?}"
        );
    }
}

/// The four routes to a process that no guard saw before the process-manager packet
/// (`docs/research/process-manager/runner-and-worker-as-built.md`, section 3), as they would
/// be written today in a module outside `ops/`, `reads/` and `process/`, and a `reads/` file
/// that constructs a `WriteAuthority`: each must fail a twin.
#[test]
fn the_unguarded_routes_to_a_process_now_fail_a_twin() {
    for (route, path, source) in [
        (
            "the binary's builder, run to completion",
            SCRATCH_FILE,
            "let out = git.read_invocation().args([\"diff-tree\", \"-z\"]).run()?;",
        ),
        (
            "the old binary builder, by its old name",
            SCRATCH_FILE,
            "let out = git.command().args([\"diff-tree\", \"-z\"]).run()?;",
        ),
        (
            "a streamed run with its kill",
            SCRATCH_FILE,
            "let running = git.read_invocation().arg(\"x\").stream()?;\n\
             running.killer().kill();",
        ),
        (
            "the builder's constructor",
            SCRATCH_FILE,
            "let out = GitCommand::new(git.path(), git.environment(), Read).run()?;",
        ),
        (
            "the environment's process, driven by hand",
            SCRATCH_FILE,
            "let out = git.environment().command(git.path(), Profile::Read).output()?;",
        ),
        (
            "a reads/ file that constructs a WriteAuthority",
            "crates/cairn-git/src/reads/changes.rs",
            "let write = git.write_invocation(crate::ops::WriteAuthority::new());",
        ),
    ] {
        assert!(
            !verdict(path, source).is_empty(),
            "the route `{route}` passes every twin: {source:?}"
        );
    }
}

/// Where a `git` process is built: the environment module inside `process/`, the only place a
/// `std::process::Command` comes into being, and the only production file that may name it.
const PROCESS_ENVIRONMENT_FILE: &str = "crates/cairn-git/src/process/environment.rs";
const PROCESS_ENVIRONMENT_TYPE: &str = "GitEnvironment";

/// Ways to start a process that are not a `Command` and are safe Rust: `nix`'s `process`
/// feature (linked for `Pid`, which `SIGTERM` needs) compiles `posix_spawn`, `posix_spawnp`
/// and the `exec` family, and none of them passes through `GitEnvironment::command`. No
/// production file may name one; `fork` is `unsafe` and already forbidden by the workspace.
const SPAWN_SPELLINGS: &[&str] = &[
    "posix_spawn",
    "posix_spawnp",
    "execv",
    "execve",
    "execvp",
    "execvpe",
    "execveat",
    "fexecve",
];

/// Every spawn spelling, as it would be written, spelled out apart from the roster so that
/// an entry dropped from [`SPAWN_SPELLINGS`] fails here rather than shrinking the check.
const SPAWN_SPELLINGS_AS_WRITTEN: &[(&str, &str)] = &[
    (
        "posix_spawn",
        "nix::spawn::posix_spawn(&program, &actions, &attr, &args, &env)",
    ),
    (
        "posix_spawnp",
        "posix_spawnp(&program, &actions, &attr, &args, &env)",
    ),
    ("execv", "nix::unistd::execv(&program, &args)"),
    ("execve", "unistd::execve(&program, &args, &env)"),
    ("execvp", "execvp(&program, &args)"),
    ("execvpe", "execvpe(&program, &args, &env)"),
    ("execveat", "execveat(dirfd, &program, &args, &env, flags)"),
    ("fexecve", "fexecve(fd, &args, &env)"),
];

/// Structural, not behavioural: the value is pinned by `cairn-git`'s own tests over the builder
/// and over a stub `git` that prints its environment. This guard is against erosion of the ONE
/// construction path those tests rely on. Scope: the product crates' `src/` (test modules
/// blanked); a test fixture may spawn what it likes.
#[test]
fn every_git_invocation_disables_the_terminal_prompt() {
    let environment_file = Path::new(PROCESS_ENVIRONMENT_FILE);
    let mut environment_source = None;
    let mut scanned = 0usize;

    for dir in PRODUCT_SOURCE_DIRS {
        for (path, source) in rust_sources(dir) {
            scanned += 1;
            if path == environment_file {
                environment_source = Some(source);
                continue;
            }
            let production = code_without_test_modules(&code_without_strings(&source));
            let hits = mentions_crate(&production, "Command");
            assert!(
                hits.is_empty(),
                "{}:{} names `Command`. A process is built only in {PROCESS_ENVIRONMENT_FILE}, \
                 by {PROCESS_ENVIRONMENT_TYPE}::command, so that every git invocation gets the \
                 explicit environment with GIT_TERMINAL_PROMPT=0; nothing else may name, hold or \
                 alias the type.",
                path.display(),
                hits[0]
            );
            let hits = constructs_process_command(&production);
            assert!(
                hits.is_empty(),
                "{}:{} builds a Command. Only {PROCESS_ENVIRONMENT_TYPE}::command in \
                 {PROCESS_ENVIRONMENT_FILE} may: it is what clears the inherited environment and \
                 applies the roster that sets GIT_TERMINAL_PROMPT=0.",
                path.display(),
                hits[0]
            );
            for spelling in SPAWN_SPELLINGS {
                let hits = mentions_crate(&production, spelling);
                assert!(
                    hits.is_empty(),
                    "{}:{} names `{spelling}`, a way to start a process that never passes \
                     through {PROCESS_ENVIRONMENT_TYPE}::command, so nothing clears the \
                     inherited environment or sets GIT_TERMINAL_PROMPT=0 for it.",
                    path.display(),
                    hits[0]
                );
            }
            let hits = configures_process_environment(&production);
            assert!(
                hits.is_empty(),
                "{}:{} sets a process environment variable directly. The roster in \
                 {PROCESS_ENVIRONMENT_FILE} is the whole environment git sees; a variable set \
                 beside it is one nobody enumerated.",
                path.display(),
                hits[0]
            );
            // Private fields are visible to a descendant module, so the literal is looked for
            // everywhere, not just in the file that declares the type — and so is an impl block,
            // which is where a `Self { .. }` for it could be written.
            let hits = constructs_named_struct(&production, PROCESS_ENVIRONMENT_TYPE);
            assert!(
                hits.is_empty(),
                "{}:{} builds a {PROCESS_ENVIRONMENT_TYPE} literal outside \
                 {PROCESS_ENVIRONMENT_FILE}; a second literal is a way to hand \
                 {PROCESS_ENVIRONMENT_TYPE}::command an environment that skips the ALWAYS table.",
                path.display(),
                hits[0]
            );
            let hits = implements_type(&production, PROCESS_ENVIRONMENT_TYPE);
            assert!(
                hits.is_empty(),
                "{}:{} implements {PROCESS_ENVIRONMENT_TYPE} outside {PROCESS_ENVIRONMENT_FILE}; \
                 an impl block is where a `Self {{ .. }}` literal for it can be written, and every \
                 way to build one belongs in the file the guard counts.",
                path.display(),
                hits[0]
            );
        }
    }
    assert!(
        scanned > 0,
        "the terminal-prompt guard scanned nothing; did the crates move?"
    );

    let source = environment_source.unwrap_or_else(|| {
        panic!("{PROCESS_ENVIRONMENT_FILE} is gone; the environment it builds is an invariant")
    });
    let production = code_without_test_modules(&code_without_strings(&source));
    for spelling in SPAWN_SPELLINGS {
        assert!(
            mentions_crate(&production, spelling).is_empty(),
            "{PROCESS_ENVIRONMENT_FILE} names `{spelling}`; the one process construction it may \
             hold is the Command that env_clear()s."
        );
    }
    // The roster is checked against a list spelled out apart from it, so an entry removed
    // from SPAWN_SPELLINGS fails here; and the matcher sees each spelling as it would be
    // written, so an empty scan above is a scan, not a miss.
    for (spelling, as_written) in SPAWN_SPELLINGS_AS_WRITTEN {
        assert!(
            SPAWN_SPELLINGS.contains(spelling),
            "`{spelling}` is a way to start a process and is no longer on SPAWN_SPELLINGS"
        );
        assert!(
            !mentions_crate(as_written, spelling).is_empty(),
            "the spawn matcher does not see `{spelling}` in `{as_written}`"
        );
    }
    assert_eq!(
        SPAWN_SPELLINGS.len(),
        SPAWN_SPELLINGS_AS_WRITTEN.len(),
        "SPAWN_SPELLINGS and SPAWN_SPELLINGS_AS_WRITTEN name different sets of spellings"
    );
    assert_eq!(
        constructs_process_command(&production).len(),
        1,
        "{PROCESS_ENVIRONMENT_FILE} should build a Command in exactly one place; a second is a \
         second way for a process to start without the explicit environment."
    );
    assert!(
        !mentions_crate(&production, "env_clear").is_empty()
            && !mentions_crate(&production, "envs").is_empty(),
        "{PROCESS_ENVIRONMENT_FILE} no longer both clears the inherited environment (env_clear) \
         and applies its own (envs); one without the other hands git either the launching \
         shell's variables or none."
    );
    assert_eq!(
        constructs_struct(&production, PROCESS_ENVIRONMENT_TYPE).len(),
        1,
        "{PROCESS_ENVIRONMENT_TYPE} should be built in exactly one place — the constructor that \
         applies the ALWAYS table — so a second literal is a way to skip GIT_TERMINAL_PROMPT=0."
    );
    assert!(
        !production.contains("mut self") && !production.contains("&mut Self"),
        "{PROCESS_ENVIRONMENT_FILE} gained a method that mutates a built \
         {PROCESS_ENVIRONMENT_TYPE}; an entry removed after construction is an entry the \
         constructor's tests never see."
    );

    // The tuple must sit inside the ALWAYS table itself, not merely somewhere in the file.
    let with_strings = code_only(&source);
    let always = with_strings
        .find("const ALWAYS")
        .map(|at| &with_strings[at..])
        .and_then(|rest| rest.find("];").map(|end| &rest[..end]))
        .unwrap_or_else(|| {
            panic!("{PROCESS_ENVIRONMENT_FILE} no longer declares a `const ALWAYS` table")
        });
    assert!(
        always.contains("(\"GIT_TERMINAL_PROMPT\", \"0\")"),
        "{PROCESS_ENVIRONMENT_FILE}'s ALWAYS table no longer carries (\"GIT_TERMINAL_PROMPT\", \
         \"0\"); without it a GUI with no terminal hangs on git's own prompt."
    );
    assert!(
        always.contains("(\"SSH_ASKPASS_REQUIRE\", \"force\")"),
        "{PROCESS_ENVIRONMENT_FILE}'s ALWAYS table no longer carries (\"SSH_ASKPASS_REQUIRE\", \
         \"force\"); without it ssh asks for a key passphrase on a terminal nobody is watching \
         (PRD R3.3)."
    );
    for editor in ["GIT_EDITOR", "GIT_SEQUENCE_EDITOR"] {
        assert!(
            always.contains(&format!("(\"{editor}\", \"false\")")),
            "{PROCESS_ENVIRONMENT_FILE}'s ALWAYS table no longer carries (\"{editor}\", \
             \"false\"); without it a verb that wants an editor launches the user's configured \
             one — on a pipe, or as a window nobody asked for — and waits on it (process-manager \
             PRD R2.2). Both are needed: `sequence.editor` outranks GIT_EDITOR."
        );
    }
    // A read's additions, the same way: the table itself, then that it is applied.
    let read_only = const_table(&with_strings, "READ_ONLY").unwrap_or_else(|| {
        panic!(
            "{PROCESS_ENVIRONMENT_FILE} no longer declares a `const READ_ONLY` table; a read \
             would refresh the index behind the user's back"
        )
    });
    let missing = missing_pins(read_only, READ_ONLY_PINS);
    assert!(
        missing.is_empty(),
        "{PROCESS_ENVIRONMENT_FILE}'s READ_ONLY table no longer carries {}",
        missing
            .iter()
            .map(|(pin, why)| format!("{pin}; {why}"))
            .collect::<Vec<_>>()
            .join(" And ")
    );
    assert!(
        mentions_crate(&production, "READ_ONLY").len() >= 2,
        "READ_ONLY is declared in {PROCESS_ENVIRONMENT_FILE} but never applied; a read must get it."
    );
    // The helper is named in the constructor, from the `Askpass` it is given, not from a table.
    let constructor = code_without_test_modules(&with_strings);
    for variable in ["\"GIT_ASKPASS\"", "\"SSH_ASKPASS\"", "SOCKET_VARIABLE"] {
        assert!(
            constructor.contains(variable),
            "{PROCESS_ENVIRONMENT_FILE} no longer sets {variable}: git or ssh would have no \
             helper to ask and, with the terminal prompt off, no way to ask at all (PRD R3.2, \
             R3.3)."
        );
    }
    // Twice each: the import and the use. The import alone is a name nobody applies.
    assert!(
        mentions_crate(&constructor, "TOKEN_VARIABLE").len() >= 2
            && mentions_crate(&constructor, "SOCKET_VARIABLE").len() >= 2,
        "{PROCESS_ENVIRONMENT_FILE} imports the askpass socket or token variable but no longer \
         applies it; a helper with no socket or token is refused by the channel, so no prompt \
         could ever be answered."
    );
    assert!(
        mentions_crate(&production, "ALWAYS").len() >= 2,
        "ALWAYS is declared in {PROCESS_ENVIRONMENT_FILE} but never consulted; the constructor \
         must apply it."
    );
}

/// What a read's environment must carry, each with what goes wrong without it (process-manager
/// PRD R2.3, and D3 as the user decided it on 2026-10-02).
const READ_ONLY_PINS: &[(&str, &str, &str)] = &[
    (
        "GIT_OPTIONAL_LOCKS",
        "0",
        "without it a `git status` read rewrites the index and holds index.lock while the \
         user's own commit needs it (process-manager PRD R2.3).",
    ),
    (
        "GIT_NO_LAZY_FETCH",
        "1",
        "without it a read in a partial clone fetches a missing object from the promisor \
         remote: a pack written and the network reached, from a read (D3, decided by the user \
         2026-10-02; git older than 2.44 ignores it).",
    ),
];

/// The text of the table `const <name>: ..` declares in `source` (strings kept, comments
/// blanked), from its declaration to its closing `];`; `None` when there is no such table. The
/// name is matched whole, so `READ_ONLY_EXTRA` is not `READ_ONLY`.
fn const_table<'a>(source: &'a str, name: &str) -> Option<&'a str> {
    let declaration = format!("const {name}:");
    let at = source.find(&declaration)?;
    let rest = &source[at..];
    rest.find("];").map(|end| &rest[..end])
}

/// Each pin `table` does not carry as a `("NAME", "value")` tuple, spelled as it should be,
/// with the reason the pin exists.
fn missing_pins<'a>(table: &str, pins: &'a [(&str, &str, &str)]) -> Vec<(String, &'a str)> {
    pins.iter()
        .filter_map(|(name, value, why)| {
            let tuple = format!("(\"{name}\", \"{value}\")");
            (!table.contains(&tuple)).then_some((tuple, *why))
        })
        .collect()
}

#[test]
fn the_process_environment_matcher_catches_the_shapes_it_claims() {
    // The read pins, spelled out apart from the roster, so a pin dropped from it fails here.
    assert_eq!(
        READ_ONLY_PINS
            .iter()
            .map(|(name, value, _)| (*name, *value))
            .collect::<Vec<_>>(),
        [("GIT_OPTIONAL_LOCKS", "0"), ("GIT_NO_LAZY_FETCH", "1")],
        "READ_ONLY_PINS no longer names exactly the variables a read must carry"
    );
    let carried = "const READ_ONLY: &[(&str, &str)] = &[\n    (\"GIT_OPTIONAL_LOCKS\", \"0\"),\n    \
                   (\"GIT_NO_LAZY_FETCH\", \"1\"),\n];\nconst OTHER: &[(&str, &str)] = &[];";
    let table = const_table(carried, "READ_ONLY")
        .unwrap_or_else(|| panic!("the table matcher missed a READ_ONLY table"));
    assert!(
        missing_pins(table, READ_ONLY_PINS).is_empty(),
        "the pin matcher fired on a table that carries every pin: {table:?}"
    );
    for (shape, source, missing) in [
        (
            "a table without the lazy-fetch pin",
            "const READ_ONLY: &[(&str, &str)] = &[(\"GIT_OPTIONAL_LOCKS\", \"0\")];",
            "GIT_NO_LAZY_FETCH",
        ),
        (
            "the pin with the wrong value",
            "const READ_ONLY: &[(&str, &str)] = &[(\"GIT_OPTIONAL_LOCKS\", \"0\"), \
             (\"GIT_NO_LAZY_FETCH\", \"0\")];",
            "GIT_NO_LAZY_FETCH",
        ),
        (
            "the pin in the next table, not this one",
            "const READ_ONLY: &[(&str, &str)] = &[(\"GIT_OPTIONAL_LOCKS\", \"0\")];\n\
             const ALWAYS: &[(&str, &str)] = &[(\"GIT_NO_LAZY_FETCH\", \"1\")];",
            "GIT_NO_LAZY_FETCH",
        ),
        (
            "a table without the optional-locks pin",
            "const READ_ONLY: &[(&str, &str)] = &[(\"GIT_NO_LAZY_FETCH\", \"1\")];",
            "GIT_OPTIONAL_LOCKS",
        ),
    ] {
        let table = const_table(source, "READ_ONLY")
            .unwrap_or_else(|| panic!("the table matcher missed the table in {shape}"));
        let found = missing_pins(table, READ_ONLY_PINS);
        assert!(
            found.len() == 1 && found[0].0.contains(missing),
            "the pin matcher did not report exactly {missing} for {shape}: {found:?}"
        );
    }
    for (shape, source) in [
        (
            "a longer name",
            "const READ_ONLY_EXTRA: &[(&str, &str)] = &[(\"GIT_NO_LAZY_FETCH\", \"1\")];",
        ),
        ("no table at all", "fn read_only() {}"),
    ] {
        assert!(
            const_table(source, "READ_ONLY").is_none(),
            "the table matcher took {shape} for the READ_ONLY table: {source:?}"
        );
    }

    for (shape, source) in [
        ("plain", "Command::new(p)"),
        ("qualified", "std::process::Command::new(\"git\")"),
        ("spaced", "Command :: new(p)"),
        ("wrapped", "let c = Command::\n    new(p);"),
    ] {
        assert!(
            !constructs_process_command(source).is_empty(),
            "the command matcher missed the {shape} shape: {source:?}"
        );
    }
    for (shape, source) in [
        ("another type's new", "GitCommand::new(p, e)"),
        ("a longer identifier", "MyCommand::new(p)"),
        ("not a constructor", "Command::from(p)"),
        ("prose", "// Command::new(\"git\") is forbidden\n"),
        ("a string", "let s = \"Command::new\";"),
    ] {
        assert!(
            constructs_process_command(source).is_empty(),
            "the command matcher fired on the {shape} shape: {source:?}"
        );
    }

    for (shape, source) in [
        ("env", "cmd.env(k, v)"),
        ("envs", "cmd.envs(map)"),
        ("env_clear", "cmd.env_clear()"),
        ("env_remove", "cmd.env_remove(k)"),
        ("a wrapped chain", "cmd\n    .env_clear()\n    .envs(x);"),
        ("spaced", "cmd . env (k, v)"),
    ] {
        assert!(
            !configures_process_environment(source).is_empty(),
            "the environment matcher missed the {shape} shape: {source:?}"
        );
    }
    for (shape, source) in [
        ("the env! macro", "env!(\"CARGO_MANIFEST_DIR\")"),
        ("std::env", "std::env::var_os(name)"),
        ("a method definition", "fn env(&self) -> &Env {}"),
        ("a longer name", "self.environment(x)"),
        ("prose", "// cmd.env(k, v)\n"),
        ("a string", "let s = \".env(\";"),
    ] {
        assert!(
            configures_process_environment(source).is_empty(),
            "the environment matcher fired on the {shape} shape: {source:?}"
        );
    }
    assert_eq!(
        configures_process_environment("let a = 1;\nlet b = 2;\ncmd.env(k, v);\n"),
        vec![3],
        "the environment matcher reports the wrong line"
    );

    for (shape, source) in [
        ("a Self literal", "Self { entries }"),
        ("a named literal", "GitEnvironment { entries }"),
        ("a multi-line literal", "Self {\n    entries,\n}"),
        ("a struct update", "GitEnvironment { ..base }"),
    ] {
        assert!(
            !constructs_struct(source, "GitEnvironment").is_empty(),
            "the struct matcher missed the {shape} shape: {source:?}"
        );
    }
    assert_eq!(
        constructs_struct("let (a, b) = (Self { x }, Self { x });", "GitEnvironment").len(),
        2,
        "the struct matcher counts literals, not lines: two on one line must be two. The \
         `== 1` count over the environment file depends on it, so a second GitEnvironment \
         literal sharing a line with the first would otherwise be invisible."
    );
    for (shape, source) in [
        ("a declaration", "pub struct GitEnvironment {"),
        ("an inherent impl", "impl GitEnvironment {"),
        ("a trait impl", "impl PartialEq for GitEnvironment {"),
        ("a return type", "fn new() -> Self {"),
        ("a reference return type", "fn get(&self) -> &Self {"),
        (
            "a mutable reference return type",
            "fn get(&mut self) -> &mut Self {",
        ),
        (
            "a lifetime-bound return type",
            "fn get<'a>(&'a self) -> &'a GitEnvironment {",
        ),
        ("a call", "GitEnvironment::new(f)"),
        ("another type", "GitEnvironmentBuilder { x }"),
        ("prose", "// Self { entries }\n"),
    ] {
        assert!(
            constructs_struct(source, "GitEnvironment").is_empty(),
            "the struct matcher fired on the {shape} shape: {source:?}"
        );
    }
    assert_eq!(
        constructs_named_struct("Self { x }; GitEnvironment { x }", "GitEnvironment"),
        vec![1],
        "the named matcher should see the named literal and not `Self`"
    );

    for (shape, source) in [
        ("an inherent impl", "impl GitEnvironment {"),
        ("a trait impl", "impl Clone for GitEnvironment {"),
        (
            "a wrapped trait impl",
            "impl Clone\n    for GitEnvironment\n{",
        ),
    ] {
        assert!(
            !implements_type(source, "GitEnvironment").is_empty(),
            "the impl matcher missed the {shape} shape: {source:?}"
        );
    }
    for (shape, source) in [
        ("a literal", "GitEnvironment { x }"),
        ("a parameter", "fn f(e: GitEnvironment) {}"),
        (
            "a type argument",
            "impl<'a> From<&'a GitEnvironment> for X {",
        ),
        ("a longer name", "impl GitEnvironmentBuilder {"),
        ("prose", "// impl GitEnvironment {\n"),
    ] {
        assert!(
            implements_type(source, "GitEnvironment").is_empty(),
            "the impl matcher fired on the {shape} shape: {source:?}"
        );
    }
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
            // `cairn_askpass` too: its `accept` blocks, and only the worker may hold it.
            if dir.starts_with("crates/cairn-app/") {
                for ident in ["cairn_git", "gix", "cairn_askpass"] {
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

/// The one type that holds a credential, and where it lives.
const SECRET_TYPE_FILE: &str = "crates/cairn-model/src/secret.rs";
const SECRET_TYPE: &str = "Secret";

/// The one way to read its bytes.
const SECRET_ACCESSOR: &str = "expose_secret";

/// Traits that would render, copy or serialise a credential; none may be given to the secret
/// type or to any type that holds one.
const SECRET_FORBIDDEN_TRAITS: &[&str] = &[
    "Debug",
    "Display",
    "Clone",
    "Copy",
    "Serialize",
    "Deserialize",
    "Encode",
    "Decode",
];

/// Production files allowed to name [`SECRET_ACCESSOR`]: the type's own, the wire encoder that
/// hands the bytes to the helper, and the helper's `main`, which hands them to git. Each is a
/// place a credential is in the open, and the list is the review.
const SECRET_READERS: &[&str] = &[
    SECRET_TYPE_FILE,
    "crates/cairn-askpass/src/protocol.rs",
    "crates/cairn-askpass/src/main.rs",
];

/// Files whose `struct`s may hold a [`SECRET_TYPE`] in a field. Empty on purpose: a secret
/// travels by value — through a function, a channel or an enum variant that is consumed once —
/// and is never kept. A row here is application state holding a credential, and a review.
const SECRET_HOLDERS: &[&str] = &[];

/// Every crate whose types and macros are read for a credential: all of them but the guards,
/// whose fixtures contain the shapes they forbid.
fn secret_scanned_crates() -> Vec<String> {
    let crates_dir = repo_root().join("crates");
    let entries = std::fs::read_dir(&crates_dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", crates_dir.display()));
    let mut crates: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("Cargo.toml").is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|krate| krate != "cairn-guards")
        .collect();
    crates.sort();
    crates
}

/// Structural, against erosion of what the TYPE enforces: `Secret` has no `Debug`, `Display`,
/// `Clone` or serialisation, so the compiler already refuses `{:?}` on it, a `#[derive(Debug)]`
/// container, and a `tracing` field with a `?` or `%` sigil (`cairn-model`'s compile-fail
/// doctests pin that). What the compiler cannot refuse is what this reads: an impl written by
/// hand for the type or for a container of it, the accessor spreading beyond the files that
/// must hand the bytes on, the accessor inside a macro that renders its arguments, and a
/// `struct` keeping a secret in a field. Scope: every crate but the guards — types and impls in
/// `src/` and `tests/` alike, the accessor and macro rules in production code only, since a
/// test that generates a secret may assert on it.
#[test]
fn no_credential_value_is_logged_printed_serialised_or_stored() {
    let (_, secret) = rust_sources("crates/cairn-model/src")
        .into_iter()
        .find(|(path, _)| path == Path::new(SECRET_TYPE_FILE))
        .unwrap_or_else(|| {
            panic!("{SECRET_TYPE_FILE} is gone; the type it defines is an invariant")
        });
    let production = code_without_test_modules(&code_without_strings(&secret));
    let declared = cairn_guards::type_declarations(&production);
    let declaration = declared
        .iter()
        .find(|declaration| declaration.name == SECRET_TYPE)
        .unwrap_or_else(|| panic!("{SECRET_TYPE_FILE} no longer declares `struct {SECRET_TYPE}`"));
    assert_eq!(
        declaration.keyword, "struct",
        "{SECRET_TYPE} is no longer a struct"
    );
    assert!(
        declaration.body.contains("Zeroizing<"),
        "{SECRET_TYPE}'s field is no longer a `Zeroizing<..>`; a plain buffer is freed with its \
         contents in place (credential-prompts L11)."
    );
    assert_eq!(
        declaration.body.matches(':').count(),
        1,
        "{SECRET_TYPE} gained a field; every field holding secret bytes must be a `Zeroizing`, \
         and the guard knows how to check one"
    );
    assert!(
        mentions_crate(&production, "derive").is_empty(),
        "{SECRET_TYPE_FILE} derives something; the type must derive nothing, so that no impl \
         can be added to it by a one-word edit."
    );
    let hits = derives_or_implements(&production, SECRET_TYPE, SECRET_FORBIDDEN_TRAITS);
    assert!(
        hits.is_empty(),
        "{SECRET_TYPE_FILE}:{} gives {SECRET_TYPE} an impl that renders, copies or serialises it \
         (CLAUDE.md, Invariants).",
        hits[0]
    );
    assert!(
        !implements_type(&production, SECRET_TYPE).is_empty()
            && production.contains("ZeroizeOnDrop for Secret"),
        "{SECRET_TYPE_FILE} no longer declares `impl ZeroizeOnDrop for {SECRET_TYPE}`; that \
         marker is the type-level promise the drop test relies on."
    );
    assert_eq!(
        production.matches("-> &[u8]").count(),
        1,
        "{SECRET_TYPE_FILE} should return the bytes from exactly one method, {SECRET_ACCESSOR}"
    );
    // The whole surface, spelled out: a new method or impl on the type is a review, because
    // any of them (`Deref`, `From`, `into_bytes`, ..) is a way out that the accessor roster
    // does not know.
    let functions: Vec<&str> = production
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub fn "))
        .filter_map(|rest| rest.split('(').next())
        .collect();
    assert_eq!(
        functions,
        ["new", "from_string", SECRET_ACCESSOR, "len", "is_empty"],
        "{SECRET_TYPE_FILE}'s public functions changed. Each one is part of how a credential \
         can be reached; a new one needs this list, the CLAUDE.md entry, and a look at whether \
         it is a second accessor."
    );
    let impls: Vec<&str> = production
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("impl"))
        .collect();
    assert_eq!(
        impls,
        [
            "impl Secret {",
            "impl Zeroize for Secret {",
            "impl ZeroizeOnDrop for Secret {}",
        ],
        "{SECRET_TYPE_FILE}'s impl blocks changed. `Deref`, `AsRef`, `Borrow`, `From`, `Into` \
         and the like would each hand the bytes out past the accessor roster."
    );
    assert!(
        production.contains(&format!("pub fn {SECRET_ACCESSOR}(&self) -> &[u8]")),
        "{SECRET_TYPE_FILE} no longer defines `pub fn {SECRET_ACCESSOR}(&self) -> &[u8]`; the \
         accessor's name is what the roster below is keyed on."
    );
    for leak in [
        "-> &str",
        "-> String",
        "-> Vec<u8>",
        "-> &Vec<u8>",
        "-> &Zeroizing",
    ] {
        assert!(
            !production.contains(leak),
            "{SECRET_TYPE_FILE} returns `{leak}` somewhere: a second way to read the bytes, one \
             the roster does not know"
        );
    }
    assert!(
        secret.contains("/// ```\n/// let secret = cairn_model::Secret::from_string"),
        "{SECRET_TYPE_FILE} lost the passing twin of its compile-fail doctests; without it \
         the refused blocks could all fail for a reason unrelated to the type (stable rustdoc \
         checks that a block fails, not why)"
    );
    assert!(
        secret.matches("```compile_fail").count() >= 4,
        "{SECRET_TYPE_FILE} lost its compile-fail doctests (PRD B7): `{{:?}}`, `{{}}`, a \
         `#[derive(Debug)]` container and `.clone()` must each be pinned to not compile"
    );

    // Containers: every type that holds a Secret, or a type that holds one, in any crate.
    let mut containers: BTreeSet<String> = BTreeSet::from([SECRET_TYPE.to_owned()]);
    let sources: Vec<(std::path::PathBuf, String)> = secret_scanned_crates()
        .iter()
        .flat_map(|krate| rust_sources(format!("crates/{krate}")))
        .collect();
    loop {
        let known: Vec<String> = containers.iter().cloned().collect();
        let known: Vec<&str> = known.iter().map(String::as_str).collect();
        let mut found = Vec::new();
        for (_, source) in &sources {
            found.extend(types_containing(source, &known));
        }
        let before = containers.len();
        containers.extend(found);
        if containers.len() == before {
            break;
        }
    }
    let containers: Vec<&str> = containers.iter().map(String::as_str).collect();

    let mut scanned = 0usize;
    for (path, source) in &sources {
        scanned += 1;
        for container in &containers {
            let hits = derives_or_implements(source, container, SECRET_FORBIDDEN_TRAITS);
            assert!(
                hits.is_empty(),
                "{}:{} gives `{container}`, which holds a credential, an impl that renders, \
                 copies or serialises it. A container that prints is the credential printing \
                 (CLAUDE.md, Invariants).",
                path.display(),
                hits[0]
            );
        }
        // Transitive: a struct keeping the enum that carries a secret keeps the secret.
        let hits = structs_with_a_field_naming(source, &containers);
        let excused = SECRET_HOLDERS.iter().any(|f| Path::new(f) == path);
        assert!(
            hits.is_empty() || excused,
            "{}:{} declares a struct with a field holding a `{SECRET_TYPE}`, or a type that \
             holds one: application state keeping a credential. A secret is passed by value \
             and consumed once; if this struct genuinely must hold one, add the file to \
             SECRET_HOLDERS, which is the review (CLAUDE.md, Invariants).",
            path.display(),
            hits.first().copied().unwrap_or_default()
        );
        let hits = renames_type(source, SECRET_TYPE);
        assert!(
            hits.is_empty(),
            "{}:{} gives `{SECRET_TYPE}` another name (`use .. as`, or a `type` alias). Past \
             it, a container or an impl can hold a credential without spelling the name this \
             guard reads (CLAUDE.md, Invariants).",
            path.display(),
            hits.first().copied().unwrap_or_default()
        );
        if path != Path::new(SECRET_TYPE_FILE) {
            let code = code_without_strings(source);
            let hits: Vec<usize> = code
                .lines()
                .enumerate()
                // An impl BLOCK opens its line; `impl Write` in a parameter list does not.
                .filter(|(_, line)| {
                    let line = line.trim_start();
                    (line.starts_with("impl ") || line.starts_with("impl<"))
                        && !cairn_guards::mentions_crate(line, SECRET_TYPE).is_empty()
                })
                .map(|(n, _)| n + 1)
                .collect();
            assert!(
                hits.is_empty(),
                "{}:{} opens an impl that names `{SECRET_TYPE}` — `impl .. for {SECRET_TYPE}`, \
                 `impl From<{SECRET_TYPE}> for ..`, or a bound on it. Every impl for the type \
                 lives in {SECRET_TYPE_FILE}, where the guard spells the whole set out \
                 (CLAUDE.md, Invariants).",
                path.display(),
                hits[0]
            );
        }

        if !path.starts_with("crates") || path.components().any(|c| c.as_os_str() == "tests") {
            continue;
        }
        let production = code_without_test_modules(&code_without_strings(source));
        let hits = renders_in_a_macro(&production, &[SECRET_ACCESSOR]);
        assert!(
            hits.is_empty(),
            "{}:{} names `{SECRET_ACCESSOR}` inside a macro that renders its arguments — a \
             format, a panic, an assertion or a log event. The bytes go to the helper and to \
             git, never into text (CLAUDE.md, Invariants).",
            path.display(),
            hits[0]
        );
        let hits = renders_in_a_macro(&production, &containers);
        assert!(
            hits.is_empty(),
            "{}:{} names a type that holds a credential inside a macro that renders its \
             arguments (CLAUDE.md, Invariants).",
            path.display(),
            hits[0]
        );
        let hits = mentions_crate(&production, SECRET_ACCESSOR);
        let excused = SECRET_READERS.iter().any(|f| Path::new(f) == path);
        assert!(
            hits.is_empty() || excused,
            "{}:{} reads a credential's bytes with `{SECRET_ACCESSOR}`. Only the files in \
             SECRET_READERS may — each is a place the bytes are handed on to the helper or to \
             git — so a new reader is a review, not an edit (CLAUDE.md, Invariants).",
            path.display(),
            hits.first().copied().unwrap_or_default()
        );
    }
    assert!(scanned > 0, "the credential guard scanned nothing");
    for reader in SECRET_READERS.iter().chain(SECRET_HOLDERS) {
        assert!(
            sources.iter().any(|(path, _)| path == Path::new(reader)),
            "the credential guard excuses `{reader}`, which does not exist; a dead roster row \
             is a hole nobody can see"
        );
    }
    for reader in SECRET_READERS {
        let reads = sources
            .iter()
            .filter(|(path, _)| path == Path::new(reader))
            .any(|(_, source)| {
                !mentions_crate(
                    &code_without_test_modules(&code_without_strings(source)),
                    SECRET_ACCESSOR,
                )
                .is_empty()
            });
        assert!(
            reads,
            "SECRET_READERS excuses `{reader}`, which no longer reads a credential in production \
             code. Either the bytes reach the helper or git some other way now (then the \
             roster is stale and the guard checks the wrong file) or the row is a dead excuse: \
             remove it."
        );
    }
}

#[test]
fn the_credential_matcher_catches_the_shapes_it_claims() {
    let forbidden = SECRET_FORBIDDEN_TRAITS;
    // Containers, direct and through another type, struct and enum, braced and tuple.
    let caught_containers = [
        (
            "a braced field",
            "struct Holder {\n    secret: Secret,\n}",
            "Holder",
        ),
        (
            "a tuple field",
            "pub struct Wrapped(pub Secret);",
            "Wrapped",
        ),
        (
            "an optional field",
            "struct State { current: Option<Secret> }",
            "State",
        ),
        (
            "an enum variant",
            "enum Request {\n    Answer(Secret),\n    Cancel,\n}",
            "Request",
        ),
        (
            "a generic field",
            "struct Boxed<T> where T: Send { inner: Box<Secret>, t: T }",
            "Boxed",
        ),
    ];
    for (shape, source, name) in caught_containers {
        assert_eq!(
            types_containing(source, &["Secret"]),
            vec![name.to_owned()],
            "the container matcher missed the {shape} shape: {source:?}"
        );
    }
    assert_eq!(
        types_containing(
            "struct Inner(Secret);\nstruct Outer { inner: Inner }",
            &["Secret", "Inner"]
        ),
        vec!["Inner".to_owned(), "Outer".to_owned()],
        "a container of a container is found once its name is known"
    );
    for (shape, source) in [
        ("a signature", "fn answer(&self, secret: &Secret) {}"),
        ("an import", "use cairn_model::Secret;"),
        (
            "a longer identifier",
            "struct Names { secret_name: String, Secrets: u8 }",
        ),
        (
            "a body of another type",
            "struct Other { x: u8 }\nlet s: Secret = x;",
        ),
        ("prose", "// struct Holder { secret: Secret }\n"),
        ("a string", "let s = \"struct Holder { secret: Secret }\";"),
    ] {
        assert!(
            types_containing(source, &["Secret"]).is_empty(),
            "the container matcher fired on the {shape} shape: {source:?}"
        );
    }
    assert_eq!(
        structs_with_a_field_naming("enum E { A(Secret) }\nstruct S { s: Secret }", &["Secret"]),
        vec![2],
        "the stored-state matcher should see the struct and not the enum"
    );
    assert_eq!(
        structs_with_a_field_naming(
            "enum Reply { Answer(Secret), Cancel }\nstruct Pending { reply: Reply }",
            &["Secret", "Reply"]
        ),
        vec![2],
        "a struct keeping the enum that carries a secret is stored state"
    );
    assert_eq!(
        types_containing(
            "pub struct Wrapped<F> where F: Fn(u8) -> u8 { s: Secret, f: F }",
            &["Secret"]
        ),
        vec!["Wrapped".to_owned()],
        "a where clause with a parenthesised bound hid the body"
    );
    assert!(
        types_containing("struct Pair(u8, Secret);\nstruct Unit;", &["Secret"]) == ["Pair"],
        "a tuple struct's body is still read"
    );

    for (shape, source) in [
        ("a use alias", "use cairn_model::Secret as Credential;"),
        (
            "a grouped use alias",
            "use cairn_model::{Oid, Secret as Credential};",
        ),
        ("a type alias", "type Credential = cairn_model::Secret;"),
        ("a public type alias", "pub type Credential = Secret;"),
        (
            "a generic type alias",
            "pub(crate) type Held<T> = Wrapper<T, Secret>;",
        ),
    ] {
        assert!(
            !renames_type(source, "Secret").is_empty(),
            "the rename matcher missed the {shape} shape: {source:?}"
        );
    }
    for (shape, source) in [
        ("a plain import", "use cairn_model::{AskpassToken, Secret};"),
        ("a signature", "fn f(s: Secret) -> Secret { s }"),
        (
            "an associated type",
            "impl X for Y {\n    type Target = [u8];\n    fn g(s: &Secret) {}\n}",
        ),
        (
            "a longer name",
            "use cairn_model::Secrets as S;\ntype T = Secrets;",
        ),
        ("prose", "// use Secret as Credential\n"),
    ] {
        assert!(
            renames_type(source, "Secret").is_empty(),
            "the rename matcher fired on the {shape} shape: {source:?}"
        );
    }

    // Derives and hand-written impls on a container.
    let caught_impls = [
        (
            "a derived Debug",
            "#[derive(Debug)]\nstruct Holder {\n    secret: Secret,\n}",
        ),
        (
            "a derived Debug among others",
            "#[derive(Clone, PartialEq, Debug)]\nstruct Holder { secret: Secret }",
        ),
        (
            "a derive past another attribute",
            "#[derive(Debug)]\n#[repr(C)]\npub struct Holder(Secret);",
        ),
        (
            "a derived Serialize by path",
            "#[derive(serde::Serialize)]\nstruct Holder { secret: Secret }",
        ),
        (
            "a derive on an enum",
            "#[derive(Debug)]\nenum Request { Answer(Secret) }",
        ),
        (
            "a hand-written Debug",
            "impl std::fmt::Debug for Holder {\n    fn fmt(&self, f: &mut Formatter) -> Result { Ok(()) }\n}",
        ),
        ("a hand-written Display", "impl fmt::Display for Holder {"),
        ("a generic impl", "impl<'a> Debug for Holder<'a> {"),
        ("a Clone", "impl Clone for Holder {"),
        (
            "a Serialize by full path",
            "impl serde::ser::Serialize for Holder {",
        ),
        ("a wrapped header", "impl Debug\n    for Holder\n{"),
        (
            "a path-qualified target",
            "impl std::fmt::Debug for self::Holder {",
        ),
        (
            "a crate-qualified target",
            "impl Debug for crate::channel::Holder<'_> {",
        ),
    ];
    for (shape, source) in caught_impls {
        assert!(
            !derives_or_implements(source, "Holder", forbidden).is_empty()
                || !derives_or_implements(source, "Request", forbidden).is_empty(),
            "the impl matcher missed the {shape} shape: {source:?}"
        );
    }
    for (shape, source) in [
        (
            "a derive on another type",
            "#[derive(Debug)]\nstruct Other { x: u8 }\nstruct Holder(Secret);",
        ),
        (
            "an allowed impl",
            "impl Drop for Holder {\n    fn drop(&mut self) {}\n}",
        ),
        (
            "an inherent impl",
            "impl Holder {\n    fn new() -> Self { todo() }\n}",
        ),
        ("a Debug for another type", "impl Debug for HolderView {"),
        (
            "a derive of something else",
            "#[derive(Default)]\nstruct Holder { secret: Secret }",
        ),
        (
            "prose",
            "// #[derive(Debug)] struct Holder\n// impl Debug for Holder {\n",
        ),
        (
            "an expect attribute",
            "#[expect(missing_debug_implementations)]\nstruct Holder(Secret);",
        ),
    ] {
        assert!(
            derives_or_implements(source, "Holder", forbidden).is_empty(),
            "the impl matcher fired on the {shape} shape: {source:?}"
        );
    }
    assert_eq!(
        derives_or_implements(
            "let a = 1;\n#[derive(Debug)]\nstruct Holder(Secret);",
            "Holder",
            forbidden
        ),
        vec![2],
        "the impl matcher reports the wrong line"
    );

    // Rendering macros naming the accessor or a container.
    let idents = &["expose_secret", "Holder"];
    for (shape, source) in [
        (
            "format of the bytes",
            "let s = format!(\"{:?}\", secret.expose_secret());",
        ),
        (
            "format of a container",
            "let s = format!(\"{:?}\", Holder { secret });",
        ),
        (
            "println",
            "println!(\"{}\", String::from_utf8_lossy(s.expose_secret()));",
        ),
        (
            "a panic",
            "panic!(\"bad secret {:?}\", secret.expose_secret())",
        ),
        (
            "an assertion",
            "assert_eq!(secret.expose_secret(), expected);",
        ),
        (
            "a tracing field",
            "tracing::info!(password = ?secret.expose_secret(), \"asked\");",
        ),
        ("a bare log macro", "debug!(\"got {:?}\", Holder::new(s));"),
        (
            "a log event",
            "log::warn!(\"{}\", s.expose_secret().len());",
        ),
        (
            "a span field",
            "let _s = tracing::info_span!(\"auth\", secret = ?holder_of::<Holder>());",
        ),
        (
            "a wrapped invocation",
            "write!(\n    f,\n    \"{:?}\",\n    secret.expose_secret()\n)",
        ),
        (
            "a bracketed invocation",
            "assert![secret.expose_secret().is_empty()];",
        ),
        // Spelled in two pieces: the Stop hook scans added lines for the literal.
        ("dbg", concat!("dbg", "!(secret.expose_secret());")),
        (
            "format_args",
            "out.write_fmt(format_args!(\"{:?}\", secret.expose_secret()))",
        ),
        (
            "log::log!",
            "log::log!(Level::Info, \"{:?}\", secret.expose_secret());",
        ),
    ] {
        assert!(
            !renders_in_a_macro(source, idents).is_empty(),
            "the rendering matcher missed the {shape} shape: {source:?}"
        );
    }
    for (shape, source) in [
        (
            "a write of the bytes",
            "stream.write_all(secret.expose_secret())?;",
        ),
        (
            "a length in a message",
            "let n = secret.len();\nformat!(\"{n} bytes\")",
        ),
        (
            "a macro naming something else",
            "format!(\"{:?}\", holder_name)",
        ),
        ("a longer identifier", "format!(\"{}\", expose_secrets)"),
        (
            "a function named like a macro",
            "info(secret.expose_secret());",
        ),
        ("prose", "// format!(\"{:?}\", secret.expose_secret())\n"),
        ("a string", "let s = \"format!(secret.expose_secret())\";"),
    ] {
        assert!(
            renders_in_a_macro(source, idents).is_empty(),
            "the rendering matcher fired on the {shape} shape: {source:?}"
        );
    }
    assert_eq!(
        renders_in_a_macro(
            "let a = 1;\nlet b = 2;\nformat!(\"{:?}\", s.expose_secret());",
            idents
        ),
        vec![3],
        "the rendering matcher reports the wrong line"
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

/// The partial-clone pin of `GIT_NO_LAZY_FETCH` skips on a git older than 2.44, which ignores
/// the variable, and a passing test's stderr is hidden, so `CAIRN_REQUIRE_NO_LAZY_FETCH` is
/// what turns that skip into a failure on the merge bar. Pinned here: CI sets it, and the
/// test still reads it in its skip branch (without that, setting it would change nothing).
#[test]
fn the_partial_clone_pin_is_required_in_ci() {
    let root = repo_root();
    let read = |path: &str| {
        std::fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("reading {path}: {e}"))
    };
    let ci = read(".github/workflows/ci.yml");
    let authority = read("crates/cairn-git/src/ops/authority.rs");

    assert!(
        ci.lines()
            .any(|line| line.trim() == "CAIRN_REQUIRE_NO_LAZY_FETCH: 1"),
        ".github/workflows/ci.yml no longer sets `CAIRN_REQUIRE_NO_LAZY_FETCH: 1`, so the \
         partial-clone test of GIT_NO_LAZY_FETCH would skip silently on a git older than 2.44."
    );
    let test = authority
        .split("fn a_read_in_a_partial_clone_does_not_fetch_a_missing_object()")
        .nth(1)
        .unwrap_or_else(|| panic!("the partial-clone test is gone from ops/authority.rs"));
    let skip_branch = test.split("eprintln!(").next().unwrap_or_default();
    assert!(
        skip_branch.contains("CAIRN_REQUIRE_NO_LAZY_FETCH"),
        "the partial-clone test no longer fails on an old git when CAIRN_REQUIRE_NO_LAZY_FETCH \
         is set, so CI's setting of it decides nothing"
    );
}

/// The ssh acceptance criteria (`crates/cairn-git/tests/fetch.rs`) skip where the fixture's
/// `sshd` cannot run, and a passing test's stderr is hidden, so `CAIRN_REQUIRE_SSH_FIXTURE`
/// is what turns a skip into a failure. Pinned here: CI sets it unconditionally (it
/// installs the server), the gate's `test-full` step sets it wherever the fixture would
/// find an `sshd`, and the gate looks for one in every directory the fixture does — a
/// directory dropped from the gate alone would bring the silent skip back on that machine.
/// Each is a line-level check, so none can pass on an empty read.
#[test]
fn the_ssh_criteria_are_required_wherever_they_can_run() {
    let root = repo_root();
    let read = |path: &str| {
        std::fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("reading {path}: {e}"))
    };
    let ci = read(".github/workflows/ci.yml");
    let gate = read("scripts/gate.sh");
    let fixture = read("crates/cairn-git/tests/remotes/ssh.rs");

    assert!(
        ci.lines()
            .any(|line| line.trim() == "CAIRN_REQUIRE_SSH_FIXTURE: 1"),
        ".github/workflows/ci.yml no longer sets `CAIRN_REQUIRE_SSH_FIXTURE: 1`, so the ssh \
         acceptance criteria would skip silently in CI wherever the fixture cannot run."
    );
    assert!(
        ci.lines().any(|line| line.contains("openssh-server")),
        ".github/workflows/ci.yml no longer installs openssh-server, so the ssh fixture has \
         no sshd to start in CI and every ssh criterion would fail there."
    );

    let test_full = gate
        .lines()
        .skip_while(|line| !line.trim_start().starts_with("run_test_full()"))
        .take_while(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        test_full.contains("require_ssh_fixture_where_possible"),
        "scripts/gate.sh's run_test_full no longer calls require_ssh_fixture_where_possible, so \
         the local merge bar would let the ssh criteria skip where they could have run."
    );

    let sbin: Vec<&str> = fixture
        .lines()
        .find(|line| line.trim_start().starts_with("const SBIN"))
        .unwrap_or_else(|| panic!("the ssh fixture no longer declares its SBIN roster"))
        .split('"')
        .skip(1)
        .step_by(2)
        .collect();
    assert!(
        !sbin.is_empty(),
        "the ssh fixture's SBIN roster is empty, so this check compared nothing"
    );
    let probe = gate
        .lines()
        .skip_while(|line| !line.starts_with("require_ssh_fixture_where_possible()"))
        .take_while(|line| line.trim() != "}")
        .collect::<Vec<_>>()
        .join("\n");
    for dir in sbin {
        assert!(
            probe.contains(&format!("{dir}/sshd")),
            "the ssh fixture looks for sshd in {dir} but scripts/gate.sh's \
             require_ssh_fixture_where_possible does not, so on a machine whose sshd is only \
             there the gate would not require the fixture and the criteria would skip silently."
        );
    }
}

/// Where `docs/design/` lives: intent, in the tense `docs/CLAUDE.md` gives it.
const DESIGN_DOCS_DIR: &str = "docs/design";

/// Phrases that date a sentence to a packet's progress rather than to the design.
const POINT_IN_TIME_PHRASES: &[&str] = &[
    "not yet built",
    "not yet implemented",
    "as built by",
    "as-built paragraph",
    "planned by the",
    "amended by the",
    "decided, not yet",
    "first draft",
    "originally said",
    "this document first",
    "previously carried",
    "until the packet",
];

/// Lock and criterion ids that live in a work dir or a PRD, never in the design:
/// brainstorm locks (L), program and packet open questions (O, Q), acceptance
/// criteria (A, C). Decisions (D), tiers and PRD requirements (R, as pointers) stay.
const WORK_ID_PREFIXES: &[char] = &['L', 'O', 'Q', 'A', 'C'];

/// Why `line` reads as point-in-time state, or `None` if it reads as intent.
fn point_in_time_state(line: &str) -> Option<String> {
    let lower = line.to_lowercase();
    if let Some(phrase) = POINT_IN_TIME_PHRASES.iter().find(|p| lower.contains(*p)) {
        return Some(format!("the phrase {phrase:?}"));
    }
    if line.contains("~~") {
        return Some("a struck-through (answered) item".to_owned());
    }
    let words: Vec<&str> = line
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
        .filter(|w| !w.is_empty())
        .collect();
    for pair in words.windows(2) {
        if pair[0].eq_ignore_ascii_case("phase")
            && pair[1].starts_with(|c: char| c.is_ascii_digit())
        {
            return Some(format!("a phase reference ({} {})", pair[0], pair[1]));
        }
    }
    for word in &words {
        let bytes = word.as_bytes();
        let is_date = bytes.len() == 10
            && bytes[4] == b'-'
            && bytes[7] == b'-'
            && bytes
                .iter()
                .enumerate()
                .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit());
        if is_date {
            return Some(format!("a date ({word})"));
        }
        let mut chars = word.chars();
        if let Some(first) = chars.next()
            && WORK_ID_PREFIXES.contains(&first)
            && !chars.as_str().is_empty()
            && chars
                .as_str()
                .chars()
                .all(|c| c.is_ascii_digit() || c == '-')
            && chars.as_str().starts_with(|c: char| c.is_ascii_digit())
        {
            return Some(format!("a work-dir or PRD id ({word})"));
        }
    }
    None
}

/// `docs/design/` states the whole design as intent. A packet's progress — dates,
/// phases, "not yet built", "as built by", brainstorm lock ids, criterion ids,
/// struck-through answers, the history of the doc's own revisions — belongs in
/// `docs/prd/`, `docs/work/` or `docs/systems/`, which is what `docs/CLAUDE.md`
/// says; this is its twin. Residual review obligation: the phrase list is finite,
/// so a sentence that dates itself in other words is the review's.
#[test]
fn design_docs_carry_no_point_in_time_state() {
    let root = repo_root();
    let dir = root.join(DESIGN_DOCS_DIR);
    let mut docs: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("reading {DESIGN_DOCS_DIR}: {e}"))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .collect();
    docs.sort();
    assert!(
        !docs.is_empty(),
        "found no markdown under {DESIGN_DOCS_DIR}, so this guard checked nothing"
    );

    let mut findings = Vec::new();
    for path in &docs {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
        let mut fenced = false;
        for (number, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("```") {
                fenced = !fenced;
                continue;
            }
            if fenced {
                continue;
            }
            if let Some(why) = point_in_time_state(line) {
                let relative = path.strip_prefix(&root).unwrap_or(path);
                findings.push(format!("{}:{}: {why}", relative.display(), number + 1));
            }
        }
    }
    assert!(
        findings.is_empty(),
        "docs/design/ is intent, not progress (docs/CLAUDE.md). State the design itself and \
         move status to docs/prd/, docs/work/ or docs/systems/:\n{}",
        findings.join("\n")
    );
}

#[test]
fn the_point_in_time_matcher_catches_the_shapes_it_claims() {
    let caught = [
        "Locked 2026-09-14 with the user.",
        "Until the packet's phase 03 lands, this is true.",
        "Phase 4 changes both halves.",
        "**Amended by the `diff-engine` packet.** Decided, not yet built.",
        "As built by the `history-graph` packet, the pool is one worker.",
        "Planned by the `diff-engine` packet (its brainstorm L8).",
        "decided in L6",
        "closing the program's O1",
        "Q3 is the one that can reach a criterion",
        "the way `history-graph`'s A7 does",
        "(`docs/prd/diff-engine.md` C15)",
        "- ~~Side-by-side diff.~~ Answered.",
        "This revises what the first draft understated.",
        "This bullet originally said conflicts open an editor.",
        "This document first said Fork puts actions on the header.",
    ];
    for line in caught {
        assert!(
            point_in_time_state(line).is_some(),
            "the point-in-time matcher missed {line:?}"
        );
    }

    let ignored = [
        "### D1 — gitoxide reads, `git` subprocess writes",
        "Spec: `docs/prd/diff-engine.md` R3; evidence: `docs/research/diff-engine/gix-diff-api.md`.",
        "## Tier 6½ — Forge links",
        "Verified against the gix 0.87.1 source.",
        "Verified against git 2.55.0's documentation.",
        "The alternative toolkits either bring a browser or bring C++.",
        "Lane colours are a six-step set.",
        "Fork's third tab, File Tree, is deferred to issue #31.",
        "A worker pool per repository; phases of a rebase are a state machine.",
        "Choosing gitoxide does not deliver speed; it makes it possible.",
    ];
    for line in ignored {
        assert_eq!(
            point_in_time_state(line),
            None,
            "the point-in-time matcher fired on {line:?}"
        );
    }
}
