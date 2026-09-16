//! Invariant guards.

use std::collections::BTreeSet;
use std::path::Path;

use cairn_guards::{
    code_only, code_without_strings, code_without_test_modules, configures_process_environment,
    constructs_named_struct, constructs_process_command, constructs_struct, declared_dependencies,
    implements_type, mentions_crate, reads_row_content_partially, repo_root, rust_sources,
    spawns_git, waits_on_work,
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

/// Structural, not behavioural: the value is pinned by `cairn-git`'s own tests over the builder
/// and over a stub `git` that prints its environment. This guard is against erosion of the ONE
/// construction path those tests rely on. Scope: the product crates' `src/` (test modules
/// blanked); a test fixture may spawn what it likes.
#[test]
fn every_git_invocation_disables_the_terminal_prompt() {
    let environment_file = Path::new(PROCESS_ENVIRONMENT_FILE);
    let mut environment_source = None;
    let mut literals_elsewhere = 0usize;
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
            literals_elsewhere += hits.len();
        }
    }
    assert!(
        scanned > 0,
        "the terminal-prompt guard scanned nothing; did the crates move?"
    );
    assert_eq!(literals_elsewhere, 0);

    let source = environment_source.unwrap_or_else(|| {
        panic!("{PROCESS_ENVIRONMENT_FILE} is gone; the environment it builds is an invariant")
    });
    let production = code_without_test_modules(&code_without_strings(&source));
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
        "the struct matcher counts lines, not literals"
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
