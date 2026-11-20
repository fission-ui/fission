#!/usr/bin/env python3
"""Strip indentation from blank lines in the vendored SQLite OPFS proxy."""

from __future__ import annotations

import argparse
import os
import stat
import tempfile
from pathlib import Path


def normalize(proxy: Path) -> None:
    original = proxy.read_text()
    mode = stat.S_IMODE(proxy.stat().st_mode)
    lines = original.splitlines()
    normalized = "\n".join(line if line.strip() else "" for line in lines) + "\n"
    if normalized == original:
        return

    temporary_path: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w",
            encoding="utf-8",
            newline="\n",
            dir=proxy.parent,
            prefix=f".{proxy.name}.",
            suffix=".tmp",
            delete=False,
        ) as temporary_file:
            temporary_path = Path(temporary_file.name)
            temporary_file.write(normalized)
        os.chmod(temporary_path, mode)
        os.replace(temporary_path, proxy)
    finally:
        if temporary_path is not None:
            temporary_path.unlink(missing_ok=True)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("proxy", type=Path)
    args = parser.parse_args()
    normalize(args.proxy)


if __name__ == "__main__":
    main()
