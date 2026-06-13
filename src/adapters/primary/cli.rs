//! CLI adapter for command-line interaction.

use crate::application::{CreateTask, ListTasks, TaskService};
use crate::domain::errors::TaskError;
use crate::domain::tasks::{Priority, TaskId, TaskState};
use crate::domain::workflows::{Workflow, WorkflowStep};
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
    Run {
        /// Task ID.
        #[arg(short, long)]
        id: String,
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
    pub async fn run(service: Arc<TaskService>) {
        let cli = Cli::parse();

        if let Err(e) = Self::handle_command(cli, service).await {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }

    async fn handle_command(cli: Cli, service: Arc<TaskService>) -> Result<(), TaskError> {
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
                    None => return Err(TaskError::NotFound(id)),
                }
            }
            Command::Cancel { id, reason } => {
                let task_id = TaskId::from_string(id);
                service.cancel_task(task_id, reason).await?;
                println!("Task cancelled");
            }
            Command::Run { id } => {
                let task_id = TaskId::from_string(id);
                let result = service.run_task(&task_id).await?;
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
                if !result.success {
                    std::process::exit(1);
                }
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
}
