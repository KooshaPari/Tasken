# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-03-25

### Added
- Initial project scaffold
- Hexagonal architecture structure
- Domain layer with tasks, workflows, scheduler
- Task state machine with transitions
- Workflow DAG definition
- Schedule management (once, interval, cron)
- Task runner implementations (sync, async, background)
- Port definitions (Storage, Queue, Notification)
- Application layer with commands and queries
- Domain events for event sourcing
- Basic tests
- CI/CD workflow
- STANDARDS.md with 126 xDD methodologies
- ADR directory for architecture decisions

### Planned
- Storage adapter implementations
- Queue adapter implementations
- Workflow persistence
- Distributed execution
- Webhook integrations
- CLI adapter
- API adapter
