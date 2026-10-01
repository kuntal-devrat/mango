#!/usr/bin/env python3
"""Chromium-vs-Mango visual parity harness.

Renders a URL twice -- once with Chromium (the reference) and once with Mango's
headless renderer -- then reports how far apart the two screenshots are.

Chromium is driven over the DevTools Protocol rather than `--screenshot`, because
that is the only way to pin the reference to the conditions Mango renders under:

  * light colour scheme (the OS theme otherwise leaks into the reference),
  * JavaScript disabled, so the comparison measures the HTML/CSS/layout engines
    instead of Mango's Boa JS engine against V8,
  * an exact device-pixel viewport and scroll offset.

Usage:
    python tools/visual_diff.py <url> [--width 1280] [--height 900]
                              [--scroll 0] [--out .freebuff/out]
                              [--grid 8] [--js] [--skip-chrome] [--eval JS]

Outputs (in --out):
    <slug>.chrome.png   reference screenshot from Chromium
    <slug>.mango.png    screenshot from Mango
    <slug>.diff.png     heat map of differing pixels

Exit code 0 always: this is a measurement tool, not a gate.
"""

from __future__ import annotations

import argparse
import asyncio
import base64
import json
import os
import socket
import subprocess
import sys
import time
import urllib.parse
import urllib.request
from pathlib import Path

from PIL import Image, ImageChops

CHROME_CANDIDATES = [
    os.environ.get("CHROME_PATH", ""),
    r"C:\Program Files\Google\Chrome\Application\chrome.exe",
    r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
    r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
]


def find_chrome() -> str:
    for candidate in CHROME_CANDIDATES:
        if candidate and Path(candidate).exists():
            return candidate
    found = subprocess.run(
        ["bash", "-lc", "command -v google-chrome chromium chromium-browser msedge"],
        capture_output=True,
        text=True,
    ).stdout.split()
    if found:
        return found[0]
    sys.exit("error: no Chromium-family browser found; set CHROME_PATH")


def free_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


class DevTools:
    """Minimal DevTools Protocol client for one page target."""

    def __init__(self, chrome: str, profile: Path):
        self.chrome = chrome
        self.profile = profile
        self.proc: subprocess.Popen | None = None
        self.ws = None
        self.next_id = 0
        self.events: list[dict] = []

    async def __aenter__(self) -> "DevTools":
        port = free_port()
        self.proc = subprocess.Popen(
            [
                self.chrome,
                "--headless=new",
                "--disable-gpu",
                "--hide-scrollbars",
                "--no-first-run",
                "--no-default-browser-check",
                f"--remote-debugging-port={port}",
                f"--user-data-dir={self.profile}",
                "about:blank",
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
        )

        deadline = time.time() + 30
        ws_url = ""
        while time.time() < deadline:
            assert self.proc.stderr is not None
            line = self.proc.stderr.readline()
            if not line:
                break
            if "DevTools listening on" in line:
                ws_url = line.split("DevTools listening on", 1)[1].strip()
                break
        if not ws_url:
            sys.exit("error: Chromium did not expose a DevTools endpoint")

        target_ws = self._page_target(port, ws_url)
        import websockets

        self.ws = await websockets.connect(target_ws, max_size=None)

        await self.send("Page.enable")
        await self.send("Runtime.enable")
        return self

    def _page_target(self, port: int, browser_ws: str) -> str:
        """Returns the WebSocket URL of the initial about:blank page target."""
        deadline = time.time() + 20
        while time.time() < deadline:
            try:
                with urllib.request.urlopen(f"http://127.0.0.1:{port}/json/list", timeout=2) as resp:
                    targets = json.load(resp)
                for target in targets:
                    if target.get("type") == "page" and target.get("webSocketDebuggerUrl"):
                        return target["webSocketDebuggerUrl"]
            except OSError:
                pass
            time.sleep(0.2)
        sys.exit("error: no page target exposed by Chromium")

    async def send(self, method: str, params: dict | None = None) -> dict:
        assert self.ws is not None
        self.next_id += 1
        message_id = self.next_id
        await self.ws.send(json.dumps({"id": message_id, "method": method, "params": params or {}}))
        while True:
            raw = await asyncio.wait_for(self.ws.recv(), timeout=60)
            message = json.loads(raw)
            if message.get("id") == message_id:
                if "error" in message:
                    raise RuntimeError(f"{method}: {message['error']}")
                return message.get("result", {})
            if "method" in message:
                self.events.append(message)

    async def wait_for_event(self, method: str, timeout: float = 30) -> dict:
        deadline = time.time() + timeout
        while time.time() < deadline:
            for event in self.events:
                if event.get("method") == method:
                    return event
            try:
                assert self.ws is not None
                raw = await asyncio.wait_for(self.ws.recv(), timeout=1)
            except asyncio.TimeoutError:
                continue
            message = json.loads(raw)
            if "method" in message:
                self.events.append(message)
            elif message.get("id"):
                # Late response to a fire-and-forget command; keep it for `send`.
                self.events.append(message)
        return {}

    async def __aexit__(self, *exc: object) -> None:
        if self.ws is not None:
            await self.ws.close()
        if self.proc is not None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.proc.kill()


async def capture_chrome(
    chrome: str,
    url: str,
    out: Path,
    width: int,
    height: int,
    scroll: int,
    script: bool,
    eval_expression: str | None = None,
) -> None:
    profile = out.parent / ".chrome-profile"
    async with DevTools(chrome, profile) as dt:
        await dt.send(
            "Emulation.setDeviceMetricsOverride",
            {"width": width, "height": height, "deviceScaleFactor": 1, "mobile": False},
        )
        await dt.send("Emulation.setScriptExecutionDisabled", {"value": not script})
        await dt.send(
            "Emulation.setEmulatedMedia",
            {"features": [{"name": "prefers-color-scheme", "value": "light"}]},
        )
        await dt.send(
            "Network.setUserAgentOverride",
            {
                "userAgent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36"
            },
        )
        await dt.send("Page.navigate", {"url": url})
        await dt.wait_for_event("Page.loadEventFired", timeout=45)
        await asyncio.sleep(1.0)  # images, fonts, first paint

        if scroll > 0:
            await dt.send(
                "Runtime.evaluate",
                {"expression": f"window.scrollTo(0, {scroll})", "awaitPromise": True},
            )
            await asyncio.sleep(0.4)

        if eval_expression:
            result = await dt.send(
                "Runtime.evaluate",
                {
                    "expression": eval_expression,
                    "returnByValue": True,
                    "awaitPromise": True,
                    "userGesture": True,
                },
            )
            value = result.get("result", {}).get("value")
            print("eval:", json.dumps(value, ensure_ascii=False)[:4000])

        shot = await dt.send("Page.captureScreenshot", {"format": "png", "fromSurface": True})
        out.write_bytes(base64.b64decode(shot["data"]))


def resolve_mango(binary: str) -> str:
    path = Path(binary)
    if not path.exists() and os.name == "nt" and not path.suffix:
        path = path.with_suffix(".exe")
    if not path.exists():
        sys.exit(f"error: mango headless binary not found at {path} (build it with `cargo build`)")
    return str(path.resolve())


def capture_mango(url: str, out: Path, width: int, height: int, scroll: int, binary: str) -> None:
    # `--page` renders the page viewport alone (no tab strip/omnibox), which is what
    # a Chromium screenshot captures, so the two images are directly comparable.
    cmd = [resolve_mango(binary), url, str(out.resolve()), str(width), str(height), str(scroll), "--page"]
    result = subprocess.run(cmd, capture_output=True, text=True, timeout=180)
    if result.returncode != 0:
        print(result.stdout[-2000:])
        print(result.stderr[-2000:], file=sys.stderr)
        sys.exit("error: mango headless render failed")


def grid_heatmap(diff: Image.Image, grid: int) -> list[tuple[int, int, float]]:
    """Return (row, col, mean-difference) for the most divergent grid cells."""
    gray = diff.convert("L")
    width, height = gray.size
    cells = []
    for row in range(grid):
        for col in range(grid):
            box = (
                col * width // grid,
                row * height // grid,
                (col + 1) * width // grid,
                (row + 1) * height // grid,
            )
            region = gray.crop(box)
            cells.append((row, col, sum(region.getdata()) / (region.width * region.height)))
    cells.sort(key=lambda cell: cell[2], reverse=True)
    return cells


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("url")
    parser.add_argument("--width", type=int, default=1280)
    parser.add_argument("--height", type=int, default=900)
    parser.add_argument("--scroll", type=int, default=0)
    parser.add_argument("--out", default=".freebuff/out")
    parser.add_argument("--grid", type=int, default=8)
    parser.add_argument("--mango", default="target/debug/headless.exe")
    parser.add_argument("--skip-chrome", action="store_true", help="reuse an existing reference PNG")
    parser.add_argument("--skip-mango", action="store_true", help="reuse an existing Mango PNG")
    parser.add_argument(
        "--js",
        action="store_true",
        help="let the Chromium reference execute JavaScript (default: disabled for a fair HTML/CSS comparison)",
    )
    parser.add_argument("--eval", dest="eval_expression", default=None, help="evaluate JS in Chromium after load")
    args = parser.parse_args()

    out_dir = Path(args.out).resolve()
    out_dir.mkdir(parents=True, exist_ok=True)
    slug = urllib.parse.urlparse(args.url).netloc + urllib.parse.urlparse(args.url).path
    slug = slug.strip("/").replace("/", "_").replace(":", "_") or "page"

    chrome_png = out_dir / f"{slug}.chrome.png"
    mango_png = out_dir / f"{slug}.mango.png"
    diff_png = out_dir / f"{slug}.diff.png"

    if not args.skip_chrome:
        chrome = find_chrome()
        print(f"chromium: {chrome}")
        asyncio.run(
            capture_chrome(
                chrome,
                args.url,
                chrome_png,
                args.width,
                args.height,
                args.scroll,
                args.js,
                args.eval_expression,
            )
        )
    elif args.eval_expression:
        sys.exit("error: --eval needs a Chromium run (drop --skip-chrome)")
    if not args.skip_mango:
        capture_mango(args.url, mango_png, args.width, args.height, args.scroll, args.mango)

    reference = Image.open(chrome_png).convert("RGB")
    candidate = Image.open(mango_png).convert("RGB")
    if reference.size != candidate.size:
        candidate = candidate.resize(reference.size)

    diff = ImageChops.difference(reference, candidate)
    pixels = list(diff.getdata())
    total = len(pixels)
    differing = sum(1 for p in pixels if max(p) > 32)
    mae = sum(sum(p) for p in pixels) / (total * 3)

    diff.save(diff_png)
    print(f"size            : {reference.size[0]}x{reference.size[1]}")
    print(f"mean abs error  : {mae:.2f} / 255")
    print(f"pixels > 32 off : {differing / total * 100:.1f}%")
    print(f"worst regions   : (row, col, mean diff) with {args.grid}x{args.grid} grid")
    for row, col, value in grid_heatmap(diff, args.grid)[:6]:
        print(f"  row {row:2d} col {col:2d}  {value:6.1f}   x={col * args.width // args.grid}-{(col + 1) * args.width // args.grid}")


if __name__ == "__main__":
    main()
