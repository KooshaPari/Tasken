"""Execution run domain entities and related value objects.

Pure data classes representing execution/run concepts in the domain model.
No I/O, no side effects, no infrastructure dependencies.
"""

import os
import socket
import uuid
from datetime import UTC, datetime
from enum import StrEnum
from typing import Any

from pydantic import BaseModel, Field


class RunState(StrEnum):
    """Run lifecycle state for state-aware orchestration (G-KD-03)."""

    RUNNING = "running"
    PAUSED = "paused"
    COMPLETED = "completed"
    FAILED = "failed"


class AgentSource(StrEnum):
    """Source of the agent process for session registry (WP-9001)."""

    THEGENT_RUN = "thegent-run"
    THEGENT_DROID = "thegent-droid"
    THEGENT_SUBAGENT = "thegent-subagent"
    IDE_MANAGED = "ide-managed"
    USER_SPAWNED = "user-spawned"
    DISCOVERED = "discovered"
    MCP_PROXY = "mcp-proxy"


class InteractivityMode(StrEnum):
    """Interactivity mode of the session (WP-9002)."""

    PTY = "pty"
    TMUX = "tmux"
    HEADLESS_LOGS = "headless-logs"
    HEADLESS_HOLDPTY = "headless-holdpty"
    READ_ONLY = "read-only"


class MAIFArtifact(BaseModel):
    """WP-3002: Model AI Information Format (MAIF) for signed artifacts."""

    version: str = "1.0"
    run_id: str
    timestamp: str = Field(default_factory=lambda: datetime.now(UTC).isoformat())
    agent: str
    model: str | None = None
    prompt_hash: str
    output_hash: str | None = None
    signature: str
    policy_result: str | None = None


class ContinuityPacket(BaseModel):
    """Compressed essence of session progress for cross-session handoffs (L3/L4).

    # @trace FR-HAX-004
    """

    intent: str
    """High-level goal of the session."""

    decisions: list[str] = Field(default_factory=list)
    """Key decisions made during the session."""

    risks: list[str] = Field(default_factory=list)
    """Identified risks or blockers."""

    context_hashes: dict[str, str] = Field(default_factory=dict)
    """SHA-256 hashes of referenced context files keyed by path string."""

    token_count: int = 0
    """Approximate token count (rough estimate)."""

    session_id: str = Field(default_factory=lambda: "")
    """Session ID this packet belongs to."""

    created_at: str = Field(default_factory=lambda: datetime.now(UTC).isoformat())
    """ISO-8601 timestamp when the packet was created."""


class RunMeta(BaseModel):
    """Metadata for a single agent/droid execution run."""

    run_id: str = Field(default_factory=lambda: f"run_{uuid.uuid4().hex[:8]}")
    correlation_id: str | None = None
    source: AgentSource = AgentSource.THEGENT_RUN
    interactivity: InteractivityMode = InteractivityMode.HEADLESS_LOGS

    # Attachment details
    attach_target: dict[str, Any] | None = None
    message_endpoint: str | None = None

    # Paths (managed sessions only)
    stdout_path: str | None = None
    stderr_path: str | None = None
    chat_path: str | None = None
    messages_path: str | None = None
    audit_path: str | None = None

    agent: str
    model: str | None = None
    mode: str = "write"
    prompt: str
    cwd: str
    owner: str
    started_at_utc: str = Field(default_factory=lambda: datetime.now(UTC).isoformat())
    ended_at_utc: str | None = None
    duration_s: float | None = None
    exit_code: int | None = None
    status: str = "started"  # started, running, completed, failed, timed_out
    error_class: str | None = None  # usage_limit, timeout, logic_error, api_error
    signature: str | None = None
    policy_result: str | None = None  # allow, deny, warn
    policy_reason: str | None = None
    override_reason: str | None = None
    override_by: str | None = None
    rationale: str | None = None  # WP-4002/4007: Full explanation
    feedback_score: float | None = None  # WP-4008
    feedback_note: str | None = None
    host: str = Field(default_factory=socket.gethostname)
    pid: int = Field(default_factory=os.getpid)
    is_background: bool = False
    lane: str = "standard"  # standard, critical, recovery
    idempotency_token: str | None = None
    confidence: float | None = None
    arbitration: str | None = None  # leader, follower, consensus
    freshness_timestamp: str | None = Field(
        default_factory=lambda: datetime.now(UTC).isoformat()
    )  # ROB-011: Timestamp for stale-state detection

    # Audit trail chaining (WP-3004)
    prev_hash: str | None = None
    hash: str | None = None

    # Optional routing contract context
    route_contract: dict[str, Any] | None = None
    route_request: dict[str, Any] | None = None

    # Task routing metadata (Terminal Bench 2.0 Pareto frontier)
    task_category: str | None = None  # fast/normal/complex/high_complex
    task_complexity_score: int | None = None  # 0-100 complexity score
    estimated_cost_usd: float | None = None  # Estimated cost for this task
    estimated_duration_s: float | None = None  # Estimated duration
    constraint_violations: list[str] | None = None  # Hard constraint failures
    routing_reason: str | None = None  # Routing decision explanation

    # WP-3006: Compliance evidence retention — domain tagging for tiered retention
    domain_tag: str | None = None  # e.g. project-id, compliance-domain, lane

    # XA4: Contract version in task/run metadata for negotiation
    contract_version: str | None = None

    # WP-16002: Teammate delegation linkage
    task_id: str | None = None
    task_metadata: dict[str, Any] | None = None


class CheckpointMeta(BaseModel):
    """Metadata for a DAG/state checkpoint."""

    checkpoint_id: str = Field(default_factory=lambda: f"ckpt_{uuid.uuid4().hex[:8]}")
    created_at_utc: str = Field(default_factory=lambda: datetime.now(UTC).isoformat())
    reason: str
    dag_content: str
    session_dir: str
    owner: str
