#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "visual regression requires macOS" >&2
  exit 1
fi

UPDATE=false
if [[ "${1:-}" == "--update" ]]; then
  UPDATE=true
elif [[ $# -gt 0 ]]; then
  echo "Usage: $0 [--update]" >&2
  exit 2
fi

BASELINE_DIR="$ROOT_DIR/tests/visual/baseline"
ARTIFACT_DIR="$ROOT_DIR/tests/visual/artifacts"
mkdir -p "$BASELINE_DIR" "$ARTIFACT_DIR"
rm -f "$ARTIFACT_DIR"/*.png

cargo build >/dev/null
BIN="$ROOT_DIR/target/debug/cliip-show"
MAX_DIFF_PERMILLE="${MAX_DIFF_PERMILLE:-120}" # 120/1000 = 12%

case_ids=(
  "ascii_short"
  "ascii_long"
  "wide_text"
  "multiline"
)

case_texts=(
  "hello clipboard"
  "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
  "日本語のコピー内容です"
  $'line1\nline2\nline3'
)

failed=0
for i in "${!case_ids[@]}"; do
  id="${case_ids[$i]}"
  text="${case_texts[$i]}"
  current="$ARTIFACT_DIR/${id}.current.png"
  baseline="$BASELINE_DIR/${id}.png"
  diff="$ARTIFACT_DIR/${id}.diff.png"

  "$BIN" --render-hud-png --text "$text" --output "$current"

  if $UPDATE; then
    cp "$current" "$baseline"
    rm -f "$diff"
    echo "updated: $baseline"
    continue
  fi

  if [[ ! -f "$baseline" ]]; then
    echo "missing baseline: $baseline (run ./scripts/visual_regression.sh --update once)" >&2
    failed=1
    continue
  fi

  if diff_output=$("$BIN" --diff-png --baseline "$baseline" --current "$current" --output "$diff" 2>&1); then
    diff_pixels="$(echo "$diff_output" | sed -n 's/.*diff_pixels=\([0-9][0-9]*\).*/\1/p' | tail -n1)"
    total_pixels="$(echo "$diff_output" | sed -n 's/.*total_pixels=\([0-9][0-9]*\).*/\1/p' | tail -n1)"
    if [[ -z "$diff_pixels" || -z "$total_pixels" ]]; then
      echo "ng: $id" >&2
      echo "  baseline: $baseline" >&2
      echo "  current : $current" >&2
      echo "  diff    : failed to parse diff output" >&2
      echo "  reason  : $diff_output" >&2
      failed=1
      continue
    fi
  else
    echo "ng: $id" >&2
    echo "  baseline: $baseline" >&2
    echo "  current : $current" >&2
    echo "  diff    : failed to generate" >&2
    if [[ -n "$diff_output" ]]; then
      echo "  reason  : $diff_output" >&2
    fi
    failed=1
    continue
  fi

  if [[ "$diff_pixels" -eq 0 ]]; then
    rm -f "$diff"
    echo "ok: $id"
  else
    if (( diff_pixels * 1000 <= total_pixels * MAX_DIFF_PERMILLE )); then
      echo "ok: $id (within tolerance ${diff_pixels}/${total_pixels}, max=${MAX_DIFF_PERMILLE}/1000)"
      rm -f "$diff"
    else
      echo "ng: $id" >&2
      echo "  baseline: $baseline" >&2
      echo "  current : $current" >&2
      echo "  diff    : $diff" >&2
      echo "  pixels  : ${diff_pixels}/${total_pixels} (max=${MAX_DIFF_PERMILLE}/1000)" >&2
      failed=1
    fi
  fi
done

if [[ "$failed" -ne 0 ]]; then
  exit 1
fi

echo "visual regression passed"
