#!/usr/bin/env python3
"""Browserless release/privacy contract checks for the Thor downstream."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
CONTRACT_PATH = ROOT / "tests" / "privacy_contract.json"


class Checks:
    def __init__(self) -> None:
        self.count = 0
        self.errors: list[str] = []

    def require(self, condition: bool, message: str) -> None:
        self.count += 1
        if not condition:
            self.errors.append(message)


def load_json(path: Path) -> Any:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def parse_gn_args(path: Path) -> dict[str, Any]:
    values: dict[str, Any] = {}
    for raw_line in path.read_text(encoding="utf-8").splitlines():
        line = raw_line.split("#", 1)[0].strip()
        match = re.fullmatch(r"([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.+)", line)
        if not match:
            continue
        name, raw_value = match.groups()
        if raw_value == "true":
            value: Any = True
        elif raw_value == "false":
            value = False
        elif re.fullmatch(r"-?[0-9]+", raw_value):
            value = int(raw_value)
        elif len(raw_value) >= 2 and raw_value[0] == raw_value[-1] == '"':
            value = raw_value[1:-1]
        else:
            value = raw_value
        values[name] = value
    return values


def parse_patch(path: Path) -> dict[str, dict[str, set[str]]]:
    changes: dict[str, dict[str, set[str]]] = {}
    current_path: str | None = None
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("diff --git a/"):
            match = re.match(r"diff --git a/(.+) b/(.+)$", line)
            current_path = match.group(2) if match else None
            if current_path:
                changes.setdefault(current_path, {"removed": set(), "added": set()})
            continue
        if current_path is None or line.startswith(("--- ", "+++ ")):
            continue
        if line.startswith("-"):
            changes[current_path]["removed"].add(line[1:])
        elif line.startswith("+"):
            changes[current_path]["added"].add(line[1:])
    return changes


def git(source: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", "-C", str(source), *args],
        check=check,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def normalized_repo_url(value: str) -> str:
    return value.strip().removesuffix(".git").rstrip("/")


def check_static_contract(contract: dict[str, Any], checks: Checks) -> None:
    manifest = load_json(ROOT / "thor_browser_manifest.json")
    checks.require(contract.get("schema_version") == 1, "unsupported privacy contract schema")
    checks.require(manifest.get("product") == "Thor", "browser manifest product must be Thor")
    checks.require(
        manifest.get("chromium", {}).get("ref") == contract.get("chromium_ref"),
        "privacy contract Chromium ref differs from thor_browser_manifest.json",
    )
    checks.require(
        re.fullmatch(r"[0-9a-f]{40}", manifest.get("chromium", {}).get("commit", "")) is not None,
        "browser manifest must contain the peeled 40-character Chromium tag commit",
    )

    patch_dir = ROOT / "browser" / "patches"
    patch_files = sorted(
        path
        for path in patch_dir.glob("[0-9][0-9][0-9][0-9]-*.patch")
        if path.is_file()
    )
    patch_names = [path.name for path in patch_files]
    checks.require(
        patch_names == contract.get("required_patches"),
        f"numbered Chromium patches differ: expected {contract.get('required_patches')}, found {patch_names}",
    )
    for patch in patch_files:
        checks.require(
            re.search(r"(?m)^/(?:Users|home)/", patch.read_text(encoding="utf-8")) is None,
            f"Chromium patch contains a local workstation path: {patch.name}",
        )

    patch_maps = {path.name: parse_patch(path) for path in patch_files}
    branding = patch_maps.get("0001-thor-product-name.patch", {}).get(
        "chrome/app/theme/chromium/BRANDING", {"added": set()}
    )
    for line in (
        "PRODUCT_FULLNAME=Thor",
        "PRODUCT_SHORTNAME=Thor",
        "MAC_BUNDLE_ID=com.thor.browser",
    ):
        checks.require(line in branding.get("added", set()), f"branding patch missing: {line}")

    privacy_changes = patch_maps.get("0002-thor-privacy-defaults.patch", {})
    for replacement in contract.get("privacy_patch_replacements", []):
        source_path = replacement["path"]
        path_changes = privacy_changes.get(source_path, {"removed": set(), "added": set()})
        checks.require(
            replacement["removed"] in path_changes["removed"],
            f"privacy patch missing removed line in {source_path}: {replacement['removed']}",
        )
        checks.require(
            replacement["added"] in path_changes["added"],
            f"privacy patch missing added line in {source_path}: {replacement['added']}",
        )

    native_gpc_changes = patch_maps.get("0003-thor-native-gpc.patch", {})
    for addition in contract.get("native_gpc_patch_additions", []):
        source_path = addition["path"]
        path_changes = native_gpc_changes.get(source_path, {"added": set()})
        checks.require(
            addition["added"] in path_changes["added"],
            f"native GPC patch missing added line in {source_path}: {addition['added']}",
        )

    native_identity_changes = patch_maps.get("0004-thor-new-identity.patch", {})
    for addition in contract.get("native_identity_patch_additions", []):
        source_path = addition["path"]
        path_changes = native_identity_changes.get(source_path, {"added": set()})
        checks.require(
            addition["added"] in path_changes["added"],
            f"native identity patch missing added line in {source_path}: {addition['added']}",
        )

    navigation_cleanup_changes = patch_maps.get(
        "0005-thor-navigation-url-cleanup.patch", {}
    )
    for addition in contract.get("navigation_cleanup_patch_additions", []):
        source_path = addition["path"]
        path_changes = navigation_cleanup_changes.get(source_path, {"added": set()})
        checks.require(
            addition["added"] in path_changes["added"],
            f"navigation cleanup patch missing added line in {source_path}: {addition['added']}",
        )

    native_profile_changes = patch_maps.get(
        "0006-thor-native-profile-policy.patch", {}
    )
    for addition in contract.get("native_profile_policy_patch_additions", []):
        source_path = addition["path"]
        path_changes = native_profile_changes.get(source_path, {"added": set()})
        checks.require(
            addition["added"] in path_changes["added"],
            f"native profile policy patch missing added line in {source_path}: {addition['added']}",
        )

    native_webgl_changes = patch_maps.get(
        "0007-Reduce-WebGL-debug-renderer-exposure.patch", {}
    )
    for addition in contract.get("native_webgl_privacy_patch_additions", []):
        source_path = addition["path"]
        path_changes = native_webgl_changes.get(source_path, {"added": set()})
        checks.require(
            addition["added"] in path_changes["added"],
            f"native WebGL privacy patch missing added line in {source_path}: {addition['added']}",
        )

    native_identity_destroy_changes = patch_maps.get(
        "0008-thor-destroy-identity.patch", {}
    )
    for addition in contract.get("native_identity_destroy_patch_additions", []):
        source_path = addition["path"]
        path_changes = native_identity_destroy_changes.get(
            source_path, {"added": set()}
        )
        checks.require(
            addition["added"] in path_changes["added"],
            f"native identity-destroy patch missing added line in {source_path}: {addition['added']}",
        )

    gn_values = parse_gn_args(ROOT / "browser" / "gn_args.thor.gn")
    for name, expected in contract.get("required_gn_args", {}).items():
        checks.require(
            gn_values.get(name) == expected,
            f"GN arg {name} must be {expected!r}, found {gn_values.get(name)!r}",
        )

    launcher = (ROOT / "scripts" / "run_thor_chromium.sh").read_text(encoding="utf-8")
    for token in contract.get("required_launcher_tokens", []):
        checks.require(token in launcher, f"launcher privacy contract missing token: {token}")

    product_launcher = (ROOT / "src" / "engine" / "product.rs").read_text(encoding="utf-8")
    checks.require(
        '"--thor-privacy-profile={}"' in product_launcher
        and ".chromium_value()" in product_launcher,
        "owned Chromium launcher must bind the selected typed privacy profile",
    )

    mcp_tools = (ROOT / "src" / "mcp" / "tools.rs").read_text(encoding="utf-8")
    for tool in contract.get("required_mcp_tools", []):
        checks.require(
            f'"name": "{tool}"' in mcp_tools,
            f"MCP privacy contract missing tool: {tool}",
        )

    acceptance = (ROOT / "tests" / "privacy_browser_acceptance.rs").read_text(encoding="utf-8")
    for token in contract.get("required_acceptance_tokens", []):
        checks.require(
            token in acceptance,
            f"live privacy acceptance missing required coverage: {token}",
        )

    release_paths = (
        ROOT / "browser" / "gn_args.thor.gn",
        ROOT / "scripts" / "bootstrap_chromium_snapshot.sh",
        ROOT / "scripts" / "build_chromium.sh",
        ROOT / "scripts" / "package_macos.sh",
        ROOT / "scripts" / "run_thor_chromium.sh",
        ROOT / ".github" / "workflows" / "release.yml",
    )
    release_surface = "\n".join(path.read_text(encoding="utf-8") for path in release_paths)
    for token in contract.get("forbidden_release_tokens", []):
        checks.require(token not in release_surface, f"release surface contains forbidden token: {token}")
    release_workflow = (ROOT / ".github" / "workflows" / "release.yml").read_text(encoding="utf-8")
    for token in ("contents: write", "softprops/action-gh-release", "GITHUB_TOKEN"):
        checks.require(
            token not in release_workflow,
            f"release workflow must not publish externally: {token}",
        )
    for workflow_path in sorted((ROOT / ".github" / "workflows").glob("*.yml")):
        workflow_text = workflow_path.read_text(encoding="utf-8")
        action_uses = re.findall(r"(?m)^\s*uses:\s*([^\s#]+)", workflow_text)
        for action_use in action_uses:
            _, separator, revision = action_use.rpartition("@")
            checks.require(
                separator == "@" and re.fullmatch(r"[0-9a-f]{40}", revision) is not None,
                f"workflow action must use a full immutable commit: {workflow_path.name}: {action_use}",
            )

    shield_dir = ROOT / "browser" / "extensions" / "thor-shield"
    for asset in ("manifest.json", "service_worker.js", "fingerprint.js", "rules.json", "filter_bundle.json"):
        asset_path = shield_dir / asset
        checks.require(asset_path.is_file() and asset_path.stat().st_size > 0, f"missing Shield asset: {asset}")
    shield_manifest = load_json(shield_dir / "manifest.json")
    checks.require(shield_manifest.get("manifest_version") == 3, "Shield must use manifest version 3")
    content_scripts = shield_manifest.get("content_scripts", [])
    checks.require(
        any(item.get("run_at") == "document_start" and item.get("world") == "MAIN" for item in content_scripts),
        "Shield must inject its MAIN-world protection at document_start",
    )
    rule_resources = shield_manifest.get("declarative_net_request", {}).get("rule_resources", [])
    checks.require(
        any(item.get("id") == "thor_shield_rules" and item.get("enabled") is True for item in rule_resources),
        "Shield ruleset must be enabled",
    )

    workflow = (ROOT / ".github" / "workflows" / "release.yml").read_text(encoding="utf-8")
    for token in (
        "thor-chromium-builder",
        "scripts/privacy_regression_check.py",
        "./scripts/fetch_chromium.sh",
        "./scripts/apply_thor_branding.sh",
        "./scripts/build_chromium.sh",
        "./scripts/package_macos.sh",
        "./scripts/verify_macos_package.sh",
        "./scripts/run_privacy_acceptance.sh",
        'NOTARIZE: "true"',
    ):
        checks.require(token in workflow, f"release workflow missing gate: {token}")
    checks.require("package_linux.sh" not in workflow, "Linux release must remain gated")
    checks.require("package_windows.ps1" not in workflow, "Windows release must remain gated")
    checks.require("package_homebrew.sh" not in workflow, "Homebrew release must remain gated")
    checks.require("package_winget.ps1" not in workflow, "Winget release must remain gated")

    ci_workflow = (ROOT / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")
    for token in (
        "python3 scripts/privacy_regression_check.py",
        "cargo test --test privacy_browser_acceptance",
    ):
        checks.require(token in ci_workflow, f"pull-request CI missing browserless gate: {token}")
    checks.require(
        "toolchain: 1.97.1" in ci_workflow and "toolchain: 1.97.1" in release_workflow,
        "CI and release workflows must install the exact Rust 1.97.1 toolchain",
    )
    integration_tests = (ROOT / "tests" / "integration_tests.rs").read_text(encoding="utf-8")
    checks.require(
        "httpbin.org" not in integration_tests,
        "browserless integration tests must not depend on public httpbin.org",
    )

    package_macos = (ROOT / "scripts" / "package_macos.sh").read_text(encoding="utf-8")
    checks.require(
        'cp "$MARKER_DIR/ThorOwnedChromium.json"' in package_macos,
        "macOS package must copy the validated build marker",
    )
    checks.require(
        'cp "thor_browser_manifest.json"' not in package_macos,
        "macOS package must not substitute the source manifest for build provenance",
    )
    for token in (
        '--sign "${CODESIGN_IDENTITY}" "${FRAMEWORKS_DIR}/Chromium.app"',
        '--output "${RESOURCES_DIR}/ThorOwnedChromium.json"',
        'ditto -c -k --sequesterRsrc --keepParent "${APP_DIR}"',
        'xcrun stapler staple "${APP_DIR}"',
        'xcrun stapler staple "${DIST_DIR}/${DMG_NAME}"',
    ):
        checks.require(token in package_macos, f"macOS package release gate missing: {token}")
    verify_macos = (ROOT / "scripts" / "verify_macos_package.sh").read_text(encoding="utf-8")
    for token in (
        '[[ -x "$candidate/Contents/MacOS/thor-labs" ]]',
        '[[ "$inner_bundle_id" == "com.thor.browser" ]]',
        'verify_app "$ZIP_APP" "ZIP application"',
        'verify_app "$DMG_APP" "DMG application"',
        'xcrun stapler validate "$signed_app"',
        'xcrun stapler validate "$DMG"',
        'spctl --assess --type execute --verbose=2 "$signed_app"',
    ):
        checks.require(token in verify_macos, f"macOS artifact verifier missing: {token}")
    apply_branding = (ROOT / "scripts" / "apply_thor_branding.sh").read_text(encoding="utf-8")
    checks.require(
        'apply --whitespace=error --check "$patch"' in apply_branding,
        "Thor patch preflight must reject whitespace errors",
    )
    checks.require(
        'apply --whitespace=error "$patch"' in apply_branding,
        "Thor patch application must reject whitespace errors",
    )
    checks.require(
        re.search(
            r'for patch in "\$\{PATCHES\[@\]\}"; do.*?'
            r'apply --whitespace=error --check "\$patch".*?'
            r'apply --whitespace=error "\$patch".*?done',
            apply_branding,
            re.DOTALL,
        )
        is not None,
        "Thor patches must be checked and applied in one ordered sequential loop",
    )
    checks.require(
        "--whitespace=nowarn" not in apply_branding,
        "Thor patch application must not suppress whitespace failures",
    )
    verify_downstream = (ROOT / "scripts" / "verify_thor_downstream.sh").read_text(
        encoding="utf-8"
    )
    checks.require(
        "--patch-state applied" in verify_downstream,
        "downstream verification must compare the complete applied patch series",
    )
    checks.require(
        'apply --reverse --check "$patch"' not in verify_downstream,
        "downstream verification must not reverse-check overlapping patches independently",
    )
    for script_name in (
        "apply_thor_branding.sh",
        "verify_thor_downstream.sh",
        "package_macos.sh",
        "verify_macos_package.sh",
        "run_thor_chromium.sh",
    ):
        script = (ROOT / "scripts" / script_name).read_text(encoding="utf-8")
        checks.require(
            "generate_thor_provenance.py" in script,
            f"{script_name} must enforce generated build provenance",
        )

    for script_name in (
        "build_chromium.sh",
        "verify_thor_downstream.sh",
        "package_macos.sh",
        "verify_macos_package.sh",
        "run_thor_chromium.sh",
    ):
        script = (ROOT / "scripts" / script_name).read_text(encoding="utf-8")
        checks.require(
            "--browser-executable" in script,
            f"{script_name} must bind/check provenance against the browser executable",
        )

    privacy_signals = shield_dir / "privacy_signals.js"
    checks.require(
        not privacy_signals.exists(),
        "Shield must not override native managed GPC through a MAIN-world script",
    )
    rules_text = (shield_dir / "rules.json").read_text(encoding="utf-8")
    checks.require(
        '"header":"DNT"' not in rules_text and '"header":"Sec-GPC"' not in rules_text,
        "static Shield rules must not override configurable native DNT/GPC",
    )
    shield_worker = (shield_dir / "service_worker.js").read_text(encoding="utf-8")
    checks.require(
        "unverified-native-readback-required" in shield_worker,
        "Shield must not claim native GPC health without browser readback",
    )
    fingerprint_text = (shield_dir / "fingerprint.js").read_text(encoding="utf-8")
    checks.require(
        "HTMLCanvasElement.prototype.toDataURL" not in fingerprint_text
        and "AudioBuffer.prototype.getChannelData" not in fingerprint_text,
        "temporary Shield fingerprint defense must not break Canvas/Audio API semantics",
    )

    for script_name in (
        "package_linux.sh",
        "package_windows.ps1",
        "package_homebrew.sh",
        "package_winget.ps1",
    ):
        script = (ROOT / "scripts" / script_name).read_text(encoding="utf-8")
        checks.require(
            re.search(
                r"THOR_DOWNSTREAM_UNSUPPORTED[\s\S]{0,512}\bexit\s+2\b",
                script,
            )
            is not None,
            f"{script_name} must fail closed until its owned Chromium package exists",
        )


def check_chromium_source(
    source: Path, patch_state: str, contract: dict[str, Any], checks: Checks
) -> None:
    checks.require((source / ".git").exists(), f"Chromium source is not a Git checkout: {source}")
    if not (source / ".git").exists():
        return

    ref = contract["chromium_ref"]
    tag_ref = f"refs/tags/{ref}^{{commit}}"
    pin_result = git(source, "rev-parse", tag_ref, check=False)
    checks.require(pin_result.returncode == 0, f"Chromium checkout lacks local tag {ref}")
    if pin_result.returncode != 0:
        return
    pin_commit = pin_result.stdout.strip()
    manifest = load_json(ROOT / "thor_browser_manifest.json")
    expected_commit = manifest["chromium"]["commit"]
    checks.require(
        pin_commit == expected_commit,
        f"Chromium tag {ref} resolves to {pin_commit}, expected {expected_commit}",
    )
    head_commit = git(source, "rev-parse", "HEAD").stdout.strip()
    checks.require(
        head_commit == expected_commit,
        f"Chromium HEAD {head_commit} differs from exact pin {expected_commit}",
    )

    origin = git(source, "remote", "get-url", "origin").stdout.strip()
    expected_origin = manifest["chromium"]["repository"]
    checks.require(
        normalized_repo_url(origin) == normalized_repo_url(expected_origin),
        f"Chromium origin differs: expected {expected_origin}, found {origin}",
    )

    for name, declaration_path in contract.get("gn_declaration_files", {}).items():
        result = git(source, "show", f"{expected_commit}:{declaration_path}", check=False)
        checks.require(result.returncode == 0, f"missing pinned GN declaration file: {declaration_path}")
        if result.returncode == 0:
            declared = re.search(rf"(?m)^\s*{re.escape(name)}\s*=", result.stdout) is not None
            checks.require(declared, f"GN arg {name} is not declared in Chromium {ref}: {declaration_path}")

    patch_dir = ROOT / "browser" / "patches"
    patch_names = contract.get("required_patches", [])
    patch_maps = {
        name: parse_patch(patch_dir / name)
        for name in patch_names
    }
    touched_paths = sorted(
        {
            source_path
            for changes in patch_maps.values()
            for source_path in changes
        }
    )
    for source_path in touched_paths:
        candidate = Path(source_path)
        checks.require(
            not candidate.is_absolute() and ".." not in candidate.parts,
            f"unsafe Chromium patch path: {source_path}",
        )

    with tempfile.TemporaryDirectory(prefix="thor-patch-check-") as temp_dir:
        expected_tree = Path(temp_dir)
        subprocess.run(
            ["git", "init", "--quiet", str(expected_tree)],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        for source_path in touched_paths:
            result = subprocess.run(
                ["git", "-C", str(source), "show", f"{expected_commit}:{source_path}"],
                check=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            if result.returncode == 0:
                destination = expected_tree / source_path
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(result.stdout)

        def compare_patch_surface(expected_state: str) -> None:
            for source_path in touched_paths:
                actual = source / source_path
                expected = expected_tree / source_path
                matches = actual.exists() == expected.exists()
                if matches and actual.exists():
                    matches = actual.read_bytes() == expected.read_bytes()
                checks.require(
                    matches,
                    f"{source_path} is not exactly {expected_state} at Chromium pin {ref}",
                )

        if patch_state == "unapplied":
            compare_patch_surface("unapplied")

        ordered_apply_ok = True
        for patch_name in patch_names:
            patch_path = patch_dir / patch_name
            check_result = git(
                expected_tree,
                "apply",
                "--check",
                "--whitespace=error",
                str(patch_path),
                check=False,
            )
            checks.require(
                check_result.returncode == 0,
                f"{patch_name} does not apply in order at Chromium pin {ref}: "
                f"{check_result.stderr.strip()}",
            )
            if check_result.returncode != 0:
                ordered_apply_ok = False
                break
            apply_result = git(
                expected_tree,
                "apply",
                "--whitespace=error",
                str(patch_path),
                check=False,
            )
            checks.require(
                apply_result.returncode == 0,
                f"{patch_name} failed ordered application at Chromium pin {ref}: "
                f"{apply_result.stderr.strip()}",
            )
            if apply_result.returncode != 0:
                ordered_apply_ok = False
                break

        if patch_state == "applied" and ordered_apply_ok:
            compare_patch_surface("applied")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--chromium-src", type=Path, help="optional pinned Chromium src checkout")
    parser.add_argument(
        "--patch-state",
        choices=("unapplied", "applied"),
        help="required patch state when --chromium-src is supplied",
    )
    args = parser.parse_args()
    if bool(args.chromium_src) != bool(args.patch_state):
        parser.error("--chromium-src and --patch-state must be supplied together")

    checks = Checks()
    contract = load_json(CONTRACT_PATH)
    check_static_contract(contract, checks)
    if args.chromium_src:
        check_chromium_source(args.chromium_src.resolve(), args.patch_state, contract, checks)

    if checks.errors:
        print(f"privacy release contract: FAIL ({len(checks.errors)} of {checks.count} checks)", file=sys.stderr)
        for error in checks.errors:
            print(f"- {error}", file=sys.stderr)
        return 1
    print(f"privacy release contract: PASS ({checks.count} browserless checks)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
