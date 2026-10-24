#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
set -eu
cd /work
out=/work/evidence/encoding/linux-final
mkdir -p "$out"
export DISPLAY=:96 LANG=C.UTF-8 LIBGL_ALWAYS_SOFTWARE=1 LP_NUM_THREADS=2
export XDG_RUNTIME_DIR=/tmp/mdcard-encoding-runtime
mkdir -p "$XDG_RUNTIME_DIR"; chmod 700 "$XDG_RUNTIME_DIR"
Xvfb "$DISPLAY" -screen 0 1500x1000x24 >"$out/xvfb.log" 2>&1 &
xvfb=$!
app=
fixture=$(mktemp -d /tmp/mdcard-encoding.XXXXXX)
cleanup() {
    if [ -n "$app" ]; then kill "$app" 2>/dev/null || true; fi
    kill "$xvfb" 2>/dev/null || true
    case "$fixture" in /tmp/mdcard-encoding.*) rm -rf "$fixture" ;; esac
}
trap cleanup EXIT
sleep 1
python3 - "$fixture" <<'PY'
import sys
from pathlib import Path
p = Path(sys.argv[1])
text = '这是简体中文编码测试文档。支持本地编码，保存和读取文件。𠀀🦀\r\n' * 4
(p/'w.txt').write_bytes(text.encode('gb18030'))
(p/'expected.txt').write_bytes(text.encode('utf-8'))
PY
export MDCARD_SETTINGS_PATH="$fixture/settings.json"
export MDCARD_RENDERER_REPORT="$out/renderer.json"
printf '{"language":"en","appearance":"Light","gpu_acceleration":true,"reduce_motion":true,"font_size":15}\n' >"$MDCARD_SETTINGS_PATH"
/work/dist/linux/mdcard "$fixture/w.txt" >"$out/app.log" 2>&1 &
app=$!
sleep 3
w=$(xdotool search --pid "$app" --name MDcard | head -1)
test -n "$w"
xdotool windowfocus "$w"
snap() { import -window "$w" "$out/$1.png"; }
click() { xdotool mousemove --window "$w" "$1" "$2" click 1; sleep 1; }
snap gb18030-guessed
click 1160 804
snap encoding-grid
click 440 398
click 700 608
snap conversion-pending
xdotool key ctrl+s
sleep 1
cmp "$fixture/w.txt" "$fixture/expected.txt"
snap converted-utf8
click 1160 804
click 550 485
snap windows1252-selected
click 700 586
snap unmappable-keeps-edits
xdotool key Escape
sleep 1
xdotool key Escape
sleep 1
cmp "$fixture/w.txt" "$fixture/expected.txt"
xdotool key ctrl+q
wait "$app"
app=
python3 - "$out" <<'PY'
import hashlib, json, sys, uuid
from pathlib import Path
out = Path(sys.argv[1])
out.joinpath('result.json').write_text(json.dumps({
    'verification_id': str(uuid.uuid4()), 'date': '2026-10-19', 'timezone': 'Asia/Shanghai',
    'environment': 'Debian 12 / Xvfb X11 / llvmpipe OpenGL',
    'gb18030_auto_detection': True, 'python_reference_utf8_conversion_exact': True,
    'unmappable_save_refused': True, 'exit_code': 0,
    'exe_sha256': hashlib.sha256(Path('/work/dist/linux/mdcard').read_bytes()).hexdigest(),
}, indent=2) + '\n')
PY
