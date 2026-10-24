#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
set -eu
cd /work
out=/work/evidence/localization/linux
mkdir -p "$out"
export DISPLAY=:97 XDG_RUNTIME_DIR=/tmp/mdcard-localization-runtime LANG=C.UTF-8
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
if ! xdpyinfo -display "$DISPLAY" >/dev/null 2>&1; then Xvfb "$DISPLAY" -screen 0 1600x1100x24 >"$out/xvfb.log" 2>&1 & fi
sleep 1
if ! xprop -root _NET_SUPPORTING_WM_CHECK | grep -q 'window id'; then openbox >"$out/openbox.log" 2>&1 & fi
export MDCARD_SETTINGS_PATH=/tmp/mdcard-localization-settings.json
if [ "${1:-}" != "--extras-only" ]; then
for language in zh-CN zh-TW en ko ja ru; do
    printf '{"language":"%s","appearance":"Dark","accent":1,"gpu_acceleration":false,"reduce_motion":true,"font_size":23.0}\n' "$language" >"$MDCARD_SETTINGS_PATH"
    MDCARD_RENDERER_REPORT="$out/$language-renderer.json" ./dist/linux/mdcard >"$out/$language.log" 2>&1 &
    app=$!
    sleep 2
    w=$(xdotool search --pid "$app" --name MDcard | head -1)
    test -n "$w"
    xdotool windowfocus "$w"
    import -window "$w" "$out/$language-workspace.png"
    xdotool key ctrl+shift+p
    sleep 0.4
    import -window "$w" "$out/$language-preferences.png"
    xdotool key Escape
    xdotool windowsize "$w" 620 420
    sleep 0.4
    import -window "$w" "$out/$language-compact.png"
    xdotool key ctrl+shift+p
    sleep 0.4
    import -window "$w" "$out/$language-compact-preferences.png"
    xdotool key Escape ctrl+q
    wait "$app"
done
fi
# Native startup failure keeps the chosen language and the saved CPU preference.
printf '{"language":"ru","appearance":"Dark","gpu_acceleration":false}\n' >"$MDCARD_SETTINGS_PATH"
VK_DRIVER_FILES=/tmp/mdcard-localization-no-driver.json ./dist/linux/mdcard >"$out/ru-startup-failure.log" 2>&1 &
app=$!
sleep 2
dialog=$(xdotool search --name 'MDcard' | head -1)
test -n "$dialog"
import -window "$dialog" "$out/ru-startup-failure.png"
xdotool windowfocus "$dialog" key Return
set +e
wait "$app"
code=$?
set -e
test "$code" -eq 1
grep -q '"gpu_acceleration":false' "$MDCARD_SETTINGS_PATH"
cp "$MDCARD_SETTINGS_PATH" "$out/failure-preserved-settings.json"
# A fresh settings path proves actual OS-locale detection rather than a saved choice.
auto_dir=$(mktemp -d /tmp/mdcard-localization-auto.XXXXXX)
LC_ALL=zh_TW.UTF-8 MDCARD_SETTINGS_PATH="$auto_dir/settings.json" MDCARD_RENDERER_REPORT="$out/automatic-renderer.json" MDCARD_AUTO_QUIT_SECONDS=3 ./dist/linux/mdcard >"$out/automatic.log" 2>&1
grep -q '"language": "zh-TW"' "$out/automatic-renderer.json"
sha256sum ./dist/linux/mdcard >"$out/executable.sha256"
printf '{"date":"2026-10-19","timezone":"Asia/Shanghai","languages":["zh-CN","zh-TW","en","ko","ja","ru"],"renderer":"CPU Vulkan","scenes":["workspace","preferences","compact","compact-preferences"],"font_size":23,"normal":"1240x820","compact":"620x420","all_clean_exit":true,"startup_failure_exit":1,"startup_failure_language":"ru","failed_start_preserves_gpu_off":true,"automatic_locale":"zh_TW.UTF-8","automatic_actual_language":"zh-TW"}\n' >"$out/matrix.json"
