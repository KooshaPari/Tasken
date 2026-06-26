# Ops Remediation

This repo is packaged as a CLI binary, so the ops baseline is container and release oriented rather than service-orchestrator oriented.

## Applied diffs

- Added a multi-stage [`Dockerfile`](../../Dockerfile) that builds a release binary and copies it into a slim runtime image.
- Added a `HEALTHCHECK` that runs `taskkit health`.
- Replaced `.env.example` with taskkit-specific runtime and observability keys.

## Graceful shutdown note

The current runtime is CLI-driven and exits after completing the selected command. That means graceful shutdown is mostly relevant to any future daemon or server mode, not the current execution model.

If this repository grows a long-running service, the process should:

- Trap SIGINT and SIGTERM.
- Stop accepting new work.
- Finish or cancel in-flight tasks within a configurable timeout.
- Flush logs/metrics before exit.

## Deployment guidance

Use the container image for:

- One-off task execution jobs.
- Smoke tests via `taskkit health`.
- Embedders that want a reproducible binary plus runtime defaults.

Use host deployment when:

- You need direct access to a shared task store.
- You want to mount a custom data directory.
- You need tighter control over shutdown and scheduler integration.

## Residual gaps

1. No Compose/Kubernetes manifests were added.
2. No secret management system was introduced.
3. No dedicated daemon supervisor is present because the app is still a CLI binary.
