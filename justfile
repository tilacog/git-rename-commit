clippy:
    cargo clippy -- -W clippy::pedantic

fmt:
    cargo fmt --check

fix:
    cargo clippy --fix --allow-dirty --allow-staged -- -W clippy::pedantic
    cargo fmt

test:
    cargo test

ci: fmt clippy test
