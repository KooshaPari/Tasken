//! CLI adapter for command-line interaction.

use clap::{Parser, Subcommand};
use crate::application::{CreateTask, ListTasks};
use crate::domain::{TaskState, Priority};

/// CLI arguments.
#[derive(Parser, Debug)]
#[command(name = "taskkit")]
#[command(about = "Universal task execution framework")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

/// CLI commands.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Create a new task.
    Create {
        /// Task name.
        #[arg(short, long)]
        name: String,
        /// Task description.
        #[arg(short, long)]
        description: Option<String>,
        /// Priority level (low, normal, high, critical).
        #[arg(short, long, default_value = "normal")]
        priority: String,
        /// Timeout in seconds.
        #[arg(short, long)]
        timeout: Option<u64>,
        /// Tags (can be specified multiple times).
        #[arg(short, long)]
        tag: Vec<String>,
    },
    /// List tasks.
    List {
        /// Filter by state.
        #[arg(short, long)]
        state: Option<String>,
        /// Filter by tag.
        #[arg(short, long)]
        tag: Option<String>,
        /// Maximum number of results.
        #[arg(short, long, default_value = "100")]
        limit: usize,
    },
    /// Get a task by ID.
    Get {
        /// Task ID.
        #[arg(short, long)]
        id: String,
    },
    /// Cancel a task.
    Cancel {
        /// Task ID.
        #[arg(short, long)]
        id: String,
        /// Cancellation reason.
        #[arg(short, long)]
        reason: Option<String>,
    },
}

impl Cli {
    /// Parse the command line.
    pub fn parse() -> Self {
        Self {
            command: clap::Parser::parse(),
        }
    }

    /// Convert priority string to enum.
    pub fn parse_priority(s: &str) -> Priority {
        match s.to_lowercase().as_str() {
            "low" => Priority::Low,
            "high" => Priority::High,
            "critical" => Priority::Critical,
            _ => Priority::Normal,
        }
    }

    /// Convert state string to enum.
    pub fn parse_state(s: &str) -> Option<TaskState> {
        match s.to_lowercase().as_str() {
            "pending" => Some(TaskState::Pending),
            "running" => Some(TaskState::Running),
            "completed" => Some(TaskState::Completed),
            "failed" => Some(TaskState::Failed),
            "cancelled" => Some(TaskState::Cancelled),
            "retrying" => Some(TaskState::Retrying),
            _ => None,
        }
    }
}

/// CLI adapter for executing commands.
pub struct CliAdapter;

impl CliAdapter {
    /// Run the CLI.
    pub fn run() {
        let cli = Cli::parse();

        match cli.command {
            Command::Create { name, description, priority, timeout, tag } => {
                println!("Creating task: {}", name);
                let priority = Cli::parse_priority(&priority);
                let _timeout = timeout.map(std::time::Duration::from_secs);

                let mut cmd = CreateTask::new(name);
                if let Some(desc) = description {
                    cmd = cmd.with_description(desc);
                }
                cmd = cmd.with_priority(priority);
                if let Some(t) = timeout {
                    cmd = cmd.with_timeout(t);
                }
                for t in tag {
                    cmd = cmd.with_tag(t);
                }

                println!("Task created: {:?}", serde_json::to_string_pretty(&cmd).unwrap());
            }
            Command::List { state, tag, limit } => {
                println!("Listing tasks (limit: {})", limit);
                let state_filter = state.and_then(|s| Cli::parse_state(&s));
                let query = ListTasks::new()
                    .with_limit(limit);
                let query = match (state_filter, tag) {
                    (Some(s), Some(t)) => query.with_state(s).with_tag(t),
                    (Some(s), None) => query.with_state(s),
                    (None, Some(t)) => query.with_tag(t),
                    (None, None) => query,
                };
                println!("Query: {:?}", serde_json::to_string_pretty(&query).unwrap());
            }
            Command::Get { id } => {
                println!("Getting task: {}", id);
            }
            Command::Cancel { id, reason } => {
                println!("Cancelling task: {}", id);
                if let Some(r) = reason {
                    println!("Reason: {}", r);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_priority() {
        assert_eq!(Cli::parse_priority("low"), Priority::Low);
        assert_eq!(Cli::parse_priority("high"), Priority::High);
        assert_eq!(Cli::parse_priority("normal"), Priority::Normal);
        assert_eq!(Cli::parse_priority("unknown"), Priority::Normal);
    }

    #[test]
    fn test_parse_state() {
        assert_eq!(Cli::parse_state("pending"), Some(TaskState::Pending));
        assert_eq!(Cli::parse_state("running"), Some(TaskState::Running));
        assert_eq!(Cli::parse_state("unknown"), None);
    }
}
