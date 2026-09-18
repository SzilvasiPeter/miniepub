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
| `rawzip` | Read EPUB archives (which are ZIP files). |
| `noflate` | Compression algorithm to use during unzip |
| `xml` | Parse the XML metadata and content inside the archive. |

## Verification

All linters are configured in `Cargo.toml` and run via just recipes:

- `just lint` - fmt, clippy, machete
- `just lint-workflows` - actionlint + zizmor on `.github/workflows/`
- `just coverage` - coverage gate (80% line minimum)
- `just safety` - audit, geiger, deny 

## TDD Workflow

All changes must follow Test-Driven Development using `cargo nextest`.

1. **Red**: Write a failing test -> `cargo nextest run --test <test_name>`
2. **Green**: Write implementation -> `cargo nextest run --test <test_name>`
3. **Refactor**: Clean up and verify -> `cargo nextest run`
