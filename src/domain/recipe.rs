// SPDX-License-Identifier: MIT OR Apache-2.0
//! Recipe/Taskenfile parser — parses TOML/YAML recipe files into domain [`Recipe`] objects.
//!
//! A Taskenfile defines a named collection of task steps with commands,
//! dependencies between them, metadata, and templated variables.
//!
//! # Format
//!
//! ```toml
//! name = "ci-pipeline"
//! description = "Continuous integration pipeline"
//! author = "Phenotype"
//!
//! [vars]
//! project = "tasken"
//! target = "x86_64"
//!
//! [[tasks]]
//! name = "lint"
//! command = "cargo clippy --all-targets"
//!
//! [[tasks]]
//! name = "build"
//! command = "cargo build --target {{ target }}"
//! depends_on = ["lint"]
//!
//! [[tasks]]
//! name = "test"
//! command = "cargo test --package {{ project }}"
//! depends_on = ["build"]
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during recipe parsing or validation.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// The file could not be read from disk.
    #[error("Failed to read Taskenfile: {0}")]
    Io(#[from] std::io::Error),

    /// TOML deserialization failed.
    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),

    /// YAML deserialization failed.
    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// A variable referenced in a command is not defined in the recipe vars.
    #[error("Undefined variable `{name}` referenced in task `{task}`")]
    UndefinedVariable {
        /// Variable name (without braces).
        name: String,
        /// Task name that references the variable.
        task: String,
    },

    /// The recipe has no tasks defined.
    #[error("Recipe must define at least one task")]
    NoTasks,

    /// A task name is empty or contains invalid characters.
    #[error("Invalid task name: `{0}`")]
    InvalidTaskName(String),

    /// A dependency refers to a task that does not exist.
    #[error("Unknown dependency `{dep}` in task `{task}`")]
    UnknownDependency {
        /// Dependency step name.
        dep: String,
        /// Task that listed the dependency.
        task: String,
    },

    /// Unsupported file extension (neither .toml nor .yaml/.yml).
    #[error("Unsupported file extension: `{0}` (expected .toml, .yaml, or .yml)")]
    UnsupportedExtension(String),

    /// Generic validation error.
    #[error("Validation error: {0}")]
    Validation(String),
}

// ---------------------------------------------------------------------------
// Raw serializable representation (mirrors TOML/YAML schema)
// ---------------------------------------------------------------------------

/// A single step definition inside a Taskenfile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStepDef {
    /// Step name (must be unique within a recipe).
    pub name: String,
    /// Optional human-readable description.
    pub description: Option<String>,
    /// Shell command to execute.
    pub command: String,
    /// Names of steps that must complete before this one runs.
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Per-step variables (merged with recipe-level vars).
    #[serde(default)]
    pub vars: HashMap<String, String>,
    /// Optional timeout in seconds.
    pub timeout: Option<u64>,
}

/// Raw parsed representation of a Taskenfile (TOML or YAML).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeFile {
    /// Recipe name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Optional author.
    pub author: Option<String>,
    /// Recipe-level variables (available to all task commands via `{{ var }}`).
    #[serde(default)]
    pub vars: HashMap<String, String>,
    /// Ordered list of task step definitions.
    pub tasks: Vec<TaskStepDef>,
}

// ---------------------------------------------------------------------------
// Domain model
// ---------------------------------------------------------------------------

/// A resolved task step ready for execution.
#[derive(Debug, Clone)]
pub struct RecipeTask {
    /// Step name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Shell command (variables resolved).
    pub command: String,
    /// Names of steps that must complete first.
    pub depends_on: Vec<String>,
    /// Merged variables available to this task.
    pub vars: HashMap<String, String>,
    /// Optional timeout.
    pub timeout: Option<Duration>,
}

/// A fully resolved recipe parsed from a Taskenfile.
///
/// All `{{ var }}` placeholders in commands have been interpolated
/// with their values from the recipe-level and task-level variable maps.
#[derive(Debug, Clone)]
pub struct Recipe {
    /// Recipe name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Optional author.
    pub author: Option<String>,
    /// Resolved task steps.
    pub tasks: Vec<RecipeTask>,
    /// Top-level dependency names (derived from task depends_on).
    pub dependencies: Vec<String>,
    /// Recipe-level variables.
    pub vars: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Parses Taskenfile content (TOML or YAML) into [`Recipe`] domain objects.
///
/// # Example
///
/// ```rust
/// use taskkit::domain::recipe::TaskenfileParser;
///
/// let toml = r#"
/// name = "example"
///
/// [vars]
/// who = "world"
///
/// [[tasks]]
/// name = "greet"
/// command = "echo hello {{ who }}"
/// "#;
///
/// let recipe = TaskenfileParser::parse_toml(toml).unwrap();
/// assert_eq!(recipe.tasks[0].command, "echo hello world");
/// ```
pub struct TaskenfileParser;

impl TaskenfileParser {
    /// Parse a TOML string into a [`Recipe`].
    pub fn parse_toml(input: &str) -> Result<Recipe, ParseError> {
        let recipe_file: RecipeFile = toml::from_str(input)?;
        Self::from_recipe_file(recipe_file)
    }

    /// Parse a YAML string into a [`Recipe`].
    pub fn parse_yaml(input: &str) -> Result<Recipe, ParseError> {
        let recipe_file: RecipeFile = serde_yaml::from_str(input)?;
        Self::from_recipe_file(recipe_file)
    }

    /// Read a file from disk and parse it based on its extension.
    ///
    /// Supports `.toml`, `.yaml`, and `.yml` extensions.
    pub fn parse_file(path: &std::path::Path) -> Result<Recipe, ParseError> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| ParseError::UnsupportedExtension(
                path.display().to_string(),
            ))?;

        if ext != "toml" && ext != "yaml" && ext != "yml" {
            return Err(ParseError::UnsupportedExtension(ext.to_string()));
        }

        let content = std::fs::read_to_string(path)?;

        match ext {
            "toml" => Self::parse_toml(&content),
            "yaml" | "yml" => Self::parse_yaml(&content),
            _ => unreachable!(),
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Convert a [`RecipeFile`] (raw deserialized) into a validated [`Recipe`].
    fn from_recipe_file(raw: RecipeFile) -> Result<Recipe, ParseError> {
        // Validate recipe has tasks
        if raw.tasks.is_empty() {
            return Err(ParseError::NoTasks);
        }

        // Collect all task names for dependency validation
        let task_names: std::collections::HashSet<&str> =
            raw.tasks.iter().map(|t| t.name.as_str()).collect();

        // Validate each task
        for task in &raw.tasks {
            if task.name.trim().is_empty() {
                return Err(ParseError::InvalidTaskName(task.name.clone()));
            }

            // Validate dependencies exist
            for dep in &task.depends_on {
                if !task_names.contains(dep.as_str()) {
                    return Err(ParseError::UnknownDependency {
                        dep: dep.clone(),
                        task: task.name.clone(),
                    });
                }
            }
        }

        // Resolve each task (interpolate variables)
        let mut tasks: Vec<RecipeTask> = Vec::with_capacity(raw.tasks.len());
        for task_def in &raw.tasks {
            // Merge recipe-level vars with task-level vars (task wins)
            let mut merged_vars = raw.vars.clone();
            for (k, v) in &task_def.vars {
                merged_vars.insert(k.clone(), v.clone());
            }

            let command = Self::interpolate(&task_def.command, &merged_vars, &task_def.name)?;

            tasks.push(RecipeTask {
                name: task_def.name.clone(),
                description: task_def.description.clone(),
                command,
                depends_on: task_def.depends_on.clone(),
                vars: merged_vars,
                timeout: task_def.timeout.map(|s| Duration::from_secs(s)),
            });
        }

        // Collect all unique dependency names (for the recipe-level list)
        let mut deps_set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for task in &tasks {
            for dep in &task.depends_on {
                deps_set.insert(dep.clone());
            }
        }
        let dependencies: Vec<String> = deps_set.into_iter().collect();

        Ok(Recipe {
            name: raw.name,
            description: raw.description,
            author: raw.author,
            tasks,
            dependencies,
            vars: raw.vars,
        })
    }

    /// Replace `{{ name }}` placeholders in `template` with values from `vars`.
    ///
    /// Returns an error if a referenced variable is not defined.
    fn interpolate(
        template: &str,
        vars: &HashMap<String, String>,
        task_name: &str,
    ) -> Result<String, ParseError> {
        let mut result = String::with_capacity(template.len());
        let mut rest = template;

        while let Some(start) = rest.find("{{") {
            // Push everything before `{{`
            result.push_str(&rest[..start]);

            let after_open = &rest[start + 2..];

            // Find the closing `}}`
            let end = after_open
                .find("}}")
                .ok_or_else(|| ParseError::Validation(
                    format!("Unclosed `{{{{` in task `{}`", task_name)
                ))?;

            let var_name = after_open[..end].trim();

            if var_name.is_empty() {
                return Err(ParseError::Validation(
                    format!("Empty variable reference in task `{}`", task_name)
                ));
            }

            // Look up the variable
            let value = vars.get(var_name).ok_or_else(|| {
                ParseError::UndefinedVariable {
                    name: var_name.to_string(),
                    task: task_name.to_string(),
                }
            })?;

            result.push_str(value);

            // Advance rest past the `}}`
            rest = &after_open[end + 2..];
        }

        // Push remaining text
        result.push_str(rest);
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Happy path ---------------------------------------------------------

    #[test]
    fn test_parse_minimal_toml() {
        let toml = r#"
name = "hello-recipe"

[[tasks]]
name = "greet"
command = "echo hello"
"#;
        let recipe = TaskenfileParser::parse_toml(toml).unwrap();
        assert_eq!(recipe.name, "hello-recipe");
        assert_eq!(recipe.tasks.len(), 1);
        assert_eq!(recipe.tasks[0].name, "greet");
        assert_eq!(recipe.tasks[0].command, "echo hello");
        assert!(recipe.tasks[0].depends_on.is_empty());
    }

    #[test]
    fn test_parse_toml_with_metadata() {
        let toml = r#"
name = "ci"
description = "CI pipeline"
author = "Phenotype"

[[tasks]]
name = "build"
command = "cargo build"
"#;
        let recipe = TaskenfileParser::parse_toml(toml).unwrap();
        assert_eq!(recipe.name, "ci");
        assert_eq!(recipe.description.as_deref(), Some("CI pipeline"));
        assert_eq!(recipe.author.as_deref(), Some("Phenotype"));
    }

    #[test]
    fn test_parse_toml_with_vars() {
        let toml = r#"
name = "vars-test"

[vars]
project = "tasken"
target = "x86_64"

[[tasks]]
name = "build"
command = "cargo build --target {{ target }} --package {{ project }}"
"#;
        let recipe = TaskenfileParser::parse_toml(toml).unwrap();
        assert_eq!(
            recipe.tasks[0].command,
            "cargo build --target x86_64 --package tasken"
        );
    }

    #[test]
    fn test_parse_toml_with_dependencies() {
        let toml = r#"
name = "dep-test"

[[tasks]]
name = "lint"
command = "cargo clippy"

[[tasks]]
name = "build"
command = "cargo build"
depends_on = ["lint"]

[[tasks]]
name = "test"
command = "cargo test"
depends_on = ["build"]
"#;
        let recipe = TaskenfileParser::parse_toml(toml).unwrap();
        assert_eq!(recipe.tasks.len(), 3);
        assert!(recipe.tasks[1].depends_on.contains(&"lint".to_string()));
        assert!(recipe.tasks[2].depends_on.contains(&"build".to_string()));
        // Recipe-level dependencies should contain all unique dep names
        assert!(recipe.dependencies.contains(&"lint".to_string()));
        assert!(recipe.dependencies.contains(&"build".to_string()));
    }

    #[test]
    fn test_parse_yaml() {
        let yaml = r#"
name: yaml-recipe
tasks:
  - name: greet
    command: echo hello
"#;
        let recipe = TaskenfileParser::parse_yaml(yaml).unwrap();
        assert_eq!(recipe.name, "yaml-recipe");
        assert_eq!(recipe.tasks[0].command, "echo hello");
    }

    #[test]
    fn test_parse_toml_with_task_vars() {
        let toml = r#"
name = "task-vars"

[vars]
env = "prod"

[[tasks]]
name = "deploy"
command = "deploy {{ region }}"
vars = { region = "us-east-1" }
"#;
        let recipe = TaskenfileParser::parse_toml(toml).unwrap();
        // Task-level var should be available
        assert_eq!(recipe.tasks[0].command, "deploy us-east-1");
        // Recipe-level vars should also be in merged vars
        assert_eq!(
            recipe.tasks[0].vars.get("env").map(|s| s.as_str()),
            Some("prod")
        );
    }

    #[test]
    fn test_parse_toml_with_timeout() {
        let toml = r#"
name = "timeout-test"

[[tasks]]
name = "long-task"
command = "sleep 10"
timeout = 30
"#;
        let recipe = TaskenfileParser::parse_toml(toml).unwrap();
        assert_eq!(recipe.tasks[0].timeout, Some(Duration::from_secs(30)));
    }

    #[test]
    fn test_parse_toml_with_description() {
        let toml = r#"
name = "desc-test"

[[tasks]]
name = "step-1"
description = "First step"
command = "echo step1"
"#;
        let recipe = TaskenfileParser::parse_toml(toml).unwrap();
        assert_eq!(
            recipe.tasks[0].description.as_deref(),
            Some("First step")
        );
    }

    // -- Variable interpolation edge cases ----------------------------------

    #[test]
    fn test_interpolation_no_vars_needed() {
        let cmd = TaskenfileParser::interpolate(
            "echo hello",
            &HashMap::new(),
            "test",
        ).unwrap();
        assert_eq!(cmd, "echo hello");
    }

    #[test]
    fn test_interpolation_multiple_vars() {
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), "1".to_string());
        vars.insert("b".to_string(), "2".to_string());
        vars.insert("c".to_string(), "3".to_string());

        let cmd = TaskenfileParser::interpolate(
            "{{ a }}-{{ b }}-{{ c }}",
            &vars,
            "test",
        ).unwrap();
        assert_eq!(cmd, "1-2-3");
    }

    #[test]
    fn test_interpolation_vars_with_extra_whitespace() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "world".to_string());

        let cmd = TaskenfileParser::interpolate(
            "echo hello {{  name  }}",
            &vars,
            "test",
        ).unwrap();
        assert_eq!(cmd, "echo hello world");
    }

    #[test]
    fn test_interpolation_no_placeholders_preserves_whitespace() {
        let cmd = TaskenfileParser::interpolate(
            "  echo  hello  ",
            &HashMap::new(),
            "test",
        ).unwrap();
        assert_eq!(cmd, "  echo  hello  ");
    }

    // -- Error cases --------------------------------------------------------

    #[test]
    fn test_reject_empty_tasks() {
        let toml = r#"
name = "empty"
tasks = []
"#;
        let err = TaskenfileParser::parse_toml(toml).unwrap_err();
        assert!(matches!(err, ParseError::NoTasks));
    }

    #[test]
    fn test_reject_undefined_variable() {
        let toml = r#"
name = "undef-var"

[[tasks]]
name = "run"
command = "echo {{ missing }}"
"#;
        let err = TaskenfileParser::parse_toml(toml).unwrap_err();
        assert!(matches!(err, ParseError::UndefinedVariable { .. }));
        if let ParseError::UndefinedVariable { ref name, ref task } = err {
            assert_eq!(name, "missing");
            assert_eq!(task, "run");
        }
    }

    #[test]
    fn test_reject_empty_task_name() {
        let toml = r#"
name = "bad"

[[tasks]]
name = ""
command = "echo"
"#;
        let err = TaskenfileParser::parse_toml(toml).unwrap_err();
        assert!(matches!(err, ParseError::InvalidTaskName(_)));
    }

    #[test]
    fn test_reject_unknown_dependency() {
        let toml = r#"
name = "bad-dep"

[[tasks]]
name = "a"
command = "echo"
depends_on = ["nonexistent"]
"#;
        let err = TaskenfileParser::parse_toml(toml).unwrap_err();
        assert!(matches!(err, ParseError::UnknownDependency { .. }));
        if let ParseError::UnknownDependency { ref dep, ref task } = err {
            assert_eq!(dep, "nonexistent");
            assert_eq!(task, "a");
        }
    }

    #[test]
    fn test_reject_malformed_toml() {
        let toml = r#"this is not valid toml [[["#;
        let err = TaskenfileParser::parse_toml(toml).unwrap_err();
        assert!(matches!(err, ParseError::Toml(_)));
    }

    #[test]
    fn test_reject_malformed_yaml() {
        let yaml = r#": : not valid yaml"#;
        let err = TaskenfileParser::parse_yaml(yaml).unwrap_err();
        assert!(matches!(err, ParseError::Yaml(_)));
    }

    #[test]
    fn test_reject_unsupported_extension() {
        let err = TaskenfileParser::parse_file(
            std::path::Path::new("recipe.json"),
        ).unwrap_err();
        assert!(matches!(err, ParseError::UnsupportedExtension(_)));
    }

    #[test]
    fn test_reject_unclosed_brace() {
        let toml = r#"
name = "bad-brace"

[[tasks]]
name = "x"
command = "echo {{ unclosed"
"#;
        let err = TaskenfileParser::parse_toml(toml).unwrap_err();
        assert!(matches!(err, ParseError::Validation(_)));
    }

    // -- File-based parsing -------------------------------------------------

    #[test]
    fn test_parse_toml_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Taskenfile.toml");
        std::fs::write(
            &path,
            r#"
name = "file-test"

[[tasks]]
name = "build"
command = "make"
"#,
        ).unwrap();

        let recipe = TaskenfileParser::parse_file(&path).unwrap();
        assert_eq!(recipe.name, "file-test");
        assert_eq!(recipe.tasks[0].command, "make");
    }

    #[test]
    fn test_parse_yaml_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Taskenfile.yaml");
        std::fs::write(
            &path,
            r#"
name: yaml-file-test
tasks:
  - name: build
    command: make
"#,
        ).unwrap();

        let recipe = TaskenfileParser::parse_file(&path).unwrap();
        assert_eq!(recipe.name, "yaml-file-test");
    }

    #[test]
    fn test_parse_file_not_found() {
        let err = TaskenfileParser::parse_file(
            std::path::Path::new("/nonexistent/Taskenfile.toml"),
        ).unwrap_err();
        assert!(matches!(err, ParseError::Io(_)));
    }

    // -- Recipe dependencies list -------------------------------------------

    #[test]
    fn test_recipe_dependencies_unique() {
        let toml = r#"
name = "deps"

[[tasks]]
name = "init"
command = "init"

[[tasks]]
name = "build"
command = "build"
depends_on = ["init"]

[[tasks]]
name = "test"
command = "test"
depends_on = ["build"]

[[tasks]]
name = "deploy"
command = "deploy"
depends_on = ["build", "test"]
"#;
        let recipe = TaskenfileParser::parse_toml(toml).unwrap();
        // init, build, test should be in dependencies (unique)
        assert_eq!(recipe.dependencies.len(), 3);
        assert!(recipe.dependencies.contains(&"init".to_string()));
        assert!(recipe.dependencies.contains(&"build".to_string()));
        assert!(recipe.dependencies.contains(&"test".to_string()));
    }

    #[test]
    fn test_recipe_no_dependencies() {
        let toml = r#"
name = "no-deps"

[[tasks]]
name = "single"
command = "echo"
"#;
        let recipe = TaskenfileParser::parse_toml(toml).unwrap();
        assert!(recipe.dependencies.is_empty());
    }
}
