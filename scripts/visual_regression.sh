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
VRT_CONFIG_PATH="$ARTIFACT_DIR/vrt-config.toml"
rm -f "$VRT_CONFIG_PATH"

failed=0

run_case() {
  local id="$1"
  local text="$2"
  shift 2

  local current="$ARTIFACT_DIR/${id}.current.png"
  local baseline="$BASELINE_DIR/${id}.png"
  local diff="$ARTIFACT_DIR/${id}.diff.png"

  local -a cmd=(
    env
    -u CLIIP_SHOW_CONFIG_PATH
    -u CLIIP_SHOW_POLL_INTERVAL_SECS
    -u CLIIP_SHOW_HUD_DURATION_SECS
    -u CLIIP_SHOW_MAX_CHARS_PER_LINE
    -u CLIIP_SHOW_MAX_LINES
    "CLIIP_SHOW_CONFIG_PATH=$VRT_CONFIG_PATH"
  )
  if [[ $# -gt 0 ]]; then
    local override
    for override in "$@"; do
      if [[ ! "$override" =~ ^[A-Za-z_][A-Za-z0-9_]*=.*$ ]]; then
        echo "invalid env override for run_case: $override (expected KEY=VALUE)" >&2
        exit 2
      fi
      cmd+=("$override")
    done
  fi
  cmd+=("$BIN" --render-hud-png --text "$text" --output "$current")
  "${cmd[@]}"

  if $UPDATE; then
    cp "$current" "$baseline"
    rm -f "$diff"
    echo "updated: $baseline"
    return
  fi

  if [[ ! -f "$baseline" ]]; then
    echo "missing baseline: $baseline (run ./scripts/visual_regression.sh --update once)" >&2
    failed=1
    return
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
      return
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
    return
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
}

run_case \
  "ascii_short" \
  "hello clipboard"

run_case \
  "ascii_long" \
  "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

run_case \
  "wide_text" \
  "日本語のコピー内容です"

run_case \
  "multiline" \
  $'line1\nline2\nline3'

# Settings profile: max_lines=2
run_case \
  "setting_max_lines_2_multiline" \
  $'line1\nline2\nline3\nline4' \
  "CLIIP_SHOW_MAX_LINES=2"

# Settings profile: max_chars_per_line=24
run_case \
  "setting_max_chars_24_ascii_long" \
  "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" \
  "CLIIP_SHOW_MAX_CHARS_PER_LINE=24"

# Settings profile: max_chars_per_line=16 and max_lines=2
run_case \
  "setting_compact_text_block" \
  $'abcdefghijklmnopqrstuvwxyz\nabcdefghijklmnopqrstuvwxyz\nabcdefghijklmnopqrstuvwxyz' \
  "CLIIP_SHOW_MAX_CHARS_PER_LINE=16" \
  "CLIIP_SHOW_MAX_LINES=2"

if $UPDATE; then
  echo "visual regression baseline updated"
  exit 0
fi

if [[ "$failed" -ne 0 ]]; then
  exit 1
fi

echo "visual regression passed"
