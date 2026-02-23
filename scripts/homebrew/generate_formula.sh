#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 || $# -gt 3 ]]; then
  cat <<USAGE >&2
Usage: $0 <github-owner> <version> [output-path]
  github-owner: GitHub user/org name (e.g. somei-tanoue)
  version:      release version without leading v (e.g. 0.1.0)
  output-path:  formula output path (default: ./cliip-show.rb)
USAGE
  exit 1
fi

OWNER="$1"
RAW_VERSION="$2"
VERSION="${RAW_VERSION#v}"
OUT_PATH="${3:-./cliip-show.rb}"
TEMPLATE_PATH="packaging/homebrew/cliip-show.rb.template"
TAG="v${VERSION}"
ARCHIVE_URL="https://github.com/${OWNER}/cliip-show/archive/refs/tags/${TAG}.tar.gz"

if [[ ! -f "${TEMPLATE_PATH}" ]]; then
  echo "Template not found: ${TEMPLATE_PATH}" >&2
  exit 1
fi

TMP_FILE="$(mktemp)"
trap 'rm -f "${TMP_FILE}"' EXIT

if ! curl -fsSL "${ARCHIVE_URL}" -o "${TMP_FILE}"; then
  cat <<ERROR >&2
Failed to download release archive:
  ${ARCHIVE_URL}

確認してください:
  1. GitHub owner が正しいこと（現在: ${OWNER}）
  2. タグ ${TAG} が GitHub に push 済みであること
  3. タグ名が ${VERSION}（v なし）ではなく ${TAG}（v あり）であること

タグ未作成の場合の例:
  git tag ${TAG}
  git push origin ${TAG}
ERROR
  exit 1
fi
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
