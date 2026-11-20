#!/usr/bin/env python3
"""Expose SQLite online backup APIs in the pinned SQLite 3.53.0 WASM build."""

from __future__ import annotations

import argparse
import subprocess
from pathlib import Path


SQLITE_COMMIT = "4ebc7fdcf459e8d88eb5b019c2949bda86565528"


def git_commit(source: Path) -> str:
    return subprocess.check_output(
        ["git", "-C", str(source), "rev-parse", "HEAD"], text=True
    ).strip()


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"expected one patch anchor in {path}, found {count}: {old!r}")
    path.write_text(text.replace(old, new, 1))


def patch(source: Path) -> None:
    actual_commit = git_commit(source)
    if actual_commit != SQLITE_COMMIT:
        raise SystemExit(
            f"SQLite source revision mismatch: expected {SQLITE_COMMIT}, got {actual_commit}"
        )

    api_dir = source / "ext/wasm/api"
    exports = api_dir / "EXPORTED_FUNCTIONS.c-pp"
    glue = api_dir / "sqlite3-api-glue.c-pp.js"
    export_text = exports.read_text()
    glue_text = glue.read_text()

    markers = (
        "_sqlite3_backup_finish",
        '["sqlite3_backup_finish", "int", "sqlite3_backup*"]',
        "('sqlite3_backup*', __xArgPtr)",
        "resultAdapter('sqlite3_backup*', __xRcPtr)",
    )
    present = [marker in export_text or marker in glue_text for marker in markers]
    if all(present):
        print("SQLite 3.53.0 WASM backup API patch is already present")
        return
    if any(present):
        raise SystemExit("SQLite WASM backup API patch is only partially applied")

    replace_once(
        exports,
        "_sqlite3_bind_blob\n",
        "_sqlite3_backup_finish\n"
        "_sqlite3_backup_init\n"
        "_sqlite3_backup_step\n"
        "_sqlite3_bind_blob\n",
    )

    replace_once(
        glue,
        '      ["sqlite3_aggregate_context","void*", "sqlite3_context*", "int"],\n'
        "      /* sqlite3_auto_extension() has a hand-written binding. */\n",
        '      ["sqlite3_aggregate_context","void*", "sqlite3_context*", "int"],\n'
        "      /* sqlite3_auto_extension() has a hand-written binding. */\n"
        '      ["sqlite3_backup_finish", "int", "sqlite3_backup*"],\n'
        '      ["sqlite3_backup_init", "sqlite3_backup*", "sqlite3*", "string", "sqlite3*", "string"],\n'
        '      ["sqlite3_backup_step", "int", "sqlite3_backup*", "int"],\n',
    )
    replace_once(
        glue,
        "    ('sqlite3_session*', __xArgPtr)\n"
        "    ('sqlite3_stmt*', (v)=>\n",
        "    ('sqlite3_session*', __xArgPtr)\n"
        "    ('sqlite3_backup*', __xArgPtr)\n"
        "    ('sqlite3_stmt*', (v)=>\n",
    )
    replace_once(
        glue,
        "    wasm.xWrap.resultAdapter('sqlite3*', __xRcPtr)\n",
        "    wasm.xWrap.resultAdapter('sqlite3_backup*', __xRcPtr)\n"
        "    ('sqlite3*', __xRcPtr)\n",
    )

    print("Patched SQLite 3.53.0 WASM exports and typed bindings")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("sqlite_source", type=Path)
    args = parser.parse_args()
    patch(args.sqlite_source.resolve())


if __name__ == "__main__":
    main()
