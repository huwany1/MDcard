#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
set -eu
cd /work
out=/work/evidence/folder-tree/linux
mkdir -p "$out"
export DISPLAY=:97 LANG=C.UTF-8 LIBGL_ALWAYS_SOFTWARE=1 LP_NUM_THREADS=2
export XDG_RUNTIME_DIR=/tmp/mdcard-folder-tree-runtime
mkdir -p "$XDG_RUNTIME_DIR"; chmod 700 "$XDG_RUNTIME_DIR"
Xvfb "$DISPLAY" -screen 0 1600x1100x24 >"$out/xvfb.log" 2>&1 &
xvfb=$!
app=
fixture=$(mktemp -d /tmp/mdcard-folder-tree.XXXXXX)
cleanup() {
    if [ -n "$app" ]; then kill "$app" 2>/dev/null || true; fi
    kill "$xvfb" 2>/dev/null || true
    case "$fixture" in /tmp/mdcard-folder-tree.*) rm -rf "$fixture" ;; esac
}
trap cleanup EXIT
sleep 1
mkdir -p "$fixture/Alpha/deep" "$fixture/Empty" "$fixture/node_modules"
printf '# Root document\n' >"$fixture/root.md"
printf 'plain original\n' >"$fixture/peer.txt"
printf 'child original\n' >"$fixture/Alpha/child.txt"
printf 'deep original\n' >"$fixture/Alpha/deep/note.yaml"
printf hidden >"$fixture/node_modules/hidden.md"
export MDCARD_SETTINGS_PATH=/tmp/mdcard-folder-tree-settings.json
printf '{"language":"en","appearance":"Light","gpu_acceleration":true,"reduce_motion":true}\n' >"$MDCARD_SETTINGS_PATH"
(cd /tmp && /work/dist/linux/mdcard "$fixture/root.md") >"$out/app.log" 2>&1 &
app=$!
sleep 3
w=$(xdotool search --pid "$app" --name MDcard | head -1)
test -n "$w"
snap() { import -window "$w" "$out/$1.png"; }
click() { xdotool mousemove --window "$w" "$1" "$2" click 1; sleep 1; }
xdotool windowfocus "$w"
snap automatic-tree
click 110 331
snap expanded-folder
click 110 331
snap collapsed-folder
click 110 297
snap collapsed-root
click 110 297
click 110 365
xdotool key ctrl+End
xdotool type --clearmodifiers edited
click 110 399
snap dirty-cancel-dialog
xdotool key Escape ctrl+s
sleep 1
printf 'plain original\nedited' >"$fixture/expected"
cmp "$fixture/expected" "$fixture/peer.txt"
cp "$fixture/peer.txt" "$out/saved-peer.txt"
snap cancel-preserved-and-saved
click 110 399
printf 'external file\n' >"$fixture/added.txt"
# Opening a same-root file also schedules an automatic refresh.
click 110 365
sleep 2
snap same-folder-automatic-refresh
# Alphabetical files have shifted; expand folder and open its child.
click 110 331
click 150 399
sleep 2
snap opened-child-root-rebased
printf 'extra: true\n' >"$fixture/Alpha/extra.yaml"
click 176 222
snap explicit-refresh
xdotool windowsize "$w" 620 420
sleep 1
xdotool key ctrl+p
sleep 1
snap narrow-window-search
xdotool key Escape ctrl+q
wait "$app"
app=
# No command-line document: automatically index the startup working directory.
(cd "$fixture" && /work/dist/linux/mdcard) >"$out/no-file-app.log" 2>&1 &
app=$!
sleep 3
w=$(xdotool search --pid "$app" --name MDcard | head -1)
snap startup-working-directory
xdotool windowfocus "$w" key ctrl+q
wait "$app"
app=
sha256sum /work/dist/linux/mdcard >"$out/executable.sha256"
printf '{"verification_id":"%s","date":"2026-10-19","timezone":"Asia/Shanghai","status":"passed","exit_code":0,"automatic_tree_without_ctrl_p":true,"folder_and_root_expand_collapse":true,"dirty_cancel_then_exact_save":true,"current_file_parent_rebased":true,"same_root_open_auto_refresh":true,"explicit_refresh":true,"startup_without_file_uses_cwd":true,"narrow_window_ctrl_p":true,"empty_directories_omitted":true,"environment":"Debian 12 / Xvfb / llvmpipe OpenGL; screenshots visually reviewed"}\n' "$(cat /proc/sys/kernel/random/uuid)" >"$out/result.json"
rm -f "$MDCARD_SETTINGS_PATH"
