# Contributing to modsh

Thank you for your interest in contributing to modsh!

## Development Setup

### Required Tools

```bash
# Rust toolchain (stable channel)
rustup update stable

# Code quality tools
cargo install cargo-audit      # Security audit
cargo install cargo-watch      # Auto-rebuild on changes
cargo install cargo-nextest    # Better test runner
cargo install cargo-skill      # Code analysis

# Pre-commit hooks (optional but recommended)
pip install pre-commit
pre-commit install
```

### Building

```bash
cargo build --workspace
cargo build --release --workspace
```

### Testing

```bash
cargo test --workspace

# With nextest
cargo nextest run
```

### Code Quality

```bash
# Format
cargo fmt --all

# Lint
cargo clippy --workspace -- -D warnings

# Security audit
cargo audit

# Check all at once (CI does this)
cargo check --workspace
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
```

## Project Structure

- `modsh-core/` — POSIX shell core (Apache-2.0)
- `modsh-interactive/` — Interactive features (Apache-2.0)
- `modsh-ai/` — AI context engine (BSL 1.1)
- `modsh-cli/` — Binary entrypoint (Apache-2.0)

## License

By contributing, you agree that your contributions will be licensed under:
- Apache License 2.0 for `modsh-core`, `modsh-interactive`, `modsh-cli`
- Business Source License 1.1 for `modsh-ai`
