# List the recipes.
[private]
default:
    @just --list --unsorted

# Run all CI jobs.
ci: fmt check clippy test doc build deny

# Build the crate.
build:
    cargo build --all-features --all-targets --locked

# Check formatting.
fmt:
    cargo fmt --all --check

# Check every feature combination.
check:
    cargo hack check --feature-powerset --all-targets --locked

# Run clippy.
clippy:
    cargo hack clippy --each-feature --all-targets --locked -- --deny warnings

# Run tests.
test:
    cargo hack test --each-feature --locked

# Run tests against the live Sessionize API.
live:
    cargo test --test live --locked -- --ignored

# Run rustdoc.
[env("RUSTDOCFLAGS", "--deny warnings")]
doc:
    cargo hack doc --each-feature --no-deps --locked

# Check dependencies for advisories, licenses and sources.
deny:
    cargo deny check
