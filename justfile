lint:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo machete

lint-workflows:
    actionlint .github/workflows/*.yml
    shuck check --extend-select ALL --ignore S081 . # Ignore S081: file-header comments are noise inside `run:` blocks.
    zizmor .

coverage:
    cargo llvm-cov --fail-under-lines 80

open:
    cargo llvm-cov --html --open

safety:
    cargo audit
    # TODO: remove the `forbid-only` flag once the https://github.com/geiger-rs/cargo-geiger/issues/577 is solved.
    cargo geiger --forbid-only
    cargo deny check licenses advisories
