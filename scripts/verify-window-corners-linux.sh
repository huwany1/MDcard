#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
set -eu
cd /work
out=/work/evidence/window-corners/linux
mkdir -p "$out"
export DISPLAY=:94 XDG_RUNTIME_DIR=/tmp/mdcard-corners-runtime LANG=C.UTF-8
mkdir -p "$XDG_RUNTIME_DIR"; chmod 700 "$XDG_RUNTIME_DIR"
Xvfb :94 -screen 0 1440x1000x24 >"$out/xvfb.log" 2>&1 &
xvfb=$!
wm=''
trap 'if [ -n "$wm" ]; then kill "$wm" 2>/dev/null || true; fi; kill "$xvfb" 2>/dev/null || true' EXIT
sleep 1
openbox >"$out/openbox.log" 2>&1 &
wm=$!
export MDCARD_SETTINGS_PATH=/tmp/mdcard-corners-settings.json
printf '{"appearance":"Dark","accent":1,"gpu_acceleration":false,"font_size":15,"language":"en"}\n' >"$MDCARD_SETTINGS_PATH"
MDCARD_RENDERER_REPORT="$out/renderer.json" MDCARD_SCREENSHOT="$out/opaque-x11-no-compositor.png" MDCARD_AUTO_QUIT_SECONDS=5 ./target/release/mdcard /work/locales/welcome-en.md >"$out/app.log" 2>&1
sha256sum target/release/mdcard >"$out/executable.sha256"
printf '{"date":"2026-10-19","timezone":"Asia/Shanghai","exit_code":0,"environment":"Xvfb / Openbox without compositing manager","opaque_window":true,"rounded_corners":"Not claimed: Linux compositor rounding policy unchanged; no transparency introduced."}\n' >"$out/result.json"
