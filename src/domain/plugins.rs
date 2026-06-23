// SPDX-License-Identifier: MIT OR Apache-2.0
//! Plugin system for custom command runners.
//!
//! Provides a [`RunnerPlugin`] trait that allows custom execution strategies
//! to be registered and resolved before falling back to the default shell
//! runner. Plugins are matched by command prefix via [`PluginRegistry`].
//!
//! # Default plugins
//!
//! - [`ShellPlugin`] — runs shell commands via `sh -c` (always available).
//! - [`NoopPlugin`] — prints the command without executing (dry-run mode).

use std::path::Path;
use std::time::{Duration, Instant};

/// Environment variable pair.
pub type EnvEntry = (String, String);

// ---------------------------------------------------------------------------
// PluginContext
// ---------------------------------------------------------------------------

/// Context passed to a [`RunnerPlugin`] at execution time.
///
/// Carries everything a plugin needs to execute a command: the raw command
/// string, an optional working directory, environment overrides, and a
/// dry-run flag.
#[derive(Debug, Clone)]
pub struct PluginContext<'a> {
    /// Working directory for command execution.
    pub working_dir: &'a Path,
    /// The raw command string extracted from the task payload.
    pub command: &'a str,
    /// Arguments parsed from the command (space-split, after the command).
    pub args: Vec<String>,
    /// Environment variable overrides (name → value).
    pub env_vars: Vec<EnvEntry>,
    /// When `true` the plugin should not perform side effects.
    pub dry_run: bool,
}

impl<'a> PluginContext<'a> {
    /// Build a minimal context for a bare command.
    pub fn new(command: &'a str) -> Self {
        Self {
            working_dir: Path::new("."),
            command,
            args: shell_words_split(command),
            env_vars: Vec::new(),
            dry_run: false,
        }
    }

    /// Set the working directory.
    pub fn with_working_dir(mut self, dir: &'a Path) -> Self {
        self.working_dir = dir;
        self
    }

    /// Enable dry-run mode.
    pub fn with_dry_run(mut self) -> Self {
        self.dry_run = true;
        self
    }

    /// Add an environment variable override.
    pub fn with_env(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.env_vars.push((key.into(), val.into()));
        self
    }
}

/// Naive shell-word splitter (space-separated, no quoting).
///
/// A proper implementation would handle quotes and escapes; this is
/// sufficient for argument hints in the plugin context.
fn shell_words_split(input: &str) -> Vec<String> {
    input.split_whitespace().map(|s| s.to_string()).collect()
}

// ---------------------------------------------------------------------------
// PluginResult
// ---------------------------------------------------------------------------

/// Outcome of a [`RunnerPlugin::execute`] call.
#[derive(Debug, Clone)]
pub struct PluginResult {
    /// Whether the plugin considers execution successful (exit code 0).
    pub success: bool,
    /// Captured standard output.
    pub stdout: String,
    /// Captured standard error.
    pub stderr: String,
    /// Process exit code (0 for success, negative for signals).
    pub exit_code: i32,
    /// Wall-clock execution duration.
    pub duration: Duration,
}

impl PluginResult {
    /// Build a successful empty result (useful for dry-run stubs).
    pub fn ok() -> Self {
        Self {
            success: true,
            stdout: String::new(),
            stderr: String::new(),
            exit_code: 0,
            duration: Duration::ZERO,
        }
    }

    /// Build a failure result with a message placed in stderr.
    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            stdout: String::new(),
            stderr: msg.into(),
            exit_code: 1,
            duration: Duration::ZERO,
        }
    }

    /// Serialize the result to a JSON value compatible with [`TaskResult`]
    /// output shapes.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "status": if self.success { "ok" } else { "error" },
            "code": self.exit_code,
            "stdout": self.stdout,
            "stderr": self.stderr,
            "duration_ms": self.duration.as_millis(),
        })
    }
}

// ---------------------------------------------------------------------------
// RunnerPlugin trait
// ---------------------------------------------------------------------------

/// A plugin that can execute shell commands or other runnable operations.
///
/// Plugins are registered with a [`PluginRegistry`] and resolved by command
/// prefix. When [`can_handle`] returns `true` for a given command, the
/// registry returns that plugin for execution.
///
/// [`can_handle`]: RunnerPlugin::can_handle
pub trait RunnerPlugin: Send + Sync {
    /// Human-readable plugin name (e.g. `"shell"`, `"docker"`, `"noop"`).
    fn name(&self) -> &str;

    /// Return `true` if this plugin can execute `command`.
    ///
    /// Typically plugins check for a command prefix. For example, a
    /// hypothetical `DockerPlugin` would return `true` for commands
    /// starting with `"docker "`.
    fn can_handle(&self, command: &str) -> bool;

    /// Execute the command with the given context.
    fn execute(&self, ctx: PluginContext<'_>) -> PluginResult;
}

// ---------------------------------------------------------------------------
// PluginRegistry
// ---------------------------------------------------------------------------

/// A registry of [`RunnerPlugin`] instances.
///
/// Plugins are stored in registration order. [`find`] returns the **first**
/// plugin whose [`can_handle`] returns `true`, so more-specific plugins
/// should be registered **before** the catch-all [`ShellPlugin`].
///
/// [`find`]: PluginRegistry::find
/// [`can_handle`]: RunnerPlugin::can_handle
pub struct PluginRegistry {
    plugins: Vec<Box<dyn RunnerPlugin>>,
}

impl PluginRegistry {
    /// Create an empty registry (no plugins).
    pub fn new() -> Self {
        Self { plugins: Vec::new() }
    }

    /// Create a registry pre-populated with the default plugins:
    ///
    /// 1. [`ShellPlugin`] — catch-all shell runner.
    pub fn with_defaults() -> Self {
        let mut reg = Self::new();
        reg.register(Box::new(ShellPlugin));
        reg
    }

    /// Create a registry with both the default plugins **and** a
    /// [`NoopPlugin`] registered first (for dry-run scenarios).
    pub fn with_defaults_and_noop() -> Self {
        let mut reg = Self::new();
        reg.register(Box::new(NoopPlugin));
        reg.register(Box::new(ShellPlugin));
        reg
    }

    /// Register a plugin.
    ///
    /// Plugins are checked in registration order — register
    /// more-specific handlers first.
    pub fn register(&mut self, plugin: Box<dyn RunnerPlugin>) {
        self.plugins.push(plugin);
    }

    /// Find the first registered plugin that can handle `command`.
    ///
    /// Returns `None` when no plugin claims the command (unlikely with
    /// the catch-all [`ShellPlugin`] registered).
    pub fn find(&self, command: &str) -> Option<&dyn RunnerPlugin> {
        self.plugins.iter().find(|p| p.can_handle(command)).map(|p| p.as_ref())
    }

    /// Return the names of all registered plugins.
    pub fn names(&self) -> Vec<&str> {
        self.plugins.iter().map(|p| p.name()).collect()
    }

    /// Number of registered plugins.
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// True when no plugins are registered.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// ---------------------------------------------------------------------------
// ShellPlugin  (default fallback)
// ---------------------------------------------------------------------------

/// Runs shell commands via `sh -c`.
///
/// Always claims any command (always returns `true` from
/// [`can_handle`]), so it **must** be registered last in the plugin
/// list to act as a catch-all fallback.
///
/// [`can_handle`]: RunnerPlugin::can_handle
pub struct ShellPlugin;

impl RunnerPlugin for ShellPlugin {
    fn name(&self) -> &str {
        "shell"
    }

    fn can_handle(&self, _command: &str) -> bool {
        true // catch-all — always claims commands
    }

    fn execute(&self, ctx: PluginContext<'_>) -> PluginResult {
        let start = Instant::now();

        let output = std::process::Command::new("sh").arg("-c").arg(ctx.command).output();

        let duration = start.elapsed();

        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code().unwrap_or(-1);
                PluginResult {
                    success: output.status.success(),
                    stdout,
                    stderr,
                    exit_code,
                    duration,
                }
            }
            Err(e) => PluginResult {
                success: false,
                stdout: String::new(),
                stderr: format!("ShellPlugin failed to spawn command: {e}"),
                exit_code: -1,
                duration,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// NoopPlugin  (dry-run)
// ---------------------------------------------------------------------------

/// A no-operation plugin that prints the command to stderr and returns
/// a successful result immediately.
///
/// Designed for dry-run mode. When registered at the front of the
/// plugin list it intercepts all commands before they reach the
/// [`ShellPlugin`].
pub struct NoopPlugin;

impl RunnerPlugin for NoopPlugin {
    fn name(&self) -> &str {
        "noop"
    }

    fn can_handle(&self, _command: &str) -> bool {
        true // always willing to handle
    }

    fn execute(&self, ctx: PluginContext<'_>) -> PluginResult {
        eprintln!("[dry-run] would execute: {}", ctx.command);
        PluginResult {
            success: true,
            stdout: String::new(),
            stderr: format!("[dry-run] would execute: {}", ctx.command),
            exit_code: 0,
            duration: Duration::ZERO,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- plugin registration ----

    #[test]
    fn test_registry_empty() {
        let reg = PluginRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
    }

    #[test]
    fn test_registry_with_defaults() {
        let reg = PluginRegistry::with_defaults();
        assert_eq!(reg.len(), 1);
        assert_eq!(reg.names(), vec!["shell"]);
    }

    #[test]
    fn test_registry_with_defaults_and_noop() {
        let reg = PluginRegistry::with_defaults_and_noop();
        assert_eq!(reg.len(), 2);
        assert_eq!(reg.names(), vec!["noop", "shell"]);
    }

    #[test]
    fn test_registry_find_shell() {
        let reg = PluginRegistry::with_defaults();
        let plugin = reg.find("echo hello").unwrap();
        assert_eq!(plugin.name(), "shell");
    }

    #[test]
    fn test_registry_find_noop_with_prefix() {
        let mut reg = PluginRegistry::new();
        reg.register(Box::new(NoopPlugin));
        reg.register(Box::new(ShellPlugin));

        // NoopPlugin handles everything when it's first
        let plugin = reg.find("anything").unwrap();
        assert_eq!(plugin.name(), "noop");
    }

    #[test]
    fn test_registry_respects_order() {
        let mut reg = PluginRegistry::new();
        reg.register(Box::new(ShellPlugin));
        reg.register(Box::new(NoopPlugin));

        // ShellPlugin is first, so it wins
        let plugin = reg.find("anything").unwrap();
        assert_eq!(plugin.name(), "shell");
    }

    #[test]
    fn test_registry_custom_plugin_takes_precedence() {
        struct CustomPlugin;

        impl RunnerPlugin for CustomPlugin {
            fn name(&self) -> &str {
                "custom"
            }
            fn can_handle(&self, cmd: &str) -> bool {
                cmd.starts_with("custom:")
            }
            fn execute(&self, _ctx: PluginContext<'_>) -> PluginResult {
                PluginResult::ok()
            }
        }

        let mut reg = PluginRegistry::new();
        reg.register(Box::new(CustomPlugin));
        reg.register(Box::new(ShellPlugin));

        // Custom plugin handles "custom:deploy"
        let p = reg.find("custom:deploy").unwrap();
        assert_eq!(p.name(), "custom");

        // ShellPlugin handles everything else
        let p = reg.find("echo hi").unwrap();
        assert_eq!(p.name(), "shell");
    }

    // ---- ShellPlugin execution ----

    #[test]
    fn test_shell_plugin_echo() {
        let plugin = ShellPlugin;
        let ctx = PluginContext::new("echo hello_from_shell");
        let result = plugin.execute(ctx);
        assert!(result.success, "shell echo should succeed");
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("hello_from_shell"));
    }

    #[test]
    fn test_shell_plugin_failure() {
        let plugin = ShellPlugin;
        let ctx = PluginContext::new("false");
        let result = plugin.execute(ctx);
        assert!(!result.success);
        assert_ne!(result.exit_code, 0);
    }

    #[test]
    fn test_shell_plugin_stderr() {
        let plugin = ShellPlugin;
        let ctx = PluginContext::new("echo warn_msg 1>&2");
        let result = plugin.execute(ctx);
        assert!(result.success);
        assert!(result.stderr.contains("warn_msg"));
    }

    #[test]
    fn test_shell_plugin_exit_code() {
        let plugin = ShellPlugin;
        let ctx = PluginContext::new("exit 42");
        let result = plugin.execute(ctx);
        assert!(!result.success);
        assert_eq!(result.exit_code, 42);
    }

    #[test]
    fn test_shell_plugin_duration_nonzero() {
        let plugin = ShellPlugin;
        let ctx = PluginContext::new(":");
        let result = plugin.execute(ctx);
        assert!(result.success);
        assert!(result.duration > Duration::ZERO);
    }

    // ---- NoopPlugin ----

    #[test]
    fn test_noop_plugin_always_succeeds() {
        let plugin = NoopPlugin;
        let ctx = PluginContext::new("any-command --flag");
        let result = plugin.execute(ctx);
        assert!(result.success);
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_noop_plugin_duration_zero() {
        let plugin = NoopPlugin;
        let ctx = PluginContext::new("anything");
        let result = plugin.execute(ctx);
        assert_eq!(result.duration, Duration::ZERO);
    }

    #[test]
    fn test_noop_plugin_stderr_contains_command() {
        let plugin = NoopPlugin;
        let ctx = PluginContext::new("deploy --env prod");
        let result = plugin.execute(ctx);
        assert!(result.stderr.contains("deploy --env prod"));
    }

    // ---- PluginResult helpers ----

    #[test]
    fn test_plugin_result_ok() {
        let r = PluginResult::ok();
        assert!(r.success);
        assert_eq!(r.exit_code, 0);
        assert_eq!(r.stdout, "");
    }

    #[test]
    fn test_plugin_result_error() {
        let r = PluginResult::error("something went wrong");
        assert!(!r.success);
        assert_eq!(r.exit_code, 1);
        assert!(r.stderr.contains("went wrong"));
    }

    #[test]
    fn test_plugin_result_to_json_success() {
        let r = PluginResult {
            success: true,
            stdout: "hello".into(),
            stderr: String::new(),
            exit_code: 0,
            duration: Duration::from_millis(50),
        };
        let json = r.to_json();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["code"], 0);
        assert_eq!(json["stdout"], "hello");
    }

    #[test]
    fn test_plugin_result_to_json_failure() {
        let r = PluginResult {
            success: false,
            stdout: String::new(),
            stderr: "error msg".into(),
            exit_code: 1,
            duration: Duration::from_millis(10),
        };
        let json = r.to_json();
        assert_eq!(json["status"], "error");
        assert_eq!(json["stderr"], "error msg");
    }

    // ---- PluginContext ----

    #[test]
    fn test_plugin_context_new() {
        let ctx = PluginContext::new("echo hello world");
        assert_eq!(ctx.command, "echo hello world");
        assert_eq!(ctx.working_dir, Path::new("."));
        assert!(!ctx.dry_run);
        assert!(ctx.env_vars.is_empty());
    }

    #[test]
    fn test_plugin_context_with_dry_run() {
        let ctx = PluginContext::new("anything").with_dry_run();
        assert!(ctx.dry_run);
    }

    #[test]
    fn test_plugin_context_with_env() {
        let ctx = PluginContext::new("test").with_env("PATH", "/usr/bin").with_env("HOME", "/root");
        assert_eq!(ctx.env_vars.len(), 2);
    }

    #[test]
    fn test_plugin_context_shell_words_split() {
        let args = shell_words_split("echo hello world");
        assert_eq!(args, vec!["echo", "hello", "world"]);
    }

    #[test]
    fn test_plugin_context_shell_words_split_empty() {
        let args = shell_words_split("");
        assert!(args.is_empty());
    }

    // ---- unknown command: registry returns None for empty registry ----

    #[test]
    fn test_registry_find_unknown() {
        let reg = PluginRegistry::new();
        assert!(reg.find("anything").is_none());
    }
}
