#!/usr/bin/env sh
set -eu

ENGINE=${FILLER_GAME_ENGINE:-./linux_game_engine}
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
PROJECT_DIR=$(dirname "$SCRIPT_DIR")
OUT_DIR="$PROJECT_DIR/replays"

if [ "$#" -eq 0 ]; then
  echo "Usage (inside /filler):" >&2
  echo "  sh /filler/solution/scripts/capture-replay.sh -f maps/map00 -p1 /filler/solution/target/docker-linux/release/filler -p2 linux_robots/wall_e" >&2
  exit 2
fi

mkdir -p "$OUT_DIR"
STAMP=$(date +%Y%m%d-%H%M%S)
OUT="$OUT_DIR/match-$STAMP.log"

echo "Saving game_engine output to: $OUT" >&2
"$ENGINE" "$@" 2>&1 | tee "$OUT"
