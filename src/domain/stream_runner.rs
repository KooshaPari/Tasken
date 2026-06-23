// SPDX-License-Identifier: MIT OR Apache-2.0
//! Strict stdout/stderr separation for shell task execution.
//!
//! Standard POSIX practice: a process has two distinct output streams,
//! stdout (file descriptor 1) and stderr (file descriptor 2), with
//! separate buffering and separate downstream handling. The classic
//! `ShellRunner` merges them into a JSON blob, which is lossy: it
//! destroys ordering, conflates them, and prevents the caller from
//! routing one stream to a TTY and the other to a log file.
//!
//! This module provides:
//!
//! - [`StreamResult`] — a struct that keeps the two streams fully
//!   separated, with byte counts and exit code.
//! - [`StreamRunner`] — a runner that captures the two streams
//!   independently using separate `tokio::process::Command` pipes.
//!
//! The streams are exposed as `Bytes` (lossless) and as `String`
//! (UTF-8 lossy) so the caller can choose.

use std::time::{Duration, Instant};

use async_trait::async_trait;

use crate::domain::errors::TaskError;
use crate::domain::runners::TaskRunner;
use crate::domain::tasks::{Task, TaskState};

/// A task result with strict stdout/stderr separation.
///
/// Unlike [`crate::domain::TaskResult`], this type never mixes the
/// two streams. Use [`StreamResult::stdout_str`] and
/// [`StreamResult::stderr_str`] for UTF-8 views; use the raw `Bytes`
/// fields for binary fidelity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamResult {
    /// Task identifier this result belongs to.
    pub task_id: String,
    /// Whether the process exited successfully.
    pub success: bool,
    /// Process exit code, if any (None for signals).
    pub exit_code: Option<i32>,
    /// Raw bytes written to stdout.
    pub stdout_bytes: Vec<u8>,
    /// Raw bytes written to stderr.
    pub stderr_bytes: Vec<u8>,
    /// Wall-clock duration of execution.
    pub duration: Duration,
}

impl StreamResult {
    /// Build from raw captured buffers and an exit status.
    pub fn from_parts(
        task_id: impl Into<String>,
        success: bool,
        exit_code: Option<i32>,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        duration: Duration,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            success,
            exit_code,
            stdout_bytes: stdout,
            stderr_bytes: stderr,
            duration,
        }
    }

    /// UTF-8 lossy view of stdout.
    pub fn stdout_str(&self) -> String {
        String::from_utf8_lossy(&self.stdout_bytes).into_owned()
    }

    /// UTF-8 lossy view of stderr.
    pub fn stderr_str(&self) -> String {
        String::from_utf8_lossy(&self.stderr_bytes).into_owned()
    }

    /// Byte length of the stdout stream.
    pub fn stdout_len(&self) -> usize {
        self.stdout_bytes.len()
    }

    /// Byte length of the stderr stream.
    pub fn stderr_len(&self) -> usize {
        self.stderr_bytes.len()
    }

    /// True when stderr is non-empty.
    pub fn has_stderr(&self) -> bool {
        !self.stderr_bytes.is_empty()
    }

    /// True when stdout is non-empty.
    pub fn has_stdout(&self) -> bool {
        !self.stdout_bytes.is_empty()
    }

    /// Serialize the result to a JSON object that keeps stdout and
    /// stderr in distinct fields, plus their byte lengths.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "task_id": self.task_id,
            "success": self.success,
            "exit_code": self.exit_code,
            "stdout": self.stdout_str(),
            "stderr": self.stderr_str(),
            "stdout_bytes": self.stdout_len(),
            "stderr_bytes": self.stderr_len(),
            "duration_ms": self.duration.as_millis(),
        })
    }
}

/// Runner that executes shell tasks with strict stream separation.
///
/// Internally uses `tokio::process::Command` with separate `Stdio::piped`
/// for stdout/stderr and reads them in parallel, preserving the
/// invariant that the two streams are never merged.
pub struct StreamRunner;

impl StreamRunner {
    /// Construct a new stream runner.
    pub fn new() -> Self {
        Self
    }

    /// Extract the shell command from a task's payload data.
    fn extract_command(task: &Task) -> Result<String, TaskError> {
        task.data
            .get("command")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| TaskError::InvalidOperation("No command in task.data['command']".into()))
    }
}

impl Default for StreamRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskRunner for StreamRunner {
    fn execute(&self, task: &mut Task) -> Result<crate::domain::TaskResult, TaskError> {
        let _ = task.transition_to(TaskState::Running);
        let cmd = Self::extract_command(task)?;
        let start = Instant::now();

        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .output()
            .map_err(|e| TaskError::ExecutionFailed(e.to_string()))?;

        let duration = start.elapsed();
        let success = output.status.success();
        let stdout = output.stdout;
        let stderr = output.stderr;

        // Build the legacy result blob, but route stdout/stderr to the
        // strict fields via the legacy output JSON for backward compat.
        let result = serde_json::json!({
            "status": if success { "ok" } else { "error" },
            "code": output.status.code(),
            "stdout": String::from_utf8_lossy(&stdout),
            "stderr": String::from_utf8_lossy(&stderr),
        });

        if success {
            let _ = task.transition_to(TaskState::Completed);
            Ok(task.success_result(result, duration))
        } else {
            let _ = task.transition_to(TaskState::Failed);
            Ok(task.failure_result(result.to_string(), duration))
        }
    }

    async fn execute_async(
        self: Box<Self>,
        mut task: Task,
    ) -> Result<crate::domain::TaskResult, TaskError> {
        let _ = task.transition_to(TaskState::Running);
        let cmd = Self::extract_command(&task)?;
        let start = Instant::now();

        // Spawn with piped stdout AND stderr so the streams are captured
        // independently. Using `output()` would still capture both, but
        // would buffer each to completion before returning, blocking on
        // whichever finishes last. By reading from the two pipes in
        // parallel with `try_join`, the runner preserves timing
        // characteristics and never blocks one stream on the other.
        let mut child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| TaskError::ExecutionFailed(e.to_string()))?;

        let mut stdout_pipe = child
            .stdout
            .take()
            .ok_or_else(|| TaskError::ExecutionFailed("Failed to capture stdout pipe".into()))?;
        let mut stderr_pipe = child
            .stderr
            .take()
            .ok_or_else(|| TaskError::ExecutionFailed("Failed to capture stderr pipe".into()))?;

        let mut stdout_buf: Vec<u8> = Vec::new();
        let mut stderr_buf: Vec<u8> = Vec::new();
        let (stdout_res, stderr_res) = tokio::try_join!(
            tokio::io::AsyncReadExt::read_to_end(&mut stdout_pipe, &mut stdout_buf),
            tokio::io::AsyncReadExt::read_to_end(&mut stderr_pipe, &mut stderr_buf),
        )
        .map_err(|e| TaskError::ExecutionFailed(format!("stream read failed: {e}")))?;
        // Discard byte counts; the populated buffers carry the data.
        let _ = (stdout_res, stderr_res);
        let stdout_bytes = stdout_buf;
        let stderr_bytes = stderr_buf;

        let status = child.wait().await.map_err(|e| TaskError::ExecutionFailed(e.to_string()))?;
        let duration = start.elapsed();
        let success = status.success();

        // Build a stream-separated result internally; surface it via
        // the legacy TaskResult by encoding the strict fields in JSON.
        let strict = StreamResult::from_parts(
            task.id.0.clone(),
            success,
            status.code(),
            stdout_bytes,
            stderr_bytes,
            duration,
        );
        let result = strict.to_json();

        if success {
            let _ = task.transition_to(TaskState::Completed);
            Ok(task.success_result(result, duration))
        } else {
            let _ = task.transition_to(TaskState::Failed);
            Ok(task.failure_result(result.to_string(), duration))
        }
    }
}

/// Run a task synchronously with strict stream separation and write
/// each stream to its own file descriptor. By convention, the result
/// is written to `stdout` as JSON, and the stderr stream is written
/// verbatim to the process's `stderr` so the user can see it live
/// even if the task fails.
pub fn run_with_streams(task: &mut Task, print_result: bool) -> Result<StreamResult, TaskError> {
    let _ = task.transition_to(TaskState::Running);
    let cmd = StreamRunner::extract_command(task)?;
    let start = Instant::now();

    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .output()
        .map_err(|e| TaskError::ExecutionFailed(e.to_string()))?;

    let duration = start.elapsed();
    let success = output.status.success();
    let strict = StreamResult::from_parts(
        task.id.0.clone(),
        success,
        output.status.code(),
        output.stdout,
        output.stderr,
        duration,
    );

    if success {
        let _ = task.transition_to(TaskState::Completed);
    } else {
        let _ = task.transition_to(TaskState::Failed);
    }

    if print_result {
        // Route stdout (the data stream) to the caller's stdout as JSON.
        println!("{}", serde_json::to_string_pretty(&strict.to_json()).unwrap());
    }
    // Always route stderr (the diagnostic stream) verbatim to the caller's
    // stderr. This preserves stream separation even in silent mode:
    // errors are always visible, only the result JSON is suppressed.
    if strict.has_stderr() {
        eprint!("{}", strict.stderr_str());
    }

    Ok(strict)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_task_with_command(command: &str) -> Task {
        Task::new("stream-test").with_command(command)
    }

    #[test]
    fn test_stream_result_from_parts() {
        let r = StreamResult::from_parts(
            "t1",
            true,
            Some(0),
            b"hello\n".to_vec(),
            b"warn\n".to_vec(),
            Duration::from_millis(10),
        );
        assert_eq!(r.task_id, "t1");
        assert!(r.success);
        assert_eq!(r.exit_code, Some(0));
        assert_eq!(r.stdout_str(), "hello\n");
        assert_eq!(r.stderr_str(), "warn\n");
        assert_eq!(r.stdout_len(), 6);
        assert_eq!(r.stderr_len(), 5);
        assert!(r.has_stderr());
        assert!(r.has_stdout());
    }

    #[test]
    fn test_stream_result_empty() {
        let r =
            StreamResult::from_parts("t1", true, Some(0), Vec::new(), Vec::new(), Duration::ZERO);
        assert!(!r.has_stderr());
        assert!(!r.has_stdout());
    }

    #[test]
    fn test_stream_result_to_json() {
        let r = StreamResult::from_parts(
            "t1",
            false,
            Some(2),
            b"out".to_vec(),
            b"err".to_vec(),
            Duration::from_millis(50),
        );
        let v = r.to_json();
        assert_eq!(v["task_id"], "t1");
        assert_eq!(v["success"], false);
        assert_eq!(v["exit_code"], 2);
        assert_eq!(v["stdout"], "out");
        assert_eq!(v["stderr"], "err");
        assert_eq!(v["stdout_bytes"], 3);
        assert_eq!(v["stderr_bytes"], 3);
    }

    #[test]
    fn test_stream_runner_runs_simple_command() {
        let runner = StreamRunner::new();
        let mut task = make_task_with_command("echo hello");
        let result = runner.execute(&mut task).unwrap();
        assert!(result.success);
        let stdout = result.output.unwrap()["stdout"].as_str().unwrap().to_string();
        assert!(stdout.contains("hello"));
    }

    #[test]
    fn test_stream_runner_separates_streams() {
        // echo to stdout, then a separate echo to stderr
        let runner = StreamRunner::new();
        let mut task = make_task_with_command("echo out; echo err 1>&2");
        let result = runner.execute(&mut task).unwrap();
        assert!(result.success);
        let json = result.output.unwrap();
        // Strict separation: stdout field must NOT contain stderr text
        let stdout = json["stdout"].as_str().unwrap();
        let stderr = json["stderr"].as_str().unwrap();
        assert!(stdout.contains("out"));
        assert!(!stdout.contains("err"));
        assert!(stderr.contains("err"));
        assert!(!stderr.contains("out"));
    }

    #[test]
    fn test_stream_runner_failure_path() {
        let runner = StreamRunner::new();
        let mut task = make_task_with_command("false");
        let result = runner.execute(&mut task).unwrap();
        assert!(!result.success);
    }

    #[test]
    fn test_stream_runner_no_command() {
        let runner = StreamRunner::new();
        let mut task = Task::new("no-cmd");
        let result = runner.execute(&mut task);
        assert!(result.is_err());
    }

    #[test]
    fn test_run_with_streams_happy_path() {
        let mut task = make_task_with_command("echo happy");
        let r = run_with_streams(&mut task, false).unwrap();
        assert!(r.success);
        assert!(r.stdout_str().contains("happy"));
    }

    #[test]
    fn test_run_with_streams_separation_preserved() {
        let mut task = make_task_with_command("printf OUT; printf ERR 1>&2");
        let r = run_with_streams(&mut task, false).unwrap();
        assert_eq!(r.stdout_str(), "OUT");
        assert_eq!(r.stderr_str(), "ERR");
    }

    #[test]
    fn test_run_with_streams_captures_exit_code() {
        let mut task = make_task_with_command("exit 7");
        let r = run_with_streams(&mut task, false).unwrap();
        assert!(!r.success);
        assert_eq!(r.exit_code, Some(7));
    }

    #[tokio::test]
    async fn test_stream_runner_async_separates_streams() {
        let runner: Box<dyn TaskRunner> = Box::new(StreamRunner::new());
        let task = make_task_with_command("echo OUT; echo ERR 1>&2");
        let result = runner.execute_async(task).await.unwrap();
        let json = result.output.unwrap();
        let stdout = json["stdout"].as_str().unwrap();
        let stderr = json["stderr"].as_str().unwrap();
        assert!(stdout.contains("OUT"));
        assert!(!stdout.contains("ERR"));
        assert!(stderr.contains("ERR"));
        assert!(!stderr.contains("OUT"));
    }

    #[tokio::test]
    async fn test_stream_runner_async_handles_binary() {
        let runner: Box<dyn TaskRunner> = Box::new(StreamRunner::new());
        // Emit a single null byte on stdout; ensure it's preserved.
        let task = make_task_with_command("printf '\\x00'");
        let result = runner.execute_async(task).await.unwrap();
        let stdout_bytes = result.output.unwrap()["stdout_bytes"].as_u64().unwrap();
        assert_eq!(stdout_bytes, 1);
    }
}
