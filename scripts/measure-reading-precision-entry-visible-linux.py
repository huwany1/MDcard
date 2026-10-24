# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Reproduce the OFF setup repair without changing the pre-frozen input driver.

Only discover mapped windows and wait for actual native foreground ownership.
The measured wheel sequence, observation scope, flags and budgets stay intact.
The exact executed helper copy from the final R10 cohort is retained under
artifacts/reading-precision/linux-final-r10-perf-visible-off/.
"""
from pathlib import Path
import hashlib

original = Path('/work/scripts/measure-reading-precision-entry-linux.py')
assert hashlib.sha256(original.read_bytes()).hexdigest() == 'b2b74f86763dc8dee7294dfe36016c521c4ed49fe3bfea4f91982e12275a373e'
source = original.read_text()
old = """def active(a):
    run('xdotool','windowraise',a['window'],'windowactivate','--sync',a['window'])
    time.sleep(.15)
    assert run('xdotool','getactivewindow').stdout.decode().strip()==a['window']
"""
new = """def active(a):
    until=time.monotonic()+5
    while time.monotonic()<until:
        subprocess.run(['xdotool','windowraise',a['window'],'windowactivate','--sync',a['window']],env=ENV,capture_output=True)
        time.sleep(.15)
        actual=subprocess.run(['xdotool','getactivewindow'],env=ENV,capture_output=True)
        if actual.returncode==0 and actual.stdout.decode().strip()==a['window']:return
    raise AssertionError('Actual native foreground not ready')
"""
assert old in source
source = source.replace(old, new).replace("'search','--all','--pid'", "'search','--all','--onlyvisible','--pid'")
exec(compile(source, 'frozen-entry-driver-with-visible-window-setup', 'exec'))
