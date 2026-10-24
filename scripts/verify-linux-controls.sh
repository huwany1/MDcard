#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
set -eu
cd /work
export DISPLAY=:95 LIBGL_ALWAYS_SOFTWARE=1
Xvfb :95 -screen 0 1600x1100x24 >evidence/linux/controls-xvfb.log 2>&1 &
xvfb_pid=$!
sleep 1
openbox >evidence/linux/controls-openbox.log 2>&1 &
wm_pid=$!
trap 'kill "$xvfb_pid" "$wm_pid" 2>/dev/null || true' EXIT
sleep 1
./target/release/mdcard >evidence/linux/controls-app.log 2>&1 &
app_pid=$!
sleep 3
window=$(xdotool search --pid "$app_pid" --name MDcard | head -1)
xdotool windowactivate --sync "$window"
xdotool mousemove --window "$window" 71 27 click 1
sleep 1
xprop -id "$window" _NET_WM_STATE >evidence/linux/maximized-state.txt
grep -q MAXIMIZED_HORZ evidence/linux/maximized-state.txt
xdotool mousemove --window "$window" 71 27 click 1
sleep 1
xprop -id "$window" _NET_WM_STATE >evidence/linux/restored-state.txt
if grep -q MAXIMIZED_HORZ evidence/linux/restored-state.txt; then exit 1; fi
xdotool getwindowgeometry --shell "$window" >evidence/linux/before-drag.txt
xdotool mousemove --window "$window" 500 27 mousedown 1
sleep 0.2
xdotool mousemove_relative -- 10 5
sleep 0.2
xdotool mousemove_relative -- 60 30
sleep 0.2
xdotool mouseup 1
sleep 0.5
xdotool getwindowgeometry --shell "$window" >evidence/linux/after-drag.txt
if cmp -s evidence/linux/before-drag.txt evidence/linux/after-drag.txt; then exit 1; fi
eval "$(xdotool getwindowgeometry --shell "$window")"
import -window "$window" evidence/linux/before-resize.png
xdotool mousemove --window "$window" "$((WIDTH-20))" "$((HEIGHT-15))"
xdotool getmouselocation --shell >evidence/linux/resize-pointer.txt
sleep 0.5
xdotool mousedown 1
sleep 0.2
xdotool mousemove_relative -- 10 5
sleep 0.2
xdotool mousemove_relative -- 50 30
sleep 0.2
xdotool mouseup 1
sleep 0.5
xdotool getwindowgeometry --shell "$window" >evidence/linux/after-resize.txt
if cmp -s evidence/linux/after-drag.txt evidence/linux/after-resize.txt; then exit 1; fi
xdotool mousemove --window "$window" 48 27 click 1
sleep 0.5
xprop -id "$window" WM_STATE >evidence/linux/minimized-state.txt
grep -q Iconic evidence/linux/minimized-state.txt
xdotool windowmap "$window" windowactivate --sync "$window"
xdotool key ctrl+n
xdotool type --clearmodifiers 'dirty close'
xdotool mousemove --window "$window" 25 27 click 1
sleep 0.5
kill -0 "$app_pid"
import -window "$window" evidence/linux/dirty-red-close.png
eval "$(xdotool getwindowgeometry --shell "$window")"
xdotool mousemove --window "$window" "$((WIDTH/2-88))" "$((HEIGHT/2+55))" click 1
wait "$app_pid"
printf '{"window_manager":"Openbox / X11","maximize_restore":true,"titlebar_drag":true,"resize_grip":true,"minimize_restore":true,"dirty_red_close_protected":true}\n' >evidence/linux/controls-result.json
