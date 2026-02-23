#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 || $# -gt 3 ]]; then
  cat <<USAGE >&2
Usage: $0 <github-owner> <version> [output-path]
  github-owner: GitHub user/org name (e.g. somei-tanoue)
  version:      release version without leading v (e.g. 0.1.0)
  output-path:  formula output path (default: ./clip-show.rb)
USAGE
  exit 1
fi

OWNER="$1"
VERSION="$2"
OUT_PATH="${3:-./clip-show.rb}"
TEMPLATE_PATH="packaging/homebrew/clip-show.rb.template"
ARCHIVE_URL="https://github.com/${OWNER}/clip-show/archive/refs/tags/v${VERSION}.tar.gz"

if [[ ! -f "${TEMPLATE_PATH}" ]]; then
  echo "Template not found: ${TEMPLATE_PATH}" >&2
  exit 1
fi

TMP_FILE="$(mktemp)"
trap 'rm -f "${TMP_FILE}"' EXIT

curl -fsSL "${ARCHIVE_URL}" -o "${TMP_FILE}"
SHA256="$(shasum -a 256 "${TMP_FILE}" | awk '{print $1}')"

mkdir -p "$(dirname "${OUT_PATH}")"

sed \
  -e "s/{{OWNER}}/${OWNER}/g" \
  -e "s/{{VERSION}}/${VERSION}/g" \
  -e "s/{{SHA256}}/${SHA256}/g" \
  "${TEMPLATE_PATH}" > "${OUT_PATH}"

echo "Generated formula: ${OUT_PATH}"
echo "URL: ${ARCHIVE_URL}"
echo "SHA256: ${SHA256}"
