#!/usr/bin/env python3
"""Choose the next Thor release from commits since the last SemVer tag."""

import argparse
import os
import re
import subprocess
from pathlib import Path
from typing import Optional


VERSION = re.compile(r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$")
CONVENTIONAL = re.compile(r"^[a-z][a-z0-9-]*(?:\([^)]+\))?(!)?:")
BREAKING = re.compile(r"^BREAKING(?: CHANGE|-CHANGE):", re.MULTILINE)
RELEASE_COMMIT = re.compile(r"^chore\(release\): bump version to v(\d+\.\d+\.\d+) \[skip ci\]$")


def git(*args: str) -> str:
    return subprocess.check_output(["git", *args], text=True).strip()


def version_tuple(value: str) -> tuple[int, int, int]:
    match = VERSION.fullmatch(value)
    if match is None:
        raise ValueError(f"Invalid semantic version: {value}")
    return tuple(map(int, match.groups()))


def bump_level(messages: list[str]) -> Optional[str]:
    level = None
    for message in messages:
        subject = message.splitlines()[0] if message else ""
        match = CONVENTIONAL.match(subject)
        if BREAKING.search(message) or (match and match.group(1)):
            return "major"
        if subject.startswith("feat:") or subject.startswith("feat("):
            level = "minor"
        elif level is None and not RELEASE_COMMIT.fullmatch(subject):
            level = "patch"
    return level


def next_version(current: str, level: str) -> str:
    major, minor, patch = version_tuple(current)
    if level == "major":
        return f"{major + 1}.0.0"
    if level == "minor":
        return f"{major}.{minor + 1}.0"
    if level == "patch":
        return f"{major}.{minor}.{patch + 1}"
    raise ValueError(f"Unknown release level: {level}")


def package_version(manifest: str) -> str:
    in_package = False
    for line in manifest.splitlines():
        if line.startswith("["):
            in_package = line == "[package]"
        elif in_package and line.startswith("version = "):
            value = line.removeprefix("version = ").strip().strip('"')
            version_tuple(value)
            return value
    raise ValueError("Cargo.toml has no [package] version")


def replace_package_version(manifest: str, version: str) -> str:
    old = package_version(manifest)
    package, remainder = manifest.split("[package]", 1)
    body, separator, suffix = remainder.partition("\n[")
    body = body.replace(f'version = "{old}"', f'version = "{version}"', 1)
    return package + "[package]" + body + (separator + suffix if separator else "")


def replace_lock_version(lock: str, version: str) -> str:
    blocks = re.split(r"(?=^\[\[package\]\]$)", lock, flags=re.MULTILINE)
    for index, block in enumerate(blocks):
        if re.search(r'^name = "thor"$', block, re.MULTILINE):
            updated, count = re.subn(
                r'^version = "[^"]+"$', f'version = "{version}"', block,
                count=1, flags=re.MULTILINE,
            )
            if count != 1:
                raise ValueError("Cargo.lock thor package has no version")
            blocks[index] = updated
            return "".join(blocks)
    raise ValueError("Cargo.lock has no thor package")


def release_plan(current: str, tagged: str, messages: list[str], head_subject: str) -> Optional[str]:
    if current != tagged:
        if RELEASE_COMMIT.fullmatch(head_subject) and head_subject == (
            f"chore(release): bump version to v{current} [skip ci]"
        ) and version_tuple(current) > version_tuple(tagged):
            return current  # Recover a version commit whose release was not published.
        raise ValueError(f"Cargo.toml version {current} differs from latest tag v{tagged}")
    level = bump_level(messages)
    return next_version(tagged, level) if level else None


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--apply", action="store_true")
    parser.add_argument("--github-output", type=Path)
    args = parser.parse_args()

    manifest_path = Path("Cargo.toml")
    lock_path = Path("Cargo.lock")
    manifest = manifest_path.read_text()
    current = package_version(manifest)
    tags = [
        (version_tuple(tag[1:]), tag)
        for tag in git("tag", "--merged", "HEAD", "--list", "v*").splitlines()
        if VERSION.fullmatch(tag[1:])
    ]
    if not tags:
        raise ValueError("No reachable vMAJOR.MINOR.PATCH tag")
    _, latest_tag = max(tags)
    messages = [
        message.strip()
        for message in git("log", "--format=%B%x00", f"{latest_tag}..HEAD").split("\x00")
        if message.strip()
    ]
    version = release_plan(current, latest_tag[1:], messages, git("log", "-1", "--format=%s"))
    if version and args.apply and version != current:
        lock = lock_path.read_text()
        updated_lock = replace_lock_version(lock, version)
        manifest_path.write_text(replace_package_version(manifest, version))
        lock_path.write_text(updated_lock)
    output = {
        "release": str(version is not None).lower(),
        "version": version or current,
        "tag": f"v{version or current}",
        "previous_tag": latest_tag,
    }
    for key, value in output.items():
        print(f"{key}={value}")
    destination = args.github_output or (Path(os.environ["GITHUB_OUTPUT"]) if "GITHUB_OUTPUT" in os.environ else None)
    if destination:
        with destination.open("a") as handle:
            for key, value in output.items():
                handle.write(f"{key}={value}\n")


if __name__ == "__main__":
    main()
