#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE="$ROOT/docs/marketing/assets/screenshots"

# A sub-prefix under the configured base path. Every CI run publishes to "preview", which is
# what makes the current gallery something you can look at without it becoming the gallery the
# website serves. Promoting is a separate, deliberate act: the caller passes an empty prefix.
SUB_PREFIX="${1-preview}"

required=(
  AWS_ACCESS_KEY_ID
  AWS_SECRET_ACCESS_KEY
  AWS_REGION
  MARKETING_SCREENSHOTS_S3_BUCKET
  MARKETING_SCREENSHOTS_S3_BASE_PATH
)
for name in "${required[@]}"; do
  if [[ -z "${!name:-}" ]]; then
    echo "error: required environment variable $name is not set" >&2
    exit 1
  fi
done

bucket="$MARKETING_SCREENSHOTS_S3_BUCKET"
if [[ "$bucket" == s3://* || "$bucket" == */* ]]; then
  echo "error: MARKETING_SCREENSHOTS_S3_BUCKET must contain only the bucket name" >&2
  exit 1
fi

base_path="${MARKETING_SCREENSHOTS_S3_BASE_PATH#/}"
base_path="${base_path%/}"
if [[ -z "$base_path" || "$base_path" == "." || "$base_path" == *".."* ]]; then
  echo "error: MARKETING_SCREENSHOTS_S3_BASE_PATH must be a non-root S3 prefix" >&2
  exit 1
fi

[[ -d "$SOURCE" ]] || {
  echo "error: generated marketing screenshot directory is missing: $SOURCE" >&2
  exit 1
}

shopt -s nullglob
screenshots=("$SOURCE"/*.png)
if (( ${#screenshots[@]} == 0 )); then
  echo "error: no generated marketing PNG files found in $SOURCE" >&2
  exit 1
fi

command -v aws >/dev/null 2>&1 || {
  echo "error: the AWS CLI is required to publish marketing screenshots" >&2
  exit 1
}

if [[ -n "$SUB_PREFIX" ]]; then
  destination="s3://$bucket/$base_path/${SUB_PREFIX%/}/"
else
  destination="s3://$bucket/$base_path/"
fi
echo "Uploading ${#screenshots[@]} marketing screenshots to $destination"
aws s3 cp "$SOURCE/" "$destination" \
  --recursive \
  --exclude "*" \
  --include "*.png" \
  --only-show-errors
echo "Uploaded marketing screenshots to $destination"
