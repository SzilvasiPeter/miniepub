coverage:
    cargo llvm-cov --fail-under-lines 80

open:
    cargo llvm-cov --html --open

lint:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo machete
    cargo audit
    cargo deny check licenses advisories

lint-workflows:
    actionlint .github/workflows/*.yml
    zizmor .
