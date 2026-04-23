default:
    @just --list

format:
    cargo fmt

lint:
    cargo clippy --all-targets --all-features -- -D warnings

test:
    cargo test

test-snapshots:
    cargo test tui::tests -- --nocapture

review-snapshots:
    cargo insta review
