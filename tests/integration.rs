// SPDX-License-Identifier: MIT OR Apache-2.0
// Integration tests for dependency cycle detection and retry storm scenarios.
//
// These tests exercise cross-module behaviour that inline `#[cfg(test)]`
// blocks cannot reach: multi-task service integration with concurrent
// retry logic, workflow DAG cycle detection, and topological sort
// validation at the integration level.
//
// Run with: `cargo test --test integration`

mod dependency_cycles;
mod retry_storm;
