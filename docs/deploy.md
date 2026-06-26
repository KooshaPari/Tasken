# Deployment

Tasken is shipped as a Rust CLI binary. The current deployment shape is intentionally simple:

1. Build the release binary in a multi-stage container.
2. Mount a writable data directory.
3. Use `taskkit health` for container-level smoke checks.

## Container defaults

- `TASKEN_DATA_DIR=/data/taskkit`
- `TASKEN_STORE_FILE=store.json`
- `TASKEN_CACHE_DIR=/data/taskkit/cache`
- `TASKEN_CACHE_FILE=cache.json`
- `TASKEN_LOG_FORMAT=json`

## Recommended run command

```bash
docker run --rm \
  -v tasken-data:/data/taskkit \
  taskkit:latest \
  health
```

## Shutdown behavior

The binary finishes the selected command and exits. For future long-running modes, add SIGTERM handling and an explicit drain timeout before process exit.
