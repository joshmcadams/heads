#!/usr/bin/env bash
# Apply the publication settings for this repository using an admin-authenticated gh.
set -euo pipefail

repo='joshmcadams/heads'
command -v gh >/dev/null || { echo 'Install GitHub CLI and run gh auth login.' >&2; exit 1; }
if [ "$(gh api "repos/$repo" --jq '.permissions.admin')" != true ]; then
  echo "Repository admin access is required for $repo." >&2
  exit 1
fi

# Do not install required checks until every configured job has passed on main.
checks=$(gh api "repos/$repo/commits/main/check-runs" --paginate \
  --jq '.check_runs[] | select(.app.slug == "github-actions" and .status == "completed" and .conclusion == "success") | .name')
for required in \
  'Test (ubuntu-latest, stable)' \
  'Test (macos-latest, stable)' \
  'Test (ubuntu-latest, 1.74.0)'; do
  if ! printf '%s\n' "$checks" | grep -Fxq "$required"; then
    echo "Wait for a successful $required check on main, then rerun this script." >&2
    exit 1
  fi
done

gh api --method PATCH "repos/$repo" --input - >/dev/null <<'JSON'
{
  "description": "Fast-forward clean Git repositories under a folder. A dependency-free Rust CLI for Linux and macOS.",
  "default_branch": "main",
  "has_issues": true,
  "has_projects": false,
  "has_wiki": false,
  "allow_squash_merge": true,
  "allow_merge_commit": false,
  "allow_rebase_merge": false,
  "delete_branch_on_merge": true,
  "allow_update_branch": true,
  "squash_merge_commit_title": "PR_TITLE",
  "squash_merge_commit_message": "PR_BODY",
  "security_and_analysis": {
    "secret_scanning": {"status": "enabled"},
    "secret_scanning_push_protection": {"status": "enabled"}
  }
}
JSON

gh api --method PUT "repos/$repo/topics" --input - >/dev/null <<'JSON'
{"names": ["git", "rust", "cli", "developer-tools", "linux", "macos"]}
JSON

gh api --method PUT "repos/$repo/actions/permissions/workflow" --input - >/dev/null <<'JSON'
{"default_workflow_permissions": "read", "can_approve_pull_request_reviews": false}
JSON

gh api --method PUT "repos/$repo/vulnerability-alerts" >/dev/null
gh api --method PUT "repos/$repo/private-vulnerability-reporting" >/dev/null

# Zero approvals lets a sole maintainer merge their own PR after CI passes.
# This replaces classic main branch protection with the policy below.
gh api --method PUT "repos/$repo/branches/main/protection" --input - >/dev/null <<'JSON'
{
  "required_status_checks": {
    "strict": true,
    "contexts": [],
    "checks": [
      {"context": "Test (ubuntu-latest, stable)", "app_id": 15368},
      {"context": "Test (macos-latest, stable)", "app_id": 15368},
      {"context": "Test (ubuntu-latest, 1.74.0)", "app_id": 15368}
    ]
  },
  "enforce_admins": true,
  "required_pull_request_reviews": {
    "dismiss_stale_reviews": true,
    "require_code_owner_reviews": false,
    "required_approving_review_count": 0
  },
  "restrictions": null,
  "required_linear_history": true,
  "allow_force_pushes": false,
  "allow_deletions": false,
  "required_conversation_resolution": true
}
JSON

echo "Configured $repo: metadata, merge policy, Actions, security, and main protection."
