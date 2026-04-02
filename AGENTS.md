# AGENTS.md — Tasken

Extends shelf-level AGENTS.md rules for Tasken.

## Project Identity

- **Name**: Tasken
- **Description**: Task execution framework with scheduling and workflow support
- **Language**: Rust

## Project-Specific Rules

### Test-First Mandate

- **For NEW modules**: test file MUST exist before implementation file
- **For BUG FIXES**: failing test MUST be written before the fix
- **For REFACTORS**: existing tests must pass before AND after

### Quality Gates

All PRs must pass:
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo fmt --check`

### Commit Messages

Format: `<type>(<scope>): <description>`

Types: `feat`, `fix`, `chore`, `docs`, `refactor`, `test`, `ci`

### File Organization

```
src/
├── lib.rs           # Main library entry
├── executor/        # Task execution engine
├── scheduler/       # Scheduling logic
├── workflow/        # Workflow definitions
└── task/            # Task traits and types
```

## Testing Requirements

- Unit tests for all public APIs
- Integration tests for workflow execution
- Property-based tests for scheduler
- Minimum 80% code coverage
