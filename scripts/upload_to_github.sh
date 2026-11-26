#!/usr/bin/env bash
# Upload GitLab CI artifacts (binaries) to a GitHub Release for version .94 (v0.9.4).
# Requires GH_PAT environment variable with a GitHub Personal Access Token (repo scope).
# Set GITHUB_REPO (e.g. "user/repo") and optionally override GITHUB_RELEASE_TAG.

set -euo pipefail

# Configuration – can be overridden via CI variables.
GITHUB_REPO="${GITHUB_REPO:-MustCodeAl/seleniumbase-rs}"
# Use the .94 tag.
GITHUB_RELEASE_TAG="${CI_COMMIT_TAG:-v0.9.4}"  # adjust if needed

# Path(s) to artifact(s) – adjust to match your build output.
ARTIFACTS_DIR="${CI_PROJECT_DIR}/target/release"
FILES=$(find "$ARTIFACTS_DIR" -type f -maxdepth 1)

if [[ -z "$FILES" ]]; then
  echo "No files found to upload in $ARTIFACTS_DIR"
  exit 1
fi

# Create or fetch the GitHub release.
create_release() {
  echo "Creating GitHub release $GITHUB_RELEASE_TAG for $GITHUB_REPO"
  RESPONSE=$(curl -s -X POST \
    -H "Authorization: token $GH_PAT" \
    -H "Accept: application/vnd.github.v3+json" \
    https://api.github.com/repos/$GITHUB_REPO/releases \
    -d "{\"tag_name\": \"$GITHUB_RELEASE_TAG\", \"name\": \"Release $GITHUB_RELEASE_TAG\", \"body\": \"Automated release from GitLab CI for .94.\", \"draft\": false, \"prerelease\": false}")
  echo "$RESPONSE" | jq .
  echo "$RESPONSE" | jq -r .upload_url | sed 's/{?name,label}//'
}

# Try to get existing release upload URL; if not found, create a new release.
UPLOAD_URL=$(curl -s -H "Authorization: token $GH_PAT" \
  https://api.github.com/repos/$GITHUB_REPO/releases/tags/$GITHUB_RELEASE_TAG | jq -r .upload_url | sed 's/{?name,label}//')

if [[ "$UPLOAD_URL" == "null" ]] || [[ -z "$UPLOAD_URL" ]]; then
  UPLOAD_URL=$(create_release)
fi

if [[ -z "$UPLOAD_URL" ]]; then
  echo "Failed to determine upload URL for GitHub release."
  exit 1
fi

echo "Uploading assets to $UPLOAD_URL"

for FILE in $FILES; do
  FILENAME=$(basename "$FILE")
  echo "Uploading $FILENAME..."
  curl -s -X POST \
    -H "Authorization: token $GH_PAT" \
    -H "Content-Type: application/octet-stream" \
    --data-binary "@$FILE" \
    "$UPLOAD_URL?name=$FILENAME"
  echo "Uploaded $FILENAME"
done

echo "All assets uploaded."
