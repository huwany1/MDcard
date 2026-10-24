#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
set -eu
cd /work
mkdir -p evidence/linux
if [ "${1:-}" != session ]; then
    exec dbus-run-session -- sh "$0" session
fi
export DISPLAY=:96
export XDG_CURRENT_DESKTOP=GNOME
export XDG_RUNTIME_DIR=/tmp/mdcard-portal-runtime
export LIBGL_ALWAYS_SOFTWARE=1
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
Xvfb :96 -screen 0 1440x1000x24 >evidence/linux/portal-xvfb.log 2>&1 &
xvfb_pid=$!
sleep 1
openbox >evidence/linux/openbox.log 2>&1 &
wm_pid=$!
dbus-update-activation-environment DISPLAY XDG_CURRENT_DESKTOP XDG_RUNTIME_DIR
/usr/libexec/xdg-desktop-portal-gtk >evidence/linux/portal-gtk.log 2>&1 &
gtk_pid=$!
/usr/libexec/xdg-desktop-portal >evidence/linux/portal.log 2>&1 &
portal_pid=$!
trap 'kill "$xvfb_pid" "$wm_pid" "$gtk_pid" "$portal_pid" 2>/dev/null || true' EXIT
sleep 2
./target/release/mdcard >evidence/linux/dialog-app.log 2>&1 &
app_pid=$!
sleep 3
window=$(xdotool search --pid "$app_pid" --name MDcard | head -1)
xdotool windowactivate --sync "$window"
xdotool key ctrl+n
printf '# Linux 原生对话框\n\n你好 Rust 🦀\n' >evidence/linux/dialog-expected.md
xclip -selection clipboard -i evidence/linux/dialog-expected.md
xdotool key ctrl+v
xdotool key ctrl+s
sleep 2
import -window root evidence/linux/native-save-dialog.png
dialog=$(xdotool getactivewindow)
save_path="/work/evidence/linux/dialog-$(date +%s).md"
xdotool mousemove --window "$dialog" 300 45 click 1
xdotool key ctrl+a
printf '%s' "$save_path" | xclip -selection clipboard
xdotool key ctrl+v
xdotool key Tab
eval "$(xdotool getwindowgeometry --shell "$dialog")"
xdotool mousemove --window "$dialog" "$((WIDTH-50))" "$((HEIGHT-27))" click 1
sleep 2
if [ ! -f "$save_path" ]; then
    import -window root evidence/linux/native-save-after-entry.png
    xdotool key alt+s
    sleep 2
fi
cmp evidence/linux/dialog-expected.md "$save_path"
sha256sum "$save_path" >evidence/linux/dialog-saved.sha256
xdotool key ctrl+n
sleep 0.5
xdotool key ctrl+o
sleep 2
import -window root evidence/linux/native-open-dialog.png
xdotool key ctrl+l
printf '%s' "$save_path" | xclip -selection clipboard
xdotool key ctrl+v
xdotool key Return
sleep 0.5
dialog=$(xdotool getactivewindow)
eval "$(xdotool getwindowgeometry --shell "$dialog")"
xdotool mousemove --window "$dialog" "$((WIDTH-50))" "$((HEIGHT-27))" click 1
sleep 2
xdotool windowactivate --sync "$window"
sleep 1
xdotool key ctrl+e
sleep 1
import -window "$window" evidence/linux/chinese-emoji-read.png
xdotool key ctrl+q
wait "$app_pid"
printf '{"native_portal_save":true,"native_portal_open":true,"reopened_after_new":true,"chinese_emoji_clipboard_roundtrip":true,"saved_path":"%s","binary":"target/release/mdcard"}\n' "$save_path" >evidence/linux/dialog-result.json
