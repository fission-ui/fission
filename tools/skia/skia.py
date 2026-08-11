#!/usr/bin/env python3
"""Pinned Skia build, package, and verification tooling for Fission.

This program deliberately has no network or third-party Python dependencies.
It consumes an already prepared exact Skia checkout or an already packaged
artifact. Release automation can wrap it without changing the artifact format.
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
from typing import Any, Iterable, Mapping, Sequence


TOOL_DIR = Path(__file__).resolve().parent
DEFAULT_CONFIG = TOOL_DIR / "config.json"
SOURCE_RECEIPT = "FISSION_SKIA_SOURCE_REVISION"
BUILD_PLAN = "fission-skia-build-plan.json"
BUILD_METADATA = "fission-skia-build.json"
MANIFEST = "manifest.json"
BUILD_PLAN_SCHEMA_VERSION = 1
BUILD_RECEIPT_SCHEMA_VERSION = 1
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
NAME_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.+-]*$")
GN_NAME_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")


class SkiaToolError(RuntimeError):
    """Actionable failure that should be printed without a traceback."""


def canonical_json(value: Any) -> str:
    return json.dumps(value, indent=2, sort_keys=True, ensure_ascii=True) + "\n"


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_json(value: Any) -> str:
    return sha256_bytes(canonical_json(value).encode("utf-8"))


def load_json(path: Path) -> Any:
    try:
        with path.open("r", encoding="utf-8") as source:
            return json.load(source)
    except FileNotFoundError as error:
        raise SkiaToolError(f"required JSON file does not exist: {path}") from error
    except json.JSONDecodeError as error:
        raise SkiaToolError(f"invalid JSON in {path}: {error}") from error


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp")
    temporary.write_text(canonical_json(value), encoding="utf-8")
    temporary.replace(path)


def require_object(value: Any, context: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise SkiaToolError(f"{context} must be a JSON object")
    return value


def require_string(value: Any, context: str) -> str:
    if not isinstance(value, str) or not value:
        raise SkiaToolError(f"{context} must be a non-empty string")
    return value


def require_string_list(value: Any, context: str) -> list[str]:
    if not isinstance(value, list) or any(not isinstance(item, str) or not item for item in value):
        raise SkiaToolError(f"{context} must be an array of non-empty strings")
    if len(set(value)) != len(value):
        raise SkiaToolError(f"{context} must not contain duplicates")
    return list(value)


def require_string_map(value: Any, context: str) -> dict[str, str]:
    result = require_object(value, context)
    for name, raw in result.items():
        if not isinstance(name, str) or not name:
            raise SkiaToolError(f"{context} contains an empty or non-string key")
        require_string(raw, f"{context}.{name}")
    return dict(result)


def load_config(path: Path) -> dict[str, Any]:
    config = require_object(load_json(path), str(path))
    if config.get("schema_version") != 1:
        raise SkiaToolError(f"unsupported Skia configuration schema in {path}")
    source = require_object(config.get("source"), "config.source")
    revision = require_string(source.get("revision"), "config.source.revision")
    if not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise SkiaToolError("config.source.revision must be an immutable 40-character Git commit")
    if source.get("qualification") != "unqualified":
        raise SkiaToolError("a source pin cannot be marked qualified by build configuration")
    bridge = require_object(config.get("bridge"), "config.bridge")
    abi = bridge.get("abi_version")
    if not isinstance(abi, int) or abi <= 0:
        raise SkiaToolError("config.bridge.abi_version must be a positive integer")
    profiles = require_object(config.get("profiles"), "config.profiles")
    for name, raw_profile in profiles.items():
        profile = require_object(raw_profile, f"config.profiles.{name}")
        licences = require_string_list(
            profile.get("required_licenses"),
            f"config.profiles.{name}.required_licenses",
        )
        if any(not NAME_RE.fullmatch(licence) for licence in licences):
            raise SkiaToolError(f"config.profiles.{name}.required_licenses has an unsafe name")
        if profile.get("build_recipe") == "available":
            libraries = require_string_list(
                profile.get("upstream_libraries"),
                f"config.profiles.{name}.upstream_libraries",
            )
            if any(not NAME_RE.fullmatch(library) for library in libraries):
                raise SkiaToolError(f"config.profiles.{name}.upstream_libraries has an unsafe name")
            if profile.get("target_recipes") is None:
                bridge_sources = require_string_list(
                    profile.get("bridge_sources"),
                    f"config.profiles.{name}.bridge_sources",
                )
                for source in bridge_sources:
                    validate_relative_path(source, f"config.profiles.{name}.bridge_sources")
                bridge_defines = require_string_map(
                    profile.get("bridge_defines"),
                    f"config.profiles.{name}.bridge_defines",
                )
                if any(not GN_NAME_RE.fullmatch(define) for define in bridge_defines):
                    raise SkiaToolError(
                        f"config.profiles.{name}.bridge_defines contains an invalid C preprocessor name"
                    )
            elif "bridge_sources" in profile or "bridge_defines" in profile:
                raise SkiaToolError(
                    f"config.profiles.{name} must keep target-selected bridge recipes only in target_recipes"
                )

    targets = require_object(config.get("targets"), "config.targets")
    for name, raw_target in targets.items():
        target = require_object(raw_target, f"config.targets.{name}")
        if target.get("kind") != "native":
            continue
        allowed = require_string_list(
            target.get("allowed_gn_overrides"),
            f"config.targets.{name}.allowed_gn_overrides",
        )
        required = require_string_list(
            target.get("required_gn_args", []),
            f"config.targets.{name}.required_gn_args",
        )
        if not set(required).issubset(allowed):
            raise SkiaToolError(
                f"config.targets.{name}.required_gn_args must be a subset of allowed_gn_overrides"
            )

    native_targets = {
        name for name, raw_target in targets.items()
        if require_object(raw_target, f"config.targets.{name}").get("kind") == "native"
    }
    for name, raw_profile in profiles.items():
        profile = require_object(raw_profile, f"config.profiles.{name}")
        raw_recipes = profile.get("target_recipes")
        if raw_recipes is None:
            continue
        recipes = require_object(raw_recipes, f"config.profiles.{name}.target_recipes")
        if set(recipes) != native_targets:
            raise SkiaToolError(
                f"config.profiles.{name}.target_recipes must explicitly classify every native "
                f"target; missing={sorted(native_targets - set(recipes))}, "
                f"extra={sorted(set(recipes) - native_targets)}"
            )
        available = 0
        for target_name, raw_recipe in recipes.items():
            context = f"config.profiles.{name}.target_recipes.{target_name}"
            recipe = require_object(raw_recipe, context)
            status = require_string(recipe.get("status"), f"{context}.status")
            if status == "available":
                available += 1
                if set(recipe) != {
                    "status",
                    "bridge_sources",
                    "bridge_defines",
                    "gn_args",
                    "system_libraries",
                    "frameworks",
                }:
                    raise SkiaToolError(f"{context} has unknown or missing available-recipe fields")
                bridge_sources = require_string_list(
                    recipe.get("bridge_sources"),
                    f"{context}.bridge_sources",
                )
                for source in bridge_sources:
                    validate_relative_path(source, f"{context}.bridge_sources")
                bridge_defines = require_string_map(
                    recipe.get("bridge_defines"),
                    f"{context}.bridge_defines",
                )
                if any(not GN_NAME_RE.fullmatch(define) for define in bridge_defines):
                    raise SkiaToolError(
                        f"{context}.bridge_defines contains an invalid C preprocessor name"
                    )
                gn_args = require_object(recipe.get("gn_args"), f"{context}.gn_args")
                if any(not GN_NAME_RE.fullmatch(argument) for argument in gn_args):
                    raise SkiaToolError(f"{context}.gn_args contains an invalid GN name")
                for value in gn_args.values():
                    gn_literal(value)
                for field in ("system_libraries", "frameworks"):
                    values = require_string_list(recipe.get(field), f"{context}.{field}")
                    if any(not NAME_RE.fullmatch(value) for value in values):
                        raise SkiaToolError(f"{context}.{field} contains an unsafe name")
            elif status in {"pending", "unsupported"}:
                if set(recipe) != {"status", "reason"}:
                    raise SkiaToolError(f"{context} has unknown or missing unavailable-recipe fields")
                require_string(recipe.get("reason"), f"{context}.reason")
            else:
                raise SkiaToolError(
                    f"{context}.status must be 'available', 'pending', or 'unsupported'"
                )
        if profile.get("build_recipe") == "available" and available == 0:
            raise SkiaToolError(f"config.profiles.{name} has no available target recipe")
    return config


def select_profile(config: Mapping[str, Any], name: str) -> dict[str, Any]:
    profiles = require_object(config.get("profiles"), "config.profiles")
    if name not in profiles:
        raise SkiaToolError(f"unknown Skia profile {name!r}; choose one of: {', '.join(sorted(profiles))}")
    return require_object(profiles[name], f"config.profiles.{name}")


def select_target(config: Mapping[str, Any], name: str) -> dict[str, Any]:
    targets = require_object(config.get("targets"), "config.targets")
    if name not in targets:
        raise SkiaToolError(f"unknown Skia target {name!r}; choose one of: {', '.join(sorted(targets))}")
    return require_object(targets[name], f"config.targets.{name}")


def select_profile_target_recipe(
    profile: Mapping[str, Any], profile_name: str, target_name: str
) -> dict[str, Any] | None:
    raw_recipes = profile.get("target_recipes")
    if raw_recipes is None:
        return None
    recipes = require_object(raw_recipes, f"profiles.{profile_name}.target_recipes")
    recipe = require_object(
        recipes.get(target_name),
        f"profiles.{profile_name}.target_recipes.{target_name}",
    )
    status = require_string(recipe.get("status"), "profile target recipe status")
    if status != "available":
        reason = require_string(recipe.get("reason"), "profile target recipe reason")
        raise SkiaToolError(
            f"profile {profile_name!r} is {status} for target {target_name!r}: {reason}"
        )
    return recipe


def validate_profile_target_links(
    profile: Mapping[str, Any],
    profile_name: str,
    target_name: str,
    links: Mapping[str, Any],
) -> None:
    recipe = select_profile_target_recipe(profile, profile_name, target_name)
    if recipe is None:
        return
    for field in ("system_libraries", "frameworks"):
        expected = require_string_list(recipe.get(field), f"profile target recipe {field}")
        if links.get(field) != expected:
            raise SkiaToolError(
                f"native.{field} does not match the declared {profile_name}/{target_name} "
                "link contract"
            )


def resolve_explicit_path(
    cli_value: str | None,
    env_name: str,
    description: str,
    *,
    must_exist: bool,
) -> Path:
    env_value = os.environ.get(env_name)
    if cli_value and env_value:
        cli_path = Path(cli_value).expanduser().resolve()
        env_path = Path(env_value).expanduser().resolve()
        if cli_path != env_path:
            raise SkiaToolError(
                f"conflicting {description}: command line selects {cli_path}, "
                f"but {env_name} selects {env_path}"
            )
    raw_value = cli_value or env_value
    if not raw_value:
        raise SkiaToolError(f"{description} is required; pass it explicitly or set {env_name}")
    result = Path(raw_value).expanduser().resolve()
    if must_exist and not result.exists():
        raise SkiaToolError(f"{description} does not exist: {result}")
    return result


def run_checked(command: Sequence[str], *, cwd: Path | None = None) -> str:
    try:
        completed = subprocess.run(
            list(command),
            cwd=cwd,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
    except FileNotFoundError as error:
        raise SkiaToolError(f"required executable was not found: {command[0]}") from error
    except subprocess.CalledProcessError as error:
        detail = (error.stderr or error.stdout or "").strip()
        suffix = f"\n{detail}" if detail else ""
        raise SkiaToolError(f"command failed ({' '.join(command)}):{suffix}") from error
    return completed.stdout.strip()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def normalized_identity_text(value: str, context: str) -> str:
    normalized = " ".join(value.split())
    if not normalized:
        raise SkiaToolError(f"{context} must not be empty")
    if len(normalized) > 512:
        raise SkiaToolError(f"{context} is unexpectedly long")
    return normalized


def verify_tool_digest(path: Path, expected_sha256: str, name: str) -> str:
    if not SHA256_RE.fullmatch(expected_sha256):
        raise SkiaToolError(
            f"expected {name} SHA-256 must be 64 lowercase hexadecimal characters"
        )
    if not path.is_file():
        raise SkiaToolError(f"required build tool does not exist: {path}")
    actual_sha256 = sha256_file(path)
    if actual_sha256 != expected_sha256:
        raise SkiaToolError(
            f"{name} SHA-256 mismatch: expected {expected_sha256}, found {actual_sha256}"
        )
    return actual_sha256


def capture_tool_identity(
    path: Path,
    expected_sha256: str,
    actual_sha256: str,
) -> dict[str, str]:
    version = normalized_identity_text(run_checked([str(path), "--version"]), "tool version")
    # Host paths are execution details, not part of an artifact's reproducible
    # identity. The digest and normalized version identify the executable used.
    return {
        "expected_sha256": expected_sha256,
        "actual_sha256": actual_sha256,
        "version": version,
    }


def verified_tool_identities(
    gn_path: Path,
    expected_gn_sha256: str,
    ninja_path: Path,
    expected_ninja_sha256: str,
) -> dict[str, dict[str, str]]:
    # Authenticate both executables before either one is allowed to run, even
    # for version discovery.
    actual_gn_sha256 = verify_tool_digest(gn_path, expected_gn_sha256, "GN")
    actual_ninja_sha256 = verify_tool_digest(
        ninja_path,
        expected_ninja_sha256,
        "Ninja",
    )
    return {
        "gn": capture_tool_identity(gn_path, expected_gn_sha256, actual_gn_sha256),
        "ninja": capture_tool_identity(
            ninja_path,
            expected_ninja_sha256,
            actual_ninja_sha256,
        ),
    }


def verify_source_checkout(
    source_dir: Path,
    expected_revision: str,
    expected_repository: str,
) -> dict[str, Any]:
    if not source_dir.is_dir():
        raise SkiaToolError(f"Skia source override is not a directory: {source_dir}")
    git_marker = source_dir / ".git"
    if git_marker.exists():
        revision = run_checked(["git", "-C", str(source_dir), "rev-parse", "HEAD"])
        if revision != expected_revision:
            raise SkiaToolError(
                f"Skia checkout revision mismatch: expected {expected_revision}, found {revision}"
            )
        dirty = run_checked(
            ["git", "-C", str(source_dir), "status", "--porcelain", "--untracked-files=no"]
        )
        if dirty:
            raise SkiaToolError("Skia checkout contains tracked modifications; use a clean pinned checkout")
        return {
            "kind": "local-git-checkout",
            "qualified": False,
            "repository": expected_repository,
            "revision": revision,
        }

    receipt = source_dir / SOURCE_RECEIPT
    if not receipt.is_file():
        raise SkiaToolError(
            f"vendored Skia source must contain {SOURCE_RECEIPT} with the exact pinned revision"
        )
    revision = receipt.read_text(encoding="utf-8").strip()
    if revision != expected_revision:
        raise SkiaToolError(
            f"vendored Skia revision mismatch: expected {expected_revision}, found {revision or '<empty>'}"
        )
    return {
        "kind": "local-vendored-source",
        "qualified": False,
        "repository": expected_repository,
        "revision": revision,
    }


def parse_named_paths(values: Iterable[str], description: str) -> dict[str, Path]:
    result: dict[str, Path] = {}
    for value in values:
        name, separator, raw_path = value.partition("=")
        if not separator or not NAME_RE.fullmatch(name):
            raise SkiaToolError(f"invalid {description} {value!r}; expected NAME=PATH")
        if name in result:
            raise SkiaToolError(f"duplicate {description} name: {name}")
        path = Path(raw_path).expanduser().resolve()
        if not path.is_file():
            raise SkiaToolError(f"{description} {name!r} does not exist or is not a file: {path}")
        result[name] = path
    return result


def parse_gn_value(raw: str) -> Any:
    if raw == "true":
        return True
    if raw == "false":
        return False
    if re.fullmatch(r"-?[0-9]+", raw):
        return int(raw)
    if not raw:
        raise SkiaToolError("GN argument values must not be empty")
    return raw


def parse_gn_overrides(values: Iterable[str]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for value in values:
        name, separator, raw = value.partition("=")
        if not separator or not GN_NAME_RE.fullmatch(name):
            raise SkiaToolError(f"invalid GN argument {value!r}; expected NAME=VALUE")
        if name in result:
            raise SkiaToolError(f"duplicate GN argument: {name}")
        result[name] = parse_gn_value(raw)
    return result


def gn_literal(value: Any) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, str):
        return json.dumps(value, ensure_ascii=True)
    if isinstance(value, list) and all(isinstance(item, str) for item in value):
        return "[" + ", ".join(json.dumps(item) for item in value) + "]"
    raise SkiaToolError(f"unsupported GN value: {value!r}")


def resolve_build_plan(
    config: Mapping[str, Any], profile_name: str, target_name: str, overrides: Mapping[str, Any]
) -> dict[str, Any]:
    profile = select_profile(config, profile_name)
    target = select_target(config, target_name)
    if profile.get("kind") != "native" or target.get("kind") != "native":
        raise SkiaToolError("build-native accepts only a native profile and native target")
    if profile.get("build_recipe") != "available":
        raise SkiaToolError(
            f"profile {profile_name!r} is declared but its build recipe is "
            f"{profile.get('build_recipe', 'unavailable')!r}; no artifact will be fabricated"
        )
    target_recipe = select_profile_target_recipe(profile, profile_name, target_name)
    common_args = require_object(config.get("common_native_gn_args"), "common_native_gn_args")
    profile_args = require_object(profile.get("gn_args"), f"profiles.{profile_name}.gn_args")
    target_args = require_object(target.get("gn_args"), f"targets.{target_name}.gn_args")
    profile_target_args = (
        require_object(target_recipe.get("gn_args"), "profile target recipe gn_args")
        if target_recipe is not None
        else {}
    )
    allowed_overrides = set(
        require_string_list(
            target.get("allowed_gn_overrides"),
            f"targets.{target_name}.allowed_gn_overrides",
        )
    )
    gn_args: dict[str, Any] = {}
    for source in (common_args, profile_args, target_args, profile_target_args):
        for name, value in source.items():
            if name in gn_args and gn_args[name] != value:
                raise SkiaToolError(f"configuration contains conflicting GN argument {name}")
            gn_args[name] = value
    for name, value in overrides.items():
        if name not in allowed_overrides:
            raise SkiaToolError(
                f"GN override {name!r} is not allowed for target {target_name!r}; "
                f"allowed overrides: {', '.join(sorted(allowed_overrides)) or '<none>'}"
            )
        if name in gn_args:
            raise SkiaToolError(
                f"GN override {name!r} would change the pinned profile; edit and review config.json instead"
            )
        gn_args[name] = value
    for required in target.get("required_gn_args", []):
        if required not in gn_args:
            raise SkiaToolError(
                f"target {target_name!r} requires --gn-arg {required}=VALUE"
            )
    if target.get("platform") == "Android":
        require_string(gn_args.get("ndk"), "Android GN argument ndk")
        ndk_api = gn_args.get("ndk_api")
        if isinstance(ndk_api, bool) or not isinstance(ndk_api, int) or ndk_api < 24:
            raise SkiaToolError("Android GN argument ndk_api must be an integer >= 24")
    bridge_source_owner = target_recipe if target_recipe is not None else profile
    bridge_define_owner = target_recipe if target_recipe is not None else profile
    return {
        "schema_version": BUILD_PLAN_SCHEMA_VERSION,
        "skia_revision": require_string(config["source"]["revision"], "source revision"),
        "bridge_abi_version": config["bridge"]["abi_version"],
        "profile": profile_name,
        "target": target_name,
        "bridge_sources": require_string_list(
            bridge_source_owner.get("bridge_sources"),
            "selected bridge_sources",
        ),
        "bridge_defines": dict(
            sorted(
                require_string_map(
                    bridge_define_owner.get("bridge_defines"),
                    "selected bridge_defines",
                ).items()
            )
        ),
        "gn_args": dict(sorted(gn_args.items())),
        "ninja_targets": require_string_list(profile.get("ninja_targets"), "profile ninja_targets"),
        "upstream_libraries": require_string_list(
            profile.get("upstream_libraries"),
            "profile upstream_libraries",
        ),
    }


def validate_build_recipe(
    raw_recipe: Any,
    config: Mapping[str, Any],
    expected_profile: str,
    expected_target: str,
) -> dict[str, Any]:
    recipe = require_object(raw_recipe, "build receipt.plan.recipe")
    expected_fields = {
        "schema_version",
        "skia_revision",
        "bridge_abi_version",
        "profile",
        "target",
        "bridge_sources",
        "bridge_defines",
        "gn_args",
        "ninja_targets",
        "upstream_libraries",
    }
    if set(recipe) != expected_fields:
        raise SkiaToolError("build receipt recipe has unknown or missing fields")
    if recipe.get("schema_version") != BUILD_PLAN_SCHEMA_VERSION:
        raise SkiaToolError("build receipt recipe schema is unsupported")
    if recipe.get("profile") != expected_profile or recipe.get("target") != expected_target:
        raise SkiaToolError("build receipt recipe profile or target does not match packaging request")

    gn_args = require_object(recipe.get("gn_args"), "build receipt.plan.recipe.gn_args")
    profile = select_profile(config, expected_profile)
    target = select_target(config, expected_target)
    target_recipe = select_profile_target_recipe(profile, expected_profile, expected_target)
    configured: dict[str, Any] = {}
    for source in (
        require_object(config.get("common_native_gn_args"), "common_native_gn_args"),
        require_object(profile.get("gn_args"), f"profiles.{expected_profile}.gn_args"),
        require_object(target.get("gn_args"), f"targets.{expected_target}.gn_args"),
        require_object(target_recipe.get("gn_args"), "profile target recipe gn_args")
        if target_recipe is not None
        else {},
    ):
        configured.update(source)
    overrides = {name: value for name, value in gn_args.items() if name not in configured}
    expected = resolve_build_plan(config, expected_profile, expected_target, overrides)
    if recipe != expected:
        raise SkiaToolError("build receipt recipe does not match the pinned configuration")
    return recipe


def check_reusable_build_dir(build_dir: Path, plan: Mapping[str, Any]) -> None:
    if not build_dir.exists():
        build_dir.mkdir(parents=True)
        return
    if not build_dir.is_dir():
        raise SkiaToolError(f"build output is not a directory: {build_dir}")
    existing_plan = build_dir / BUILD_PLAN
    if any(build_dir.iterdir()) and not existing_plan.is_file():
        raise SkiaToolError(
            f"refusing to reuse non-empty build directory without {BUILD_PLAN}: {build_dir}"
        )
    if existing_plan.is_file() and load_json(existing_plan) != plan:
        raise SkiaToolError(
            f"build directory was configured for a different source/profile/target: {build_dir}"
        )


def regular_file_record(path: Path, *, name: str, relative_path: str) -> dict[str, Any]:
    try:
        metadata = path.stat(follow_symlinks=False)
    except FileNotFoundError as error:
        raise SkiaToolError(f"required build output does not exist: {path}") from error
    if not stat.S_ISREG(metadata.st_mode):
        raise SkiaToolError(f"required build output is not a regular file: {path}")
    return {
        "name": name,
        "path": relative_path,
        "sha256": sha256_file(path),
        "size": metadata.st_size,
    }


def collect_build_outputs(build_dir: Path, recipe: Mapping[str, Any]) -> list[dict[str, Any]]:
    target = require_string(recipe.get("target"), "build recipe target")
    libraries = require_string_list(
        recipe.get("upstream_libraries"),
        "build recipe upstream_libraries",
    )
    outputs: list[dict[str, Any]] = []
    for name in libraries:
        relative = canonical_library_filename(name, target)
        outputs.append(
            regular_file_record(build_dir / relative, name=name, relative_path=relative)
        )
    return outputs


def validate_digest_record(raw: Any, context: str) -> dict[str, Any]:
    record = require_object(raw, context)
    if set(record) != {"name", "path", "sha256", "size"}:
        raise SkiaToolError(f"{context} has unknown or missing fields")
    name = require_string(record.get("name"), f"{context}.name")
    relative = validate_relative_path(record.get("path"), f"{context}.path").as_posix()
    digest = record.get("sha256")
    size = record.get("size")
    if not NAME_RE.fullmatch(name):
        raise SkiaToolError(f"{context}.name is unsafe")
    if not isinstance(digest, str) or not SHA256_RE.fullmatch(digest):
        raise SkiaToolError(f"{context}.sha256 is invalid")
    if not isinstance(size, int) or isinstance(size, bool) or size < 0:
        raise SkiaToolError(f"{context}.size is invalid")
    return {"name": name, "path": relative, "sha256": digest, "size": size}


def validate_source_identity(raw: Any, config: Mapping[str, Any]) -> dict[str, Any]:
    source = require_object(raw, "build receipt.plan.source")
    if set(source) != {"kind", "qualified", "repository", "revision"}:
        raise SkiaToolError("build receipt source identity has unknown or missing fields")
    if source.get("kind") not in {"local-git-checkout", "local-vendored-source"}:
        raise SkiaToolError("build receipt source is not an accepted explicit local source")
    if source.get("qualified") is not False:
        raise SkiaToolError("local and vendored source receipts must remain unqualified")
    if source.get("repository") != config["source"]["repository"]:
        raise SkiaToolError("build receipt source repository does not match pinned configuration")
    if source.get("revision") != config["source"]["revision"]:
        raise SkiaToolError("build receipt source revision does not match pinned configuration")
    return source


def validate_tool_identity(raw: Any, context: str) -> dict[str, str]:
    identity = require_object(raw, context)
    if set(identity) != {"expected_sha256", "actual_sha256", "version"}:
        raise SkiaToolError(f"{context} has unknown or missing fields")
    expected = identity.get("expected_sha256")
    actual = identity.get("actual_sha256")
    if not isinstance(expected, str) or not SHA256_RE.fullmatch(expected):
        raise SkiaToolError(f"{context}.expected_sha256 is invalid")
    if not isinstance(actual, str) or not SHA256_RE.fullmatch(actual):
        raise SkiaToolError(f"{context}.actual_sha256 is invalid")
    if expected != actual:
        raise SkiaToolError(f"{context} expected and actual SHA-256 values differ")
    version = require_string(identity.get("version"), f"{context}.version")
    if normalized_identity_text(version, f"{context}.version") != version:
        raise SkiaToolError(f"{context}.version is not normalized")
    return {
        "expected_sha256": expected,
        "actual_sha256": actual,
        "version": version,
    }


def validate_build_receipt(
    raw_receipt: Any,
    config: Mapping[str, Any],
    expected_profile: str,
    expected_target: str,
) -> dict[str, Any]:
    receipt = require_object(raw_receipt, "build receipt")
    if set(receipt) != {"schema_version", "result", "plan", "plan_sha256", "outputs"}:
        raise SkiaToolError("build receipt has unknown or missing fields")
    if receipt.get("schema_version") != BUILD_RECEIPT_SCHEMA_VERSION:
        raise SkiaToolError("build receipt schema is unsupported")
    if receipt.get("result") != "complete":
        raise SkiaToolError("build receipt does not describe a complete build")

    plan = require_object(receipt.get("plan"), "build receipt.plan")
    if set(plan) != {"schema_version", "recipe", "source", "toolchain_id", "tools"}:
        raise SkiaToolError("build receipt plan has unknown or missing fields")
    if plan.get("schema_version") != BUILD_PLAN_SCHEMA_VERSION:
        raise SkiaToolError("build receipt plan schema is unsupported")
    recipe = validate_build_recipe(
        plan.get("recipe"), config, expected_profile, expected_target
    )
    validate_source_identity(plan.get("source"), config)
    toolchain_id = require_string(
        plan.get("toolchain_id"),
        "build receipt.plan.toolchain_id",
    )
    if normalized_identity_text(toolchain_id, "build receipt.plan.toolchain_id") != toolchain_id:
        raise SkiaToolError("build receipt plan toolchain_id is not normalized")
    tools = require_object(plan.get("tools"), "build receipt.plan.tools")
    if set(tools) != {"gn", "ninja"}:
        raise SkiaToolError("build receipt tools must contain exactly gn and ninja")
    validate_tool_identity(tools.get("gn"), "build receipt.plan.tools.gn")
    validate_tool_identity(tools.get("ninja"), "build receipt.plan.tools.ninja")

    expected_plan_digest = sha256_json(plan)
    if receipt.get("plan_sha256") != expected_plan_digest:
        raise SkiaToolError("build receipt plan digest does not match its canonical plan")

    raw_outputs = receipt.get("outputs")
    if not isinstance(raw_outputs, list):
        raise SkiaToolError("build receipt outputs must be an array")
    outputs = [
        validate_digest_record(raw, f"build receipt.outputs[{index}]")
        for index, raw in enumerate(raw_outputs)
    ]
    names = [output["name"] for output in outputs]
    if len(names) != len(set(names)):
        raise SkiaToolError("build receipt output names must not contain duplicates")
    expected_names = recipe["upstream_libraries"]
    if names != expected_names:
        raise SkiaToolError(
            "build receipt outputs must match the profile's ordered upstream library list"
        )
    for output in outputs:
        expected_path = canonical_library_filename(output["name"], expected_target)
        if output["path"] != expected_path:
            raise SkiaToolError(
                f"build receipt output {output['name']!r} has unexpected path {output['path']!r}"
            )
    return receipt


def build_native(args: argparse.Namespace, config: dict[str, Any]) -> None:
    source_dir = resolve_explicit_path(
        args.source_dir, "FISSION_SKIA_SOURCE_DIR", "pinned Skia source directory", must_exist=True
    )
    build_dir = resolve_explicit_path(
        args.build_dir, "FISSION_SKIA_BUILD_DIR", "Skia GN output directory", must_exist=False
    )
    if build_dir == source_dir or build_dir in source_dir.parents:
        raise SkiaToolError("build output must not be the Skia source directory or one of its parents")
    expected_revision = config["source"]["revision"]
    source_identity = verify_source_checkout(
        source_dir,
        expected_revision,
        config["source"]["repository"],
    )
    license_path = source_dir / config["source"]["license_file"]
    if not license_path.is_file():
        raise SkiaToolError(f"pinned Skia checkout is missing its licence: {license_path}")

    overrides = parse_gn_overrides(args.gn_arg)
    recipe = resolve_build_plan(config, args.profile, args.target, overrides)

    gn_path = Path(args.gn).expanduser().resolve() if args.gn else source_dir / "bin" / "gn"
    ninja_path = (
        Path(args.ninja).expanduser().resolve()
        if args.ninja
        else source_dir / "third_party" / "ninja" / ("ninja.exe" if os.name == "nt" else "ninja")
    )
    plan = {
        "schema_version": BUILD_PLAN_SCHEMA_VERSION,
        "recipe": recipe,
        "source": source_identity,
        "toolchain_id": normalized_identity_text(
            require_string(args.toolchain_id, "--toolchain-id"),
            "--toolchain-id",
        ),
        "tools": verified_tool_identities(
            gn_path,
            args.gn_sha256,
            ninja_path,
            args.ninja_sha256,
        ),
    }
    check_reusable_build_dir(build_dir, plan)
    write_json(build_dir / BUILD_PLAN, plan)

    rendered_args = " ".join(
        f"{name}={gn_literal(value)}" for name, value in recipe["gn_args"].items()
    )
    verify_tool_digest(gn_path, args.gn_sha256, "GN")
    run_checked(
        [str(gn_path), "gen", str(build_dir), f"--args={rendered_args}", "--fail-on-unused-args"],
        cwd=source_dir,
    )
    verify_tool_digest(ninja_path, args.ninja_sha256, "Ninja")
    run_checked(
        [str(ninja_path), "-C", str(build_dir), *recipe["ninja_targets"]],
        cwd=source_dir,
    )
    metadata = {
        "schema_version": BUILD_RECEIPT_SCHEMA_VERSION,
        "result": "complete",
        "plan": plan,
        "plan_sha256": sha256_json(plan),
        "outputs": collect_build_outputs(build_dir, recipe),
    }
    write_json(build_dir / BUILD_METADATA, metadata)
    print(build_dir / BUILD_METADATA)


def load_deployment_metadata(path: Path, target: Mapping[str, Any]) -> dict[str, Any]:
    metadata = require_object(load_json(path), str(path))
    if set(metadata) != {"deployment", "toolchain"}:
        raise SkiaToolError("deployment metadata must contain exactly 'deployment' and 'toolchain'")
    deployment = require_object(metadata["deployment"], "deployment metadata.deployment")
    toolchain = require_object(metadata["toolchain"], "deployment metadata.toolchain")
    for required in target.get("deployment_fields", []):
        require_string(deployment.get(required), f"deployment.{required}")
    if target.get("platform") == "Android" and deployment.get("cxx_runtime") != "libc++_shared":
        raise SkiaToolError("Android deployment.cxx_runtime must be 'libc++_shared'")
    require_string(toolchain.get("id"), "toolchain.id")
    require_string(toolchain.get("compiler"), "toolchain.compiler")
    require_string(toolchain.get("runtime_abi"), "toolchain.runtime_abi")
    return metadata


def load_link_metadata(path: Path, expected_libraries: Sequence[str]) -> dict[str, Any]:
    metadata = require_object(load_json(path), str(path))
    allowed = {"link_search_paths", "static_libraries", "system_libraries", "frameworks"}
    unknown = set(metadata) - allowed
    if unknown:
        raise SkiaToolError(f"unknown native link metadata keys: {', '.join(sorted(unknown))}")
    search = require_string_list(metadata.get("link_search_paths"), "native.link_search_paths")
    static = require_string_list(metadata.get("static_libraries"), "native.static_libraries")
    system = require_string_list(metadata.get("system_libraries", []), "native.system_libraries")
    frameworks = require_string_list(metadata.get("frameworks", []), "native.frameworks")
    if search != ["lib"]:
        raise SkiaToolError("native.link_search_paths must be exactly ['lib']")
    if not static or static[0] != "fission_skia_bridge":
        raise SkiaToolError("native.static_libraries must begin with fission_skia_bridge")
    if static != list(expected_libraries):
        raise SkiaToolError(
            "native.static_libraries does not match the profile's required link order"
        )
    return {
        "link_search_paths": search,
        "static_libraries": static,
        "system_libraries": system,
        "frameworks": frameworks,
    }


def assert_empty_output(path: Path) -> None:
    if path.exists():
        if not path.is_dir():
            raise SkiaToolError(f"artifact output is not a directory: {path}")
        if any(path.iterdir()):
            raise SkiaToolError(f"refusing to overwrite non-empty artifact output: {path}")
    else:
        path.mkdir(parents=True)


def copy_file(source: Path, destination: Path) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, destination)
    os.chmod(destination, 0o644)


def payload_record(path: Path, relative_path: str) -> dict[str, Any]:
    record = regular_file_record(path, name="payload", relative_path=relative_path)
    return {key: record[key] for key in ("path", "sha256", "size")}


def validate_bridge_header(path: Path, expected_abi: int) -> None:
    try:
        raw = path.read_text(encoding="utf-8")
    except UnicodeDecodeError as error:
        raise SkiaToolError(f"bridge header is not valid UTF-8: {path}") from error
    matches = re.findall(
        r"(?m)^\s*#\s*define\s+FISSION_SKIA_ABI_VERSION\s+([0-9]+)[uUlL]*\s*$",
        raw,
    )
    if len(matches) != 1:
        raise SkiaToolError("bridge header must define FISSION_SKIA_ABI_VERSION exactly once")
    if int(matches[0]) != expected_abi:
        raise SkiaToolError(
            f"bridge header ABI mismatch: expected {expected_abi}, found {matches[0]}"
        )


def artifact_tree_entries(root: Path) -> list[Path]:
    try:
        root_metadata = root.stat(follow_symlinks=False)
    except FileNotFoundError as error:
        raise SkiaToolError(f"artifact directory does not exist: {root}") from error
    if not stat.S_ISDIR(root_metadata.st_mode):
        raise SkiaToolError(f"artifact root must be a regular directory, not a link: {root}")

    entries: list[Path] = []

    def visit(directory: Path) -> None:
        try:
            with os.scandir(directory) as iterator:
                children = sorted(iterator, key=lambda entry: entry.name)
        except OSError as error:
            raise SkiaToolError(f"cannot inspect artifact directory {directory}: {error}") from error
        for child in children:
            path = Path(child.path)
            if child.is_symlink():
                raise SkiaToolError(f"artifact must not contain symbolic links: {path}")
            try:
                if child.is_dir(follow_symlinks=False):
                    entries.append(path)
                    visit(path)
                elif child.is_file(follow_symlinks=False):
                    entries.append(path)
                else:
                    raise SkiaToolError(
                        f"artifact must contain only regular files and directories: {path}"
                    )
            except OSError as error:
                raise SkiaToolError(f"cannot inspect artifact entry {path}: {error}") from error

    visit(root)
    return entries


def listed_files(root: Path) -> list[dict[str, Any]]:
    files: list[dict[str, Any]] = []
    for path in artifact_tree_entries(root):
        relative = path.relative_to(root).as_posix()
        if path.is_file() and relative != MANIFEST:
            files.append({"path": relative, "sha256": sha256_file(path), "size": path.stat().st_size})
    return files


def canonical_library_filename(name: str, target_name: str) -> str:
    if "windows-msvc" in target_name:
        return f"{name}.lib"
    return f"lib{name}.a"


def package_native(args: argparse.Namespace, config: dict[str, Any]) -> None:
    profile = select_profile(config, args.profile)
    target = select_target(config, args.target)
    if profile.get("kind") != "native" or target.get("kind") != "native":
        raise SkiaToolError("package-native accepts only a native profile and native target")
    if profile.get("build_recipe") != "available":
        raise SkiaToolError(f"profile {args.profile!r} does not yet have a supported build recipe")
    select_profile_target_recipe(profile, args.profile, args.target)

    output = Path(args.output).expanduser().resolve()
    archive = Path(args.archive).expanduser().resolve() if args.archive else None
    if archive is not None and (archive == output or output in archive.parents):
        raise SkiaToolError(
            "artifact archive must be outside the staged artifact directory"
        )

    build_metadata_path = Path(args.build_metadata).expanduser().resolve()
    header = Path(args.bridge_header).expanduser().resolve()
    if not header.is_file():
        raise SkiaToolError(f"bridge header does not exist: {header}")
    libraries = parse_named_paths(args.library, "static library")
    required_library_order = [
        "fission_skia_bridge",
        *require_string_list(profile.get("upstream_libraries"), "profile upstream_libraries"),
    ]
    required_libraries = set(required_library_order)
    if set(libraries) != required_libraries:
        raise SkiaToolError(
            "static library set does not match the profile; "
            f"missing={sorted(required_libraries - set(libraries))}, "
            f"extra={sorted(set(libraries) - required_libraries)}"
        )
    links = load_link_metadata(
        Path(args.link_metadata).expanduser().resolve(),
        required_library_order,
    )
    validate_profile_target_links(profile, args.profile, args.target, links)
    licences = parse_named_paths(args.license, "licence")
    required_licences = set(
        require_string_list(profile.get("required_licenses"), "profile required_licenses")
    )
    if set(licences) != required_licences:
        raise SkiaToolError(
            "licence set does not match the profile; "
            f"missing={sorted(required_licences - set(licences))}, "
            f"extra={sorted(set(licences) - required_licences)}"
        )
    deployment = load_deployment_metadata(
        Path(args.deployment_metadata).expanduser().resolve(), target
    )
    assert_empty_output(output)

    copy_file(header, output / config["bridge"]["header"])
    for name, path in libraries.items():
        copy_file(path, output / "lib" / canonical_library_filename(name, args.target))
    for name, path in licences.items():
        copy_file(path, output / "licenses" / f"{name}.txt")
    copy_file(build_metadata_path, output / config["artifact"]["build_metadata"])

    copied_build_metadata = output / config["artifact"]["build_metadata"]
    build_receipt = validate_build_receipt(
        load_json(copied_build_metadata),
        config,
        args.profile,
        args.target,
    )
    copied_header = output / config["bridge"]["header"]
    validate_bridge_header(copied_header, config["bridge"]["abi_version"])
    expected_outputs = {
        entry["name"]: entry for entry in build_receipt["outputs"]
    }
    for name in build_receipt["plan"]["recipe"]["upstream_libraries"]:
        relative = f"lib/{canonical_library_filename(name, args.target)}"
        actual = regular_file_record(output / relative, name=name, relative_path=relative)
        expected = expected_outputs[name]
        if actual["sha256"] != expected["sha256"] or actual["size"] != expected["size"]:
            raise SkiaToolError(
                f"packaged library {name!r} does not match the completed build receipt"
            )

    fission_version = require_string(args.fission_version, "--fission-version")
    if not NAME_RE.fullmatch(fission_version):
        raise SkiaToolError("--fission-version contains characters unsafe for an artifact identity")
    artifact_id = (
        f"fission-skia-{fission_version}-abi{config['bridge']['abi_version']}-"
        f"{args.profile}-{args.target}"
    )
    bridge_library_path = f"lib/{canonical_library_filename('fission_skia_bridge', args.target)}"
    manifest = {
        "schema_version": config["artifact"]["schema_version"],
        "artifact_id": artifact_id,
        "origin": "local-build",
        "qualified": False,
        "fission_version": fission_version,
        "skia": {
            "repository": config["source"]["repository"],
            "revision": config["source"]["revision"],
            "milestone_hint": config["source"]["milestone_hint"],
        },
        "bridge_abi_version": config["bridge"]["abi_version"],
        "bridge": {
            "abi_version": config["bridge"]["abi_version"],
            "header": payload_record(copied_header, config["bridge"]["header"]),
            "library": payload_record(output / bridge_library_path, bridge_library_path),
            "sources": build_receipt["plan"]["recipe"]["bridge_sources"],
            "defines": build_receipt["plan"]["recipe"]["bridge_defines"],
        },
        "build_receipt": {
            **payload_record(
                copied_build_metadata,
                config["artifact"]["build_metadata"],
            ),
            "plan_sha256": build_receipt["plan_sha256"],
        },
        "target": args.target,
        "platform": target["platform"],
        "profile": args.profile,
        "features": profile["features"],
        "toolchain": deployment["toolchain"],
        "deployment": deployment["deployment"],
        "native": links,
        "files": listed_files(output),
    }
    write_json(output / MANIFEST, manifest)
    verify_artifact_directory(output, config, expected_profile=args.profile, expected_target=args.target)
    print(output / MANIFEST)

    if archive is not None:
        epoch_raw = args.source_date_epoch or os.environ.get("SOURCE_DATE_EPOCH")
        if epoch_raw is None or not re.fullmatch(r"[0-9]+", epoch_raw):
            raise SkiaToolError("--source-date-epoch or numeric SOURCE_DATE_EPOCH is required for archives")
        digest = create_archive_with_sidecar(output, archive, artifact_id, int(epoch_raw))
        print(f"{digest}  {archive}")


def validate_relative_path(raw: Any, context: str) -> PurePosixPath:
    value = require_string(raw, context)
    path = PurePosixPath(value)
    if (
        "\\" in value
        or path.is_absolute()
        or ".." in path.parts
        or "." in path.parts
        or str(path) != value
    ):
        raise SkiaToolError(f"unsafe or non-canonical artifact path in {context}: {value!r}")
    return path


def validate_payload_binding(
    raw: Any,
    context: str,
    expected_path: str,
    files: Mapping[str, Mapping[str, Any]],
) -> dict[str, Any]:
    binding = require_object(raw, context)
    if set(binding) != {"path", "sha256", "size"}:
        raise SkiaToolError(f"{context} has unknown or missing fields")
    path = validate_relative_path(binding.get("path"), f"{context}.path").as_posix()
    if path != expected_path:
        raise SkiaToolError(f"{context}.path must be {expected_path!r}")
    declared = files.get(path)
    if declared is None:
        raise SkiaToolError(f"{context} refers to an undeclared artifact file")
    if binding.get("sha256") != declared["sha256"] or binding.get("size") != declared["size"]:
        raise SkiaToolError(f"{context} digest or size does not match manifest.files")
    return binding


def verify_artifact_directory(
    root: Path,
    config: Mapping[str, Any],
    *,
    expected_profile: str | None = None,
    expected_target: str | None = None,
    expected_manifest_sha256: str | None = None,
) -> dict[str, Any]:
    tree_entries = artifact_tree_entries(root)
    tree_files = {
        path.relative_to(root).as_posix(): path
        for path in tree_entries
        if path.is_file()
    }
    manifest_path = root / MANIFEST
    if expected_manifest_sha256:
        if not SHA256_RE.fullmatch(expected_manifest_sha256):
            raise SkiaToolError("expected manifest SHA-256 must be 64 lowercase hexadecimal characters")
        actual = sha256_file(manifest_path)
        if actual != expected_manifest_sha256:
            raise SkiaToolError(f"manifest digest mismatch: expected {expected_manifest_sha256}, found {actual}")
    manifest = require_object(load_json(manifest_path), str(manifest_path))
    if manifest.get("schema_version") != config["artifact"]["schema_version"]:
        raise SkiaToolError("artifact manifest schema does not match the pinned configuration")
    if manifest.get("bridge_abi_version") != config["bridge"]["abi_version"]:
        raise SkiaToolError("artifact bridge ABI does not match the pinned configuration")
    skia = require_object(manifest.get("skia"), "manifest.skia")
    if skia.get("repository") != config["source"]["repository"]:
        raise SkiaToolError("artifact Skia repository does not match the pinned configuration")
    if skia.get("revision") != config["source"]["revision"]:
        raise SkiaToolError("artifact Skia revision does not match the pinned configuration")
    profile_name = require_string(manifest.get("profile"), "manifest.profile")
    target_name = require_string(manifest.get("target"), "manifest.target")
    profile = select_profile(config, profile_name)
    target = select_target(config, target_name)
    if profile.get("kind") != "native" or target.get("kind") != "native":
        raise SkiaToolError("native artifact identifies a non-native profile or target")
    select_profile_target_recipe(profile, profile_name, target_name)
    if expected_profile is not None and profile_name != expected_profile:
        raise SkiaToolError(f"artifact profile mismatch: expected {expected_profile}, found {profile_name}")
    if expected_target is not None and target_name != expected_target:
        raise SkiaToolError(f"artifact target mismatch: expected {expected_target}, found {target_name}")
    if manifest.get("qualified") is not False:
        raise SkiaToolError("foundation tooling does not accept artifacts claiming production qualification")
    if manifest.get("origin") != "local-build":
        raise SkiaToolError("foundation tooling accepts only explicitly local, unqualified artifacts")

    entries = manifest.get("files")
    if not isinstance(entries, list) or not entries:
        raise SkiaToolError("manifest.files must be a non-empty array")
    declared: set[str] = set()
    declared_entries: dict[str, dict[str, Any]] = {}
    for index, raw_entry in enumerate(entries):
        entry = require_object(raw_entry, f"manifest.files[{index}]")
        if set(entry) != {"path", "sha256", "size"}:
            raise SkiaToolError(f"manifest.files[{index}] has unknown or missing fields")
        relative = validate_relative_path(entry["path"], f"manifest.files[{index}].path")
        relative_text = relative.as_posix()
        if relative_text in declared:
            raise SkiaToolError(f"artifact file is declared more than once: {relative_text}")
        declared.add(relative_text)
        digest = entry["sha256"]
        size = entry["size"]
        if not isinstance(digest, str) or not SHA256_RE.fullmatch(digest):
            raise SkiaToolError(f"invalid SHA-256 for {relative_text}")
        if not isinstance(size, int) or isinstance(size, bool) or size < 0:
            raise SkiaToolError(f"invalid size for {relative_text}")
        path = tree_files.get(relative_text)
        if path is None:
            raise SkiaToolError(f"declared artifact file is missing or not regular: {relative_text}")
        if path.stat().st_size != size:
            raise SkiaToolError(f"artifact size mismatch: {relative_text}")
        if sha256_file(path) != digest:
            raise SkiaToolError(f"artifact digest mismatch: {relative_text}")
        declared_entries[relative_text] = {
            "path": relative_text,
            "sha256": digest,
            "size": size,
        }

    actual = set(tree_files) - {MANIFEST}
    if actual != declared:
        raise SkiaToolError(
            f"artifact file set differs from manifest; missing={sorted(declared - actual)}, "
            f"undeclared={sorted(actual - declared)}"
        )
    header = config["bridge"]["header"]
    if header not in declared:
        raise SkiaToolError(f"artifact does not declare required bridge header: {header}")
    native = require_object(manifest.get("native"), "manifest.native")
    links = load_link_metadata_object(native)
    validate_profile_target_links(profile, profile_name, target_name, links)
    expected_library_paths = {
        f"lib/{canonical_library_filename(name, target_name)}" for name in links["static_libraries"]
    }
    if not expected_library_paths.issubset(declared):
        raise SkiaToolError(
            f"artifact is missing declared static libraries: {sorted(expected_library_paths - declared)}"
        )

    required_licences = set(
        require_string_list(profile.get("required_licenses"), "profile required_licenses")
    )
    actual_licence_paths = {
        path for path in declared if path.startswith("licenses/")
    }
    expected_licence_paths = {f"licenses/{name}.txt" for name in required_licences}
    if actual_licence_paths != expected_licence_paths:
        raise SkiaToolError(
            "artifact licence set does not match its profile; "
            f"missing={sorted(expected_licence_paths - actual_licence_paths)}, "
            f"extra={sorted(actual_licence_paths - expected_licence_paths)}"
        )

    bridge = require_object(manifest.get("bridge"), "manifest.bridge")
    if set(bridge) != {"abi_version", "header", "library", "sources", "defines"}:
        raise SkiaToolError("manifest.bridge has unknown or missing fields")
    if bridge.get("abi_version") != config["bridge"]["abi_version"]:
        raise SkiaToolError("manifest.bridge ABI does not match the pinned configuration")
    validate_payload_binding(
        bridge.get("header"),
        "manifest.bridge.header",
        header,
        declared_entries,
    )
    bridge_library_path = f"lib/{canonical_library_filename('fission_skia_bridge', target_name)}"
    validate_payload_binding(
        bridge.get("library"),
        "manifest.bridge.library",
        bridge_library_path,
        declared_entries,
    )
    validate_bridge_header(root / header, config["bridge"]["abi_version"])

    build_binding = require_object(manifest.get("build_receipt"), "manifest.build_receipt")
    if set(build_binding) != {"path", "sha256", "size", "plan_sha256"}:
        raise SkiaToolError("manifest.build_receipt has unknown or missing fields")
    build_path = config["artifact"]["build_metadata"]
    validate_payload_binding(
        {key: build_binding[key] for key in ("path", "sha256", "size")},
        "manifest.build_receipt",
        build_path,
        declared_entries,
    )
    receipt = validate_build_receipt(
        load_json(root / build_path),
        config,
        profile_name,
        target_name,
    )
    recipe = receipt["plan"]["recipe"]
    if bridge.get("sources") != recipe["bridge_sources"]:
        raise SkiaToolError("manifest bridge sources do not match the completed build receipt")
    if bridge.get("defines") != recipe["bridge_defines"]:
        raise SkiaToolError("manifest bridge defines do not match the completed build receipt")
    if build_binding.get("plan_sha256") != receipt["plan_sha256"]:
        raise SkiaToolError("manifest build receipt plan digest does not match the receipt")
    expected_link_order = [
        "fission_skia_bridge",
        *receipt["plan"]["recipe"]["upstream_libraries"],
    ]
    if links["static_libraries"] != expected_link_order:
        raise SkiaToolError("manifest native libraries do not match the build receipt link order")

    expected_outputs = {entry["name"]: entry for entry in receipt["outputs"]}
    for name in receipt["plan"]["recipe"]["upstream_libraries"]:
        path = f"lib/{canonical_library_filename(name, target_name)}"
        declared_output = declared_entries.get(path)
        expected_output = expected_outputs[name]
        if declared_output is None or (
            declared_output["sha256"] != expected_output["sha256"]
            or declared_output["size"] != expected_output["size"]
        ):
            raise SkiaToolError(
                f"artifact library {name!r} does not match its completed build receipt"
            )
    return manifest


def load_link_metadata_object(metadata: Mapping[str, Any]) -> dict[str, Any]:
    if set(metadata) != {"link_search_paths", "static_libraries", "system_libraries", "frameworks"}:
        raise SkiaToolError("manifest.native contains unknown or missing fields")
    search = require_string_list(metadata["link_search_paths"], "manifest.native.link_search_paths")
    static = require_string_list(metadata["static_libraries"], "manifest.native.static_libraries")
    system = require_string_list(metadata["system_libraries"], "manifest.native.system_libraries")
    frameworks = require_string_list(metadata["frameworks"], "manifest.native.frameworks")
    if search != ["lib"]:
        raise SkiaToolError("manifest.native.link_search_paths must be exactly ['lib']")
    if not static or static[0] != "fission_skia_bridge":
        raise SkiaToolError("manifest.native.static_libraries must begin with fission_skia_bridge")
    return {
        "link_search_paths": search,
        "static_libraries": static,
        "system_libraries": system,
        "frameworks": frameworks,
    }


def validate_archive_destination(archive: Path) -> None:
    if archive.suffixes[-2:] != [".tar", ".gz"]:
        raise SkiaToolError("artifact archive name must end in .tar.gz")


def write_reproducible_archive(root: Path, archive: Path, archive_root: str, epoch: int) -> None:
    if not NAME_RE.fullmatch(archive_root):
        raise SkiaToolError("archive root contains unsafe characters")
    if epoch < 0:
        raise SkiaToolError("archive epoch must not be negative")
    entries = artifact_tree_entries(root)
    with archive.open("xb") as raw_output:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw_output, mtime=epoch) as compressed:
            with tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as output:
                for source in entries:
                    relative = source.relative_to(root).as_posix()
                    info = output.gettarinfo(str(source), arcname=f"{archive_root}/{relative}")
                    info.uid = 0
                    info.gid = 0
                    info.uname = ""
                    info.gname = ""
                    info.mtime = epoch
                    info.mode = 0o755 if source.is_dir() else 0o644
                    if source.is_file():
                        with source.open("rb") as payload:
                            output.addfile(info, payload)
                    else:
                        output.addfile(info)
        raw_output.flush()
        os.fsync(raw_output.fileno())


def publish_file_no_replace(source: Path, destination: Path) -> tuple[int, int]:
    identity = source.stat(follow_symlinks=False)
    try:
        os.link(source, destination, follow_symlinks=False)
    except FileExistsError as error:
        raise SkiaToolError(f"refusing to overwrite existing file: {destination}") from error
    except OSError as error:
        raise SkiaToolError(f"cannot atomically publish {destination}: {error}") from error
    source.unlink()
    return identity.st_dev, identity.st_ino


def unlink_if_identity_matches(path: Path, identity: tuple[int, int]) -> None:
    try:
        metadata = path.stat(follow_symlinks=False)
    except FileNotFoundError:
        return
    if (metadata.st_dev, metadata.st_ino) == identity:
        path.unlink()


def create_reproducible_archive(root: Path, archive: Path, archive_root: str, epoch: int) -> None:
    validate_archive_destination(archive)
    if archive.exists():
        raise SkiaToolError(f"refusing to overwrite archive: {archive}")
    archive.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=f".{archive.name}.", dir=archive.parent) as temporary:
        staged = Path(temporary) / "payload.tar.gz"
        write_reproducible_archive(root, staged, archive_root, epoch)
        publish_file_no_replace(staged, archive)


def create_archive_with_sidecar(
    root: Path,
    archive: Path,
    archive_root: str,
    epoch: int,
) -> str:
    validate_archive_destination(archive)
    sidecar = archive.with_suffix(archive.suffix + ".sha256")
    existing = [path for path in (archive, sidecar) if path.exists()]
    if existing:
        raise SkiaToolError(
            "refusing to overwrite archive output: " + ", ".join(str(path) for path in existing)
        )
    archive.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=f".{archive.name}.", dir=archive.parent) as temporary:
        staging = Path(temporary)
        staged_archive = staging / "payload.tar.gz"
        staged_sidecar = staging / "payload.tar.gz.sha256"
        write_reproducible_archive(root, staged_archive, archive_root, epoch)
        digest = sha256_file(staged_archive)
        with staged_sidecar.open("x", encoding="ascii") as output:
            output.write(f"{digest}  {archive.name}\n")
            output.flush()
            os.fsync(output.fileno())
        sidecar_identity = publish_file_no_replace(staged_sidecar, sidecar)
        try:
            publish_file_no_replace(staged_archive, archive)
        except BaseException:
            # The sidecar link is ours because publication never replaces an
            # existing path. Roll it back so callers never receive half a pair.
            unlink_if_identity_matches(sidecar, sidecar_identity)
            raise
    return digest


def validated_archive_members(source: tarfile.TarFile) -> tuple[str, list[tarfile.TarInfo]]:
    members = source.getmembers()
    if not members:
        raise SkiaToolError("artifact archive is empty")
    roots: set[str] = set()
    names: set[str] = set()
    casefolded_names: set[str] = set()
    has_manifest = False
    for member in members:
        raw_name = member.name
        path = PurePosixPath(raw_name)
        if (
            "\\" in raw_name
            or path.is_absolute()
            or ".." in path.parts
            or "." in path.parts
            or not path.parts
            or str(path) != raw_name
        ):
            raise SkiaToolError(f"unsafe or non-canonical path in artifact archive: {raw_name!r}")
        if not member.isfile() and not member.isdir():
            raise SkiaToolError(
                f"artifact archives may contain only regular files and directories: {raw_name!r}"
            )
        if raw_name in names or raw_name.casefold() in casefolded_names:
            raise SkiaToolError(f"duplicate or case-colliding artifact archive path: {raw_name!r}")
        names.add(raw_name)
        casefolded_names.add(raw_name.casefold())
        roots.add(path.parts[0])
        if len(path.parts) == 2 and path.parts[1] == MANIFEST and member.isfile():
            has_manifest = True
    if len(roots) != 1:
        raise SkiaToolError("artifact archive must contain exactly one top-level directory")
    root = next(iter(roots))
    if not NAME_RE.fullmatch(root):
        raise SkiaToolError("artifact archive has an unsafe top-level directory name")
    if not has_manifest:
        raise SkiaToolError("artifact archive does not contain a root manifest.json")
    return root, members


def extract_validated_archive(
    source: tarfile.TarFile,
    destination: Path,
) -> str:
    root_name, members = validated_archive_members(source)
    destination_root = destination.resolve(strict=True)
    for member in sorted(members, key=lambda item: (len(PurePosixPath(item.name).parts), item.name)):
        relative = PurePosixPath(member.name)
        target = destination.joinpath(*relative.parts)
        parent = target.parent
        parent.mkdir(parents=True, exist_ok=True)
        if not parent.resolve(strict=True).is_relative_to(destination_root):
            raise SkiaToolError(f"archive extraction escaped its destination: {member.name!r}")
        try:
            if member.isdir():
                target.mkdir(exist_ok=True)
                os.chmod(target, 0o755)
                continue
            payload = source.extractfile(member)
            if payload is None:
                raise SkiaToolError(f"cannot read archive payload: {member.name!r}")
            written = 0
            with payload, target.open("xb") as output:
                for chunk in iter(lambda: payload.read(1024 * 1024), b""):
                    output.write(chunk)
                    written += len(chunk)
            if written != member.size:
                raise SkiaToolError(f"archive payload size changed while extracting: {member.name!r}")
            os.chmod(target, 0o644)
        except FileExistsError as error:
            raise SkiaToolError(f"archive contains conflicting paths: {member.name!r}") from error
        except OSError as error:
            raise SkiaToolError(f"cannot extract archive member {member.name!r}: {error}") from error
    return root_name


def copy_archive_once(source: Path, destination: Path) -> str:
    flags = os.O_RDONLY
    if hasattr(os, "O_BINARY"):
        flags |= os.O_BINARY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        source_fd = os.open(source, flags)
    except FileNotFoundError as error:
        raise SkiaToolError(f"artifact archive does not exist: {source}") from error
    except OSError as error:
        raise SkiaToolError(f"cannot open artifact archive {source}: {error}") from error
    source_metadata = os.fstat(source_fd)
    if not stat.S_ISREG(source_metadata.st_mode):
        os.close(source_fd)
        raise SkiaToolError(f"artifact archive is not a regular file: {source}")
    digest = hashlib.sha256()
    try:
        with os.fdopen(source_fd, "rb") as input_file, destination.open("xb") as output_file:
            for chunk in iter(lambda: input_file.read(1024 * 1024), b""):
                digest.update(chunk)
                output_file.write(chunk)
            output_file.flush()
            os.fsync(output_file.fileno())
    except OSError as error:
        raise SkiaToolError(f"cannot snapshot artifact archive {source}: {error}") from error
    return digest.hexdigest()


def verify_archive(
    archive: Path,
    expected_sha256: str,
    config: Mapping[str, Any],
    expected_profile: str | None,
    expected_target: str | None,
) -> dict[str, Any]:
    if not SHA256_RE.fullmatch(expected_sha256):
        raise SkiaToolError("archive SHA-256 must be 64 lowercase hexadecimal characters")
    with tempfile.TemporaryDirectory(prefix="fission-skia-verify-") as temporary:
        destination = Path(temporary)
        snapshot = destination / "input.tar.gz"
        actual_digest = copy_archive_once(archive, snapshot)
        if actual_digest != expected_sha256:
            raise SkiaToolError(
                f"archive digest mismatch: expected {expected_sha256}, found {actual_digest}"
            )
        try:
            with tarfile.open(snapshot, "r:gz") as source:
                root_name = extract_validated_archive(source, destination)
        except (tarfile.TarError, EOFError) as error:
            raise SkiaToolError(f"invalid artifact archive: {error}") from error
        return verify_artifact_directory(
            destination / root_name,
            config,
            expected_profile=expected_profile,
            expected_target=expected_target,
        )


def verify_command(args: argparse.Namespace, config: dict[str, Any]) -> None:
    if bool(args.artifact_dir) == bool(args.archive):
        raise SkiaToolError("verify requires exactly one of --artifact-dir or --archive")
    if args.artifact_dir:
        manifest = verify_artifact_directory(
            Path(args.artifact_dir).expanduser().resolve(),
            config,
            expected_profile=args.profile,
            expected_target=args.target,
            expected_manifest_sha256=args.manifest_sha256,
        )
    else:
        if not args.sha256:
            raise SkiaToolError("--sha256 is required when verifying an archive")
        manifest = verify_archive(
            Path(args.archive).expanduser().resolve(),
            args.sha256,
            config,
            args.profile,
            args.target,
        )
    print(
        f"verified {manifest['artifact_id']} "
        f"(profile={manifest['profile']}, target={manifest['target']}, qualified=false)"
    )


def show_plan(args: argparse.Namespace, config: dict[str, Any]) -> None:
    plan = resolve_build_plan(config, args.profile, args.target, parse_gn_overrides(args.gn_arg))
    print(canonical_json(plan), end="")


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--config", default=str(DEFAULT_CONFIG), help="pinned Skia JSON configuration")
    subcommands = result.add_subparsers(dest="command", required=True)

    plan = subcommands.add_parser("show-plan", help="print a resolved native build plan without building")
    plan.add_argument("--profile", required=True)
    plan.add_argument("--target", required=True)
    plan.add_argument("--gn-arg", action="append", default=[])
    plan.set_defaults(action=show_plan)

    build = subcommands.add_parser("build-native", help="build pinned upstream native Skia libraries")
    build.add_argument("--source-dir")
    build.add_argument("--build-dir")
    build.add_argument("--profile", required=True)
    build.add_argument("--target", required=True)
    build.add_argument("--toolchain-id", required=True)
    build.add_argument("--gn-arg", action="append", default=[])
    build.add_argument("--gn", help="explicit pinned GN executable")
    build.add_argument("--ninja", help="explicit pinned Ninja executable")
    build.add_argument("--gn-sha256", required=True, help="expected SHA-256 of the GN executable")
    build.add_argument(
        "--ninja-sha256",
        required=True,
        help="expected SHA-256 of the Ninja executable",
    )
    build.set_defaults(action=build_native)

    package = subcommands.add_parser("package-native", help="assemble a verified native artifact")
    package.add_argument("--profile", required=True)
    package.add_argument("--target", required=True)
    package.add_argument("--fission-version", required=True)
    package.add_argument("--build-metadata", required=True)
    package.add_argument("--bridge-header", required=True)
    package.add_argument("--library", action="append", default=[], help="NAME=PATH; repeat in any order")
    package.add_argument("--license", action="append", default=[], help="NAME=PATH; repeat as needed")
    package.add_argument("--link-metadata", required=True)
    package.add_argument("--deployment-metadata", required=True)
    package.add_argument("--output", required=True)
    package.add_argument("--archive")
    package.add_argument("--source-date-epoch")
    package.set_defaults(action=package_native)

    verify = subcommands.add_parser("verify", help="verify an installed artifact or archive")
    verify.add_argument("--artifact-dir")
    verify.add_argument("--archive")
    verify.add_argument("--sha256", help="required expected archive SHA-256")
    verify.add_argument("--manifest-sha256", help="optional expected installed manifest SHA-256")
    verify.add_argument("--profile")
    verify.add_argument("--target")
    verify.set_defaults(action=verify_command)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        config = load_config(Path(args.config).expanduser().resolve())
        args.action(args, config)
    except SkiaToolError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
