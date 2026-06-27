Feature: Single Task Execution (FR-1)
  The system shall execute a single named task via the CLI.

  Scenario: Execute a registered task
    Given a registered task "build" with command "cargo build"
    When the user runs "tasken run build"
    Then the command shall be spawned as a subprocess
    And the exit code shall be returned to the caller
    And stdout/stderr shall be streamed to the user

  Scenario: Execute an unregistered task
    Given no task named "nonexistent" is registered
    When the user runs "tasken run nonexistent"
    Then the exit code shall be non-zero
    And an error message shall indicate the task was not found

Feature: Task Definition & Listing (FR-2)
  The system shall allow users to define, list, and inspect tasks.

  Scenario: List all tasks
    Given tasks "build", "test", and "deploy" are registered
    When the user runs "tasken list"
    Then the output shall contain "build", "test", and "deploy"
    And each task name shall appear with a one-line summary

  Scenario: Show a single task
    Given a task "build" is registered with command "cargo build"
    When the user runs "tasken show build"
    Then the output shall contain the full task definition
    And the output shall contain the command "cargo build"

Feature: Cron-Based Scheduling (FR-3)
  The system shall support recurring execution via cron expressions.

  Scenario: Schedule a task with cron
    Given a task "daily-report" with schedule "0 9 * * 1-5"
    When the scheduler evaluates the cron expression
    Then the task shall be dispatched at 09:00 on weekdays
    And the dispatch shall occur with sub-minute precision

  Scenario: Schedule with invalid cron expression
    Given a task with an invalid cron expression "not-a-cron"
    When the configuration is loaded
    Then a structured error shall be emitted
    And the scheduler shall not register the task

Feature: DAG Workflow Execution (FR-4)
  The system shall execute workflows as a dependency-ordered DAG.

  Scenario: Execute a linear workflow
    Given a workflow with steps: step_a -> step_b -> step_c
    When the workflow is executed
    Then step_b shall not start before step_a completes
    And step_c shall not start before step_b completes
    And all steps shall eventually complete

  Scenario: Execute a parallel workflow
    Given a workflow with independent steps d and e, and step f depending on both
    When the workflow is executed
    Then step_d and step_e may run concurrently
    And step_f shall start only after both d and e complete

  Scenario: Workflow with circular dependency
    Given a workflow with a cycle: a -> b -> c -> a
    When the workflow is submitted for execution
    Then a CycleDetected error shall be raised
    And no tasks shall be executed

Feature: Task Grouping (FR-5)
  The system shall organize tasks into named groups with nesting.

  Scenario: List groups
    Given groups "frontend" and "backend" with multiple tasks each
    When the user runs "tasken groups"
    Then both "frontend" and "backend" shall appear in the output

  Scenario: Run all tasks in a group
    Given group "frontend" contains tasks "lint" and "build"
    When the user runs "tasken run frontend"
    Then "lint" and "build" shall execute
    And the group exit code shall reflect all task results

Feature: Multi-Backend Task Runners (FR-6)
  The system shall support shell, Docker, and plugin backends.

  Scenario: Execute a shell task
    Given a task with runner "shell" and command "echo hello"
    When the task is executed
    Then the command shall run in a subprocess
    And the stdout "hello\n" shall be captured

  Scenario: Execute a Docker task
    Given a task with runner "docker" and image "alpine:latest"
    When the task is executed
    Then the command shall run inside a Docker container
    And logs shall be streamed back

  Scenario: Execute a plugin task
    Given a task with runner "plugin" and a valid .wasm path
    When the task is executed
    Then the plugin host shall load the WASM module
    And the module shall be invoked

Feature: Task Dependencies (FR-7)
  The system shall allow tasks to declare dependencies on other tasks.

  Scenario: Execute task with dependencies
    Given task "deploy" depends on "test" and "build"
    When "tasken run deploy" is invoked
    Then "build" and "test" shall run first
    And "deploy" shall run only after both complete successfully

  Scenario: Dependency with failure propagation
    Given task "deploy" depends on "test"
    And task "test" fails
    When "tasken run deploy" is invoked
    Then "deploy" shall not execute
    And a DependencyFailed error shall be reported

Feature: Task Visualization (FR-8)
  The system shall render a visual graph of workflow dependencies.

  Scenario: Visualize a workflow
    Given a workflow with three steps
    When "tasken visualize <workflow>" is executed
    Then the output shall be a valid Mermaid flowchart string
    And the string shall render correctly in a Mermaid renderer

Feature: File Watch & Trigger (FR-9)
  The system shall watch files and trigger tasks on changes.

  Scenario: Watch and trigger on file change
    Given a watch on "./src/" with glob "*.rs" and action "build"
    When a .rs file is modified
    Then the "build" task shall be dispatched within 1 second

  Scenario: Watch non-matching file
    Given a watch on "./src/" with glob "*.rs"
    When a .md file is modified
    Then no task shall be dispatched

Feature: Rate Limiting (FR-10)
  The system shall enforce per-task rate limits.

  Scenario: Rate limit enforcement
    Given a rate limit of "5/min" on a task
    When 6 execution requests arrive within one minute
    Then the 6th request shall be rejected with a RateLimited error

  Scenario: Rate limit window reset
    Given a rate limit of "5/min" on a task
    When 5 executions complete within a minute
    And a 6th request arrives after the minute window
    Then the 6th request shall be accepted

Feature: Event Bus & Observability (FR-11)
  The system shall emit structured events for observability.

  Scenario: Task lifecycle events
    Given a task that runs successfully
    When the task executes
    Then a TaskStarted event shall be emitted
    And a TaskCompleted event shall be emitted

  Scenario: OpenTelemetry spans (feature-gated)
    Given the "otel" feature is enabled
    When a single task executes
    Then spans "tasken.run", "tasken.resolve", "tasken.execute" shall be produced
    And "tasken.execute" shall be a child of "tasken.run"

Feature: Recipe / Blueprint System (FR-12)
  The system shall support reusable task blueprints with variables.

  Scenario: Instantiate a recipe
    Given a recipe "compile" with variable "{{lang}}"
    And a configuration mapping "lang=rust"
    When tasks are loaded
    Then a concrete task shall be created with the resolved command

  Scenario: Recipe with missing variable
    Given a recipe "compile" with variable "{{lang}}"
    And no mapping for "lang"
    When tasks are loaded
    Then an error shall be emitted for the unresolved variable

Feature: Plugin Management (FR-13)
  The system shall support plugin lifecycle management.

  Scenario: Install a plugin
    Given a valid plugin package
    When "tasken plugin install <path>" is run
    Then the plugin shall be registered
    And appear in "tasken plugin list" output

  Scenario: Remove a plugin
    Given an installed plugin "my-plugin"
    When "tasken plugin remove my-plugin" is run
    Then the plugin shall be unregistered
    And shall not appear in "tasken plugin list" output

Feature: Task Export (FR-14)
  The system shall export tasks to interchange formats.

  Scenario: Export tasks as JSON
    Given a configured task with name, command, schedule, and dependencies
    When "tasken export json" is run
    Then the output shall be valid JSON
    And contain the task name, command, schedule, and dependencies

  Scenario: Export tasks as YAML
    Given a configured task
    When "tasken export yaml" is run
    Then the output shall be valid YAML

Feature: Persistent Task Store (FR-15)
  The system shall support in-memory and file-backed stores.

  Scenario: File-backed persistence
    Given a file-backed store
    When a task is created
    And the process restarts
    Then the task shall still be present after reload

  Scenario: In-memory ephemeral store
    Given an in-memory store
    When a task is created
    And the process restarts
    Then the store shall be empty

Feature: Caching Layer (FR-16)
  The system shall provide TTL-eviction and persistent caching.

  Scenario: TTL cache hit
    Given a cached task result with TTL 60s
    When the result is retrieved within 60s
    Then it shall be returned from cache

  Scenario: TTL cache miss
    Given a cached task result with TTL 60s
    When the result is retrieved after 60s
    Then the cache shall return a miss

Feature: Python SDK / Bindings (FR-17)
  The system shall expose a Python SDK for programmatic access.

  Scenario: Run a task from Python
    Given a Python environment with the tasken package installed
    When "tasken.run('build')" is called
    Then it shall dispatch the task
    And return a result dict with "exit_code" and "output"

  Scenario: List tasks from Python
    Given tasks are registered
    When "tasken.list_tasks()" is called
    Then a list of task definitions shall be returned
