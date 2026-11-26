#!/usr/bin/env bash
# Upload GitLab CI release artifacts to a GitHub Release.
# Requires GH_PAT environment variable with a GitHub Personal Access Token (repo contents write scope).
# Set GITHUB_REPO (e.g. "owner/repo") and optionally override GITHUB_RELEASE_TAG or ARTIFACTS_DIR.

set -euo pipefail

if [[ -z "${GH_PAT:-}" ]]; then
  echo "Error: GH_PAT environment variable is not set." >&2
  exit 1
fi

GITHUB_REPO="${GITHUB_REPO:-MustCodeAl/seleniumbase-rs}"
GITHUB_RELEASE_TAG="${GITHUB_RELEASE_TAG:-${CI_COMMIT_TAG:-v0.9.4}}"
PROJECT_ROOT="${CI_PROJECT_DIR:-.}"
ARTIFACTS_DIR="${ARTIFACTS_DIR:-${PROJECT_ROOT}/dist}"

collect_dist_files() {
  local dir="$1"
  shopt -s nullglob
  for candidate in \
    "$dir"/*.tgz \
    "$dir"/*.tar.gz \
    "$dir"/*.zip \
    "$dir"/SHA256SUMS \
    "$dir"/*.sha256; do
    if [[ -f "$candidate" ]]; then
      FILES+=("$candidate")
    fi
  done
  shopt -u nullglob
}

collect_fallback_binaries() {
  local dir="$1"
  for bin in sbrs sbrs-mcp sbrs.exe sbrs-mcp.exe; do
    if [[ -f "${dir}/${bin}" ]]; then
      FILES+=("${dir}/${bin}")
    fi
  done
}

FILES=()
if [[ -d "$ARTIFACTS_DIR" ]]; then
  collect_dist_files "$ARTIFACTS_DIR"
  if [[ "${#FILES[@]}" -eq 0 ]]; then
    collect_fallback_binaries "$ARTIFACTS_DIR"
  fi
elif [[ -d "${PROJECT_ROOT}/target/release" ]]; then
  ARTIFACTS_DIR="${PROJECT_ROOT}/target/release"
  collect_fallback_binaries "$ARTIFACTS_DIR"
fi

if [[ "${#FILES[@]}" -eq 0 ]]; then
  echo "No release artifacts found to upload in $ARTIFACTS_DIR" >&2
  exit 1
fi

create_release() {
  echo "Creating GitHub release $GITHUB_RELEASE_TAG for $GITHUB_REPO" >&2
  local payload response status body
  payload=$(jq -n \
    --arg tag "$GITHUB_RELEASE_TAG" \
    --arg name "Release $GITHUB_RELEASE_TAG" \
    --arg body "Automated release from GitLab CI for $GITHUB_RELEASE_TAG." \
    '{tag_name: $tag, name: $name, body: $body, draft: false, prerelease: false}')

  response=$(curl -sSL -w "\n%{http_code}" -X POST \
    -H "Authorization: Bearer $GH_PAT" \
    -H "Accept: application/vnd.github+json" \
    "https://api.github.com/repos/${GITHUB_REPO}/releases" \
    -d "$payload")

  status="${response##*$'\n'}"
  body="${response%$'\n'*}"
  if [[ "$status" -ge 400 ]]; then
    echo "Failed to create release $GITHUB_RELEASE_TAG (HTTP $status):" >&2
    printf '%s\n' "$body" | jq . >&2 || printf '%s\n' "$body" >&2
    return 1
  fi

  printf '%s\n' "$body" | jq '{id, tag_name, html_url}' >&2
  printf '%s\n' "$body"
}

fetch_or_create_release() {
  local response status body
  response=$(curl -sSL -w "\n%{http_code}" \
    -H "Authorization: Bearer $GH_PAT" \
    -H "Accept: application/vnd.github+json" \
    "https://api.github.com/repos/${GITHUB_REPO}/releases/tags/${GITHUB_RELEASE_TAG}")

  status="${response##*$'\n'}"
  body="${response%$'\n'*}"
  if [[ "$status" == "404" ]]; then
    create_release
    return
  fi
  if [[ "$status" -ge 400 ]]; then
    echo "Failed to query release $GITHUB_RELEASE_TAG (HTTP $status):" >&2
    printf '%s\n' "$body" | jq . >&2 || printf '%s\n' "$body" >&2
    return 1
  fi

  printf '%s\n' "$body"
}

RELEASE_JSON=$(fetch_or_create_release)
UPLOAD_URL=$(printf '%s\n' "$RELEASE_JSON" | jq -r '.upload_url // empty' | sed 's/{?name,label}//')

if [[ -z "$UPLOAD_URL" ]]; then
  echo "Failed to determine upload URL for GitHub release $GITHUB_RELEASE_TAG." >&2
  exit 1
fi

echo "Uploading ${#FILES[@]} asset(s) to $GITHUB_REPO ($GITHUB_RELEASE_TAG)..." >&2

for FILE in "${FILES[@]}"; do
  FILENAME=$(basename "$FILE")
  EXISTING_ASSET_ID=$(printf '%s\n' "$RELEASE_JSON" \
    | jq -r --arg name "$FILENAME" '.assets[]? | select(.name == $name) | .id' \
    | head -n 1)

  if [[ -n "$EXISTING_ASSET_ID" ]]; then
    echo "Replacing existing asset $FILENAME (id=$EXISTING_ASSET_ID)..." >&2
    curl -fsSL -X DELETE \
      -H "Authorization: Bearer $GH_PAT" \
      -H "Accept: application/vnd.github+json" \
      "https://api.github.com/repos/${GITHUB_REPO}/releases/assets/${EXISTING_ASSET_ID}" >/dev/null
  fi

  echo "Uploading $FILENAME..." >&2
  curl -fsSL -X POST \
    -H "Authorization: Bearer $GH_PAT" \
    -H "Accept: application/vnd.github+json" \
    -H "Content-Type: application/octet-stream" \
    --data-binary "@$FILE" \
    "${UPLOAD_URL}?name=${FILENAME}" >/dev/null
  echo "Uploaded $FILENAME" >&2
done

echo "All assets uploaded." >&2
