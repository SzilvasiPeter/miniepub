# AGENTS.md

Guidance for AI agents working in this repository.

## Project goal

A minimal EPUB reader library in Rust. Parse a ZIP of XML files into structured chapters. Parsing pipeline: `container.xml -> OPF -> spine resolution -> chapters`.

Design priorities: tiny dependency graph, low complexity, 80%+ line coverage, TDD (red-green-refactor).

## Code style constraints

1. **No macros, lifetimes, unsafe, or async** — keep the code as simple as possible
2. **Readability first** — prefer explicit, clear code over clever or terse patterns
3. **Deterministic linting** — prefer lints with concrete rules over subjective style preferences

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
