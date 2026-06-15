//! CLI adapter for command-line interaction.

use crate::application::{
    compose_command, CreateTask, ForwardedArgs, ListTasks, TaskService,
};
use crate::domain::errors::TaskError;
use crate::domain::tasks::{Priority, TaskId, TaskState};
use crate::domain::workflows::{Workflow, WorkflowStep};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::sync::Arc;

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
        #[arg(short = 'T', long)]
        timeout: Option<u64>,
        /// Tags (can be specified multiple times).
        #[arg(short, long)]
        tag: Vec<String>,
        /// Shell command to execute.
        #[arg(short, long)]
        command: Option<String>,
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
    /// Run a task by ID (shell execution).
    ///
    /// Forwarded args: everything after `--` is appended to the task's
    /// shell command (if the task has a `command` field in its data).
    ///
    /// Example:
    ///   taskkit run --id build -- --release --target=x86_64
    Run {
        /// Task ID.
        #[arg(short, long)]
        id: String,
        /// Forwarded arguments (after `--`). Appended to the task command.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Run a task with raw arguments forwarded after `--`.
    ///
    /// This is a SOTA-style "args passthrough" command: anything after
    /// the literal `--` separator is captured as forwarded arguments
    /// and appended to the task's command. This avoids flag collisions
    /// with the tasken's own flags.
    ///
    /// Example:
    ///   taskkit run-args --id build -- --release --target=x86_64
    RunArgs {
        /// Task ID.
        #[arg(short, long)]
        id: String,
        /// Raw arguments forwarded to the task. Everything after `--`
        /// is captured as-is; hyphen-prefixed values are preserved.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Create a task from a raw command line. Anything after `--` is
    /// appended verbatim to the command.
    CreateRaw {
        /// Task name.
        #[arg(short, long)]
        name: String,
        /// Optional task description.
        #[arg(short, long)]
        description: Option<String>,
        /// Priority level (low, normal, high, critical).
        #[arg(short, long, default_value = "normal")]
        priority: String,
        /// Timeout in seconds.
        #[arg(short = 'T', long)]
        timeout: Option<u64>,
        /// Tags (can be specified multiple times).
        #[arg(short, long)]
        tag: Vec<String>,
        /// Base command (executable + its own arguments). When `--` is
        /// present, forwarded args are appended after shell-quoting.
        #[arg(short, long)]
        command: Option<String>,
        /// Forwarded args (after `--`). Hyphenated values are preserved.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Workflow commands.
    Workflow {
        #[command(subcommand)]
        command: WorkflowCommand,
    },
}

/// Workflow subcommands.
#[derive(Subcommand, Debug)]
pub enum WorkflowCommand {
    /// Create a new workflow.
    Create {
        /// Workflow name.
        #[arg(short, long)]
        name: String,
    },
    /// List workflows.
    List,
    /// Get a workflow by ID.
    Get {
        /// Workflow ID.
        #[arg(short, long)]
        id: String,
    },
    /// Add a step to a workflow.
    AddStep {
        /// Workflow ID.
        #[arg(short, long)]
        workflow_id: String,
        /// Step name.
        #[arg(short, long)]
        name: String,
        /// Task ID to execute.
        #[arg(short, long)]
        task_id: String,
        /// Dependencies (step names).
        #[arg(short, long)]
        depends_on: Vec<String>,
    },
    /// Run a workflow.
    Run {
        /// Workflow ID.
        #[arg(short, long)]
        id: String,
    },
}

impl Cli {
    /// Parse the command line.
    pub fn parse() -> Self {
        <Self as clap::Parser>::parse()
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
    pub async fn run(service: Arc<TaskService>) -> anyhow::Result<()> {
        let cli = Cli::parse();

        Self::handle_command(cli, service)
            .await
            .context("CLI command execution failed")?;
        Ok(())
    }

    async fn handle_command(cli: Cli, service: Arc<TaskService>) -> anyhow::Result<()> {
        match cli.command {
            Command::Create {
                name,
                description,
                priority,
                timeout,
                tag,
                command,
            } => {
                let priority = Cli::parse_priority(&priority);

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
                if let Some(c) = command {
                    cmd = cmd.with_command(c);
                }

                let task = cmd.execute(&*service).await?;
                println!("{}", serde_json::to_string_pretty(&task).unwrap());
            }
            Command::List { state, tag, limit } => {
                let state_filter = state.and_then(|s| Cli::parse_state(&s));
                let query = ListTasks::new().with_limit(limit);
                let query = match (state_filter, tag) {
                    (Some(s), Some(t)) => query.with_state(s).with_tag(t),
                    (Some(s), None) => query.with_state(s),
                    (None, Some(t)) => query.with_tag(t),
                    (None, None) => query,
                };
                let tasks = query.execute(&*service).await?;
                println!("{}", serde_json::to_string_pretty(&tasks).unwrap());
            }
            Command::Get { id } => {
                let task = service.get_task(&TaskId::from_string(id.clone())).await?;
                match task {
                    Some(t) => println!("{}", serde_json::to_string_pretty(&t).unwrap()),
                    None => return Err(TaskError::NotFound(id).into()),
                }
            }
            Command::Cancel { id, reason } => {
                let task_id = TaskId::from_string(id);
                service.cancel_task(task_id, reason).await?;
                println!("Task cancelled");
            }
            Command::Run { id, args } => {
                let task_id = TaskId::from_string(id);
                if args.is_empty() {
                    // Standard run using the service (with cache)
                    let result = service.run_task(&task_id).await?;
                    println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    if !result.success {
                        std::process::exit(1);
                    }
                } else {
                    // Argument forwarding: compose and run with streams
                    let mut task = service
                        .get_task(&task_id)
                        .await?
                        .ok_or_else(|| TaskError::NotFound(task_id.0.clone()))?;
                    let forwarded = ForwardedArgs::from_slice(&args);
                    let base = task
                        .data
                        .get("command")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let composed = compose_command(&base, &forwarded);
                    task.data = serde_json::json!({"command": composed});
                    crate::domain::run_with_streams(&mut task, true).context("stream execution failed")?;
                }
            }
            Command::RunArgs { id, args } => {
                // Argument forwarding: append shell-quoted args to the
                // existing task command and execute as a one-off run.
                let task_id = TaskId::from_string(id);
                let mut task = service
                    .get_task(&task_id)
                    .await?
                    .ok_or_else(|| TaskError::NotFound(task_id.0.clone()))?;
                let forwarded = ForwardedArgs::from_slice(&args);
                let base = task
                    .data
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let composed = compose_command(&base, &forwarded);
                task.data = serde_json::json!({"command": composed});
                crate::domain::run_with_streams(&mut task, true)?;
            }
            Command::CreateRaw {
                name,
                description,
                priority,
                timeout,
                tag,
                command,
                args,
            } => {
                let priority = Cli::parse_priority(&priority);
                let forwarded = ForwardedArgs::from_slice(&args);
                let composed = match command {
                    Some(base) => compose_command(&base, &forwarded),
                    None => forwarded.shell_quote(),
                };
                let mut cmd = CreateTask::new(name).with_command(composed);
                if let Some(desc) = description {
                    cmd = cmd.with_description(desc);
                }
                cmd = cmd.with_priority(priority);
                if let Some(t) = timeout {
                    cmd = cmd.with_timeout(t);
                }
                for tg in tag {
                    cmd = cmd.with_tag(tg);
                }
                let task = cmd.execute(&*service).await?;
                println!("{}", serde_json::to_string_pretty(&task).unwrap());
            }
            Command::Workflow { command } => {
                Self::handle_workflow_command(command, service).await?;
            }
        }
        Ok(())
    }

    async fn handle_workflow_command(
        command: WorkflowCommand,
        service: Arc<TaskService>,
    ) -> Result<(), TaskError> {
        match command {
            WorkflowCommand::Create { name } => {
                let workflow = Workflow::new(name);
                let created = service.create_workflow(workflow).await?;
                println!("{}", serde_json::to_string_pretty(&created).unwrap());
            }
            WorkflowCommand::List => {
                let workflows = service.list_workflows().await?;
                println!("{}", serde_json::to_string_pretty(&workflows).unwrap());
            }
            WorkflowCommand::Get { id } => {
                use crate::domain::workflows::WorkflowId;
                let workflow = service.get_workflow(&WorkflowId::from_string(id.clone())).await?;
                match workflow {
                    Some(w) => println!("{}", serde_json::to_string_pretty(&w).unwrap()),
                    None => return Err(TaskError::NotFound(id)),
                }
            }
            WorkflowCommand::AddStep {
                workflow_id,
                name,
                task_id,
                depends_on,
            } => {
                use crate::domain::workflows::WorkflowId;
                let w_id = WorkflowId::from_string(workflow_id);
                let mut workflow = service
                    .get_workflow(&w_id)
                    .await?
                    .ok_or_else(|| TaskError::NotFound(w_id.0.clone()))?;
                let mut step = WorkflowStep::new(name).with_task(TaskId::from_string(task_id));
                for dep in depends_on {
                    step = step.with_dependency(dep);
                }
                workflow = workflow.with_step(step);
                let updated = service.create_workflow(workflow).await?;
                println!("{}", serde_json::to_string_pretty(&updated).unwrap());
            }
            WorkflowCommand::Run { id } => {
                use crate::domain::workflows::WorkflowId;
                let w_id = WorkflowId::from_string(id);
                let results = service.execute_workflow(&w_id).await?;
                println!("{}", serde_json::to_string_pretty(&results).unwrap());
            }
        }
        Ok(())
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

    #[test]
    fn test_cli_parses_create_raw_with_forwarded_args() {
        let cli = Cli::try_parse_from([
            "taskkit",
            "create-raw",
            "--name",
            "build",
            "--priority",
            "high",
            "--command",
            "cargo build",
            "--",
            "--release",
            "--target=x86_64",
            "--features",
            "tokio,serde",
        ])
        .expect("parse should succeed");

        match cli.command {
            Command::CreateRaw {
                name,
                priority,
                command,
                args,
                ..
            } => {
                assert_eq!(name, "build");
                assert_eq!(priority, "high");
                assert_eq!(command.as_deref(), Some("cargo build"));
                assert_eq!(
                    args,
                    vec![
                        "--release".to_string(),
                        "--target=x86_64".to_string(),
                        "--features".to_string(),
                        "tokio,serde".to_string(),
                    ]
                );
            }
            other => panic!("expected CreateRaw, got {other:?}"),
        }
    }

    #[test]
    fn test_cli_parses_run_args_with_forwarded() {
        let cli = Cli::try_parse_from([
            "taskkit",
            "run-args",
            "--id",
            "abc-123",
            "--",
            "--release",
            "--",
            "nested-arg",
        ])
        .expect("parse should succeed");

        match cli.command {
            Command::RunArgs { id, args } => {
                assert_eq!(id, "abc-123");
                assert_eq!(
                    args,
                    vec![
                        "--release".to_string(),
                        "--".to_string(),
                        "nested-arg".to_string()
                    ]
                );
            }
            other => panic!("expected RunArgs, got {other:?}"),
        }
    }

    #[test]
    fn test_cli_create_raw_without_command() {
        let cli = Cli::try_parse_from([
            "taskkit",
            "create-raw",
            "--name",
            "echo-task",
            "--",
            "echo",
            "hello world",
        ])
        .expect("parse should succeed");
        match cli.command {
            Command::CreateRaw { command, args, .. } => {
                assert!(command.is_none());
                assert_eq!(args, vec!["echo".to_string(), "hello world".to_string()]);
            }
            other => panic!("expected CreateRaw, got {other:?}"),
        }
    }

    #[test]
    fn test_cli_rejects_missing_required_name() {
        let res = Cli::try_parse_from(["taskkit", "create-raw", "--", "echo"]);
        assert!(res.is_err());
    }

    #[test]
    fn test_cli_parses_run_with_forwarded_args() {
        let cli = Cli::try_parse_from([
            "taskkit",
            "run",
            "--id",
            "abc-123",
            "--",
            "--release",
            "--target=x86_64",
        ])
        .expect("parse should succeed");

        match cli.command {
            Command::Run { id, args } => {
                assert_eq!(id, "abc-123");
                assert_eq!(
                    args,
                    vec![
                        "--release".to_string(),
                        "--target=x86_64".to_string(),
                    ]
                );
            }
            other => panic!("expected Run, got {other:?}"),
        }
    }

    #[test]
    fn test_cli_parses_run_without_forwarded_args() {
        let cli = Cli::try_parse_from([
            "taskkit",
            "run",
            "--id",
            "abc-123",
        ])
        .expect("parse should succeed");

        match cli.command {
            Command::Run { id, args } => {
                assert_eq!(id, "abc-123");
                assert!(args.is_empty());
            }
            other => panic!("expected Run, got {other:?}"),
        }
    }
}
