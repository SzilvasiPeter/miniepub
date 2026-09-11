# AGENTS.md

Guidance for AI agents working in this repository.

## Project goal

A minimal EPUB reader library in Rust. Parse a ZIP of XML files into structured chapters. Parsing pipeline: `container.xml -> OPF -> spine resolution -> chapters`.

Design priorities: tiny dependency graph, low complexity, 80%+ line coverage, TDD (red-green-refactor).

## Code style constraints

1. **No macros, lifetimes, unsafe, or async** — keep the code as simple as possible.
2. **Minimal and readable** — prefer the smallest code that expresses intent. Avoid ceremony, boilerplate, and scaffolding (temp files, multi-line setup) when a single idiomatic expression covers it.
3. **No branching where an idiom fits** — fold conditionals into the language's standard shorthand instead of `if/else` blocks. In Rust, use combinators (`unwrap_or`, `or_else`, `filter_map`, iterator chains); in shell, use parameter expansion (`${VAR:-default}`) and `case` patterns.
4. **Do not reimplement the wheel** — prefer an existing tool over hand-rolled text munging. Query or change manifests with cargo subcommands (`cargo set-version`, `cargo get package.version`) instead of `grep`/`sed`/`awk`; let the tool validate input.
5. **Deterministic linting** — use lints with concrete rules over subjective style preferences.

## Dependencies

Add new dependencies with `cargo add <crate> --no-default-features` to keep the default build lean. Anything heavy goes behind an optional feature flag.

| Crate | Why |
|---|---|
| `zip` | Read EPUB archives (which are ZIP files). |
| `quick-xml` | Parse the XML metadata and content inside the archive. |

## Verification

All linters are configured in `Cargo.toml` and run via just recipes:

- `just lint` — format, clippy, deny, machete
- `just lint-workflows` — actionlint + zizmor on `.github/workflows/`
- `just check` — format, clippy, coverage gate (80% line minimum)
- `just fmt` / `just clippy` / `just deny` / `just machete` / `just coverage` — individual steps

## Project structure

Single crate, no workspace unless a CLI/bin is added.

Planned modules (each maps 1:1 to an EPUB spec artifact):

| Module | Responsibility |
|---|---|
| `lib.rs` | Public API (`Book`, `open`, `parse`) — thin re-exports only |
| `error.rs` | Error enum + Result alias |
| `container.rs` | META-INF/container.xml -> rootfile path |
| `opf.rs` | metadata, manifest, spine |
| `toc.rs` | EPUB2 NCX + EPUB3 nav |
| `spine.rs` | Reading order resolution |
| `chapter.rs` | Chapter content wrapper |

Tests: `tests/fixtures.rs` (in-memory EPUB builder), `tests/end_to_end.rs`.

## TDD workflow

Never depend on fixture files on disk — build EPUBs in-memory in test helpers.

Development loop:

1. `cargo nextest run --lib` — fast feedback while developing.
2. `cargo nextest run --lib <module>::` — focus on current module.
3. `just coverage` — coverage gate after a feature lands.

Corrupt-input tests: hand-craft broken byte vectors, assert exact `Error` variant.

## Release workflow

Handled by `release.sh` — mirror its shape when changing it:

1. `cargo set-version --bump <major|minor|patch>` or `cargo set-version X.Y.Z` — no manual semver parsing or validation.
2. `cargo check` to refresh `Cargo.lock`.
3. Read the new version back with `cargo get package.version`.
4. Commit `Cargo.toml` and `Cargo.lock` as `release: <version>`, tag `v<version>` (annotated), push current branch with tags.
