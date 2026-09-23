//! Invariant guards.

use std::collections::BTreeSet;
use std::path::Path;

use cairn_guards::{
    code_only, code_without_strings, code_without_test_modules, configures_process_environment,
    constructs_named_struct, constructs_process_command, constructs_struct, declared_dependencies,
    derives_or_implements, implements_type, mentions_crate, reads_row_content_partially,
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

/// Where a `git` process is built: the environment module, the only place a
/// `std::process::Command` comes into being, and the only production file that may name it.
const PROCESS_ENVIRONMENT_FILE: &str = "crates/cairn-git/src/ops/environment.rs";
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

#[test]
fn the_process_environment_matcher_catches_the_shapes_it_claims() {
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
