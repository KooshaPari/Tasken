// SPDX-License-Identifier: MIT OR Apache-2.0
//! CLI adapter for command-line interaction.

use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use crate::adapters::primary::ColorChoice;
use crate::application::visualize::{generate_dot, generate_mermaid, GraphFormat};
use crate::application::{
    compose_command, watcher::FileWatcher, CreateTask, ForwardedArgs, ListTasks, TaskService,
};
use crate::domain::errors::TaskError;
use crate::domain::groups::Group;
use crate::domain::rate_limiter::{parse_rate_limit, TokenBucket};
use crate::domain::tasks::{Priority, TaskId, TaskState};
use crate::domain::workflows::{Workflow, WorkflowStep};
use crate::infrastructure::observability;

/// CLI arguments.
#[derive(Parser, Debug)]
#[command(name = "taskkit")]
#[command(about = "Universal task execution framework")]
#[command(subcommand_required = false, arg_required_else_help = true)]
pub struct Cli {
    /// Print what would be done without executing.
    #[arg(long, global = true)]
    pub dry_run: bool,
    /// Suppress output.
    #[arg(short = 's', long, global = true)]
    pub silent: bool,
    /// Maximum number of tasks that can run concurrently (rate limiter capacity).
    #[arg(long, global = true, default_value = "0")]
    pub max_concurrency: u64,
    /// Rate limit for dispatch, e.g. "10/s", "60/m", "3600/h".
    /// When set, task execution will be throttled to this rate.
    #[arg(long, global = true)]
    pub rate_limit: Option<String>,
    /// Colour output mode: auto, always, never.
    ///
    /// When not set, respects the `NO_COLOR`, `CLICOLOR_FORCE`, and
    /// `CLICOLOR` environment variables.
    #[arg(long, global = true, default_value = "auto", value_parser = clap::value_parser!(ColorChoice))]
    pub color: ColorChoice,
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// CLI commands.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Show task summary (default action when no subcommand is given).
    Default,
    /// Create a new task.
    Create {
        /// Task name.
        #[arg(long)]
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
        #[arg(long)]
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
    /// Group commands.
    Group {
        #[command(subcommand)]
        command: GroupCommand,
    },
    /// Watch a directory for changes and re-run the default action.
    Watch {
        /// Path to watch (file or directory).
        #[arg(short, long, default_value = ".")]
        path: String,
        /// Debounce interval in milliseconds.
        #[arg(short, long, default_value = "500")]
        debounce_ms: u64,
    },
    /// Generate a dependency graph from a recipe file.
    Graph {
        /// Path to the recipe file (TOML or YAML).
        recipe_file: String,
        /// Output format: dot or mermaid (default: dot).
        #[arg(long, default_value = "dot")]
        format: String,
    },
    /// Run a local health check.
    Health,
    /// Alias for `health` for readiness probes.
    Ready,
}

/// Workflow subcommands.
#[derive(Subcommand, Debug, Clone)]
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

/// Group subcommands.
#[derive(Subcommand, Debug, Clone)]
pub enum GroupCommand {
    /// Create a new group.
    Create {
        /// Group name.
        name: String,
        /// Optional description.
        #[arg(short, long)]
        description: Option<String>,
    },
    /// List all groups.
    List,
    /// Show group details.
    Show {
        /// Group name.
        name: String,
    },
    /// Run all tasks in a group.
    Run {
        /// Group name.
        name: String,
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
            "scheduled" => Some(TaskState::Scheduled),
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
        // Auto-load .env file (no error if missing)
        let _ = dotenvy::dotenv();

        let cli = Cli::parse();
        observability::metrics().record_command_started();

        // Resolve effective color choice: explicit CLI flag takes priority,
        // otherwise fall back to env vars (NO_COLOR, CLICOLOR_FORCE, CLICOLOR).
        let _color =
            if cli.color == ColorChoice::Auto { ColorChoice::from_env() } else { cli.color };

        if cli.dry_run && !cli.silent {
            eprintln!("[dry-run] would execute command");
        }

        // Attach rate limiter to service if --rate-limit or --max-concurrency is set.
        if cli.rate_limit.is_some() || cli.max_concurrency > 0 {
            // Resolve capacity: use --max-concurrency if explicitly set (>0),
            // otherwise fall back to a sensible default derived from the rate limit.
            let capacity = if cli.max_concurrency > 0 {
                cli.max_concurrency
            } else {
                // Default capacity = rate_limit (rounded up), min 1.
                if let Some(ref rl) = cli.rate_limit {
                    parse_rate_limit(rl).map(|r| (r.ceil() as u64).max(1)).unwrap_or(10)
                } else {
                    10
                }
            };

            let refill_rate =
                cli.rate_limit.as_ref().and_then(|rl| parse_rate_limit(rl)).unwrap_or(10.0);

            let bucket = TokenBucket::new(capacity, refill_rate, None);
            service.set_rate_limiter(bucket).await;

            if !cli.silent && capacity > 0 {
                eprintln!("[rate-limiter] capacity={capacity}, rate={refill_rate}/s");
            }
        }

        let outcome =
            Self::handle_command(cli, service).await.context("CLI command execution failed");
        observability::metrics().record_command_finished(outcome.is_ok());
        outcome?;
        Ok(())
    }

    async fn handle_command(cli: Cli, service: Arc<TaskService>) -> anyhow::Result<()> {
        match cli.command {
            None | Some(Command::Default) => {
                // Default: show task summary
                let tasks = service.list_tasks(None, None, None).await?;
                if cli.dry_run && !cli.silent {
                    eprintln!("[dry-run] would list {} tasks", tasks.len());
                }
                if !cli.silent {
                    let by_state = [
                        ("pending", TaskState::Pending),
                        ("running", TaskState::Running),
                        ("completed", TaskState::Completed),
                        ("failed", TaskState::Failed),
                        ("cancelled", TaskState::Cancelled),
                    ];
                    for (label, state) in &by_state {
                        let count = tasks.iter().filter(|t| t.state == *state).count();
                        println!("  {label:>10}: {count}");
                    }
                    println!("  {:>10}: {}", "total", tasks.len());
                }
            }
            Some(Command::Create { name, description, priority, timeout, tag, command }) => {
                if cli.dry_run {
                    if !cli.silent {
                        eprintln!("[dry-run] would create task '{name}'");
                        if let Some(c) = &command {
                            eprintln!("[dry-run]   command: {c}");
                        }
                    }
                    return Ok(());
                }
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

                let task = cmd.execute(&service).await?;
                if !cli.silent {
                    println!("{}", serde_json::to_string_pretty(&task).unwrap());
                }
            }
            Some(Command::List { state, tag, limit }) => {
                let state_filter = state.and_then(|s| Cli::parse_state(&s));
                let query = ListTasks::new().with_limit(limit);
                let query = match (state_filter, tag) {
                    (Some(s), Some(t)) => query.with_state(s).with_tag(t),
                    (Some(s), None) => query.with_state(s),
                    (None, Some(t)) => query.with_tag(t),
                    (None, None) => query,
                };
                let tasks = query.execute(&service).await?;
                if !cli.silent {
                    println!("{}", serde_json::to_string_pretty(&tasks).unwrap());
                }
            }
            Some(Command::Get { id }) => {
                let task = service.get_task(&TaskId::from_string(id.clone())).await?;
                match task {
                    Some(t) => {
                        if !cli.silent {
                            println!("{}", serde_json::to_string_pretty(&t).unwrap());
                        }
                    }
                    None => return Err(TaskError::NotFound(id).into()),
                }
            }
            Some(Command::Cancel { id, reason }) => {
                if cli.dry_run {
                    if !cli.silent {
                        eprintln!("[dry-run] would cancel task '{id}'");
                    }
                    return Ok(());
                }
                let task_id = TaskId::from_string(id);
                service.cancel_task(task_id, reason).await?;
                if !cli.silent {
                    println!("Task cancelled");
                }
            }
            Some(Command::Run { id, args }) => {
                if cli.dry_run {
                    if !cli.silent {
                        eprintln!("[dry-run] would run task '{id}'");
                    }
                    return Ok(());
                }
                let task_id = TaskId::from_string(id);
                if args.is_empty() {
                    // Standard run using the service (with cache)
                    let result = service.run_task(&task_id, cli.dry_run).await?;
                    if !cli.silent {
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    }
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
                    let base =
                        task.data.get("command").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let composed = compose_command(&base, &forwarded);
                    task.data = serde_json::json!({"command": composed});
                    crate::domain::run_with_streams(&mut task, !cli.silent)
                        .context("stream execution failed")?;
                }
            }
            Some(Command::RunArgs { id, args }) => {
                if cli.dry_run {
                    if !cli.silent {
                        eprintln!("[dry-run] would run task '{id}' with args: {args:?}");
                    }
                    return Ok(());
                }
                // Argument forwarding: append shell-quoted args to the
                // existing task command and execute as a one-off run.
                let task_id = TaskId::from_string(id);
                let mut task = service
                    .get_task(&task_id)
                    .await?
                    .ok_or_else(|| TaskError::NotFound(task_id.0.clone()))?;
                let forwarded = ForwardedArgs::from_slice(&args);
                let base =
                    task.data.get("command").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let composed = compose_command(&base, &forwarded);
                task.data = serde_json::json!({"command": composed});
                crate::domain::run_with_streams(&mut task, !cli.silent)?;
            }
            Some(Command::CreateRaw {
                name,
                description,
                priority,
                timeout,
                tag,
                command,
                args,
            }) => {
                if cli.dry_run {
                    if !cli.silent {
                        eprintln!("[dry-run] would create task '{name}'");
                        if let Some(c) = &command {
                            eprintln!("[dry-run]   command: {c}");
                        }
                    }
                    return Ok(());
                }
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
                let task = cmd.execute(&service).await?;
                if !cli.silent {
                    println!("{}", serde_json::to_string_pretty(&task).unwrap());
                }
            }
            Some(Command::Workflow { ref command }) => {
                Self::handle_workflow_command(&cli, command.clone(), service).await?;
            }
            Some(Command::Watch { path, debounce_ms }) => {
                if cli.dry_run {
                    if !cli.silent {
                        eprintln!("[dry-run] would watch path '{path}'");
                    }
                    return Ok(());
                }
                let watch_path = std::path::PathBuf::from(&path);
                let watcher = FileWatcher::new().with_debounce(debounce_ms);
                let svc = service.clone();
                watcher
                    .watch_and_run(&watch_path, move || {
                        let rt = tokio::runtime::Runtime::new()
                            .expect("failed to create runtime for watcher callback");
                        rt.block_on(async {
                            let tasks = svc.list_tasks(None, None, None).await;
                            match tasks {
                                Ok(tasks) => {
                                    let by_state = [
                                        ("pending", crate::domain::tasks::TaskState::Pending),
                                        ("running", crate::domain::tasks::TaskState::Running),
                                        ("completed", crate::domain::tasks::TaskState::Completed),
                                        ("failed", crate::domain::tasks::TaskState::Failed),
                                        ("cancelled", crate::domain::tasks::TaskState::Cancelled),
                                    ];
                                    for (label, state) in &by_state {
                                        let count =
                                            tasks.iter().filter(|t| t.state == *state).count();
                                        println!("  {label:>10}: {count}");
                                    }
                                    println!("  {:>10}: {}", "total", tasks.len());
                                }
                                Err(e) => eprintln!("[watcher] failed to list tasks: {e}"),
                            }
                        });
                    })
                    .map_err(|e| anyhow::anyhow!("file watcher error: {e}"))?;
            }
            Some(Command::Graph { recipe_file, format }) => {
                let fmt: GraphFormat = format
                    .parse()
                    .map_err(|e: String| anyhow::anyhow!(e))
                    .context("Invalid graph format")?;
                let path = std::path::Path::new(&recipe_file);
                let recipe = crate::domain::recipe::TaskenfileParser::parse_file(path)
                    .context("Failed to parse recipe file")?;
                let output = match fmt {
                    GraphFormat::Dot => generate_dot(&recipe.tasks),
                    GraphFormat::Mermaid => generate_mermaid(&recipe.tasks),
                };
                if !cli.silent {
                    print!("{output}");
                }
            }
            Some(Command::Health) | Some(Command::Ready) => {
                observability::metrics().record_health_check();
                let storage_ok = service.list_tasks(None, None, Some(1)).await.is_ok();
                let ready = storage_ok;
                if !cli.silent {
                    let payload = serde_json::json!({
                        "status": if ready { "ok" } else { "degraded" },
                        "ready": ready,
                        "storage": storage_ok,
                    });
                    println!("{}", serde_json::to_string_pretty(&payload).unwrap());
                }
                if !ready {
                    return Err(anyhow::anyhow!("health check failed"));
                }
            }
            Some(Command::Group { ref command }) => {
                Self::handle_group_command(&cli, command.clone(), service).await?;
            }
        }
        Ok(())
    }

    async fn handle_workflow_command(
        cli: &Cli,
        command: WorkflowCommand,
        service: Arc<TaskService>,
    ) -> Result<(), TaskError> {
        match command {
            WorkflowCommand::Create { name } => {
                if cli.dry_run {
                    if !cli.silent {
                        eprintln!("[dry-run] would create workflow '{name}'");
                    }
                    return Ok(());
                }
                let workflow = Workflow::new(name);
                let created = service.create_workflow(workflow).await?;
                if !cli.silent {
                    println!("{}", serde_json::to_string_pretty(&created).unwrap());
                }
            }
            WorkflowCommand::List => {
                let workflows = service.list_workflows().await?;
                if !cli.silent {
                    println!("{}", serde_json::to_string_pretty(&workflows).unwrap());
                }
            }
            WorkflowCommand::Get { id } => {
                use crate::domain::workflows::WorkflowId;
                let workflow = service.get_workflow(&WorkflowId::from_string(id.clone())).await?;
                match workflow {
                    Some(w) => {
                        if !cli.silent {
                            println!("{}", serde_json::to_string_pretty(&w).unwrap());
                        }
                    }
                    None => return Err(TaskError::NotFound(id)),
                }
            }
            WorkflowCommand::AddStep { workflow_id, name, task_id, depends_on } => {
                if cli.dry_run {
                    if !cli.silent {
                        eprintln!("[dry-run] would add step '{name}' to workflow '{workflow_id}'");
                    }
                    return Ok(());
                }
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
                if !cli.silent {
                    println!("{}", serde_json::to_string_pretty(&updated).unwrap());
                }
            }
            WorkflowCommand::Run { id } => {
                use crate::domain::workflows::WorkflowId;
                let w_id = WorkflowId::from_string(id);
                let results = service.execute_workflow(&w_id, cli.dry_run).await?;
                if !cli.silent {
                    println!("{}", serde_json::to_string_pretty(&results).unwrap());
                }
            }
        }
        Ok(())
    }

    async fn handle_group_command(
        cli: &Cli,
        command: GroupCommand,
        service: Arc<TaskService>,
    ) -> Result<(), TaskError> {
        match command {
            GroupCommand::Create { name, description } => {
                if cli.dry_run {
                    if !cli.silent {
                        eprintln!("[dry-run] would create group '{name}'");
                    }
                    return Ok(());
                }
                let mut group = Group::new(name);
                if let Some(desc) = description {
                    group = group.with_description(desc);
                }
                let created = service.create_group(group).await?;
                if !cli.silent {
                    println!("{}", serde_json::to_string_pretty(&created).unwrap());
                }
            }
            GroupCommand::List => {
                let groups = service.list_groups().await?;
                if !cli.silent {
                    println!("{}", serde_json::to_string_pretty(&groups).unwrap());
                }
            }
            GroupCommand::Show { name } => {
                // Look up by name: find in the list, then show details.
                let groups = service.list_groups().await?;
                let group = groups
                    .into_iter()
                    .find(|g| g.name == name)
                    .ok_or_else(|| TaskError::NotFound(name.clone()))?;
                if !cli.silent {
                    println!("{}", serde_json::to_string_pretty(&group).unwrap());
                }
            }
            GroupCommand::Run { name } => {
                if cli.dry_run {
                    if !cli.silent {
                        eprintln!("[dry-run] would run group '{name}'");
                    }
                    return Ok(());
                }
                // Look up by name.
                let groups = service.list_groups().await?;
                let group = groups
                    .into_iter()
                    .find(|g| g.name == name)
                    .ok_or_else(|| TaskError::NotFound(name.clone()))?;
                let results = service.run_group(&group.id, cli.dry_run).await?;
                if !cli.silent {
                    println!("{}", serde_json::to_string_pretty(&results).unwrap());
                }
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
            Some(Command::CreateRaw { name, priority, command, args, .. }) => {
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
            Some(Command::RunArgs { id, args }) => {
                assert_eq!(id, "abc-123");
                assert_eq!(
                    args,
                    vec!["--release".to_string(), "--".to_string(), "nested-arg".to_string()]
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
            Some(Command::CreateRaw { command, args, .. }) => {
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
            Some(Command::Run { id, args }) => {
                assert_eq!(id, "abc-123");
                assert_eq!(args, vec!["--release".to_string(), "--target=x86_64".to_string(),]);
            }
            other => panic!("expected Run, got {other:?}"),
        }
    }

    #[test]
    fn test_cli_parses_run_without_forwarded_args() {
        let cli = Cli::try_parse_from(["taskkit", "run", "--id", "abc-123"])
            .expect("parse should succeed");

        match cli.command {
            Some(Command::Run { id, args }) => {
                assert_eq!(id, "abc-123");
                assert!(args.is_empty());
            }
            other => panic!("expected Run, got {other:?}"),
        }
    }

    #[test]
    fn test_cli_default_color_is_auto() {
        let cli = Cli::try_parse_from(["taskkit", "default"]).expect("parse should succeed");
        assert_eq!(cli.color, ColorChoice::Auto);
    }

    #[test]
    fn test_cli_color_flag_always() {
        let cli =
            Cli::try_parse_from(["taskkit", "--color", "always"]).expect("parse should succeed");
        assert_eq!(cli.color, ColorChoice::Always);
    }

    #[test]
    fn test_cli_color_flag_never() {
        let cli =
            Cli::try_parse_from(["taskkit", "--color", "never"]).expect("parse should succeed");
        assert_eq!(cli.color, ColorChoice::Never);
    }

    #[test]
    fn test_cli_color_flag_invalid_rejected() {
        let res = Cli::try_parse_from(["taskkit", "--color", "bogus"]);
        assert!(res.is_err());
    }
}
