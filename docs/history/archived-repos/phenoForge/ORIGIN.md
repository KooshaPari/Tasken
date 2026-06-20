# Origin: phenoForge

Source repo: KooshaPari/phenoForge
Absorbed into: KooshaPari/Tasken/docs/history/archived-repos/phenoForge
Absorption date: 2026-06-20

## Product decision

phenoForge contained a strong Rust build/task-runner product contract and SOTA research for a build orchestrator, but its implementation was a small TaskGraph resolver and CLI stub. Tasken is the better end product because it already owns active task execution, scheduling, workflows, plugin architecture, storage adapters, runtime tests, and CLI integration.

The phenoForge contract is preserved here as historical product/research input for Tasken rather than as a standalone repo or new crate.
