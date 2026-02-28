#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

usage() {
  cat <<'EOF'
Usage: ./scripts/local_check.sh [options]

Options:
  --position <top|center|bottom>                      HUD position (default: top)
  --scale <0.5-2.0>                                   HUD scale (default: 1.5)
  --color <default|yellow|blue|green|red|purple>      HUD background color (default: blue)
  --text <TEXT>                                        Clipboard text to copy after startup
  --config-path <PATH>                                 Temp config path (default: /tmp/cliip-show-local-check.toml)
  --no-stop-brew                                       Do not stop `brew services cliip-show`
  --no-build                                           Skip `cargo build`
  --no-copy                                            Do not auto-copy test text
  -h, --help                                           Show this help

Environment overrides:
  LOCAL_CHECK_POSITION
  LOCAL_CHECK_SCALE
  LOCAL_CHECK_COLOR
  LOCAL_CHECK_TEXT
  LOCAL_CHECK_CONFIG_PATH
EOF
}

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "local_check requires macOS" >&2
  exit 1
fi

for required in cargo pbcopy; do
  if ! command -v "$required" >/dev/null 2>&1; then
    echo "required command not found: $required" >&2
    exit 1
  fi
done

POSITION="${LOCAL_CHECK_POSITION:-top}"
SCALE="${LOCAL_CHECK_SCALE:-1.5}"
COLOR="${LOCAL_CHECK_COLOR:-blue}"
TEXT="${LOCAL_CHECK_TEXT:-}"
TEXT_EXPLICIT=false
CONFIG_PATH="${LOCAL_CHECK_CONFIG_PATH:-/tmp/cliip-show-local-check.toml}"
STOP_BREW=true
DO_BUILD=true
AUTO_COPY=true

while [[ $# -gt 0 ]]; do
  case "$1" in
    --position)
      [[ $# -ge 2 ]] || { echo "missing value for --position" >&2; exit 2; }
      POSITION="$2"
      shift 2
      ;;
    --scale)
      [[ $# -ge 2 ]] || { echo "missing value for --scale" >&2; exit 2; }
      SCALE="$2"
      shift 2
      ;;
    --color)
      [[ $# -ge 2 ]] || { echo "missing value for --color" >&2; exit 2; }
      COLOR="$2"
      shift 2
      ;;
    --text)
      [[ $# -ge 2 ]] || { echo "missing value for --text" >&2; exit 2; }
      TEXT="$2"
      TEXT_EXPLICIT=true
      shift 2
      ;;
    --config-path)
      [[ $# -ge 2 ]] || { echo "missing value for --config-path" >&2; exit 2; }
      CONFIG_PATH="$2"
      shift 2
      ;;
    --no-stop-brew)
      STOP_BREW=false
      shift
      ;;
    --no-build)
      DO_BUILD=false
      shift
      ;;
    --no-copy)
      AUTO_COPY=false
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

case "$POSITION" in
  top|center|bottom) ;;
  *)
    echo "invalid --position: $POSITION (allowed: top, center, bottom)" >&2
    exit 2
    ;;
esac

case "$COLOR" in
  default|yellow|blue|green|red|purple) ;;
  *)
    echo "invalid --color: $COLOR (allowed: default, yellow, blue, green, red, purple)" >&2
    exit 2
    ;;
esac

if [[ ! "$SCALE" =~ ^[0-9]+([.][0-9]+)?$ ]] || ! awk -v s="$SCALE" 'BEGIN { exit !(s >= 0.5 && s <= 2.0) }'; then
  echo "invalid --scale: $SCALE (allowed range: 0.5 - 2.0)" >&2
  exit 2
fi

if ! $TEXT_EXPLICIT; then
  TEXT="local check: ${POSITION}/${SCALE}/${COLOR}"
fi

if $STOP_BREW && command -v brew >/dev/null 2>&1; then
  echo "[local_check] stopping brew service: cliip-show"
  brew services stop cliip-show >/dev/null 2>&1 || true
fi

if $DO_BUILD; then
  echo "[local_check] cargo build"
  cargo build >/dev/null
fi

BIN="$ROOT_DIR/target/debug/cliip-show"
if [[ ! -x "$BIN" ]]; then
  echo "binary not found: $BIN (run cargo build or remove --no-build)" >&2
  exit 1
fi

echo "[local_check] config path: $CONFIG_PATH"
rm -f "$CONFIG_PATH"
CLIIP_SHOW_CONFIG_PATH="$CONFIG_PATH" "$BIN" --config init >/dev/null
CLIIP_SHOW_CONFIG_PATH="$CONFIG_PATH" "$BIN" --config set hud_position "$POSITION" >/dev/null
CLIIP_SHOW_CONFIG_PATH="$CONFIG_PATH" "$BIN" --config set hud_scale "$SCALE" >/dev/null
CLIIP_SHOW_CONFIG_PATH="$CONFIG_PATH" "$BIN" --config set hud_background_color "$COLOR" >/dev/null
CLIIP_SHOW_CONFIG_PATH="$CONFIG_PATH" "$BIN" --config show

APP_PID=""
cleanup() {
  if [[ -n "$APP_PID" ]]; then
    kill "$APP_PID" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT INT TERM

echo "[local_check] starting cliip-show (Ctrl+C to stop)"
CLIIP_SHOW_CONFIG_PATH="$CONFIG_PATH" "$BIN" &
APP_PID="$!"
sleep 1

if $AUTO_COPY; then
  printf '%s' "$TEXT" | pbcopy
  echo "[local_check] copied text to clipboard: $TEXT"
else
  echo "[local_check] auto-copy disabled (--no-copy)"
fi

wait "$APP_PID"
