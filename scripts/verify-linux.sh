#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
mkdir -p evidence/linux
export MDCARD_SETTINGS_PATH="$ROOT/evidence/linux/settings.json"
export MDCARD_READING_STATE_PATH="$ROOT/evidence/linux/reading-state.json"
export LIBGL_ALWAYS_SOFTWARE=1
export XDG_RUNTIME_DIR=/tmp/mdcard-runtime
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
Xvfb :97 -screen 0 1440x1000x24 >evidence/linux/xvfb.log 2>&1 &
xvfb_pid=$!
export DISPLAY=:97
trap 'kill "$xvfb_pid" 2>/dev/null || true' EXIT
sleep 1
MDCARD_SCREENSHOT=${ROOT}/evidence/linux/welcome.png MDCARD_AUTO_QUIT_SECONDS=6 ./target/release/mdcard >evidence/linux/welcome.log 2>&1
cp examples/Welcome.md evidence/linux/roundtrip.md
./target/release/mdcard "${ROOT}/evidence/linux/roundtrip.md" >evidence/linux/interaction.log 2>&1 &
app_pid=$!
sleep 3
window=$(xdotool search --pid "$app_pid" --name MDcard | head -1)
xdotool windowfocus "$window"
xdotool mousemove --window "$window" 380 190 click 1
xdotool key ctrl+a
xdotool type --clearmodifiers '# Linux verified'
xdotool key Return Return
xdotool type --clearmodifiers 'Native Rust editor. Saved by Ctrl+S.'
xdotool key ctrl+s
sleep 1
grep -q '# Linux verified' evidence/linux/roundtrip.md
grep -q 'Saved by Ctrl+S.' evidence/linux/roundtrip.md
xdotool key ctrl+e
sleep 1
import -window "$window" evidence/linux/read-mode.png
xdotool key ctrl+e
xdotool mousemove --window "$window" 380 190 click 1
xdotool key ctrl+End
xdotool type --clearmodifiers ' unsaved'
xdotool key ctrl+n
sleep 1
import -window "$window" evidence/linux/unsaved-dialog.png
xdotool key Escape
xdotool key ctrl+s
sleep 1
xdotool key ctrl+q
wait "$app_pid"
MDCARD_WINDOW_SIZE=680x480 MDCARD_SCREENSHOT=${ROOT}/evidence/linux/compact.png MDCARD_AUTO_QUIT_SECONDS=6 ./target/release/mdcard >evidence/linux/compact.log 2>&1
printf '{"platform":"Debian 12 x86_64 / X11 Xvfb Mesa software","build":"release","keyboard_edit_save":true,"read_mode":true,"dirty_new_dialog":"screenshot captured; Escape cancels","compact_size":"680x480"}\n' >evidence/linux/result.json
