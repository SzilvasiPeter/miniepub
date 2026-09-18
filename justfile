lint:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo machete

lint-workflows:
    actionlint .github/workflows/*.yml
    zizmor .

coverage:
    cargo llvm-cov --fail-under-lines 80

open:
    cargo llvm-cov --html --open

safety:
    cargo audit
    # Remove the `forbid-only` flag if the https://github.com/geiger-rs/cargo-geiger/issues/577 is solved.
    cargo geiger --forbid-only
    cargo deny check licenses advisories
