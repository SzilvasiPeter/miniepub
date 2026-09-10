fmt:
    cargo fmt --check

clippy:
    cargo clippy --all-targets -- -D warnings

deny:
    cargo deny check licenses advisories

machete:
    cargo machete

coverage:
    cargo llvm-cov --fail-under-lines 80

lint: fmt clippy deny machete

check: fmt clippy coverage
