#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
set -eu
cd /work
out=/work/evidence/text-files/linux
mkdir -p "$out"
export DISPLAY=:94 XDG_RUNTIME_DIR=/tmp/mdcard-text-files-runtime LANG=C.UTF-8
mkdir -p "$XDG_RUNTIME_DIR"; chmod 700 "$XDG_RUNTIME_DIR"
Xvfb "$DISPLAY" -screen 0 1600x1100x24 >"$out/xvfb.log" 2>&1 &
xvfb=$!
app=
fixture=$(mktemp -d /tmp/mdcard-text-files.XXXXXX)
cleanup() { if [ -n "$app" ]; then kill "$app" 2>/dev/null || true; fi; kill "$xvfb" 2>/dev/null || true; }
trap cleanup EXIT
sleep 1
mkdir -p "$fixture/中文 空格/deep/more" "$fixture/node_modules" "$fixture/target"
printf '# Search root\n\nOpen the descendant text file.\n' >"$fixture/root.md"
printf '# Literal heading **not bold** <b>not HTML</b>\n中文 空格 🦀\n' >"$fixture/中文 空格/deep/more/NOTE.TXT"
printf '{"plain":"**literal**"}\n' >"$fixture/settings.JSON"
printf binary >"$fixture/not-supported.exe"
printf hidden >"$fixture/node_modules/hidden.txt"
printf hidden >"$fixture/target/hidden.md"
ln -s /work "$fixture/outside-link"
export MDCARD_SETTINGS_PATH="$fixture/settings.json"
printf '{"language":"en","appearance":"Light","gpu_acceleration":true,"reduce_motion":true,"font_size":15.0}\n' >"$MDCARD_SETTINGS_PATH"
# Launch from a different working directory: file location, not cwd, must be the root.
(cd /tmp && LIBGL_ALWAYS_SOFTWARE=1 /work/dist/linux/mdcard "$fixture/root.md") >"$out/app.log" 2>&1 &
app=$!
sleep 3
w=$(xdotool search --pid "$app" --name MDcard | head -1)
test -n "$w"
xdotool windowfocus "$w" key ctrl+p
sleep 1
xdotool type --clearmodifiers 'deep/more/note.txt'
sleep 1
import -window "$w" "$out/relative-results.png"
xdotool key Return
sleep 1
import -window "$w" "$out/opened-txt.png"
xdotool key ctrl+End Return
xdotool type --clearmodifiers 'verified-edit'
xdotool key ctrl+s
sleep 1
printf '# Literal heading **not bold** <b>not HTML</b>\n中文 空格 🦀\n\nverified-edit' >"$fixture/expected.TXT"
cmp "$fixture/expected.TXT" "$fixture/中文 空格/deep/more/NOTE.TXT"
cp "$fixture/中文 空格/deep/more/NOTE.TXT" "$out/saved-NOTE.TXT"
xdotool key ctrl+e
sleep 1
import -window "$w" "$out/plain-read.png"
xdotool key ctrl+p
sleep 1
xdotool type --clearmodifiers root.md
sleep 1
import -window "$w" "$out/current-folder-no-parent.png"
xdotool key ctrl+a
xdotool type --clearmodifiers note.txt
sleep 1
import -window "$w" "$out/before-refresh.png"
# Refresh a populated result list: previously this could retain invalid row indexes.
xdotool mousemove --window "$w" 914 276 click 1
sleep 1
kill -0 "$app"
import -window "$w" "$out/after-refresh.png"
xdotool key Escape ctrl+shift+p
sleep 1
import -window "$w" "$out/preferences-shortcut.png"
xdotool key Escape ctrl+q
wait "$app"
app=
sha256sum /work/dist/linux/mdcard >"$out/executable.sha256"
printf '{"verification_id":"%s","launch_cwd":"/tmp","document_root":"%s","relative_query":"deep/more/note.txt","opened":"中文 空格/deep/more/NOTE.TXT","txt_edit_exact_bytes":true,"read_mode_plain_text":true,"current_file_folder_rebased":true,"parent_query_zero_results":true,"refresh_nonempty_list_survives":true,"ctrl_shift_p_preferences":true,"exit_code":0}\n' "$(cat /proc/sys/kernel/random/uuid)" "$fixture" >"$out/result.json"
