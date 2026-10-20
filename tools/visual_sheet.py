#!/usr/bin/env python3
"""Build a side-by-side HTML sheet from a visual-diff run.

Given the outputs of tools/visual_diff.py for one page, this writes an HTML file
with the Chromium reference, the Mango render and the diff heat map stacked so
the three can be eyeballed together (or screenshotted by the agent/Preview tab).

Usage:
    python tools/visual_sheet.py .freebuff/out/<slug> [--out .freebuff/out/compare.html]
"""

from __future__ import annotations

import argparse
import base64
import io
import sys
from pathlib import Path

from PIL import Image


def data_uri(path: Path) -> str:
    payload = base64.b64encode(path.read_bytes()).decode("ascii")
    return f"data:image/png;base64,{payload}"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("prefix", help="path prefix without the .chrome/.mango/.diff suffix")
    parser.add_argument("--out", default=None)
    parser.add_argument("--max-width", type=int, default=1100)
    args = parser.parse_args()

    prefix = Path(args.prefix)
    chrome_png = Path(f"{prefix}.chrome.png")
    mango_png = Path(f"{prefix}.mango.png")
    diff_png = Path(f"{prefix}.diff.png")
    for path in (chrome_png, mango_png):
        if not path.exists():
            sys.exit(f"error: missing {path}")

    out = Path(args.out) if args.out else prefix.parent / (prefix.name + ".compare.html")

    cells = [
        ("Chromium (reference)", chrome_png),
        ("Mango", mango_png),
    ]
    if diff_png.exists():
        cells.append(("Diff (heat map)", diff_png))

    html = [
        "<!doctype html><meta charset=utf-8><title>mango visual comparison</title>",
        "<style>",
        "body{margin:0;background:#14161a;color:#e8eaed;font:13px/1.4 system-ui,sans-serif}",
        "figure{margin:0 0 18px}figcaption{padding:6px 10px;color:#f39c4a;font-weight:600}",
        f"img{{display:block;width:min(100%,{args.max_width}px);height:auto;outline:1px solid #333}}",
        "</style>",
        "<h1 style='font-size:15px;padding:10px'>Visual comparison</h1>",
    ]
    for caption, path in cells:
        html.append(f"<figure><figcaption>{caption} — {path.name}</figcaption>")
        html.append(f"<img src='{data_uri(path)}'></figure>")
    out.write_text("".join(html), encoding="utf-8")
    print(f"wrote {out} ({out.stat().st_size // 1024} KB)")


if __name__ == "__main__":
    main()
