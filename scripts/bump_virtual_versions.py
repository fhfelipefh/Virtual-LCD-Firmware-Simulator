#!/usr/bin/env python3
from __future__ import annotations

import re
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent

MANIFESTS = [
    ROOT / "virtual-lcd-sdk" / "Cargo.toml",
    ROOT / "virtual-lcd-core" / "Cargo.toml",
    ROOT / "virtual-lcd-renderer" / "Cargo.toml",
    ROOT / "virtual-lcd-examples" / "Cargo.toml",
]

DEPENDENCY_PATTERNS = {
    ROOT / "virtual-lcd-core" / "Cargo.toml": [
        r'(virtual-lcd-sdk = \{ version = ")(\d+\.\d+\.\d+)(".*\})',
    ],
    ROOT / "virtual-lcd-renderer" / "Cargo.toml": [
        r'(virtual-lcd-core = \{ version = ")(\d+\.\d+\.\d+)(".*\})',
    ],
    ROOT / "virtual-lcd-examples" / "Cargo.toml": [
        r'(virtual-lcd-core = \{ version = ")(\d+\.\d+\.\d+)(".*\})',
        r'(virtual-lcd-renderer = \{ version = ")(\d+\.\d+\.\d+)(".*\})',
        r'(virtual-lcd-sdk = \{ version = ")(\d+\.\d+\.\d+)(".*\})',
    ],
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


def update_dependency_versions(manifest_path: Path, new_version: str) -> None:
    text = manifest_path.read_text()
    for pattern in DEPENDENCY_PATTERNS.get(manifest_path, []):
        text, count = re.subn(pattern, rf"\g<1>{new_version}\g<3>", text)
        if count != 1:
            raise SystemExit(f"Could not update dependency version in {manifest_path}: {pattern}")
    manifest_path.write_text(text)


def main() -> None:
    versions = {read_version(path) for path in MANIFESTS}
    if len(versions) != 1:
        raise SystemExit(f"Expected a single shared version across manifests, got: {sorted(versions)}")

    current_version = versions.pop()
    new_version = bump_patch(current_version)

    for manifest_path in MANIFESTS:
        replace_first_package_version(manifest_path, new_version)

    for manifest_path in MANIFESTS:
        update_dependency_versions(manifest_path, new_version)

    print(new_version)


if __name__ == "__main__":
    main()
