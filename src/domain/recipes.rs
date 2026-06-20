// SPDX-License-Identifier: MIT OR Apache-2.0
//! Recipe data model: variables, settings, and interpolation.
//!
//! This module provides the building blocks for defining, configuring,
//! and rendering recipes — reusable task templates with parameterized
//! commands, environment settings, and variable substitution.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// VarType
// ---------------------------------------------------------------------------

/// Type hint for a recipe variable.
///
/// Used for documentation, validation, and UI rendering. Does **not**
/// enforce type coercion at interpolation time — all values are stored
/// as strings internally.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VarType {
    String,
    Number,
    Bool,
    Path,
    Choice(Vec<String>),
}

impl Default for VarType {
    fn default() -> Self {
        Self::String
    }
}

impl VarType {
    /// Human-readable name of the type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Number => "number",
            Self::Bool => "boolean",
            Self::Path => "path",
            Self::Choice(_) => "choice",
        }
    }
}

// ---------------------------------------------------------------------------
// VarDefinition
// ---------------------------------------------------------------------------

/// Metadata describing a single variable that a recipe accepts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VarDefinition {
    /// Variable name (used in `{{ name }}` interpolation).
    pub name: String,
    /// Expected type hint.
    #[serde(rename = "type")]
    pub var_type: VarType,
    /// Default value when none is provided.
    pub default: Option<String>,
    /// Human-readable description of what this variable controls.
    pub description: String,
    /// Whether the variable must be provided (no default).
    #[serde(default)]
    pub required: bool,
}

// ---------------------------------------------------------------------------
// Vars
// ---------------------------------------------------------------------------

/// Runtime variable store for a single recipe execution.
///
/// Combines:
/// - **definitions** — metadata describing accepted variables
/// - **values** — concrete key/value pairs supplied by the caller or defaults
///
/// The store also automatically populates pre-defined variables
/// (`os`, `arch`, `timestamp`, `pid`, `user`) on construction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vars {
    /// Variable metadata (schema).
    pub definitions: Vec<VarDefinition>,
    /// Concrete variable values.
    pub values: HashMap<String, String>,
}

impl Vars {
    /// Create an empty variable store with no definitions and only
    /// pre-defined variables populated.
    pub fn empty() -> Self {
        Self {
            definitions: Vec::new(),
            values: predefined_vars(),
        }
    }

    /// Create a variable store with the given definitions, applying
    /// defaults for any definition that has one.  Pre-defined variables
    /// are always included.
    pub fn new(definitions: Vec<VarDefinition>, overrides: HashMap<String, String>) -> Self {
        let mut values = predefined_vars();

        // Apply defaults from definitions.
        for def in &definitions {
            if let Some(ref default) = def.default {
                values.entry(def.name.clone()).or_insert_with(|| default.clone());
            }
        }

        // Apply caller-supplied overrides.
        for (k, v) in overrides {
            values.insert(k, v);
        }

        Self { definitions, values }
    }

    /// Get a variable value by name.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(|s| s.as_str())
    }

    /// Set a variable value at runtime.
    pub fn set(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.values.insert(name.into(), value.into());
    }

    /// Check whether a variable is defined (in definitions or pre-defined).
    pub fn contains_key(&self, name: &str) -> bool {
        self.values.contains_key(name)
    }

    /// Number of stored values.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether the store is empty (no values at all, *not* just no definitions).
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Pre-defined variables
// ---------------------------------------------------------------------------

/// Return the map of pre-defined variables that are always available for
/// interpolation in any recipe.
pub fn predefined_vars() -> HashMap<String, String> {
    let mut map = HashMap::new();

    // Operating system.
    map.insert("os".to_string(), std::env::consts::OS.to_string());

    // CPU architecture.
    map.insert("arch".to_string(), std::env::consts::ARCH.to_string());

    // Current UTC timestamp in ISO‑8601 format.
    map.insert(
        "timestamp".to_string(),
        Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
    );

    // Process id.
    map.insert("pid".to_string(), std::process::id().to_string());

    // Current user, if available.
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".to_string());
    map.insert("user".to_string(), user);

    map
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

/// Global settings that control *how* a recipe is executed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Shell to use for executing commands (e.g. `/bin/bash`, `powershell.exe`).
    /// `None` means the system default (`sh -c` on Unix, `cmd /C` on Windows).
    pub shell: Option<String>,
    /// Working directory for the recipe.
    /// `None` means the current process working directory.
    pub work_dir: Option<String>,
    /// Environment variables to set for the recipe.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Maximum number of concurrent task executions.
    /// `None` means no limit (unbounded parallelism).
    pub max_concurrency: Option<usize>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            shell: None,
            work_dir: None,
            env: HashMap::new(),
            max_concurrency: None,
        }
    }
}

impl Settings {
    /// Create a new `Settings` with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the shell.
    pub fn with_shell(mut self, shell: impl Into<String>) -> Self {
        self.shell = Some(shell.into());
        self
    }

    /// Set the working directory.
    pub fn with_work_dir(mut self, dir: impl Into<String>) -> Self {
        self.work_dir = Some(dir.into());
        self
    }

    /// Add an environment variable.
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Set the maximum concurrency.
    pub fn with_max_concurrency(mut self, n: usize) -> Self {
        self.max_concurrency = Some(n);
        self
    }
}

// ---------------------------------------------------------------------------
// Interpolation
// ---------------------------------------------------------------------------

/// Errors that can occur during variable interpolation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InterpolationError {
    #[error("Undefined variable `{name}` referenced in template")]
    UndefinedVariable { name: String },
}

/// Interpolate `{{ variable }}` placeholders in a template string.
///
/// # Behaviour
///
/// - Looks up each `{{ name }}` in `vars` and replaces it with its value.
/// - If `fail_on_undefined` is `true`, returns
///   `InterpolationError::UndefinedVariable` for missing variables.
/// - If `fail_on_undefined` is `false` (default), missing variables are
///   silently left as-is (e.g. `{{ missing }}` stays unchanged).
/// - Pre-defined variables (`os`, `arch`, `timestamp`, `pid`, `user`) are
///   resolved just like user-supplied ones.
///
/// # Examples
///
/// ```
/// use taskkit::domain::recipes::{Vars, interpolate};
/// use std::collections::HashMap;
///
/// let vars = Vars::new(vec![], HashMap::from([
///     ("name".into(), "world".into()),
/// ]));
///
/// let result = interpolate("hello {{ name }}!", &vars, false);
/// assert_eq!(result, "hello world!");
/// ```
pub fn interpolate(template: &str, vars: &Vars, fail_on_undefined: bool) -> String {
    let mut result = String::with_capacity(template.len());
    let mut pos = 0;

    while pos < template.len() {
        // Find opening `{{`.
        if let Some(start_offset) = template[pos..].find("{{") {
            let abs_start = pos + start_offset;

            // Push everything before `{{`.
            result.push_str(&template[pos..abs_start]);

            let after_open = &template[abs_start + 2..];

            // Find the closing `}}`.
            if let Some(end_offset) = after_open.find("}}") {
                let var_name = after_open[..end_offset].trim();
                let abs_end = abs_start + 2 + end_offset + 2; // past the `}}`

                if var_name.is_empty() {
                    // Empty placeholder `{{ }}` — leave as-is.
                    result.push_str("{{ }}");
                } else if let Some(value) = vars.get(var_name) {
                    result.push_str(value);
                } else if fail_on_undefined {
                    result.push_str(&format!("{{{{ undefined: {} }}}}", var_name));
                } else {
                    // Leave the placeholder unchanged.
                    result.push_str(&template[abs_start..abs_end]);
                }

                pos = abs_end;
            } else {
                // No closing `}}` — push everything remaining.
                result.push_str(&template[abs_start..]);
                pos = template.len();
            }
        } else {
            // No more `{{` — push the tail.
            result.push_str(&template[pos..]);
            break;
        }
    }

    result
}

/// Interpolate a template and return an error on undefined variables.
///
/// This is a convenience wrapper around [`interpolate`] that returns
/// `Result` instead of silently keeping missing placeholders.
pub fn interpolate_strict(template: &str, vars: &Vars) -> Result<String, InterpolationError> {
    let mut result = String::with_capacity(template.len());
    let mut pos = 0;

    while pos < template.len() {
        if let Some(start_offset) = template[pos..].find("{{") {
            let abs_start = pos + start_offset;
            result.push_str(&template[pos..abs_start]);

            let after_open = &template[abs_start + 2..];

            if let Some(end_offset) = after_open.find("}}") {
                let var_name = after_open[..end_offset].trim();
                let abs_end = abs_start + 2 + end_offset + 2;

                if var_name.is_empty() {
                    result.push_str("{{ }}");
                } else if let Some(value) = vars.get(var_name) {
                    result.push_str(value);
                } else {
                    return Err(InterpolationError::UndefinedVariable {
                        name: var_name.to_string(),
                    });
                }

                pos = abs_end;
            } else {
                result.push_str(&template[abs_start..]);
                pos = template.len();
            }
        } else {
            result.push_str(&template[pos..]);
            break;
        }
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // -- VarType tests -------------------------------------------------------

    #[test]
    fn test_var_type_default_is_string() {
        assert_eq!(VarType::default(), VarType::String);
    }

    #[test]
    fn test_var_type_as_str() {
        assert_eq!(VarType::String.as_str(), "string");
        assert_eq!(VarType::Number.as_str(), "number");
        assert_eq!(VarType::Bool.as_str(), "boolean");
        assert_eq!(VarType::Path.as_str(), "path");
        assert_eq!(VarType::Choice(vec!["a".into()]).as_str(), "choice");
    }

    // -- Vars tests ----------------------------------------------------------

    #[test]
    fn test_vars_empty() {
        let vars = Vars::empty();
        // Pre-defined vars are always present.
        assert!(vars.contains_key("os"));
        assert!(vars.contains_key("arch"));
        assert!(vars.contains_key("timestamp"));
        assert!(vars.contains_key("pid"));
        assert!(vars.contains_key("user"));
        assert!(vars.definitions.is_empty());
    }

    #[test]
    fn test_vars_new_with_defaults() {
        let defs = vec![
            VarDefinition {
                name: "greeting".into(),
                var_type: VarType::String,
                default: Some("hello".into()),
                description: "The greeting message".into(),
                required: false,
            },
            VarDefinition {
                name: "count".into(),
                var_type: VarType::Number,
                default: Some("42".into()),
                description: "Loop count".into(),
                required: false,
            },
        ];
        let vars = Vars::new(defs, HashMap::new());
        assert_eq!(vars.get("greeting"), Some("hello"));
        assert_eq!(vars.get("count"), Some("42"));
    }

    #[test]
    fn test_vars_overrides() {
        let defs = vec![VarDefinition {
            name: "greeting".into(),
            var_type: VarType::String,
            default: Some("hello".into()),
            description: "".into(),
            required: false,
        }];
        let vars = Vars::new(defs, HashMap::from([("greeting".into(), "hi".into())]));
        assert_eq!(vars.get("greeting"), Some("hi"));
    }

    #[test]
    fn test_vars_set_and_get() {
        let mut vars = Vars::empty();
        vars.set("foo", "bar");
        assert_eq!(vars.get("foo"), Some("bar"));
        assert!(!vars.is_empty());
    }

    #[test]
    fn test_vars_len() {
        let mut vars = Vars::empty();
        let pre_count = vars.len();
        vars.set("extra", "value");
        assert_eq!(vars.len(), pre_count + 1);
    }

    // -- Pre-defined vars tests ----------------------------------------------

    #[test]
    fn test_predefined_vars_os() {
        let pv = predefined_vars();
        assert!(pv.contains_key("os"));
        // Should match the compiled target OS.
        assert_eq!(pv.get("os").unwrap(), &std::env::consts::OS);
    }

    #[test]
    fn test_predefined_vars_arch() {
        let pv = predefined_vars();
        assert!(pv.contains_key("arch"));
        assert_eq!(pv.get("arch").unwrap(), &std::env::consts::ARCH);
    }

    #[test]
    fn test_predefined_vars_timestamp_format() {
        let pv = predefined_vars();
        let ts = pv.get("timestamp").unwrap();
        // ISO-8601 with milliseconds: e.g. "2026-06-20T12:34:56.789Z"
        assert!(
            ts.len() >= 24,
            "timestamp '{}' should be ISO-8601 format",
            ts
        );
        assert!(
            ts.ends_with('Z'),
            "timestamp '{}' should end with Z",
            ts
        );
    }

    #[test]
    fn test_predefined_vars_pid_is_numeric() {
        let pv = predefined_vars();
        let pid = pv.get("pid").unwrap();
        assert!(
            pid.parse::<u32>().is_ok(),
            "pid '{}' should be a numeric string",
            pid
        );
    }

    // -- Settings tests ------------------------------------------------------

    #[test]
    fn test_settings_default() {
        let s = Settings::default();
        assert!(s.shell.is_none());
        assert!(s.work_dir.is_none());
        assert!(s.env.is_empty());
        assert!(s.max_concurrency.is_none());
    }

    #[test]
    fn test_settings_builder_methods() {
        let s = Settings::new()
            .with_shell("/bin/bash")
            .with_work_dir("/tmp")
            .with_env("FOO", "bar")
            .with_max_concurrency(4);

        assert_eq!(s.shell, Some("/bin/bash".into()));
        assert_eq!(s.work_dir, Some("/tmp".into()));
        assert_eq!(s.env.get("FOO"), Some(&"bar".into()));
        assert_eq!(s.max_concurrency, Some(4));
    }

    // -- Interpolation tests -------------------------------------------------

    #[test]
    fn test_interpolate_basic() {
        let vars = Vars::new(
            vec![],
            HashMap::from([("name".into(), "world".into())]),
        );
        let result = interpolate("hello {{ name }}!", &vars, false);
        assert_eq!(result, "hello world!");
    }

    #[test]
    fn test_interpolate_multiple_vars() {
        let vars = Vars::new(
            vec![],
            HashMap::from([
                ("first".into(), "John".into()),
                ("last".into(), "Doe".into()),
            ]),
        );
        let result = interpolate("{{ first }} {{ last }}", &vars, false);
        assert_eq!(result, "John Doe");
    }

    #[test]
    fn test_interpolate_predefined_os() {
        let vars = Vars::empty();
        let result = interpolate("os={{ os }}", &vars, false);
        assert_eq!(result, format!("os={}", std::env::consts::OS));
    }

    #[test]
    fn test_interpolate_predefined_arch() {
        let vars = Vars::empty();
        let result = interpolate("arch={{ arch }}", &vars, false);
        assert_eq!(result, format!("arch={}", std::env::consts::ARCH));
    }

    #[test]
    fn test_interpolate_predefined_timestamp() {
        let vars = Vars::empty();
        let result = interpolate("ts={{ timestamp }}", &vars, false);
        assert!(result.starts_with("ts="));
        assert!(result.len() > 10);
    }

    #[test]
    fn test_interpolate_missing_var_silent() {
        let vars = Vars::empty();
        let result = interpolate("hello {{ missing }}!", &vars, false);
        // When fail_on_undefined is false, missing vars stay as-is.
        assert_eq!(result, "hello {{ missing }}!");
    }

    #[test]
    fn test_interpolate_missing_var_strict() {
        let vars = Vars::empty();
        let result = interpolate("hello {{ missing }}!", &vars, true);
        // When fail_on_undefined is true, it emits an error marker.
        assert_eq!(result, "hello {{ undefined: missing }}!");
    }

    #[test]
    fn test_interpolate_empty_placeholder() {
        let vars = Vars::empty();
        let result = interpolate("hello {{ }}!", &vars, false);
        assert_eq!(result, "hello {{ }}!");
    }

    #[test]
    fn test_interpolate_no_placeholders() {
        let vars = Vars::empty();
        let result = interpolate("hello world", &vars, false);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_interpolate_missing_closing_braces() {
        let vars = Vars::empty();
        let result = interpolate("hello {{ name", &vars, false);
        assert_eq!(result, "hello {{ name");
    }

    #[test]
    fn test_interpolate_adjacent_vars() {
        let vars = Vars::new(
            vec![],
            HashMap::from([("a".into(), "x".into()), ("b".into(), "y".into())]),
        );
        let result = interpolate("{{a}}{{b}}", &vars, false);
        assert_eq!(result, "xy");
    }

    #[test]
    fn test_interpolate_with_whitespace() {
        let vars = Vars::new(
            vec![],
            HashMap::from([("name".into(), "world".into())]),
        );
        let result = interpolate("hello {{name}}!", &vars, false);
        assert_eq!(result, "hello world!");
    }

    #[test]
    fn test_interpolate_command_with_vars() {
        let vars = Vars::new(
            vec![],
            HashMap::from([
                ("file".into(), "data.txt".into()),
                ("dest".into(), "/tmp".into()),
            ]),
        );
        let cmd = "cp {{ file }} {{ dest }}/";
        let result = interpolate(cmd, &vars, false);
        assert_eq!(result, "cp data.txt /tmp/");
    }

    // -- interpolate_strict tests --------------------------------------------

    #[test]
    fn test_interpolate_strict_ok() {
        let vars = Vars::new(
            vec![],
            HashMap::from([("name".into(), "world".into())]),
        );
        let result = interpolate_strict("hello {{ name }}!", &vars);
        assert_eq!(result, Ok("hello world!".into()));
    }

    #[test]
    fn test_interpolate_strict_undefined() {
        let vars = Vars::empty();
        let result = interpolate_strict("hello {{ missing }}!", &vars);
        assert_eq!(
            result,
            Err(InterpolationError::UndefinedVariable {
                name: "missing".into()
            })
        );
    }

    // -- Serialization tests -------------------------------------------------

    #[test]
    fn test_vars_serialize_roundtrip() {
        let vars = Vars::new(
            vec![VarDefinition {
                name: "msg".into(),
                var_type: VarType::String,
                default: Some("hi".into()),
                description: "A message".into(),
                required: false,
            }],
            HashMap::from([("msg".into(), "hello".into())]),
        );
        let json = serde_json::to_string(&vars).unwrap();
        let restored: Vars = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.get("msg"), Some("hello"));
        assert_eq!(restored.definitions.len(), 1);
    }

    #[test]
    fn test_settings_serialize_roundtrip() {
        let s = Settings::new()
            .with_shell("/bin/zsh")
            .with_work_dir("/home")
            .with_env("K", "v")
            .with_max_concurrency(8);
        let json = serde_json::to_string(&s).unwrap();
        let restored: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.shell, Some("/bin/zsh".into()));
        assert_eq!(restored.max_concurrency, Some(8));
    }
}
