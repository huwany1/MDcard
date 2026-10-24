# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Launch a freshly extracted Linux archive without development renderer overrides."""
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import subprocess
import tarfile
import tempfile
import time
import uuid
import argparse
import tomllib

ROOT = Path(__file__).resolve().parent.parent
version = tomllib.loads((ROOT / "Cargo.toml").read_text('utf8'))['package']['version']
parser = argparse.ArgumentParser()
parser.add_argument('--archive', type=Path, default=ROOT / f'dist/MDcard-{version}-linux-x64.tar.gz')
parser.add_argument('--out', type=Path, default=ROOT / f'artifacts/release-{version}/linux-gui')
args = parser.parse_args()
OUT = args.out
OUT.mkdir(parents=True, exist_ok=True)
for previous in ("result.json", "report.json"):
    (OUT / previous).unlink(missing_ok=True)
RUN = str(uuid.uuid4())
archive = args.archive
env = {key: value for key, value in os.environ.items() if not key.startswith("MDCARD_")}
env.update(DISPLAY=":114", LANG="C.UTF-8", LIBGL_ALWAYS_SOFTWARE="1", LP_NUM_THREADS="2",
           MDCARD_AUTO_QUIT_SECONDS="14", MDCARD_SETTINGS_PATH=str(OUT / "settings.json"),
           MDCARD_RICH_REPORT=str(OUT / "report.json"))
source = r"""# Packaged engines

Inline $x^2$.

```math
\ce{2H2 + O2 -> 2H2O}
```

```mermaid
flowchart LR
A[Source] --> B[Preview]
```

```plantuml
@startmindmap
* Editor
** Math
** Diagram
@endmindmap
```

```flow
st=>start: Start
e=>end: Done
st->e
```
"""
fixture = OUT / "fixture.md"
fixture.write_text(source)
(OUT / "settings.json").write_text(json.dumps(dict(language="en", appearance="Light",
    gpu_acceleration=True, reduce_motion=True, font_size=12)))
children = []
try:
    with tempfile.TemporaryDirectory(prefix="mdcard-rich-package-") as temporary:
        folder = Path(temporary)
        with tarfile.open(archive) as handle:
            for member in handle.getmembers():
                name = PurePosixPath(member.name)
                assert not name.is_absolute() and ".." not in name.parts
                assert member.isdir() or member.isfile(), "Archive links/devices are forbidden"
            # Debian 12 Python 3.11.2 predates extraction filters. All members
            # have already been restricted to contained regular files/directories.
            handle.extractall(folder)
        assert (folder / "mdcard").stat().st_mode & 0o111
        assert (folder / "renderer/runtime/node").stat().st_mode & 0o111
        children.append(subprocess.Popen(["Xvfb", env["DISPLAY"], "-screen", "0", "1600x1600x24"],
            stdout=(OUT / "xvfb.log").open("wb"), stderr=subprocess.STDOUT))
        time.sleep(.6)
        children.append(subprocess.Popen(["openbox"], env=env,
            stdout=(OUT / "openbox.log").open("wb"), stderr=subprocess.STDOUT))
        time.sleep(.6)
        app = subprocess.Popen([str(folder / "mdcard"), str(fixture)], cwd=folder, env=env,
            stdout=(OUT / "app.log").open("wb"), stderr=subprocess.STDOUT)
        children.append(app)
        window = None
        for _ in range(80):
            time.sleep(.1)
            found = subprocess.run(["xdotool", "search", "--pid", str(app.pid), "--name", "MDcard"],
                env=env, capture_output=True)
            if found.returncode == 0:
                window = found.stdout.decode().splitlines()[0]
                break
        assert window, "Packaged editor never opened a native window"
        subprocess.run(["xdotool", "windowsize", window, "1240", "1450"], env=env, check=True)
        deadline = time.monotonic() + 10
        report = None
        while time.monotonic() < deadline:
            try:
                report = json.loads((OUT / "report.json").read_text())
                if report["counters"]["completed"] >= 5:
                    break
            except (OSError, json.JSONDecodeError):
                pass
            time.sleep(.15)
        assert report is not None, "No current native telemetry was produced"
        assert report["nodes"] == 5, report
        assert report["counters"]["completed"] == 5, report["counters"]
        assert report["counters"]["errors"] == 0, report["counters"]
        direct_children = subprocess.check_output(["pgrep", "-P", str(app.pid)], text=True).split()
        runtime_paths = [str(Path(f"/proc/{pid}/exe").resolve()) for pid in direct_children]
        assert str(folder / "renderer/runtime/node") in runtime_paths, runtime_paths
        subprocess.run(["import", "-window", window, str(OUT / "packaged.png")], env=env, check=True)
        app.wait(timeout=18)
        assert app.returncode == 0
        assert fixture.read_text() == source
        result = dict(runId=RUN, date=time.strftime("%Y-%m-%dT%H:%M:%S%z"), passed=True,
            executableSha256=hashlib.sha256((folder / "mdcard").read_bytes()).hexdigest(),
            bundledNodeUsed=True, developmentRendererOverride=False, realEngines=5,
            runtimeProcesses=runtime_paths, sourceUnchanged=True, counters=report["counters"])
        (OUT / "result.json").write_text(json.dumps(result, indent=2))
        print(json.dumps(result, indent=2))
finally:
    for child in reversed(children):
        if child.poll() is None:
            child.terminate()
            try:
                child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait()
