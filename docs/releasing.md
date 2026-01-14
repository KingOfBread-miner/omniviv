# Release Flow

This document describes how releases are managed for the Omniviv project.

## Overview

The project uses tag-based releases with version verification. Each component (API, Frontend, Deployment) can be released independently using specific tag prefixes.

## Version Sources

Each component maintains its version in its native configuration file:

| Component  | Version File                      | Version Field          |
|------------|-----------------------------------|------------------------|
| API        | `api/Cargo.toml`                  | `version`              |
| Frontend   | `web/package.json`                | `version`              |
| Deployment | `deployment/mows-manifest.yaml`   | `metadata.version`     |

## Automatic Builds (Main Branch)

When code is pushed to `main`, Docker images are automatically built and tagged as `latest` for components with changes:

- Changes in `api/**` trigger `omniviv-api:latest` build
- Changes in `web/**` trigger `omniviv-frontend:latest` build

## Creating a Release

### API Release

1. Update the version in `api/Cargo.toml`
2. Commit the change
3. Create and push a tag: `git tag api-v<VERSION> && git push origin api-v<VERSION>`

Example:
```bash
# After updating api/Cargo.toml to version = "1.2.0"
git tag api-v1.2.0
git push origin api-v1.2.0
```

This builds and publishes:
- `ghcr.io/<owner>/omniviv-api:1.2.0`
- `ghcr.io/<owner>/omniviv-api:latest`

### Frontend Release

1. Update the version in `web/package.json`
2. Commit the change
3. Create and push a tag: `git tag frontend-v<VERSION> && git push origin frontend-v<VERSION>`

Example:
```bash
# After updating web/package.json to "version": "1.2.0"
git tag frontend-v1.2.0
git push origin frontend-v1.2.0
```

This builds and publishes:
- `ghcr.io/<owner>/omniviv-frontend:1.2.0`
- `ghcr.io/<owner>/omniviv-frontend:latest`

### Deployment Release

1. Update the version in `deployment/mows-manifest.yaml` under `metadata.version`
2. Commit the change
3. Create and push a tag: `git tag mpm-compose-omniviv-v<VERSION> && git push origin mpm-compose-omniviv-v<VERSION>`

Example:
```bash
# After updating deployment/mows-manifest.yaml to version: "1.2.0"
git tag mpm-compose-omniviv-v1.2.0
git push origin mpm-compose-omniviv-v1.2.0
```

## Version Verification

All release workflows verify that the tag version matches the version in the respective configuration file. If they don't match, the build will fail with an error message indicating the mismatch.

## Docker Images

Images are published to GitHub Container Registry (ghcr.io):

| Component | Image                                    |
|-----------|------------------------------------------|
| API       | `ghcr.io/<owner>/omniviv-api`            |
| Frontend  | `ghcr.io/<owner>/omniviv-frontend`       |

## Tag Summary

| Tag Pattern               | Component  | Verification File               |
|---------------------------|------------|---------------------------------|
| `api-v*`                  | API        | `api/Cargo.toml`                |
| `frontend-v*`             | Frontend   | `web/package.json`              |
| `mpm-compose-omniviv-v*`  | Deployment | `deployment/mows-manifest.yaml` |
