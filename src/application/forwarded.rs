// SPDX-License-Identifier: MIT OR Apache-2.0
//! Argument forwarding utilities for CLI invocations.
//!
//! Provides support for the standard Unix convention where everything
//! after a literal `--` token is treated as a positional argument
//! rather than as a flag. This is essential when a task's command
//! may itself contain flags that conflict with the wrapper's flags.
//!
//! # Example
//!
//! ```text
//! taskkit run my-task -- --release --target=x86_64 --features "tokio,serde"
//! ```
//!
//! Everything after `--` is forwarded verbatim to the underlying command.

/// A wrapper that captures everything after `--` as a forward list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ForwardedArgs {
    /// Raw positional arguments collected after the `--` separator.
    /// Does not include the separator itself.
    pub args: Vec<String>,
}

impl ForwardedArgs {
    /// Construct a new empty forward list.
    pub fn new() -> Self {
        Self { args: Vec::new() }
    }

    /// Append a single forwarded argument.
    pub fn push(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Build a forwarded args list from a slice.
    pub fn from_slice<S: AsRef<str>>(slice: &[S]) -> Self {
        Self { args: slice.iter().map(|s| s.as_ref().to_string()).collect() }
    }

    /// True when no arguments were forwarded.
    pub fn is_empty(&self) -> bool {
        self.args.is_empty()
    }

    /// Number of forwarded arguments.
    pub fn len(&self) -> usize {
        self.args.len()
    }

    /// Quote each argument with POSIX shell rules and join with spaces.
    ///
    /// Empty arguments are preserved as `''`. Arguments containing
    /// whitespace or shell metacharacters are wrapped in single quotes.
    pub fn shell_quote(&self) -> String {
        self.args.iter().map(|a| shell_quote_single(a)).collect::<Vec<_>>().join(" ")
    }

    /// Render as a JSON array.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::Value::Array(
            self.args.iter().map(|s| serde_json::Value::String(s.clone())).collect(),
        )
    }
}

impl std::fmt::Display for ForwardedArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.shell_quote())
    }
}

impl std::iter::FromIterator<String> for ForwardedArgs {
    fn from_iter<I: IntoIterator<Item = String>>(iter: I) -> Self {
        Self { args: iter.into_iter().collect() }
    }
}

impl<'a> std::iter::FromIterator<&'a str> for ForwardedArgs {
    fn from_iter<I: IntoIterator<Item = &'a str>>(iter: I) -> Self {
        Self { args: iter.into_iter().map(|s| s.to_string()).collect() }
    }
}

/// Quote a single argument per POSIX rules.
///
/// - Empty string -> `''`
/// - No metacharacters -> returned as-is
/// - Otherwise wrapped in single quotes; embedded `'` becomes `'\''`
pub fn shell_quote_single(arg: &str) -> String {
    if arg.is_empty() {
        return "''".to_string();
    }
    if !arg.bytes().any(is_shell_metachar) {
        return arg.to_string();
    }
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('\'');
    for ch in arg.chars() {
        if ch == '\'' {
            out.push('\'');
            out.push('\\');
            out.push('\'');
            out.push('\'');
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

/// Characters that have special meaning in POSIX shells and would
/// require quoting when round-tripping an argument through a shell.
fn is_shell_metachar(b: u8) -> bool {
    matches!(
        b,
        b' ' | b'\t'
            | b'\n'
            | b'\''
            | b'"'
            | b'\\'
            | b'$'
            | b'`'
            | b'&'
            | b'|'
            | b';'
            | b'<'
            | b'>'
            | b'('
            | b')'
            | b'{'
            | b'}'
            | b'*'
            | b'?'
            | b'['
            | b']'
            | b'#'
            | b'~'
            | b'!'
            | b','
    )
}

/// Split a raw command line at the first `--` token.
///
/// Returns `(flags_part, forwarded)` where `flags_part` is everything
/// before `--` (which clap will have already consumed) and `forwarded`
/// is the post-separator argument vector. If no `--` is present, all
/// arguments are considered forwarded.
pub fn split_at_separator<I, S>(args: I) -> (Vec<String>, Vec<String>)
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let collected: Vec<String> = args.into_iter().map(Into::into).collect();
    if let Some(idx) = collected.iter().position(|a| a == "--") {
        let before = collected[..idx].to_vec();
        let after = collected[idx + 1..].to_vec();
        (before, after)
    } else {
        (Vec::new(), collected)
    }
}

/// Compose a command by appending forwarded arguments to a base command.
///
/// The base command may already include arguments; the forwarded list
/// is appended verbatim. Shell metacharacters in forwarded arguments
/// are quoted before joining, so the resulting string is safe to feed
/// to `sh -c` as a single command line.
pub fn compose_command(base: &str, forwarded: &ForwardedArgs) -> String {
    if forwarded.is_empty() {
        return base.to_string();
    }
    let quoted = forwarded.shell_quote();
    if quoted.is_empty() {
        base.to_string()
    } else if base.is_empty() {
        quoted
    } else {
        format!("{base} {quoted}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_forwarded() {
        let f = ForwardedArgs::new();
        assert!(f.is_empty());
        assert_eq!(f.len(), 0);
        assert_eq!(f.shell_quote(), "");
    }

    #[test]
    fn test_builder_push() {
        let f = ForwardedArgs::new().push("--release").push("--target=x86_64");
        assert_eq!(f.args, vec!["--release", "--target=x86_64"]);
    }

    #[test]
    fn test_from_slice() {
        let arr = ["a", "b", "c"];
        let f = ForwardedArgs::from_slice(&arr);
        assert_eq!(f.args, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_from_iter_string() {
        let f: ForwardedArgs = vec!["a".to_string(), "b".to_string()].into_iter().collect();
        assert_eq!(f.args.len(), 2);
    }

    #[test]
    fn test_from_iter_str() {
        let f: ForwardedArgs = vec!["a", "b"].into_iter().collect();
        assert_eq!(f.args, vec!["a", "b"]);
    }

    #[test]
    fn test_shell_quote_simple() {
        let f = ForwardedArgs::new().push("--release").push("x86_64");
        assert_eq!(f.shell_quote(), "--release x86_64");
    }

    #[test]
    fn test_shell_quote_empty_arg() {
        let f = ForwardedArgs::new().push("");
        assert_eq!(f.shell_quote(), "''");
    }

    #[test]
    fn test_shell_quote_with_space() {
        let f = ForwardedArgs::new().push("hello world");
        assert_eq!(f.shell_quote(), "'hello world'");
    }

    #[test]
    fn test_shell_quote_with_quote() {
        let f = ForwardedArgs::new().push("it's");
        // Single quote inside: end quote, escaped quote, start quote
        assert_eq!(f.shell_quote(), "'it'\\''s'");
    }

    #[test]
    fn test_shell_quote_with_dollar() {
        let f = ForwardedArgs::new().push("$HOME");
        assert_eq!(f.shell_quote(), "'$HOME'");
    }

    #[test]
    fn test_shell_quote_single_helper() {
        assert_eq!(shell_quote_single(""), "''");
        assert_eq!(shell_quote_single("plain"), "plain");
        assert_eq!(shell_quote_single("a b"), "'a b'");
        assert_eq!(shell_quote_single("a'b"), "'a'\\''b'");
    }

    #[test]
    fn test_to_json() {
        let f = ForwardedArgs::new().push("--foo").push("bar");
        let json = f.to_json();
        assert_eq!(json, serde_json::json!(["--foo", "bar"]));
    }

    #[test]
    fn test_display() {
        let f = ForwardedArgs::new().push("hello");
        assert_eq!(format!("{f}"), "hello");
    }

    #[test]
    fn test_split_at_separator() {
        let (before, after) = split_at_separator(vec!["a", "b", "--", "c", "d"]);
        assert_eq!(before, vec!["a", "b"]);
        assert_eq!(after, vec!["c", "d"]);
    }

    #[test]
    fn test_split_at_separator_no_sep() {
        let (before, after) = split_at_separator(vec!["a", "b", "c"]);
        assert!(before.is_empty());
        assert_eq!(after, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_split_at_separator_empty() {
        let (before, after): (Vec<String>, Vec<String>) = split_at_separator(Vec::<String>::new());
        assert!(before.is_empty());
        assert!(after.is_empty());
    }

    #[test]
    fn test_split_at_separator_only_separator() {
        let (before, after) = split_at_separator(vec!["--", "x"]);
        assert!(before.is_empty());
        assert_eq!(after, vec!["x"]);
    }

    #[test]
    fn test_compose_command_empty() {
        let f = ForwardedArgs::new();
        assert_eq!(compose_command("echo hello", &f), "echo hello");
    }

    #[test]
    fn test_compose_command_with_args() {
        let f = ForwardedArgs::new().push("--release").push("x86_64");
        assert_eq!(compose_command("cargo build", &f), "cargo build --release x86_64");
    }

    #[test]
    fn test_compose_command_quotes_special_chars() {
        let f = ForwardedArgs::new().push("hello world");
        assert_eq!(compose_command("echo", &f), "echo 'hello world'");
    }

    #[test]
    fn test_is_shell_metachar() {
        assert!(is_shell_metachar(b' '));
        assert!(is_shell_metachar(b'$'));
        assert!(is_shell_metachar(b'|'));
        assert!(!is_shell_metachar(b'a'));
        assert!(!is_shell_metachar(b'1'));
    }
}
