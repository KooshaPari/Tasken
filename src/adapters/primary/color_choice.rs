// SPDX-License-Identifier: MIT OR Apache-2.0
//! Color output configuration for terminal styling.
//!
//! Supports the [`NO_COLOR`](https://no-color.org/) convention and the
//! classic `CLICOLOR` / `CLICOLOR_FORCE` environment variables.
//!
//! # Resolution order (highest priority first)
//!
//! 1. Explicit `--color` / `--no-color` CLI flag
//! 2. `NO_COLOR` environment variable (any non-empty value disables colour)
//! 3. `CLICOLOR_FORCE=1` environment variable (forces colour on)
//! 4. `CLICOLOR=0` environment variable (forces colour off)
//! 5. `Auto` — colour only when stdout is a terminal (TTY)

use std::fmt;

/// Colour output mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorChoice {
    /// Colour only when stdout is a terminal (the default).
    #[default]
    Auto,
    /// Always emit ANSI colour sequences.
    Always,
    /// Never emit ANSI colour sequences.
    Never,
}

impl ColorChoice {
    /// Determine the effective colour mode from environment variables.
    ///
    /// Respects `NO_COLOR`, `CLICOLOR_FORCE`, and `CLICOLOR` in that
    /// order.  Use this when no explicit `--color` flag was given.
    pub fn from_env() -> Self {
        // NO_COLOR: any non-empty value → disable colour
        let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
        if no_color {
            return Self::Never;
        }

        // CLICOLOR_FORCE=1 → force colour on
        if let Ok(v) = std::env::var("CLICOLOR_FORCE") {
            if v == "1" {
                return Self::Always;
            }
        }

        // CLICOLOR=0 → force colour off
        if let Ok(v) = std::env::var("CLICOLOR") {
            if v == "0" {
                return Self::Never;
            }
        }

        Self::Auto
    }

    /// Returns `true` if colour output should be emitted.
    ///
    /// When `Auto`, this checks whether stdout is a terminal (TTY).
    pub fn want_color(self) -> bool {
        match self {
            Self::Always => true,
            Self::Never => false,
            Self::Auto => {
                // Saferm: no `atty` dependency – use `libc::isatty` directly.
                // SAFETY: `libc::isatty` is safe with valid fd numbers.
                unsafe { libc::isatty(libc::STDOUT_FILENO) == 1 }
            }
        }
    }

    /// Return an ANSI style string for the given colour code when
    /// colour is enabled, or an empty string otherwise.
    pub fn ansi<'a>(&self, code: &'a str) -> &'a str {
        if self.want_color() {
            code
        } else {
            ""
        }
    }

    /// Return the ANSI reset sequence when colour is enabled.
    pub fn reset(&self) -> &str {
        if self.want_color() {
            "\x1b[0m"
        } else {
            ""
        }
    }
}

impl fmt::Display for ColorChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::Always => write!(f, "always"),
            Self::Never => write!(f, "never"),
        }
    }
}

impl std::str::FromStr for ColorChoice {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "auto" => Ok(Self::Auto),
            "always" | "yes" | "true" => Ok(Self::Always),
            "never" | "no" | "false" => Ok(Self::Never),
            _ => Err("expected one of: auto, always, never"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display() {
        assert_eq!(ColorChoice::Auto.to_string(), "auto");
        assert_eq!(ColorChoice::Always.to_string(), "always");
        assert_eq!(ColorChoice::Never.to_string(), "never");
    }

    #[test]
    fn test_from_str() {
        assert_eq!("auto".parse::<ColorChoice>().unwrap(), ColorChoice::Auto);
        assert_eq!("always".parse::<ColorChoice>().unwrap(), ColorChoice::Always);
        assert_eq!("never".parse::<ColorChoice>().unwrap(), ColorChoice::Never);
        assert_eq!("yes".parse::<ColorChoice>().unwrap(), ColorChoice::Always);
        assert_eq!("no".parse::<ColorChoice>().unwrap(), ColorChoice::Never);
        assert!("bogus".parse::<ColorChoice>().is_err());
    }

    #[test]
    fn test_want_color_always() {
        assert!(ColorChoice::Always.want_color());
    }

    #[test]
    fn test_want_color_never() {
        assert!(!ColorChoice::Never.want_color());
    }

    #[test]
    fn test_ansi_reset_in_always() {
        let cc = ColorChoice::Always;
        assert_eq!(cc.ansi("\x1b[31m"), "\x1b[31m");
        assert_eq!(cc.reset(), "\x1b[0m");
    }

    #[test]
    fn test_ansi_reset_in_never() {
        let cc = ColorChoice::Never;
        assert_eq!(cc.ansi("\x1b[31m"), "");
        assert_eq!(cc.reset(), "");
    }

    #[test]
    fn test_from_env_respects_no_color() {
        temp_env::with_var("NO_COLOR", Some("1"), || {
            assert_eq!(ColorChoice::from_env(), ColorChoice::Never);
        });
    }

    #[test]
    fn test_from_env_respects_clicolor_force() {
        temp_env::with_vars(
            vec![("NO_COLOR", None::<&str>), ("CLICOLOR_FORCE", Some("1"))],
            || {
                assert_eq!(ColorChoice::from_env(), ColorChoice::Always);
            },
        );
    }

    #[test]
    fn test_from_env_respects_clicolor_off() {
        temp_env::with_vars(
            vec![
                ("NO_COLOR", None::<&str>),
                ("CLICOLOR_FORCE", None::<&str>),
                ("CLICOLOR", Some("0")),
            ],
            || {
                assert_eq!(ColorChoice::from_env(), ColorChoice::Never);
            },
        );
    }

    #[test]
    fn test_from_env_default_auto() {
        temp_env::with_vars(
            vec![
                ("NO_COLOR", None::<&str>),
                ("CLICOLOR_FORCE", None::<&str>),
                ("CLICOLOR", None::<&str>),
            ],
            || {
                assert_eq!(ColorChoice::from_env(), ColorChoice::Auto);
            },
        );
    }

    #[test]
    fn test_from_env_no_color_takes_priority() {
        temp_env::with_vars(vec![("NO_COLOR", Some("1")), ("CLICOLOR_FORCE", Some("1"))], || {
            // NO_COLOR takes priority over CLICOLOR_FORCE
            assert_eq!(ColorChoice::from_env(), ColorChoice::Never);
        });
    }
}
