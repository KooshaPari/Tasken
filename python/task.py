"""Task domain entity and related value objects.

@trace TASK-001: Task Definition
@trace TASK-003: Dependency Graph
@trace TASK-006: Async/Await

Pure data classes representing task concepts in the domain model.
"""
from datetime import datetime
from enum import StrEnum
from typing import Any

from pydantic import BaseModel, Field, field_validator


class SubagentType(StrEnum):
    """Subagent type enumeration."""

    WORKER = "worker"
    FLASH = "flash"
    RESEARCHER = "researcher"
    REVIEWER = "reviewer"
    PLANNER = "planner"


class Priority(StrEnum):
    """Priority level enumeration."""

    P1 = "P1"
    P2 = "P2"
    P3 = "P3"


class TaskVisibility(StrEnum):
    """Task visibility level."""

    PUBLIC = "public"
    PRIVATE = "private"
    RESTRICTED = "restricted"
    INTERNAL = "internal"


class Complexity(StrEnum):
    """Task complexity level."""

    SIMPLE = "simple"
    MODERATE = "moderate"
    COMPLEX = "complex"


class TaskStep(BaseModel):
    """A single step in a task."""

    number: int = Field(ge=1, description="Step number")
    description: str = Field(min_length=1, description="Step description")
    deliverables: list[str] = Field(default_factory=list, description="Deliverables for this step")
    estimated_minutes: int | None = Field(None, ge=0, description="Estimated minutes for this step")


class TaskMetadata(BaseModel):
    """Task metadata."""

    estimated_hours: float | None = Field(None, ge=0, le=1000, description="Estimated hours to complete")
    complexity: Complexity | None = Field(None, description="Task complexity level")
    tags: list[str] = Field(default_factory=list, description="Tags for categorization")
    assignee: str | None = Field(None, description="Assigned agent or user")
    created: datetime | None = Field(None, description="ISO 8601 creation timestamp")
    updated: datetime | None = Field(None, description="ISO 8601 last update timestamp")
    related_tasks: list[str] = Field(default_factory=list, description="Related task IDs")
    references: list[dict[str, Any]] = Field(default_factory=list, description="Reference links")


class Task(BaseModel):
    """Task input model."""

    id: str = Field(pattern=r"^[a-z0-9-]+$", min_length=3, max_length=100, description="Unique task identifier")
    title: str = Field(min_length=1, max_length=200, description="Brief, descriptive task title")
    subagent_type: SubagentType = Field(default=SubagentType.WORKER, description="Type of agent to execute")
    description: str | None = Field(None, max_length=5000, description="Detailed task description")
    priority: Priority = Field(default=Priority.P2, description="Task priority level")
    depends: list[str] = Field(default_factory=list, description="List of task IDs this depends on")
    source: str | None = Field(None, description="Source document this task originated from")
    metadata: TaskMetadata = Field(
        default_factory=lambda: TaskMetadata(
            estimated_hours=None, complexity=None, assignee=None, created=None, updated=None
        ),
        description="Task metadata",
    )
    implementation_details: str | None = Field(None, max_length=10000, description="Technical implementation details")
    steps: list[TaskStep] = Field(default_factory=list, description="Step-by-step instructions")
    deliverables: list[str] = Field(default_factory=list, description="Expected outputs")
    acceptance_criteria: list[str] = Field(default_factory=list, description="Acceptance criteria")
    research_questions: list[str] = Field(default_factory=list, description="Research questions (for researcher tasks)")
    expected_outcomes: list[str] = Field(default_factory=list, description="Expected research outcomes")
    methodology: list[str] = Field(default_factory=list, description="Research methodology")
    review_criteria: list[str] = Field(default_factory=list, description="Review criteria (for reviewer tasks)")
    files_to_review: list[str] = Field(default_factory=list, description="Files to review")
    quality_gates: dict[str, Any] | None = Field(None, description="Quality gates")
    visibility: TaskVisibility = Field(default=TaskVisibility.PUBLIC, description="Task visibility level")
    allowed_agents: list[str] = Field(default_factory=list, description="Allowed agents for restricted tasks")

    @field_validator("depends")
    @classmethod
    def validate_depends(cls, v: list[str]) -> list[str]:
        """Validate dependency IDs."""
        import re

        for dep_id in v:
            if not re.match(r"^[a-z0-9-]+$", dep_id):
                raise ValueError(f"Invalid dependency ID format: {dep_id}")
        return v

    @field_validator("allowed_agents")
    @classmethod
    def validate_allowed_agents(cls, v: list[str], info) -> list[str]:
        """Validate allowed_agents is set when visibility is restricted."""
        if info.data.get("visibility") == TaskVisibility.RESTRICTED and not v:
            raise ValueError("allowed_agents must be specified when visibility is 'restricted'")
        return v

    model_config = {
        "json_schema_extra": {
            "example": {
                "id": "docgen-sticky-nav",
                "title": "Implement sticky sidebar",
                "subagent_type": "worker",
                "priority": "P1",
            }
        }
    }


class TaskOutputStatus(StrEnum):
    """Task output status."""

    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"
    IN_PROGRESS = "in_progress"


class Deliverable(BaseModel):
    """A deliverable artifact."""

    name: str = Field(description="Deliverable name")
    path: str = Field(description="Path to deliverable")
    type: str = Field(default="file", description="Deliverable type")
    description: str | None = Field(None, description="Deliverable description")


class TaskOutput(BaseModel):
    """Task execution output."""

    task_id: str = Field(pattern=r"^[a-z0-9-]+$", description="Task ID that was executed")
    status: TaskOutputStatus = Field(description="Task execution status")
    timestamp: datetime = Field(default_factory=datetime.now, description="ISO 8601 timestamp")
    agent: str | None = Field(None, description="Agent that executed the task")
    session_id: str | None = Field(None, description="Session ID")
    run_id: str | None = Field(None, description="Run ID")
    duration_seconds: float | None = Field(None, ge=0, description="Execution duration")
    output: str | None = Field(None, max_length=100000, description="Task output")
    deliverables: list[Deliverable] = Field(default_factory=list, description="Delivered artifacts")
    errors: list[dict[str, Any]] = Field(default_factory=list, description="Errors encountered")
    warnings: list[str] = Field(default_factory=list, description="Warnings")
    metrics: dict[str, Any] | None = Field(None, description="Execution metrics")
    acceptance_criteria_met: list[bool] = Field(default_factory=list, description="Acceptance criteria results")
    next_steps: list[str] = Field(default_factory=list, description="Suggested next steps")
