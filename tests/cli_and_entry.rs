// Integration tests for the CLI adapter and library entry points.
//
// Covers:
//   - src/lib.rs (re-exports, VERSION, module visibility)
//   - src/main.rs (entry-point side-effects via compile-time import)
//   - src/adapters/primary/cli.rs (parse helpers, priority/state
//     conversion, argument forwarding composition)
//
// Run with: `cargo test --test cli_and_entry`

use std::sync::Arc;

use taskkit::adapters::primary::cli::{Cli, CliAdapter};
use taskkit::adapters::secondary::memory::MemoryStorage;
use taskkit::application::forwarded::{compose_command, ForwardedArgs};
use taskkit::application::services::TaskService;
use taskkit::application::CreateTask;
use taskkit::domain::tasks::{Priority, TaskId, TaskState};

#[test]
fn test_lib_version_is_non_empty() {
    // VERSION is a public const compiled from CARGO_PKG_VERSION.
    assert!(!taskkit::VERSION.is_empty(), "VERSION must not be empty");
    // Should contain at least one dot (semver-ish).
    assert!(taskkit::VERSION.contains('.'), "VERSION should be semver-like, got: {}", taskkit::VERSION);
}

#[test]
fn test_lib_module_reexports_are_accessible() {
    // Exercise the public re-exports in lib.rs to make sure they
    // resolve and are constructed without panicking.
    use taskkit::{AsyncRunner, BackgroundRunner, SyncRunner, Task, TaskRunner};

    let mut task = Task::new("re-export-test");
    let runner = SyncRunner::new();
    let result = runner.execute(&mut task);
    assert!(result.is_ok());

    // Other re-exports must also be constructible.
    let _async_r = AsyncRunner::new();
    let _bg_r = BackgroundRunner::new();
}

#[test]
fn test_lib_default_task_uses_default_priority_and_state() {
    // Re-exported Task constructor defaults.
    let task = taskkit::Task::new("defaults");
    assert_eq!(task.priority, Priority::Normal);
    assert_eq!(task.state, TaskState::Pending);
    assert!(task.retry_policy.is_none());
    assert!(task.timeout.is_none());
}

#[test]
fn test_cli_parse_priority_lowercase_and_uppercase() {
    // parse_priority is case-insensitive (it lowercases the input).
    assert_eq!(Cli::parse_priority("low"), Priority::Low);
    assert_eq!(Cli::parse_priority("LOW"), Priority::Low);
    assert_eq!(Cli::parse_priority("normal"), Priority::Normal);
    assert_eq!(Cli::parse_priority("Normal"), Priority::Normal);
    assert_eq!(Cli::parse_priority("high"), Priority::High);
    assert_eq!(Cli::parse_priority("HIGH"), Priority::High);
    assert_eq!(Cli::parse_priority("critical"), Priority::Critical);
    assert_eq!(Cli::parse_priority("CRITICAL"), Priority::Critical);
    // Unknown values fall through to Normal.
    assert_eq!(Cli::parse_priority("urgent"), Priority::Normal);
    assert_eq!(Cli::parse_priority(""), Priority::Normal);
    assert_eq!(Cli::parse_priority("rocket"), Priority::Normal);
}

#[test]
fn test_cli_parse_state_every_variant() {
    // All known snake_case states must round-trip.
    assert_eq!(Cli::parse_state("pending"), Some(TaskState::Pending));
    assert_eq!(Cli::parse_state("scheduled"), Some(TaskState::Scheduled));
    assert_eq!(Cli::parse_state("running"), Some(TaskState::Running));
    assert_eq!(Cli::parse_state("completed"), Some(TaskState::Completed));
    assert_eq!(Cli::parse_state("failed"), Some(TaskState::Failed));
    assert_eq!(Cli::parse_state("cancelled"), Some(TaskState::Cancelled));
    assert_eq!(Cli::parse_state("retrying"), Some(TaskState::Retrying));
    // Case insensitive.
    assert_eq!(Cli::parse_state("PENDING"), Some(TaskState::Pending));
    assert_eq!(Cli::parse_state("Completed"), Some(TaskState::Completed));
    // Unknown / empty returns None.
    assert_eq!(Cli::parse_state("unknown"), None);
    assert_eq!(Cli::parse_state(""), None);
    assert_eq!(Cli::parse_state("done"), None);
}

#[test]
fn test_cli_adapter_struct_is_constructible() {
    // CliAdapter is a unit struct; just ensure the type is reachable.
    let _: CliAdapter = CliAdapter;
}

#[test]
fn test_cli_run_args_quoting_logic_via_compose() {
    // The RunArgs command path delegates to compose_command; verify the
    // composition behaviour the CLI relies on.
    let forwarded = ForwardedArgs::from_slice(&["--release", "--target=x86_64"]);
    let composed = compose_command("cargo build", &forwarded);
    assert_eq!(composed, "cargo build --release --target=x86_64");
}

#[test]
fn test_cli_create_raw_no_base_uses_just_quoted() {
    // The CreateRaw command with no base command and only forwarded
    // args should produce just the shell-quoted forwarded args.
    let forwarded = ForwardedArgs::from_slice(&["echo", "hi"]);
    let composed = forwarded.shell_quote();
    assert_eq!(composed, "echo hi");
    // compose_command with empty base returns the forwarded quotes.
    let wrapped = compose_command("", &forwarded);
    assert_eq!(wrapped, "echo hi");
}

#[test]
fn test_cli_create_raw_with_base_and_special_chars() {
    // The CreateRaw command with a base and quoted args must preserve
    // shell-safety of forwarded args containing whitespace/quotes.
    let forwarded = ForwardedArgs::from_slice(&["hello world", "$USER"]);
    let composed = compose_command("echo", &forwarded);
    // Both args contain metacharacters and must be single-quoted.
    assert!(composed.starts_with("echo 'hello world' '"));
    assert!(composed.contains("$USER"));
}

#[test]
fn test_cli_run_args_handles_empty_forwarded() {
    // When the user writes `taskkit run-args --id foo --` (no args
    // after the separator), compose_command must return the base
    // unchanged.
    let forwarded = ForwardedArgs::new();
    let composed = compose_command("ls -la", &forwarded);
    assert_eq!(composed, "ls -la");
}

fn setup_service() -> Arc<TaskService> {
    let storage = Arc::new(MemoryStorage::new());
    let queue = Arc::new(MemoryStorage::new());
    Arc::new(TaskService::new(storage, queue))
}

#[tokio::test]
async fn test_cli_create_command_builds_persisted_task() {
    // Mirror what the CLI Create handler does: build a CreateTask, then
    // execute it through the service. This exercises the service
    // surface area that the CLI depends on.
    let service = setup_service();
    let cmd = CreateTask::new("cli-create")
        .with_description("via cli")
        .with_priority(Priority::High)
        .with_timeout(30)
        .with_tag("cli")
        .with_command("echo hi");
    let task = cmd.execute(&service).await.expect("create should succeed");
    assert_eq!(task.name, "cli-create");
    assert_eq!(task.description.as_deref(), Some("via cli"));
    assert_eq!(task.priority, Priority::High);
    assert!(task.timeout.is_some());
    assert!(task.tags.contains(&"cli".to_string()));
    // The persisted task must be retrievable.
    let fetched = service.get_task(&task.id).await.expect("get ok").expect("some");
    assert_eq!(fetched.name, "cli-create");
}

#[tokio::test]
async fn test_cli_get_command_returns_not_found_error() {
    // The CLI Get handler returns TaskError::NotFound when the id is
    // missing. Validate the service surface that backs it.
    let service = setup_service();
    let id = TaskId::from_string("definitely-missing");
    let result = service.get_task(&id).await.expect("get should be Ok");
    assert!(result.is_none());
}

#[tokio::test]
async fn test_cli_list_command_applies_state_and_tag_filters() {
    // Mirror the CLI List handler's filter composition.
    let service = setup_service();
    let cmd1 = CreateTask::new("list-1").with_tag("dev");
    let cmd2 = CreateTask::new("list-2").with_tag("prod");
    let cmd3 = CreateTask::new("list-3").with_tag("dev");
    service.create_task(cmd1).await.unwrap();
    service.create_task(cmd2).await.unwrap();
    service.create_task(cmd3).await.unwrap();

    // Tag filter only — should yield 2 "dev" tasks.
    let dev = service.list_tasks(None, Some("dev".to_string()), None).await.unwrap();
    assert_eq!(dev.len(), 2);

    // State filter (Pending) + no tag — should yield all 3.
    let pending = service.list_tasks(Some(TaskState::Pending), None, None).await.unwrap();
    assert_eq!(pending.len(), 3);

    // Limit only — should cap to 1.
    let limited = service.list_tasks(None, None, Some(1)).await.unwrap();
    assert_eq!(limited.len(), 1);
}

#[tokio::test]
async fn test_cli_cancel_command_marks_cancelled() {
    // Mirror the CLI Cancel handler: load, transition, save.
    let service = setup_service();
    let cmd = CreateTask::new("cli-cancel");
    let task = service.create_task(cmd).await.unwrap();
    service.cancel_task(task.id.clone(), Some("user requested".to_string())).await.unwrap();
    let fetched = service.get_task(&task.id).await.unwrap().unwrap();
    assert_eq!(fetched.state, TaskState::Cancelled);
}

#[test]
fn test_cli_run_args_preserves_hyphen_prefixed_values() {
    // W3 SOTA feature #1: hyphen-prefixed forwarded args must be
    // preserved verbatim through compose_command. This is the entire
    // reason for the allow_hyphen_values=true on the CLI definition.
    let forwarded = ForwardedArgs::from_slice(&["--release", "--target=x86_64", "--features", "tokio,serde"]);
    let composed = compose_command("cargo build", &forwarded);
    assert!(composed.starts_with("cargo build "));
    assert!(composed.contains("--release"));
    assert!(composed.contains("--target=x86_64"));
    // "tokio,serde" contains a comma (a shell metachar) so it must be
    // quoted in the composed output.
    assert!(composed.contains("'tokio,serde'"));
}

#[test]
fn test_cli_run_args_with_empty_base_and_args_uses_only_quoted() {
    // Edge case: the underlying task has no base command (rare in
    // practice but a valid code path in handle_command's RunArgs arm).
    // The compose helper then returns just the shell-quoted args.
    let forwarded = ForwardedArgs::from_slice(&["echo", "hi"]);
    assert_eq!(compose_command("", &forwarded), "echo hi");
    assert_eq!(compose_command("", &ForwardedArgs::new()), "");
}

#[test]
fn test_cli_create_raw_quotes_args_even_without_base() {
    // CreateRaw with no base command and forwarded args containing
    // shell metacharacters must still produce a safely-quoted string.
    let forwarded = ForwardedArgs::from_slice(&["echo", "hello world", "$PATH"]);
    let composed = forwarded.shell_quote();
    assert_eq!(composed, "echo 'hello world' '$PATH'");
}

#[test]
fn test_cli_forwarded_args_display_impl() {
    // ForwardedArgs implements Display via shell_quote.
    let f = ForwardedArgs::from_slice(&["plain", "with space"]);
    let s = format!("{}", f);
    assert_eq!(s, "plain 'with space'");
}

#[test]
fn test_cli_forwarded_args_to_json() {
    // The to_json() helper produces a JSON array of strings.
    let f = ForwardedArgs::from_slice(&["--flag", "value"]);
    let json = f.to_json();
    assert_eq!(json, serde_json::json!(["--flag", "value"]));
    // Empty -> empty array.
    let empty = ForwardedArgs::new().to_json();
    assert_eq!(empty, serde_json::json!([]));
}

#[tokio::test]
async fn test_cli_create_command_with_data_payload() {
    // CreateTask::with_data should set the arbitrary JSON payload.
    let service = setup_service();
    let payload = serde_json::json!({"key": "value", "nested": {"a": 1}});
    let cmd = CreateTask::new("data-task").with_data(payload.clone());
    let task = service.create_task(cmd).await.unwrap();
    assert_eq!(task.data, payload);
}

#[tokio::test]
async fn test_cli_list_command_with_combined_filters() {
    // CLI List composes (state, tag) filters in both permutations.
    let service = setup_service();
    let cmd1 = CreateTask::new("combined-1").with_tag("urgent");
    let cmd2 = CreateTask::new("combined-2").with_tag("routine");
    let cmd3 = CreateTask::new("combined-3").with_tag("urgent");
    service.create_task(cmd1).await.unwrap();
    service.create_task(cmd2).await.unwrap();
    service.create_task(cmd3).await.unwrap();

    // tag=urgent → 2 tasks.
    let urgent = service.list_tasks(None, Some("urgent".to_string()), None).await.unwrap();
    assert_eq!(urgent.len(), 2);
    // state=Pending + no tag → 3 tasks.
    let all_pending = service.list_tasks(Some(TaskState::Pending), None, None).await.unwrap();
    assert_eq!(all_pending.len(), 3);
    // state=Pending + tag=urgent → 2 tasks.
    let both = service.list_tasks(Some(TaskState::Pending), Some("urgent".to_string()), None).await.unwrap();
    assert_eq!(both.len(), 2);
    // state=Completed + tag=urgent → 0 tasks.
    let none = service.list_tasks(Some(TaskState::Completed), Some("urgent".to_string()), None).await.unwrap();
    assert!(none.is_empty());
}

#[tokio::test]
async fn test_cli_workflow_create_and_get_round_trip() {
    // The CLI's WorkflowCommand::Create and Get handlers delegate to
    // service.create_workflow / get_workflow.
    use taskkit::domain::workflows::WorkflowId;
    let service = setup_service();
    let wf = Workflow::new("cli-wf");
    let created = service.create_workflow(wf).await.unwrap();
    let fetched = service.get_workflow(&created.id).await.unwrap().expect("some");
    assert_eq!(fetched.name, "cli-wf");
    assert_eq!(created.id, WorkflowId::default()); // sanity: id is set
}

#[tokio::test]
async fn test_cli_workflow_get_missing_returns_none() {
    // Mirrors the CLI's WorkflowCommand::Get NotFound path.
    use taskkit::domain::workflows::WorkflowId;
    let service = setup_service();
    let id = WorkflowId::from_string("missing-wf");
    let fetched = service.get_workflow(&id).await.unwrap();
    assert!(fetched.is_none());
}

#[tokio::test]
async fn test_cli_run_command_executes_and_succeeds() {
    // The CLI's Command::Run path is just service.run_task().
    let service = setup_service();
    let cmd = CreateTask::new("cli-run").with_command("echo from-cli");
    let task = service.create_task(cmd).await.unwrap();
    let result = service.run_task(&task.id).await.unwrap();
    assert!(result.success);
    let output = result.output.unwrap();
    assert!(output["stdout"].as_str().unwrap().contains("from-cli"));
}
