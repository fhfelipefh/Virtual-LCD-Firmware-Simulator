#!/usr/bin/env python3
from __future__ import annotations

import argparse
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent

MANIFESTS = {
    "virtual-lcd-sdk": ROOT / "virtual-lcd-sdk" / "Cargo.toml",
    "virtual-lcd-core": ROOT / "virtual-lcd-core" / "Cargo.toml",
    "virtual-lcd-renderer": ROOT / "virtual-lcd-renderer" / "Cargo.toml",
    "virtual-lcd-examples": ROOT / "virtual-lcd-examples" / "Cargo.toml",
}

VERSION_RE = re.compile(r'(?m)^version = "(\d+)\.(\d+)\.(\d+)"$')


def bump_patch(version: str) -> str:
    major, minor, patch = map(int, version.split("."))
    return f"{major}.{minor}.{patch + 1}"


def read_version(manifest_path: Path) -> str:
    match = VERSION_RE.search(manifest_path.read_text())
    if not match:
        raise SystemExit(f"Could not find package version in {manifest_path}")
    return ".".join(match.groups())


def replace_first_package_version(manifest_path: Path, new_version: str) -> None:
    text = manifest_path.read_text()
    updated, count = VERSION_RE.subn(f'version = "{new_version}"', text, count=1)
    if count != 1:
        raise SystemExit(f"Could not update package version in {manifest_path}")
    manifest_path.write_text(updated)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Bump patch version only for selected virtual-lcd crates"
    )
    parser.add_argument(
        "crates",
        nargs="+",
        help="Crate names to bump (e.g. virtual-lcd-renderer virtual-lcd-core)",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    selected = []

    for crate in args.crates:
        if crate not in MANIFESTS:
            valid = ", ".join(sorted(MANIFESTS.keys()))
            raise SystemExit(f"Unknown crate '{crate}'. Valid options: {valid}")
        selected.append(crate)

    bumped = []
    for crate in selected:
        manifest_path = MANIFESTS[crate]
        current_version = read_version(manifest_path)
        new_version = bump_patch(current_version)
        replace_first_package_version(manifest_path, new_version)
        bumped.append(f"{crate}:{new_version}")

    print(" ".join(bumped))


if __name__ == "__main__":
    main()
