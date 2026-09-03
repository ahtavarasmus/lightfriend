#!/usr/bin/env bash
set -euo pipefail

VIDEO_DIR="$(cd "$(dirname "$0")" && pwd)"
SESSION_ID="$(agent-browser session id --scope worktree --prefix lightfriend-video)"
RAW_VIDEO="$VIDEO_DIR/lightfriend-important-things.webm"
FINAL_VIDEO="$VIDEO_DIR/lightfriend-important-things.mp4"

agent-browser --session "$SESSION_ID" open "file://$VIDEO_DIR/index.html"
agent-browser --session "$SESSION_ID" set viewport 1920 1080
agent-browser --session "$SESSION_ID" record start "$RAW_VIDEO"
# Chromium's recorder can take several seconds to begin emitting frames on a
# fresh session. Keep the animation paused until the recorder is warm.
agent-browser --session "$SESSION_ID" wait 12000
agent-browser --session "$SESSION_ID" eval "window.startFilm()"
agent-browser --session "$SESSION_ID" wait 37000
agent-browser --session "$SESSION_ID" record stop
agent-browser --session "$SESSION_ID" close

RAW_DURATION="$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$RAW_VIDEO")"
TRIM_START="$(awk -v duration="$RAW_DURATION" 'BEGIN { trim = duration - 37; if (trim < 0) trim = 0; printf "%.3f", trim }')"

ffmpeg -y -ss "$TRIM_START" -i "$RAW_VIDEO" -t 36 \
  -vf "fps=30,format=yuv420p" \
  -c:v libx264 -preset slow -crf 18 -movflags +faststart \
  -an "$FINAL_VIDEO"

printf 'Rendered %s\n' "$FINAL_VIDEO"
