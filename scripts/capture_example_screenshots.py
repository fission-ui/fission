#!/usr/bin/env python3
"""Capture documentation and README screenshots of the example apps.

Builds each example, launches it with live test control, sets a fixed window size,
performs any setup the shot needs, and writes a PNG to documentation/static/img/examples/.

    python3 scripts/capture_example_screenshots.py            # every shot
    python3 scripts/capture_example_screenshots.py inbox      # shots whose name starts with "inbox"

Chart reference screenshots are produced separately by chart-gallery's
`generate_real_chart_doc_screenshots` live test.
"""

from __future__ import annotations

import json
import os
import shutil
import socket
import subprocess
import sys
import time
import urllib.request
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT_DIR = ROOT / "documentation" / "static" / "img" / "examples"
TARGET_DIR = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))


@dataclass
class Shot:
    name: str
    package: str
    binary: str
    width: int = 1280
    height: int = 800
    # Live test commands sent after the resize, in order, each followed by a short settle.
    steps: list[dict] = field(default_factory=list)
    settle_ms: int = 1200
    # Apps that show their working directory start in a neutral sample project instead of the
    # checkout, so no machine-specific path or branch appears in published images.
    in_demo_project: bool = False


def label(text: str) -> dict:
    return {"selector": {"kind": "label", "label": text}, "index": 0}


SHOTS = [
    Shot("inbox", "inbox", "inbox"),
    # The same app at a phone-sized window, where it switches to its compact layout.
    Shot("inbox-compact", "inbox", "inbox", width=390, height=844),
    Shot(
        "inbox-compose",
        "inbox",
        "inbox",
        steps=[{"cmd": "TapSelector", "query": label("Compose")}],
    ),
    Shot(
        "editor",
        "fission-editor",
        "fission-editor",
        # File tree rows are identified by path rather than labelled, so open them by their text.
        steps=[
            {"cmd": "TapText", "text": "src"},
            {"cmd": "TapText", "text": "app.rs"},
            {"cmd": "TapSelector", "query": label("TERMINAL")},
        ],
        in_demo_project=True,
    ),
    Shot("widget-gallery", "widget-gallery", "widget-gallery"),
    Shot("chart-gallery", "chart-gallery", "chart-gallery"),
    Shot("text-lab", "text-lab", "text-lab"),
    Shot(
        "terminal",
        "terminal",
        "terminal",
        width=1100,
        height=700,
        # The shell starts asynchronously and the view takes keyboard input once clicked.
        steps=[
            {"cmd": "Wait", "ms": 2000},
            {"cmd": "Tap", "x": 550, "y": 350},
            {"cmd": "TypeText", "text": "ls"},
            {"cmd": "PressKey", "key": "Enter", "modifiers": 0},
            {"cmd": "TypeText", "text": "cat src/app.rs"},
            {"cmd": "PressKey", "key": "Enter", "modifiers": 0},
        ],
        in_demo_project=True,
    ),
    Shot("animation-gallery", "animation-gallery", "animation-gallery"),
]


def demo_project() -> Path:
    """A fresh copy of the counter example's sources, outside any git checkout."""
    # A short, fixed location: the terminal and editor print the working directory.
    project = Path("/tmp/fission-screenshots/my-app")
    shutil.rmtree(project, ignore_errors=True)
    (project / "src").mkdir(parents=True)
    counter = ROOT / "examples" / "counter"
    for name in ("Cargo.toml", "fission.toml", "README.md"):
        shutil.copy(counter / name, project / name)
    for source in (counter / "src").glob("*.rs"):
        shutil.copy(source, project / "src" / source.name)
    return project


def free_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def command(port: int, payload: dict, timeout: float = 60.0) -> dict:
    request = urllib.request.Request(
        f"http://127.0.0.1:{port}/cmd",
        data=json.dumps(payload).encode(),
        headers={"content-type": "application/json"},
    )
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return json.loads(response.read())


def wait_ready(port: int, process: subprocess.Popen, timeout_s: float = 60.0) -> None:
    deadline = time.monotonic() + timeout_s
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"app exited early with status {process.returncode}")
        try:
            if "nodes" in command(port, {"cmd": "GetTree"}, timeout=3.0):
                return
        except OSError:
            pass
        time.sleep(0.5)
    raise RuntimeError("app did not answer live test control in time")


def build(packages: list[str]) -> None:
    # Debug builds render identically and avoid the release profile's slow whole-program optimisation.
    args = ["cargo", "build"]
    for package in packages:
        args += ["-p", package]
    print("building:", " ".join(packages), flush=True)
    subprocess.run(args, cwd=ROOT, check=True)


def capture(shot: Shot) -> Path:
    binary = TARGET_DIR / "debug" / shot.binary
    port = free_port()
    env = dict(os.environ, FISSION_TEST_CONTROL_PORT=str(port), FISSION_BACKGROUND_TEST="1")
    log = open(OUT_DIR.parent / f".capture-{shot.name}.log", "w")
    cwd = demo_project() if shot.in_demo_project else ROOT
    process = subprocess.Popen([str(binary)], cwd=cwd, env=env, stdout=log, stderr=log)
    try:
        wait_ready(port, process)
        command(port, {"cmd": "SimulateResize", "width": shot.width, "height": shot.height})
        time.sleep(shot.settle_ms / 1000)
        for step in shot.steps:
            response = command(port, step)
            if response.get("status") not in (None, "Ok"):
                raise RuntimeError(f"{shot.name}: step {step['cmd']} failed: {response}")
            time.sleep(shot.settle_ms / 1000)
        path = OUT_DIR / f"{shot.name}.png"
        response = command(port, {"cmd": "Screenshot", "path": str(path)})
        if response.get("status") != "Ok":
            raise RuntimeError(f"{shot.name}: screenshot failed: {response}")
        return path
    finally:
        try:
            command(port, {"cmd": "Quit"}, timeout=5.0)
        except (OSError, ValueError):
            pass
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
        log.close()


def main() -> int:
    prefixes = sys.argv[1:]
    shots = [s for s in SHOTS if not prefixes or any(s.name.startswith(p) for p in prefixes)]
    if not shots:
        print("no shots match", prefixes)
        return 1
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    build(sorted({s.package for s in shots}))
    failures = 0
    for shot in shots:
        try:
            path = capture(shot)
            print(f"captured {shot.name}: {path.relative_to(ROOT)}", flush=True)
        except Exception as error:  # noqa: BLE001 - report every shot, then exit non-zero
            failures += 1
            print(f"FAILED {shot.name}: {error}", flush=True)
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
