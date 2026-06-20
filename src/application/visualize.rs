// SPDX-License-Identifier: MIT OR Apache-2.0
//! Dependency graph visualization for recipe tasks.
//!
//! This module provides functions to generate graph representations
//! of task dependencies in DOT (Graphviz) and Mermaid.js formats.
//!
//! # Examples
//!
//! ```rust
//! use taskkit::domain::recipe::RecipeTask;
//! use taskkit::application::visualize::generate_dot;
//! use std::collections::HashMap;
//!
//! let tasks = vec![
//!     RecipeTask {
//!         name: "lint".into(),
//!         description: None,
//!         command: "cargo clippy".into(),
//!         depends_on: vec![],
//!         vars: HashMap::new(),
//!         timeout: None,
//!         condition: None,
//!     },
//!     RecipeTask {
//!         name: "build".into(),
//!         description: None,
//!         command: "cargo build".into(),
//!         depends_on: vec!["lint".into()],
//!         vars: HashMap::new(),
//!         timeout: None,
//!         condition: None,
//!     },
//! ];
//!
//! let dot = generate_dot(&tasks);
//! assert!(dot.contains("digraph"));
//! assert!(dot.contains("\"lint\" -> \"build\""));
//! ```
//!
//! # Formats
//!
//! ## DOT (Graphviz)
//!
//! ```dot
//! digraph G {
//!     rankdir=LR;
//!     "lint";
//!     "build";
//!     "lint" -> "build";
//! }
//! ```
//!
//! ## Mermaid flowchart
//!
//! ```mermaid
//! flowchart LR
//!     lint --> build
//! ```

use crate::domain::recipe::RecipeTask;

/// Format option for graph output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphFormat {
    /// DOT format (Graphviz).
    Dot,
    /// Mermaid.js flowchart format.
    Mermaid,
}

impl std::str::FromStr for GraphFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "dot" => Ok(GraphFormat::Dot),
            "mermaid" => Ok(GraphFormat::Mermaid),
            other => Err(format!(
                "Unknown format `{other}`. Supported formats: dot, mermaid"
            )),
        }
    }
}

/// Generate a dependency graph in DOT format.
///
/// Produces a `digraph` where each task is a node and each dependency
/// is a directed edge from the dependency to the dependent task.
pub fn generate_dot(tasks: &[RecipeTask]) -> String {
    let mut output = String::from("digraph G {\n");
    output.push_str("    rankdir=LR;\n");
    output.push_str("    node [shape=box, style=rounded];\n\n");

    // Emit all task nodes
    for task in tasks {
        let label = escape_dot_label(&task.name);
        output.push_str(&format!("    {label};\n"));
    }

    // Emit dependency edges
    if !tasks.is_empty() {
        output.push('\n');
        for task in tasks {
            for dep in &task.depends_on {
                let from = escape_dot_label(dep);
                let to = escape_dot_label(&task.name);
                output.push_str(&format!("    {from} -> {to};\n"));
            }
        }
    }

    output.push_str("}\n");
    output
}

/// Generate a dependency graph in Mermaid.js flowchart format.
///
/// Produces a `flowchart LR` (left-to-right) with directed edges
/// representing task dependencies.
pub fn generate_mermaid(tasks: &[RecipeTask]) -> String {
    let mut output = String::from("flowchart LR\n");

    // Emit dependency edges (Mermaid infers nodes from edges)
    for task in tasks {
        if task.depends_on.is_empty() {
            // Isolated node — declare it so it appears in the diagram
            let name = escape_mermaid_id(&task.name);
            output.push_str(&format!("    {name}[{label}]\n",
                label = mermaid_label(&task.name)
            ));
        } else {
            for dep in &task.depends_on {
                let from = escape_mermaid_id(dep);
                let to = escape_mermaid_id(&task.name);
                output.push_str(&format!("    {from} --> {to}\n"));
            }
        }
    }

    output
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Escape special characters for DOT identifiers/labels.
fn escape_dot_label(s: &str) -> String {
    // DOT identifiers with special chars need quoting.
    // We always quote with double quotes and escape internal quotes.
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// Escape special characters for Mermaid node IDs.
fn escape_mermaid_id(s: &str) -> String {
    // Mermaid node IDs should not contain spaces or special chars.
    // We replace problematic characters with underscores.
    let sanitized: String = s
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    if sanitized.is_empty() {
        "node".to_string()
    } else {
        sanitized
    }
}

/// Create a Mermaid node label with proper quoting.
fn mermaid_label(s: &str) -> String {
    if s.contains('"') || s.contains('\n') {
        // Use single quotes for labels containing double quotes
        format!("'{s}'")
    } else {
        format!("\"{s}\"")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_task(name: &str, deps: Vec<&str>) -> RecipeTask {
        RecipeTask {
            name: name.to_string(),
            description: None,
            command: String::new(),
            depends_on: deps.into_iter().map(String::from).collect(),
            vars: HashMap::new(),
            timeout: None,
            condition: None,
        }
    }

    // -- DOT tests ---------------------------------------------------------

    #[test]
    fn test_dot_empty_tasks() {
        let dot = generate_dot(&[]);
        assert_eq!(dot, "digraph G {\n    rankdir=LR;\n    node [shape=box, style=rounded];\n\n}\n");
    }

    #[test]
    fn test_dot_single_task_no_deps() {
        let tasks = vec![make_task("hello", vec![])];
        let dot = generate_dot(&tasks);
        assert!(dot.contains("\"hello\";"));
        assert!(!dot.contains("->"));
    }

    #[test]
    fn test_dot_simple_dag() {
        let tasks = vec![
            make_task("lint", vec![]),
            make_task("build", vec!["lint"]),
            make_task("test", vec!["build"]),
        ];
        let dot = generate_dot(&tasks);
        assert!(dot.contains("\"lint\" -> \"build\""));
        assert!(dot.contains("\"build\" -> \"test\""));
    }

    #[test]
    fn test_dot_complex_dependencies() {
        let tasks = vec![
            make_task("init", vec![]),
            make_task("lint", vec!["init"]),
            make_task("build", vec!["lint"]),
            make_task("test", vec!["build"]),
            make_task("deploy", vec!["build", "test"]),
        ];
        let dot = generate_dot(&tasks);
        assert!(dot.contains("\"init\" -> \"lint\""));
        assert!(dot.contains("\"lint\" -> \"build\""));
        assert!(dot.contains("\"build\" -> \"test\""));
        assert!(dot.contains("\"build\" -> \"deploy\""));
        assert!(dot.contains("\"test\" -> \"deploy\""));
    }

    #[test]
    fn test_dot_fan_out() {
        let tasks = vec![
            make_task("parse", vec![]),
            make_task("validate", vec!["parse"]),
            make_task("transform", vec!["parse"]),
            make_task("output", vec!["validate", "transform"]),
        ];
        let dot = generate_dot(&tasks);
        assert!(dot.contains("\"parse\" -> \"validate\""));
        assert!(dot.contains("\"parse\" -> \"transform\""));
        assert!(dot.contains("\"validate\" -> \"output\""));
        assert!(dot.contains("\"transform\" -> \"output\""));
    }

    #[test]
    fn test_dot_no_edges_for_isolated_tasks() {
        let tasks = vec![
            make_task("a", vec![]),
            make_task("b", vec![]),
            make_task("c", vec![]),
        ];
        let dot = generate_dot(&tasks);
        for task in &tasks {
            assert!(dot.contains(&format!("\"{name}\";", name = task.name)));
        }
        // No arrows
        assert_eq!(dot.matches("->").count(), 0);
    }

    // -- Mermaid tests -----------------------------------------------------

    #[test]
    fn test_mermaid_empty_tasks() {
        let mermaid = generate_mermaid(&[]);
        assert_eq!(mermaid, "flowchart LR\n");
    }

    #[test]
    fn test_mermaid_single_task_no_deps() {
        let tasks = vec![make_task("hello", vec![])];
        let mermaid = generate_mermaid(&tasks);
        // Isolated nodes use the `id[label]` syntax
        assert!(mermaid.contains("hello[\"hello\"]"));
    }

    #[test]
    fn test_mermaid_simple_dag() {
        let tasks = vec![
            make_task("lint", vec![]),
            make_task("build", vec!["lint"]),
        ];
        let mermaid = generate_mermaid(&tasks);
        assert!(mermaid.contains("lint --> build"));
    }

    #[test]
    fn test_mermaid_complex_dependencies() {
        let tasks = vec![
            make_task("init", vec![]),
            make_task("build", vec!["init"]),
            make_task("test", vec!["build"]),
            make_task("deploy", vec!["build", "test"]),
        ];
        let mermaid = generate_mermaid(&tasks);
        assert!(mermaid.contains("init --> build"));
        assert!(mermaid.contains("build --> test"));
        assert!(mermaid.contains("build --> deploy"));
        assert!(mermaid.contains("test --> deploy"));
    }

    #[test]
    fn test_mermaid_sanitizes_names() {
        let tasks = vec![make_task("my task (1)", vec![])];
        let mermaid = generate_mermaid(&tasks);
        let id = "my_task__1_";
        assert!(mermaid.contains(&format!("{id}[")));
    }

    // -- Format parsing ----------------------------------------------------

    #[test]
    fn test_graph_format_from_str() {
        assert_eq!("dot".parse::<GraphFormat>().unwrap(), GraphFormat::Dot);
        assert_eq!("DOT".parse::<GraphFormat>().unwrap(), GraphFormat::Dot);
        assert_eq!("mermaid".parse::<GraphFormat>().unwrap(), GraphFormat::Mermaid);
        assert_eq!("MERMAID".parse::<GraphFormat>().unwrap(), GraphFormat::Mermaid);
    }

    #[test]
    fn test_graph_format_invalid() {
        let err = "pdf".parse::<GraphFormat>().unwrap_err();
        assert!(err.contains("pdf"));
        assert!(err.contains("dot"));
        assert!(err.contains("mermaid"));
    }
}
