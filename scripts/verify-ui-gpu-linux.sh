#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
out=${ROOT}/evidence/ui-gpu/linux
mkdir -p "$out"
export DISPLAY=:96 XDG_RUNTIME_DIR=/tmp/mdcard-ui-runtime MDCARD_SETTINGS_PATH=/tmp/mdcard-ui-settings.json
mkdir -p "$XDG_RUNTIME_DIR"; chmod 700 "$XDG_RUNTIME_DIR"
# Use a dedicated test display. The caller's desktop and configuration stay untouched.
if ! xdpyinfo -display "$DISPLAY" >/dev/null 2>&1; then Xvfb "$DISPLAY" -screen 0 1600x1100x24 >"$out/xvfb.log" 2>&1 & fi
sleep 1
if ! xprop -root _NET_SUPPORTING_WM_CHECK | grep -q 'window id'; then openbox >"$out/openbox.log" 2>&1 & fi
cat >"$MDCARD_SETTINGS_PATH" <<'JSON'
{"appearance":"Dark","accent":1,"gpu_acceleration":false,"reduce_motion":false,"font_size":23.0}
JSON
MDCARD_RENDERER_REPORT="$out/cpu-renderer.json" ./target/release/mdcard "${ROOT}/examples/图片验证.md" >"$out/cpu.log" 2>&1 &
app=$!
sleep 3
w=$(xdotool search --pid "$app" --name MDcard | head -1)
xdotool windowfocus "$w"
import -window "$w" "$out/cpu-dark-23.png"
xdotool key ctrl+comma; sleep 1
import -window "$w" "$out/preferences-shortcut.png"
xdotool key Escape
xdotool key ctrl+e; sleep 1
xdotool mousemove --window "$w" 1000 650 click --repeat 16 --delay 40 5
sleep 1
import -window "$w" "$out/dark-code-preview.png"
xdotool key ctrl+q
wait "$app"
# Failure path: make the Vulkan loader expose no drivers. Off stays off.
set +e
VK_DRIVER_FILES=/tmp/mdcard-no-such-driver.json ./target/release/mdcard >"$out/no-cpu-startup.log" 2>&1 &
app=$!
sleep 3
dialog=$(xdotool search --name 'MDcard could not start' | head -1)
if [ -z "$dialog" ]; then echo 'No visible startup failure dialog' >&2; exit 1; fi
import -window "$dialog" "$out/no-cpu-startup-dialog.png"
xdotool windowfocus "$dialog" key Return
wait "$app"
code=$?
set -e
[ "$code" -eq 1 ]
grep -q '"gpu_acceleration":false' "$MDCARD_SETTINGS_PATH"
cp "$MDCARD_SETTINGS_PATH" "$out/off-after-failed-start.json"
# Animated slide, source/preview crossfade and switch use the same shared UI code.
cat >"$MDCARD_SETTINGS_PATH" <<'JSON'
{"appearance":"Dark","accent":1,"gpu_acceleration":true,"reduce_motion":false,"font_size":15.0}
JSON
LIBGL_ALWAYS_SOFTWARE=1 MDCARD_RENDERER_REPORT="$out/opengl-renderer.json" ./target/release/mdcard >"$out/animation-app.log" 2>&1 &
app=$!
sleep 3
w=$(xdotool search --pid "$app" --name MDcard | head -1)
xdotool windowfocus "$w"
import -window "$w" "$out/sidebar-frame-0.png"
xdotool key ctrl+b
import -window "$w" /tmp/mdcard-animation-1.miff
import -window "$w" /tmp/mdcard-animation-2.miff
sleep 0.25
import -window "$w" /tmp/mdcard-animation-3.miff
for frame in 1 2 3; do convert "/tmp/mdcard-animation-$frame.miff" "$out/sidebar-frame-$frame.png"; done
convert -delay 15 "$out/sidebar-frame-0.png" "$out/sidebar-frame-1.png" "$out/sidebar-frame-2.png" "$out/sidebar-frame-3.png" -loop 0 "$out/sidebar-animation.gif"
xdotool key ctrl+q
wait "$app"
printf '{"date":"%s","timezone":"UTC","cpu_only":"Vulkan llvmpipe / DeviceType::Cpu","gpu_off_startup_failure_exit":1,"failure_dialog_visible":true,"failure_preserves_gpu_off":true,"ctrl_comma_preferences":true,"font_size":23,"animation_frames":4}\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >"$out/result.json"
sha256sum target/release/mdcard >"$out/executable.sha256"
