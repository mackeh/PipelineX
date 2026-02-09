# GitHub App PR Analysis Webhook

PipelineX dashboard exposes a webhook endpoint that analyzes CI workflow files changed in a pull request and posts (or updates) a comment on the PR.

## Endpoint

`POST /api/github/app/webhook`

## Supported GitHub Events

- `pull_request` with actions:
  - `opened`
  - `reopened`
  - `synchronize`
  - `ready_for_review`

Draft pull requests are ignored.

## Required Environment

- `GITHUB_APP_TOKEN` (recommended)  
  Token with repository read permissions and issue/PR comment write permissions.
- `GITHUB_APP_WEBHOOK_SECRET` (recommended)  
  HMAC secret for validating `x-hub-signature-256`.

Fallbacks:

- If `GITHUB_APP_TOKEN` is not set, `GITHUB_TOKEN` is used.
- If `GITHUB_APP_WEBHOOK_SECRET` is not set, `GITHUB_WEBHOOK_SECRET` is used.

## Behavior

On supported PR events, PipelineX:

1. Lists changed files in the PR.
2. Filters to supported CI workflow formats.
3. Fetches each changed workflow file at PR head SHA.
4. Runs `pipelinex analyze --format json` on fetched content.
5. Creates or updates a PR comment (idempotent marker-based update).

The comment includes:

- Provider, current/optimized duration, potential savings
- Severity counts
- Top critical/high hotspots

## Notes

- Maximum analyzed workflow files per webhook run is capped to avoid long-running comment updates.
- Files that fail fetch or parse are listed in an "Analysis Warnings" section.
