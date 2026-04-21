"""Tiny screenshot capture helper used by the screenshot-driven E2E flow.

The integration test requests screenshots by hitting this HTTP endpoint. For
simulator-based runs the server shells out to `xcrun simctl io booted
screenshot ...`; for real macOS desktop runs it falls back to
`screencapture ...`. The output directory and port are configurable so the
workflow can be run locally without hardcoded user paths.
"""

import os
import subprocess
import time
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse

SCREENSHOT_DIR = Path(
    os.environ.get("WORKA_SCREENSHOT_DIR", str(Path.home() / ".gemini/tmp/worka/screenshots"))
)
SCREENSHOT_PORT = int(os.environ.get("WORKA_SCREENSHOT_PORT", "8999"))
SCREENSHOT_MODE = os.environ.get("WORKA_SCREENSHOT_MODE", "auto").strip().lower()
ACTIVATE_APP = os.environ.get("WORKA_SCREENSHOT_ACTIVATE_APP", "").strip()
ACTIVATE_BUNDLE_ID = os.environ.get("WORKA_SCREENSHOT_ACTIVATE_BUNDLE_ID", "").strip()
ACTIVATE_DELAY_MS = int(os.environ.get("WORKA_SCREENSHOT_ACTIVATE_DELAY_MS", "400"))

SCREENSHOT_DIR.mkdir(parents=True, exist_ok=True)


class ReusableHTTPServer(HTTPServer):
    allow_reuse_address = True


def _has_booted_simulator() -> bool:
    result = subprocess.run(
        ["xcrun", "simctl", "list", "devices", "booted"],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return False
    return "(Booted)" in result.stdout


def _build_capture_command(filename: Path) -> list[str]:
    mode = SCREENSHOT_MODE
    if mode == "auto":
        mode = "simulator" if _has_booted_simulator() else "desktop"

    if mode == "simulator":
        return ["xcrun", "simctl", "io", "booted", "screenshot", str(filename)]

    if mode == "desktop":
        window_info = _find_window_info()
        if window_info is not None:
            window_id, bounds = window_info
            x = int(bounds.get("X", 0))
            y = int(bounds.get("Y", 0))
            width = int(bounds.get("Width", 0))
            height = int(bounds.get("Height", 0))
            if width > 0 and height > 0:
                print(
                    f"Capturing target window id {window_id} via region "
                    f"{x},{y},{width},{height}",
                    flush=True,
                )
                return [
                    "screencapture",
                    "-x",
                    "-R",
                    f"{x},{y},{width},{height}",
                    str(filename),
                ]
            print(f"Capturing target window id {window_id}", flush=True)
            return ["screencapture", "-x", "-l", str(window_id), str(filename)]
        print("No target window found; falling back to full desktop capture", flush=True)
        return ["screencapture", "-x", str(filename)]

    raise ValueError(f"Unsupported WORKA_SCREENSHOT_MODE={SCREENSHOT_MODE!r}")


def _fallback_capture_command(filename: Path) -> list[str] | None:
    mode = SCREENSHOT_MODE
    if mode == "auto":
        mode = "simulator" if _has_booted_simulator() else "desktop"

    if mode == "desktop":
        return ["screencapture", "-x", str(filename)]

    return None


def _activate_target_app() -> None:
    if not ACTIVATE_APP and not ACTIVATE_BUNDLE_ID:
        return

    print(
        "Activating target app "
        f"(bundle_id={ACTIVATE_BUNDLE_ID or '<none>'}, app={ACTIVATE_APP or '<none>'})",
        flush=True,
    )
    commands: list[list[str]] = []
    if ACTIVATE_BUNDLE_ID:
        commands.append(
            [
                "osascript",
                "-e",
                f'tell application id "{ACTIVATE_BUNDLE_ID}" to activate',
            ]
        )
        commands.append(["open", "-b", ACTIVATE_BUNDLE_ID])
    if ACTIVATE_APP:
        commands.append(
            ["osascript", "-e", f'tell application "{ACTIVATE_APP}" to activate']
        )
        commands.append(["open", "-a", ACTIVATE_APP])

    for command in commands:
        result = subprocess.run(
            command,
            check=False,
            capture_output=True,
            text=True,
        )
        if result.stdout.strip():
            print(result.stdout.strip(), flush=True)
        if result.stderr.strip():
            print(result.stderr.strip(), flush=True)
        if result.returncode == 0:
            break

    if ACTIVATE_APP:
        subprocess.run(
            [
                "osascript",
                "-e",
                (
                    'tell application "System Events" '
                    f'to tell process "{ACTIVATE_APP}" to set frontmost to true'
                ),
            ],
            check=False,
            capture_output=True,
            text=True,
        )

    if ACTIVATE_DELAY_MS > 0:
        time.sleep(ACTIVATE_DELAY_MS / 1000.0)


def _find_window_info() -> tuple[int, dict[str, float]] | None:
    if not ACTIVATE_APP and not ACTIVATE_BUNDLE_ID:
        return None

    script = f"""
import AppKit
import CoreGraphics

let targetBundleId = "{ACTIVATE_BUNDLE_ID}"
let targetAppName = "{ACTIVATE_APP}".lowercased()
let targetPids = Array(
    NSRunningApplication.runningApplications(withBundleIdentifier: targetBundleId)
        .map {{ Int($0.processIdentifier) }}
)
let preferredPid = targetPids.max() ?? -1
let windows = CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID) as? [[String: Any]] ?? []
for window in windows {{
    let owner = (window[kCGWindowOwnerName as String] as? String ?? "").lowercased()
    let ownerPid = window[kCGWindowOwnerPID as String] as? Int ?? -1
    let layer = window[kCGWindowLayer as String] as? Int ?? -1
    let bounds = window[kCGWindowBounds as String] as? [String: Any] ?? [:]
    let width = bounds["Width"] as? Double ?? 0
    let height = bounds["Height"] as? Double ?? 0
    let matchesPid = preferredPid != -1 && ownerPid == preferredPid
    let matchesName = !targetAppName.isEmpty && owner == targetAppName
    if (matchesPid || matchesName) && layer == 0 && width > 200 && height > 200,
       let number = window[kCGWindowNumber as String] as? Int {{
        let x = bounds["X"] as? Double ?? 0
        let y = bounds["Y"] as? Double ?? 0
        print("\\(number),\\(ownerPid),\\(Int(x)),\\(Int(y)),\\(Int(width)),\\(Int(height))")
        if matchesPid {{
            break
        }}
    }}
}}
"""
    result = subprocess.run(
        ["swift", "-e", script],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return None

    output = result.stdout.strip().splitlines()
    if not output:
        return None

    try:
        parts = [part.strip() for part in output[0].split(",")]
        if len(parts) != 6:
            return None
        return (
            int(parts[0]),
            {
                "X": float(parts[2]),
                "Y": float(parts[3]),
                "Width": float(parts[4]),
                "Height": float(parts[5]),
            },
        )
    except ValueError:
        return None


class RequestHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        query = parse_qs(urlparse(self.path).query)
        name = self.headers.get('x-worka-screenshot-name') or query.get(
            'name', ['screenshot']
        )[0]
        # Remove any path traversal or invalid characters from name
        name = os.path.basename(name)
        filename = SCREENSHOT_DIR / f"{name}.png"
        print(f"Taking screenshot: {filename}", flush=True)
        try:
            _activate_target_app()
            command = _build_capture_command(filename)
            print(f"Running capture command: {' '.join(command)}", flush=True)
        except ValueError as error:
            self.send_response(500)
            self.end_headers()
            self.wfile.write(str(error).encode("utf-8", errors="replace"))
            return
        result = subprocess.run(
            command,
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            fallback = _fallback_capture_command(filename)
            if fallback is not None and fallback != command:
                result = subprocess.run(
                    fallback,
                    check=False,
                    capture_output=True,
                    text=True,
                )
        if result.returncode != 0:
            self.send_response(500)
            self.end_headers()
            self.wfile.write(result.stderr.encode("utf-8", errors="replace"))
            return

        self.send_response(200)
        self.end_headers()
        self.wfile.write(b'OK')

print(f"Starting screenshot server on port {SCREENSHOT_PORT}...", flush=True)
print(f"Writing screenshots to {SCREENSHOT_DIR}", flush=True)
print(f"Screenshot mode: {SCREENSHOT_MODE}", flush=True)
httpd = ReusableHTTPServer(('0.0.0.0', SCREENSHOT_PORT), RequestHandler)
httpd.serve_forever()
