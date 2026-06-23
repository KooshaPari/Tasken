// SPDX-License-Identifier: MIT OR Apache-2.0
//! `@import` resolver for Tasken recipe files.
//!
//! Tasken recipe files are JSON documents containing `tasks` and `workflows`
//! arrays. They may begin with `@import` directives that reference other recipe
//! files by path (relative to the importing file):
//!
//! ```text
//! @import "./shared/tasks.json"
//! @import "../common/ci-workflows.json"
//!
//! {
//!   "tasks": [ ... ],
//!   "workflows": [ ... ]
//! }
//! ```
//!
//! The `ImportResolver` resolves these directives recursively, merges all
//! imported recipes' tasks and workflows, and detects circular imports.

use std::path::{Path, PathBuf};

use crate::domain::tasks::Task;
use crate::domain::workflows::Workflow;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A single @import directive parsed from a recipe file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportDirective {
    /// The unprocessed path string from the @import line (e.g. `"./foo.json"`).
    pub raw_path: String,
    /// The resolved, absolute path (set during resolution).
    pub resolved_path: PathBuf,
}

/// The result of resolving a recipe file and all its imports.
#[derive(Debug, Clone, Default)]
pub struct ImportResult {
    /// Merged tasks from the recipe and every transitively imported recipe.
    pub tasks: Vec<Task>,
    /// Merged workflows from the recipe and every transitively imported recipe.
    pub workflows: Vec<Workflow>,
}

/// Errors that can occur during import resolution.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    /// A circular import was detected (the path has already been visited).
    #[error("Circular import detected: {path}")]
    CircularImport { path: PathBuf },

    /// The referenced file does not exist.
    #[error("Imported file not found: {path}")]
    FileNotFound { path: PathBuf },

    /// An I/O error occurred while reading a file.
    #[error("I/O error reading {path}: {source}")]
    IoError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// The JSON content of a recipe file could not be parsed.
    #[error("Failed to parse recipe file {path}: {source}")]
    ParseError {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    /// An @import directive could not be interpreted (e.g. missing quotes).
    #[error("Invalid @import directive at {path}:{line}: {message}")]
    InvalidImport { path: PathBuf, line: usize, message: String },

    /// The recipe file contained neither tasks nor workflows after imports.
    #[error("No tasks or workflows found in {path}")]
    EmptyRecipe { path: PathBuf },
}

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

/// The JSON structure of a recipe file.
#[derive(Debug, Clone, serde::Deserialize)]
struct RecipeFile {
    #[serde(default)]
    tasks: Vec<Task>,
    #[serde(default)]
    workflows: Vec<Workflow>,
}

// ---------------------------------------------------------------------------
// ImportResolver
// ---------------------------------------------------------------------------

/// Resolves `@import` directives in Tasken recipe files.
///
/// # Example
///
/// ```ignore
/// let resolver = ImportResolver::new();
/// let result = resolver.resolve_file("./recipes/ci.json")?;
/// for task in &result.tasks {
///     println!("Imported task: {}", task.name);
/// }
/// ```
#[derive(Debug, Clone, Default)]
pub struct ImportResolver;

impl ImportResolver {
    /// Create a new `ImportResolver`.
    pub fn new() -> Self {
        Self
    }

    /// Resolve a recipe file at `path`, processing all `@import` directives
    /// recursively and returning the merged tasks and workflows.
    ///
    /// The path may be relative or absolute. If relative, it is resolved
    /// against the current working directory.
    pub fn resolve_file(&self, path: impl AsRef<Path>) -> Result<ImportResult, ImportError> {
        let path = path.as_ref();
        let canonical = std::fs::canonicalize(path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => ImportError::FileNotFound { path: path.to_path_buf() },
            _ => ImportError::IoError { path: path.to_path_buf(), source: e },
        })?;

        let mut visited: Vec<PathBuf> = Vec::new();
        self.resolve_inner(&canonical, &mut visited)
    }

    /// Resolve an already-canonical path, tracking visited files to detect
    /// cycles.
    fn resolve_inner(
        &self,
        path: &Path,
        visited: &mut Vec<PathBuf>,
    ) -> Result<ImportResult, ImportError> {
        // --- Circular import check ---
        if visited.iter().any(|p| p == path) {
            return Err(ImportError::CircularImport { path: path.to_path_buf() });
        }
        visited.push(path.to_path_buf());

        // --- Read the raw file content ---
        let content = std::fs::read_to_string(path)
            .map_err(|e| ImportError::IoError { path: path.to_path_buf(), source: e })?;

        // --- Parse @import directives ---
        let parent_dir = path.parent().unwrap_or(Path::new("."));
        let directives = Self::parse_import_directives(&content, path, parent_dir)?;

        // --- Remove @import lines from the content before JSON parsing ---
        let json_content = Self::strip_import_lines(&content);

        // --- Parse the JSON body ---
        let recipe: RecipeFile = serde_json::from_str(&json_content)
            .map_err(|e| ImportError::ParseError { path: path.to_path_buf(), source: e })?;

        // --- Recursively resolve each import and merge ---
        let mut result = ImportResult { tasks: recipe.tasks, workflows: recipe.workflows };

        for directive in &directives {
            // Canonicalise the imported path now that we are about to read it.
            // This converts relative paths to absolute and validates existence.
            let canonical = directive.resolved_path.canonicalize().map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => {
                    ImportError::FileNotFound { path: directive.resolved_path.clone() }
                }
                _ => ImportError::IoError { path: directive.resolved_path.clone(), source: e },
            })?;
            let imported = self.resolve_inner(&canonical, visited)?;
            result.tasks.extend(imported.tasks);
            result.workflows.extend(imported.workflows);
        }

        // Pop this file from the visited set (backtracking) so sibling imports
        // can share transitive dependencies without false cycle detection.
        visited.pop();

        Ok(result)
    }

    /// Extract all `@import` directives from the raw content of a recipe file.
    ///
    /// Lines matching `@import "<path>"` are parsed. The path is resolved
    /// relative to `parent_dir`. **No file I/O is performed** — path
    /// existence and canonicalisation are deferred to the caller
    /// ([`resolve_inner`]).
    fn parse_import_directives(
        content: &str,
        file_path: &Path,
        parent_dir: &Path,
    ) -> Result<Vec<ImportDirective>, ImportError> {
        let mut directives = Vec::new();

        for (line_idx, line) in content.lines().enumerate() {
            let line = line.trim();
            if !line.starts_with("@import") {
                continue;
            }

            let lineno = line_idx + 1; // 1-based

            // Expect: @import "./path"  or @import "./path"# comment
            let after_directive = line.strip_prefix("@import").unwrap().trim();

            // The path must be a quoted string
            if !after_directive.starts_with('"') {
                return Err(ImportError::InvalidImport {
                    path: file_path.to_path_buf(),
                    line: lineno,
                    message: "expected quoted path after @import".to_string(),
                });
            }

            // Find the closing quote
            let path_str = if let Some(end) = after_directive[1..].find('"') {
                &after_directive[1..=end]
            } else {
                return Err(ImportError::InvalidImport {
                    path: file_path.to_path_buf(),
                    line: lineno,
                    message: "unclosed quote in @import path".to_string(),
                });
            };

            // Resolve relative to the importing file's directory.
            // We do NOT canonicalise here — that would require the file to
            // exist on disk. The caller handles I/O and canonicalisation.
            let resolved = parent_dir.join(path_str);

            directives
                .push(ImportDirective { raw_path: path_str.to_string(), resolved_path: resolved });
        }

        Ok(directives)
    }

    /// Remove all lines that start with `@import` from the content, returning
    /// only the JSON body.
    fn strip_import_lines(content: &str) -> String {
        content
            .lines()
            .filter(|line| !line.trim().starts_with("@import"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use super::*;

    // -- Helper: create a temporary recipe file and return its path -----------

    fn create_recipe(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        let mut file = fs::File::create(&path).expect("failed to create test recipe");
        write!(file, "{body}").expect("failed to write test recipe");
        path
    }

    // -- Helper: build a minimal task JSON snippet ----------------------------

    fn task_json(name: &str, command: &str) -> String {
        format!(
            r#"{{
                "id": "{}",
                "name": "{}",
                "state": "pending",
                "data": {{"command": "{}"}},
                "tags": [],
                "depends_on": [],
                "priority": "normal",
                "retry_count": 0,
                "created_at": "2025-01-01T00:00:00Z",
                "updated_at": "2025-01-01T00:00:00Z"
            }}"#,
            uuid::Uuid::new_v4(),
            name,
            command
        )
    }

    // -- Helper: build a minimal workflow JSON snippet ------------------------

    fn workflow_json(name: &str) -> String {
        format!(
            r#"{{
                "id": "{}",
                "name": "{}",
                "state": "draft",
                "steps": [],
                "created_at": "2025-01-01T00:00:00Z",
                "updated_at": "2025-01-01T00:00:00Z"
            }}"#,
            uuid::Uuid::new_v4(),
            name
        )
    }

    // ---------------------------------------------------------------------------
    // Tests
    // ---------------------------------------------------------------------------

    #[test]
    fn test_parse_import_directives_single() {
        let content = r#"@import "./base.json"
        {
            "tasks": [],
            "workflows": []
        }"#;

        let dir = Path::new("/tmp");
        let directives =
            ImportResolver::parse_import_directives(content, &dir.join("recipe.json"), dir)
                .expect("should parse");
        assert_eq!(directives.len(), 1);
        assert_eq!(directives[0].raw_path, "./base.json");
    }

    #[test]
    fn test_parse_import_directives_multiple() {
        let content = r#"@import "./base.json"
        @import "../common/ci.json"

        {
            "tasks": [],
            "workflows": []
        }"#;

        let dir = Path::new("/tmp");
        let directives =
            ImportResolver::parse_import_directives(content, &dir.join("recipe.json"), dir)
                .expect("should parse");
        assert_eq!(directives.len(), 2);
        assert_eq!(directives[0].raw_path, "./base.json");
        assert_eq!(directives[1].raw_path, "../common/ci.json");
    }

    #[test]
    fn test_parse_import_directives_no_imports() {
        let content = r#"{
            "tasks": [],
            "workflows": []
        }"#;

        let dir = Path::new("/tmp");
        let directives =
            ImportResolver::parse_import_directives(content, &dir.join("recipe.json"), dir)
                .expect("should parse");
        assert!(directives.is_empty());
    }

    #[test]
    fn test_strip_import_lines() {
        let content = r#"@import "./base.json"
        @import "./more.json"

        { "tasks": [], "workflows": [] }"#;

        let stripped = ImportResolver::strip_import_lines(content);
        assert!(!stripped.contains("@import"));
        assert!(stripped.contains("tasks"));
    }

    #[test]
    fn test_strip_import_lines_preserves_rest() {
        let content = r#"@import "./base.json"

        {
            "tasks": [
                { "name": "build" }
            ]
        }"#;

        let stripped = ImportResolver::strip_import_lines(content);
        assert!(!stripped.contains("@import"));
        assert!(stripped.contains("build"));
    }

    #[test]
    fn test_resolve_simple_import() {
        let dir = tempfile::tempdir().expect("tempdir");

        // -- base recipe (no imports) --
        let base_body = format!(
            r#"{{
            "tasks": [{}],
            "workflows": []
        }}"#,
            task_json("build", "cargo build")
        );
        create_recipe(dir.path(), "base.json", &base_body);

        // -- main recipe imports base --
        let main_body = format!(
            r#"@import "./base.json"

        {{
            "tasks": [{}],
            "workflows": []
        }}"#,
            task_json("deploy", "cargo deploy")
        );
        let main_path = create_recipe(dir.path(), "main.json", &main_body);

        let resolver = ImportResolver::new();
        let result = resolver.resolve_file(&main_path).expect("should resolve");

        // Should contain tasks from both files
        assert_eq!(result.tasks.len(), 2);
        let names: Vec<&str> = result.tasks.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"build"));
        assert!(names.contains(&"deploy"));
    }

    #[test]
    fn test_resolve_nested_imports() {
        let dir = tempfile::tempdir().expect("tempdir");

        // -- level 3 (deepest) --
        let l3_body = format!(
            r#"{{
            "tasks": [{}],
            "workflows": []
        }}"#,
            task_json("l3-task", "echo l3")
        );
        create_recipe(dir.path(), "level3.json", &l3_body);

        // -- level 2 imports level 3 --
        let l2_body = format!(
            r#"@import "./level3.json"

        {{
            "tasks": [{}],
            "workflows": []
        }}"#,
            task_json("l2-task", "echo l2")
        );
        create_recipe(dir.path(), "level2.json", &l2_body);

        // -- level 1 (main) imports level 2 --
        let l1_body = format!(
            r#"@import "./level2.json"

        {{
            "tasks": [{}],
            "workflows": []
        }}"#,
            task_json("l1-task", "echo l1")
        );
        let main_path = create_recipe(dir.path(), "main.json", &l1_body);

        let resolver = ImportResolver::new();
        let result = resolver.resolve_file(&main_path).expect("should resolve");

        assert_eq!(result.tasks.len(), 3);
        let names: Vec<&str> = result.tasks.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"l1-task"));
        assert!(names.contains(&"l2-task"));
        assert!(names.contains(&"l3-task"));
    }

    #[test]
    fn test_detect_circular_import() {
        let dir = tempfile::tempdir().expect("tempdir");

        // a.json imports b.json
        let a_body = r#"@import "./b.json"

        { "tasks": [], "workflows": [] }"#;
        let a_path = create_recipe(dir.path(), "a.json", a_body);

        // b.json imports a.json — cycle!
        let b_body = r#"@import "./a.json"

        { "tasks": [], "workflows": [] }"#;
        create_recipe(dir.path(), "b.json", b_body);

        let resolver = ImportResolver::new();
        let err = resolver.resolve_file(&a_path).expect_err("should detect cycle");

        match err {
            ImportError::CircularImport { path } => {
                let filename = path.file_name().unwrap().to_string_lossy();
                // Could be a.json or b.json depending on canonicalization ordering
                assert!(
                    filename == "a.json" || filename == "b.json",
                    "unexpected filename: {filename}"
                );
            }
            other => panic!("expected CircularImport, got: {other}"),
        }
    }

    #[test]
    fn test_handle_missing_file() {
        let dir = tempfile::tempdir().expect("tempdir");

        // main.json imports a file that doesn't exist
        let main_body = r#"@import "./nonexistent.json"

        { "tasks": [], "workflows": [] }"#;
        let main_path = create_recipe(dir.path(), "main.json", main_body);

        let resolver = ImportResolver::new();
        let err = resolver.resolve_file(&main_path).expect_err("should error");

        match err {
            ImportError::FileNotFound { path } => {
                assert!(path.to_string_lossy().contains("nonexistent"));
            }
            other => panic!("expected FileNotFound, got: {other}"),
        }
    }

    #[test]
    fn test_import_merges_workflows() {
        let dir = tempfile::tempdir().expect("tempdir");

        let base_body = format!(
            r#"{{
            "tasks": [],
            "workflows": [{}]
        }}"#,
            workflow_json("ci")
        );
        create_recipe(dir.path(), "base.json", &base_body);

        let main_body = format!(
            r#"@import "./base.json"

        {{
            "tasks": [],
            "workflows": [{}]
        }}"#,
            workflow_json("deploy")
        );
        let main_path = create_recipe(dir.path(), "main.json", &main_body);

        let resolver = ImportResolver::new();
        let result = resolver.resolve_file(&main_path).expect("should resolve");

        assert_eq!(result.workflows.len(), 2);
        let names: Vec<&str> = result.workflows.iter().map(|w| w.name.as_str()).collect();
        assert!(names.contains(&"ci"));
        assert!(names.contains(&"deploy"));
    }

    #[test]
    fn test_invalid_import_directive_missing_quotes() {
        let content = r#"@import ./base.json
        { "tasks": [], "workflows": [] }"#;

        let dir = tempfile::tempdir().expect("tempdir");
        let recipe_path = create_recipe(dir.path(), "bad_import.json", content);

        let err = ImportResolver::new().resolve_file(&recipe_path).expect_err("should error");

        match err {
            ImportError::InvalidImport { line: 1, .. } => {} // expected
            other => panic!("expected InvalidImport at line 1, got: {other}"),
        }
    }

    #[test]
    fn test_import_with_subdirectory() {
        let dir = tempfile::tempdir().expect("tempdir");

        // Create subdirectory for shared recipes
        let shared_dir = dir.path().join("shared");
        fs::create_dir_all(&shared_dir).expect("create shared dir");

        let shared_body = format!(
            r#"{{
            "tasks": [{}],
            "workflows": []
        }}"#,
            task_json("shared-task", "echo shared")
        );
        create_recipe(&shared_dir, "shared.json", &shared_body);

        // Main recipe imports from subdirectory
        let main_body = format!(
            r#"@import "./shared/shared.json"

        {{
            "tasks": [{}],
            "workflows": []
        }}"#,
            task_json("main-task", "echo main")
        );
        let main_path = create_recipe(dir.path(), "main.json", &main_body);

        let resolver = ImportResolver::new();
        let result = resolver.resolve_file(&main_path).expect("should resolve");

        assert_eq!(result.tasks.len(), 2);
        let names: Vec<&str> = result.tasks.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"shared-task"));
        assert!(names.contains(&"main-task"));
    }

    #[test]
    fn test_resolve_file_not_found() {
        let resolver = ImportResolver::new();
        let err = resolver
            .resolve_file("/tmp/tasken-nonexistent-file-12345.json")
            .expect_err("should error");

        match err {
            ImportError::FileNotFound { .. } => {} // expected
            other => panic!("expected FileNotFound, got: {other}"),
        }
    }

    #[test]
    fn test_import_self_cycle_detected() {
        let dir = tempfile::tempdir().expect("tempdir");

        // A file that tries to import itself
        let body = r#"@import "./self.json"

        { "tasks": [], "workflows": [] }"#;
        let path = create_recipe(dir.path(), "self.json", body);

        let resolver = ImportResolver::new();
        let err = resolver.resolve_file(&path).expect_err("should detect self-cycle");

        match err {
            ImportError::CircularImport { .. } => {} // expected
            other => panic!("expected CircularImport, got: {other}"),
        }
    }
}
