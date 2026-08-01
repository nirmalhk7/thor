#!/usr/bin/env python3
"""Generate or verify Thor-owned Chromium build provenance."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
MANIFEST_PATH = ROOT / "thor_browser_manifest.json"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def sha256_tree(root: Path) -> str:
    digest = hashlib.sha256()
    files = sorted(path for path in root.rglob("*") if path.is_file())
    if not files:
        raise ValueError(f"provenance input tree is empty: {root}")
    for path in files:
        if path.is_symlink():
            raise ValueError(f"provenance input must not contain symlinks: {path}")
        relative = path.relative_to(root).as_posix().encode("utf-8")
        digest.update(relative)
        digest.update(b"\0")
        with path.open("rb") as handle:
            for block in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(block)
        digest.update(b"\0")
    return digest.hexdigest()


def git(source: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(source), *args],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return result.stdout.strip()


def normalize_repo(value: str) -> str:
    return value.strip().removesuffix(".git").rstrip("/")


def validate_source(source: Path, manifest: dict[str, Any]) -> None:
    if not (source / ".git").exists():
        raise ValueError(f"Chromium source is not a Git checkout: {source}")
    chromium = manifest["chromium"]
    expected_commit = chromium["commit"]
    tag_commit = git(source, "rev-parse", f"refs/tags/{chromium['ref']}^{{commit}}")
    head_commit = git(source, "rev-parse", "HEAD")
    origin = git(source, "remote", "get-url", "origin")
    if tag_commit != expected_commit:
        raise ValueError(
            f"Chromium tag {chromium['ref']} resolves to {tag_commit}, expected {expected_commit}"
        )
    if head_commit != expected_commit:
        raise ValueError(f"Chromium HEAD is {head_commit}, expected {expected_commit}")
    if normalize_repo(origin) != normalize_repo(chromium["repository"]):
        raise ValueError(
            f"Chromium origin is {origin}, expected {chromium['repository']}"
        )


def build_provenance(
    source: Path | None,
    shield_dir: Path,
    browser_executable: Path | None,
) -> dict[str, Any]:
    with MANIFEST_PATH.open(encoding="utf-8") as handle:
        manifest = json.load(handle)
    if manifest.get("product") != "Thor":
        raise ValueError("browser manifest product must be Thor")
    commit = manifest.get("chromium", {}).get("commit", "")
    if len(commit) != 40 or any(character not in "0123456789abcdef" for character in commit):
        raise ValueError("browser manifest requires a lowercase 40-character Chromium commit")
    if source is not None:
        validate_source(source, manifest)

    patches = sorted((ROOT / "browser" / "patches").glob("[0-9][0-9][0-9][0-9]-*.patch"))
    if not patches:
        raise ValueError("no numbered Thor Chromium patches found")
    rules_path = shield_dir / "rules.json"
    if not rules_path.is_file():
        raise ValueError(f"Shield filter rules are missing: {rules_path}")

    chromium = manifest["chromium"]
    provenance: dict[str, Any] = {
        "schema_version": 2,
        "product": "Thor",
        "source": {
            "repository": chromium["repository"],
            "ref": chromium["ref"],
            "commit": chromium["commit"],
        },
        "inputs": {
            "manifest_sha256": sha256_file(MANIFEST_PATH),
            "gn_args": {
                "path": "browser/gn_args.thor.gn",
                "sha256": sha256_file(ROOT / "browser" / "gn_args.thor.gn"),
            },
            "patches": [
                {
                    "path": f"browser/patches/{patch.name}",
                    "sha256": sha256_file(patch),
                }
                for patch in patches
            ],
            "shield": {
                "tree_sha256": sha256_tree(shield_dir),
                "filter_rules_sha256": sha256_file(rules_path),
            },
        },
        "features": manifest.get("features", {}),
    }
    if browser_executable is not None:
        if browser_executable.is_symlink() or not browser_executable.is_file():
            raise ValueError(
                f"browser executable must be a regular non-symlink file: {browser_executable}"
            )
        provenance["artifact"] = {
            "kind": "chromium_browser_executable",
            "filename": browser_executable.name,
            "size": browser_executable.stat().st_size,
            "sha256": sha256_file(browser_executable),
        }
    return provenance


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--chromium-src", type=Path)
    parser.add_argument(
        "--shield-dir",
        type=Path,
        default=ROOT / "browser" / "extensions" / "thor-shield",
    )
    parser.add_argument(
        "--browser-executable",
        type=Path,
        help="bind provenance to the exact built Chromium browser executable",
    )
    parser.add_argument(
        "--source-inputs-only",
        action="store_true",
        help="check source inputs before a browser artifact has been rebuilt",
    )
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--output", type=Path)
    action.add_argument("--check", type=Path)
    args = parser.parse_args()
    if args.source_inputs_only and args.browser_executable:
        parser.error("--source-inputs-only cannot be combined with --browser-executable")

    try:
        if args.check:
            with args.check.open(encoding="utf-8") as handle:
                actual = json.load(handle)
            if (
                "artifact" in actual
                and args.browser_executable is None
                and not args.source_inputs_only
            ):
                raise ValueError(
                    "artifact-bound provenance requires --browser-executable"
                )
        provenance = build_provenance(
            args.chromium_src.resolve() if args.chromium_src else None,
            args.shield_dir.resolve(),
            args.browser_executable.resolve() if args.browser_executable else None,
        )
        if args.check:
            comparable_actual = dict(actual)
            if args.source_inputs_only:
                comparable_actual.pop("artifact", None)
            if comparable_actual != provenance:
                raise ValueError(f"Thor provenance marker is stale or invalid: {args.check}")
            print(f"✓ Verified Thor provenance marker: {args.check}")
            return 0

        output = args.output.resolve()
        output.parent.mkdir(parents=True, exist_ok=True)
        serialized = json.dumps(provenance, indent=2, sort_keys=True) + "\n"
        with tempfile.NamedTemporaryFile(
            mode="w", encoding="utf-8", dir=output.parent, delete=False
        ) as handle:
            handle.write(serialized)
            temporary = Path(handle.name)
        temporary.replace(output)
        print(f"✓ Wrote Thor provenance marker: {output}")
        return 0
    except (OSError, ValueError, subprocess.CalledProcessError, json.JSONDecodeError) as error:
        print(f"Thor provenance check failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
