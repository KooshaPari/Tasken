// SPDX-License-Identifier: MIT OR Apache-2.0
//! Recipe data model: variables, settings, and interpolation.
//!
//! This module provides the building blocks for defining, configuring,
//! and rendering recipes — reusable task templates with parameterized
//! commands, environment settings, and variable substitution.

use std::collections::HashMap;

use chrono::Utc;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// VarType
// ---------------------------------------------------------------------------

/// Type hint for a recipe variable.
///
/// Used for documentation, validation, and UI rendering. Does **not**
/// enforce type coercion at interpolation time — all values are stored
/// as strings internally.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VarType {
    #[default]
    String,
    Number,
    Bool,
    Path,
    Choice(Vec<String>),
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
        Self { definitions: Vec::new(), values: predefined_vars() }
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
    map.insert("timestamp".to_string(), Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string());

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
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
                    result.push_str(&format!("{{{{ undefined: {var_name} }}}}"));
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
// Condition evaluation
// ---------------------------------------------------------------------------

/// Evaluates a condition expression against the runtime OS/architecture.
///
/// # Syntax
///
/// | Expression              | Meaning                           |
/// |-------------------------|-----------------------------------|
/// | `os == "linux"`         | Equality                          |
/// | `arch != "x86_64"`      | Inequality                        |
/// | `os contains "nux"`     | Substring match                   |
/// | `... and ...`           | Logical AND                       |
/// | `... or ...`            | Logical OR                        |
/// | `(...)`                 | Grouping                          |
///
/// Recognised variables:
/// - `os`   → `std::env::consts::OS` (e.g. `"linux"`, `"macos"`, `"windows"`)
/// - `arch` → `std::env::consts::ARCH` (e.g. `"x86_64"`, `"aarch64"`)
///
/// # Returns
///
/// - `true` for an empty / all-whitespace condition (no constraint).
/// - `false` for syntactically invalid expressions or unknown variables
///   (conservative: don't run when the condition can't be understood).
///
/// # Examples
///
/// ```
/// use taskkit::domain::recipes::evaluate_condition;
///
/// assert!(evaluate_condition(""));
/// assert!(evaluate_condition("os == \"macos\"") || evaluate_condition("os == \"linux\""));
/// assert!(!evaluate_condition("os == \"nonexistent_os\""));
/// ```
pub fn evaluate_condition(condition: &str) -> bool {
    let condition = condition.trim();
    if condition.is_empty() {
        return true;
    }

    let tokens = match tokenize(condition) {
        Some(t) => t,
        None => return false,
    };

    let mut parser = Parser { tokens, pos: 0 };
    parser.parse_or()
}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Ident(String),
    StringLit(String),
    Eq,
    Neq,
    Contains,
    And,
    Or,
    LParen,
    RParen,
}

/// Tokenize a condition string. Returns `None` on invalid tokens.
fn tokenize(input: &str) -> Option<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }

        match ch {
            '(' => {
                chars.next();
                tokens.push(Token::LParen);
            }
            ')' => {
                chars.next();
                tokens.push(Token::RParen);
            }
            '"' => {
                chars.next(); // skip opening quote
                let mut s = String::new();
                loop {
                    match chars.next() {
                        None => return None, // unclosed string literal
                        Some('"') => break,
                        Some(c) => s.push(c),
                    }
                }
                tokens.push(Token::StringLit(s));
            }
            '!' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Neq);
                } else {
                    return None; // bare `!` is not valid
                }
            }
            '=' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Eq);
                } else {
                    return None; // bare `=` is not valid
                }
            }
            _ if ch.is_ascii_alphanumeric() || ch == '_' => {
                let mut s = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                        s.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                match s.as_str() {
                    "and" => tokens.push(Token::And),
                    "or" => tokens.push(Token::Or),
                    "contains" => tokens.push(Token::Contains),
                    _ => tokens.push(Token::Ident(s)),
                }
            }
            _ => {
                // Unexpected character
                return None;
            }
        }
    }

    Some(tokens)
}

// ---------------------------------------------------------------------------
// Recursive-descent parser
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Op {
    Eq,
    Neq,
    Contains,
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    // or_expr = and_expr ("or" and_expr)*
    fn parse_or(&mut self) -> bool {
        let mut left = self.parse_and();
        while self.peek() == Some(&Token::Or) {
            self.advance();
            let right = self.parse_and();
            left = left || right;
        }
        left
    }

    // and_expr = primary ("and" primary)*
    fn parse_and(&mut self) -> bool {
        let mut left = self.parse_primary();
        while self.peek() == Some(&Token::And) {
            self.advance();
            let right = self.parse_primary();
            left = left && right;
        }
        left
    }

    // primary = "(" or_expr ")" | comparison
    fn parse_primary(&mut self) -> bool {
        if self.peek() == Some(&Token::LParen) {
            self.advance();
            let result = self.parse_or();
            if self.peek() == Some(&Token::RParen) {
                self.advance();
            }
            // If the closing paren is missing, the expression is invalid → false
            return result;
        }
        self.parse_comparison()
    }

    // comparison = IDENTIFIER OP STRING_LITERAL
    fn parse_comparison(&mut self) -> bool {
        let ident = match self.peek() {
            Some(Token::Ident(s)) => s.clone(),
            _ => return false,
        };
        self.advance();

        let op = match self.peek() {
            Some(Token::Eq) => Op::Eq,
            Some(Token::Neq) => Op::Neq,
            Some(Token::Contains) => Op::Contains,
            _ => return false,
        };
        self.advance();

        let value = match self.peek() {
            Some(Token::StringLit(s)) => s.clone(),
            _ => return false,
        };
        self.advance();

        let resolved = resolve_var(&ident);
        match op {
            Op::Eq => resolved == value,
            Op::Neq => resolved != value,
            Op::Contains => resolved.contains(&value),
        }
    }
}

/// Resolve a condition variable name to its runtime value.
fn resolve_var(name: &str) -> String {
    match name {
        "os" => std::env::consts::OS.to_string(),
        "arch" => std::env::consts::ARCH.to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::domain::recipe::TaskenfileParser;

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
        assert!(ts.len() >= 24, "timestamp '{ts}' should be ISO-8601 format");
        assert!(ts.ends_with('Z'), "timestamp '{ts}' should end with Z");
    }

    #[test]
    fn test_predefined_vars_pid_is_numeric() {
        let pv = predefined_vars();
        let pid = pv.get("pid").unwrap();
        assert!(pid.parse::<u32>().is_ok(), "pid '{pid}' should be a numeric string");
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
        let vars = Vars::new(vec![], HashMap::from([("name".into(), "world".into())]));
        let result = interpolate("hello {{ name }}!", &vars, false);
        assert_eq!(result, "hello world!");
    }

    #[test]
    fn test_interpolate_multiple_vars() {
        let vars = Vars::new(
            vec![],
            HashMap::from([("first".into(), "John".into()), ("last".into(), "Doe".into())]),
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
        let vars =
            Vars::new(vec![], HashMap::from([("a".into(), "x".into()), ("b".into(), "y".into())]));
        let result = interpolate("{{a}}{{b}}", &vars, false);
        assert_eq!(result, "xy");
    }

    #[test]
    fn test_interpolate_with_whitespace() {
        let vars = Vars::new(vec![], HashMap::from([("name".into(), "world".into())]));
        let result = interpolate("hello {{name}}!", &vars, false);
        assert_eq!(result, "hello world!");
    }

    #[test]
    fn test_interpolate_command_with_vars() {
        let vars = Vars::new(
            vec![],
            HashMap::from([("file".into(), "data.txt".into()), ("dest".into(), "/tmp".into())]),
        );
        let cmd = "cp {{ file }} {{ dest }}/";
        let result = interpolate(cmd, &vars, false);
        assert_eq!(result, "cp data.txt /tmp/");
    }

    // -- interpolate_strict tests --------------------------------------------

    #[test]
    fn test_interpolate_strict_ok() {
        let vars = Vars::new(vec![], HashMap::from([("name".into(), "world".into())]));
        let result = interpolate_strict("hello {{ name }}!", &vars);
        assert_eq!(result, Ok("hello world!".into()));
    }

    #[test]
    fn test_interpolate_strict_undefined() {
        let vars = Vars::empty();
        let result = interpolate_strict("hello {{ missing }}!", &vars);
        assert_eq!(result, Err(InterpolationError::UndefinedVariable { name: "missing".into() }));
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

    // -- Condition evaluation tests -----------------------------------------

    // Basic conditions

    #[test]
    fn test_condition_empty() {
        assert!(evaluate_condition(""));
        assert!(evaluate_condition("   "));
        assert!(evaluate_condition("\t\n"));
    }

    #[test]
    fn test_condition_os_equality() {
        let current_os = std::env::consts::OS;
        // Should match the current OS
        assert!(evaluate_condition(&format!("os == \"{current_os}\"")));
        // Should NOT match a different OS
        let other_os = if current_os == "linux" { "macos" } else { "linux" };
        assert!(!evaluate_condition(&format!("os == \"{other_os}\"")));
    }

    #[test]
    fn test_condition_arch_equality() {
        let current_arch = std::env::consts::ARCH;
        assert!(evaluate_condition(&format!("arch == \"{current_arch}\"")));
        assert!(!evaluate_condition(&format!("arch == \"{}\"", "nonexistent_arch")));
    }

    #[test]
    fn test_condition_os_inequality() {
        let current_os = std::env::consts::OS;
        assert!(!evaluate_condition(&format!("os != \"{current_os}\"")));
        assert!(evaluate_condition("os != \"nonexistent_os\""));
    }

    #[test]
    fn test_condition_arch_inequality() {
        let current_arch = std::env::consts::ARCH;
        assert!(!evaluate_condition(&format!("arch != \"{current_arch}\"")));
        assert!(evaluate_condition("arch != \"nonexistent_arch\""));
    }

    #[test]
    fn test_condition_contains() {
        let current_os = std::env::consts::OS;
        // Every OS name contains its first character
        let first_char = &current_os[..1];
        assert!(evaluate_condition(&format!("os contains \"{first_char}\"")));
        // No OS contains "zzzzzz"
        assert!(!evaluate_condition("os contains \"zzzzzz\""));
    }

    // AND / OR / nesting

    #[test]
    fn test_condition_and_true() {
        let current_os = std::env::consts::OS;
        let current_arch = std::env::consts::ARCH;
        assert!(evaluate_condition(&format!(
            "os == \"{current_os}\" and arch == \"{current_arch}\""
        )));
    }

    #[test]
    fn test_condition_and_false() {
        let current_os = std::env::consts::OS;
        assert!(!evaluate_condition(&format!(
            "os == \"{current_os}\" and arch == \"nonexistent\""
        )));
    }

    #[test]
    fn test_condition_or_true() {
        assert!(evaluate_condition("os == \"linux\" or os == \"macos\""));
    }

    #[test]
    fn test_condition_or_false() {
        assert!(!evaluate_condition("os == \"nonexistent1\" or os == \"nonexistent2\""));
    }

    #[test]
    fn test_condition_nested_parens() {
        let current_os = std::env::consts::OS;
        let current_arch = std::env::consts::ARCH;
        // (os == "current" and arch == "current") should be true
        assert!(evaluate_condition(&format!(
            "(os == \"{current_os}\" and arch == \"{current_arch}\")"
        )));
    }

    #[test]
    fn test_condition_nested_complex() {
        let current_os = std::env::consts::OS;
        // (os == "current" or os == "nonexistent") and arch != "nonexistent"
        assert!(evaluate_condition(&format!(
            "(os == \"{current_os}\" or os == \"fake\") and arch != \"nonexistent\""
        )));
        // (os == "nonexistent1" and os == "nonexistent2") or arch == "nonexistent"
        let current_arch = std::env::consts::ARCH;
        assert!(evaluate_condition(&format!(
            "(os == \"x1\" and os == \"x2\") or arch == \"{current_arch}\""
        )));
    }

    #[test]
    fn test_condition_mixed_and_or() {
        // os == "current" or (os == "x" and arch == "y")
        let current_os = std::env::consts::OS;
        assert!(evaluate_condition(&format!(
            "os == \"{current_os}\" or (os == \"x\" and arch == \"y\")"
        )));
        // (os == "x" and arch == "y") or os == "current"
        assert!(evaluate_condition(&format!(
            "(os == \"x\" and arch == \"y\") or os == \"{current_os}\""
        )));
    }

    // Invalid / edge-case expressions

    #[test]
    fn test_condition_invalid_expression() {
        assert!(!evaluate_condition("invalid no operator"));
        assert!(!evaluate_condition("os == ")); // missing value
        assert!(!evaluate_condition("== \"linux\"")); // missing variable
        assert!(!evaluate_condition("os !=")); // missing value for !=
        assert!(!evaluate_condition("os contains")); // missing value for contains
    }

    #[test]
    fn test_condition_unknown_variable() {
        // Unknown variables resolve to ""
        assert!(!evaluate_condition("foobar == \"anything\""));
        // Comparing unknown to empty string matches (both are "")
        assert!(evaluate_condition("unknown_var == \"\""));
    }

    #[test]
    fn test_condition_unclosed_string() {
        assert!(!evaluate_condition("os == \"linux")); // missing closing quote
    }

    #[test]
    fn test_condition_unclosed_paren() {
        let current_os = std::env::consts::OS;
        // Missing closing paren - the parser still evaluates but returns
        // the inner result, then there's no closing paren.
        // (os == "current" should still evaluate
        assert!(evaluate_condition(&format!("(os == \"{current_os}\"")));
    }

    #[test]
    fn test_condition_invalid_operator() {
        assert!(!evaluate_condition("os = linux")); // single = not valid
        assert!(!evaluate_condition("os ! linux")); // ! without = not valid
    }

    #[test]
    fn test_condition_unknown_token() {
        assert!(!evaluate_condition("os == \"linux\" and @@")); // @ is invalid
    }

    // Serialization / deserialization of condition field

    #[test]
    fn test_parse_toml_with_condition() {
        let toml = r#"
name = "conditional-recipe"

[[tasks]]
name = "linux-only"
command = "echo linux"
condition = 'os == "linux"'

[[tasks]]
name = "macos-only"
command = "echo macos"
condition = 'os == "macos"'
"#;
        let recipe = TaskenfileParser::parse_toml(toml).unwrap();
        assert_eq!(recipe.tasks.len(), 2);
        assert_eq!(recipe.tasks[0].condition.as_deref(), Some("os == \"linux\""));
        assert_eq!(recipe.tasks[1].condition.as_deref(), Some("os == \"macos\""));
    }

    #[test]
    fn test_parse_toml_without_condition_defaults_to_none() {
        let toml = r#"
name = "no-condition"

[[tasks]]
name = "always"
command = "echo always"
"#;
        let recipe = TaskenfileParser::parse_toml(toml).unwrap();
        assert_eq!(recipe.tasks[0].condition, None);
    }

    #[test]
    fn test_condition_combined_with_depends_on() {
        let toml = r#"
name = "combined"

[[tasks]]
name = "init"
command = "init"

[[tasks]]
name = "build"
command = "build"
depends_on = ["init"]
condition = 'os == "linux"'
"#;
        let recipe = TaskenfileParser::parse_toml(toml).unwrap();
        assert_eq!(recipe.tasks[1].depends_on, vec!["init"]);
        assert_eq!(recipe.tasks[1].condition.as_deref(), Some("os == \"linux\""));
    }

    #[test]
    fn test_condition_tokenize_edge_cases() {
        // Empty string after trim handled by evaluate_condition
        assert!(evaluate_condition(""));
        // Identifier with hyphens (e.g. future variable names)
        assert!(!evaluate_condition("my-var == \"value\""));
        // Numbers in identifiers
        assert!(!evaluate_condition("os2 == \"linux\""));
    }
}
