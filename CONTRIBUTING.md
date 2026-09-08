# Contributing to OpenAccounting

Thank you for considering contributing to OpenAccounting.

## Development Setup

1. Clone the repository
2. Install Rust (stable) via rustup
3. Copy `.env.example` to `.env` and configure
4. Run `docker compose up -d` for PostgreSQL
5. Run `cargo sqlx migrate run` to apply migrations
6. Run `cargo test --features test-support` to verify

## Code Standards

- `cargo fmt --check` must pass on every commit
- `cargo clippy -- -D warnings` must be clean
- `cargo test` must pass (including integration tests)
- All new public functions require at least one unit test
- All new HTTP routes require an integration test
- All new domain invariants require a property test

## Pull Request Process

1. Fork the repository and create a feature branch
2. Write tests first (see AGENTS.md §4)
3. Implement the feature
4. Ensure all checks pass
5. Update documentation if behavior changes
6. Submit a pull request with a clear description

## Commit Messages

- Use imperative mood ("Add feature" not "Added feature")
- First line ≤ 72 characters
- Reference issues where applicable

## Architecture

See `AGENTS.md` for the full architecture contract.
The layered model (domain → reports → handlers → templates)
must not be violated.

## Code Review

All pull requests require review before merge.
Reviewers should check:
- Test coverage for new functionality
- No unwrap/expect in production code
- Documentation updates for user-facing changes
- Migration reversibility for schema changes
