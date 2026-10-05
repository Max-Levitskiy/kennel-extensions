#!/usr/bin/env python3
"""Add or replace one extension's entry in index.toml.

usage: update_index.py <index.toml> <name> <version> <wasm_url> <manifest_url> <sha256> <manifest_sha256>

The field set is exactly kennel-gui's StoreEntry (crates/kennel-gui/src/store.rs
in Max-Levitskiy/kennel): both hashes are required, because the GUI refuses an
install whose monitor.wasm or manifest.toml doesn't match. Entries are written
sorted by name so a release only ever changes its own lines.
"""
import json
import sys
import tomllib

FIELDS = ["name", "version", "wasm_url", "manifest_url", "sha256", "manifest_sha256"]


def render(entries):
    if not entries:
        return "extensions = []\n"
    blocks = []
    for entry in sorted(entries, key=lambda e: e["name"]):
        # json.dumps of an ASCII string is a valid TOML basic string.
        lines = [f"{field} = {json.dumps(entry[field])}" for field in FIELDS]
        blocks.append("[[extensions]]\n" + "\n".join(lines) + "\n")
    return "\n".join(blocks)


def main(argv):
    if len(argv) != 8:
        sys.exit(__doc__)
    path, *values = argv[1:]
    new = dict(zip(FIELDS, values))
    with open(path, "rb") as f:
        entries = tomllib.load(f).get("extensions", [])
    entries = [e for e in entries if e["name"] != new["name"]] + [new]
    with open(path, "w") as f:
        f.write(render(entries))


if __name__ == "__main__":
    main(sys.argv)
