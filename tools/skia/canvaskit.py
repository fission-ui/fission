#!/usr/bin/env python3
"""Build, package, and verify Fission's pinned CanvasKit profiles.

The tool is intentionally offline. It consumes exact local Skia and emsdk
checkouts plus independently pinned tool digests; it never fetches or activates
either checkout and never emits an artifact claiming production qualification.
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path, PurePosixPath
import re
import stat
import sys
import tarfile
import tempfile
from typing import Any, Mapping, Sequence

import skia as foundation


DEFAULT_CONFIG = Path(__file__).resolve().parent / "config.json"
PRODUCTION_PROFILE = "canvaskit-production"
SOFTWARE_PROFILE = "canvaskit-software-qualification"
CANVASKIT_PROFILES = (PRODUCTION_PROFILE, SOFTWARE_PROFILE)
# Kept as the default for callers which imported the original foundation tool.
PROFILE = PRODUCTION_PROFILE
TARGET = "wasm32-unknown-unknown"
EMSDK_REPOSITORY = "https://skia.googlesource.com/external/github.com/emscripten-core/emsdk.git"
EMSDK_REVISION = "c69d433d8509c5c64564c2f0d054bf102a5cf67e"
EMSCRIPTEN_VERSION = "4.0.7"
EMSDK_RECEIPT = "FISSION_CANVASKIT_EMSDK_REVISION"
BUILD_PLAN = "fission-canvaskit-build-plan.json"
BUILD_METADATA = "fission-canvaskit-build.json"
BUILD_PLAN_SCHEMA_VERSION = 1
BUILD_RECEIPT_SCHEMA_VERSION = 1
WEB_MANIFEST_SCHEMA_VERSION = 1
WEB_PROTOCOL_RE = re.compile(
    r"(?m)^\s*export\s+const\s+PROTOCOL_VERSION\s*=\s*([0-9]+)\s*;\s*$"
)
MAX_ARTIFACT_FILES = 64
MAX_ARTIFACT_BYTES = 256 * 1024 * 1024
MAX_SINGLE_FILE_BYTES = 192 * 1024 * 1024
WASM_MEMORY_POLICY = "initial=128MiB; growth=enabled"
CANVASKIT_REQUIRED_LICENSES = [
    "brotli",
    "fission",
    "freetype",
    "harfbuzz",
    "icu",
    "libjpeg-turbo",
    "libpng",
    "libwebp",
    "skia",
    "wuffs",
    "zlib",
]
BRIDGE_ASSETS = {
    "fission_web_bridge": ("fission_skia_web.js", "fission-wire-bridge"),
    "fission_command_decoder": ("fission_skia_commands.js", "fission-command-decoder"),
    "fission_frame_executor": ("fission_skia_executor.js", "fission-frame-executor"),
    "fission_paragraph_wire": ("fission_skia_paragraph_wire.js", "fission-paragraph-wire"),
    "fission_paragraph_unicode": (
        "fission_skia_paragraph_unicode.js",
        "fission-paragraph-unicode",
    ),
    "fission_paragraph_host": ("fission_skia_paragraph.js", "fission-paragraph-host"),
}
BRIDGE_FILES = tuple(value[0] for value in BRIDGE_ASSETS.values())
REQUIRED_BRIDGE_IMPORTS = {
    "fission_skia_commands.js": ("./fission_skia_web.js",),
    "fission_skia_executor.js": (
        "./fission_skia_web.js",
        "./fission_skia_commands.js",
    ),
    "fission_skia_paragraph.js": (
        "./fission_skia_paragraph_wire.js",
        "./fission_skia_paragraph_unicode.js",
    ),
}


# This is the shared release branch of the exact pinned upstream compile.sh,
# made explicit so profile changes are reviewable inputs rather than shell
# substring switches. CanvasKit's BUILD.gn owns the corresponding linker flags,
# including WebGL 2, 128 MiB initial memory, and memory growth.
COMMON_GN_ARGS: dict[str, Any] = {
    "is_canvaskit": True,
    "is_component_build": False,
    "is_debug": False,
    "is_official_build": True,
    "is_trivial_abi": True,
    "skia_build_for_debugger": False,
    "skia_canvaskit_enable_alias_font": False,
    "skia_canvaskit_enable_bidi": False,
    "skia_canvaskit_enable_canvas_bindings": False,
    "skia_canvaskit_enable_debugger": False,
    "skia_canvaskit_enable_effects_deserialization": True,
    "skia_canvaskit_enable_embedded_font": False,
    "skia_canvaskit_enable_font": True,
    "skia_canvaskit_enable_matrix_helper": True,
    "skia_canvaskit_enable_paragraph": True,
    "skia_canvaskit_enable_pathops": True,
    "skia_canvaskit_enable_rt_shader": True,
    "skia_canvaskit_enable_skp_serialization": False,
    "skia_canvaskit_enable_webgl": False,
    "skia_canvaskit_enable_webgpu": False,
    "skia_canvaskit_force_tracing": False,
    "skia_canvaskit_include_viewer": False,
    "skia_canvaskit_legacy_draw_vertices_blend_mode": False,
    "skia_canvaskit_profile_build": False,
    "skia_enable_fontmgr_custom_directory": False,
    "skia_enable_fontmgr_custom_embedded": True,
    "skia_enable_fontmgr_custom_empty": True,
    "skia_enable_ganesh": False,
    "skia_enable_graphite": False,
    "skia_enable_pdf": False,
    "skia_enable_skottie": False,
    "skia_enable_skparagraph": True,
    "skia_enable_skshaper": True,
    "skia_enable_svg": False,
    "skia_enable_tools": False,
    "skia_use_angle": False,
    "skia_use_bidi": False,
    "skia_use_client_icu": False,
    "skia_use_dawn": False,
    "skia_use_dng_sdk": False,
    "skia_use_expat": False,
    "skia_use_fontconfig": False,
    "skia_use_freetype": True,
    "skia_use_freetype_woff2": True,
    "skia_use_harfbuzz": True,
    "skia_use_icu": True,
    "skia_use_icu4x": False,
    "skia_use_libgrapheme": False,
    "skia_use_libjpeg_turbo_decode": True,
    "skia_use_libjpeg_turbo_encode": True,
    "skia_use_libpng_decode": True,
    "skia_use_libpng_encode": True,
    "skia_use_libwebp_decode": True,
    "skia_use_libwebp_encode": True,
    "skia_use_lua": False,
    "skia_use_no_jpeg_encode": False,
    "skia_use_no_png_encode": False,
    "skia_use_no_webp_encode": False,
    "skia_use_piex": False,
    "skia_use_system_freetype2": False,
    "skia_use_system_harfbuzz": False,
    "skia_use_system_icu": False,
    "skia_use_system_libjpeg_turbo": False,
    "skia_use_system_libpng": False,
    "skia_use_system_libwebp": False,
    "skia_use_system_zlib": False,
    "skia_use_vulkan": False,
    "skia_use_webgl": False,
    "skia_use_webgpu": False,
    "skia_use_wuffs": True,
    "skia_use_zlib": True,
    "target_cpu": "wasm",
    "werror": True,
}


def profile_gn_args(profile: str) -> dict[str, Any]:
    result = dict(COMMON_GN_ARGS)
    if profile == PRODUCTION_PROFILE:
        result.update(
            {
                "skia_canvaskit_enable_webgl": True,
                "skia_enable_ganesh": True,
                "skia_use_webgl": True,
            }
        )
    elif profile != SOFTWARE_PROFILE:
        raise foundation.SkiaToolError(f"unsupported CanvasKit profile: {profile!r}")
    return result


def profile_lane(profile: str) -> str:
    if profile == PRODUCTION_PROFILE:
        return "webgl-ganesh"
    if profile == SOFTWARE_PROFILE:
        return "software-raster"
    raise foundation.SkiaToolError(f"unsupported CanvasKit profile: {profile!r}")


def browser_contract(profile: str, platform: str) -> dict[str, Any]:
    if profile == PRODUCTION_PROFILE:
        graphics_backend = "Ganesh"
        gpu_api = "WebGL 2"
        software_only = False
    elif profile == SOFTWARE_PROFILE:
        graphics_backend = "Skia Raster"
        gpu_api = "none"
        software_only = True
    else:
        raise foundation.SkiaToolError(f"unsupported CanvasKit profile: {profile!r}")
    return {
        "graphics_backend": graphics_backend,
        "gpu_api": gpu_api,
        "software_fallback": True,
        "software_only": software_only,
        "wasm_memory_policy": WASM_MEMORY_POLICY,
        "platform": platform,
    }


def deployment_contract(profile: str) -> dict[str, str]:
    browser_api = "WebGL 2" if profile == PRODUCTION_PROFILE else "CanvasKit software surface"
    return {
        "emscripten": EMSCRIPTEN_VERSION,
        "browser_api": browser_api,
        "wasm_memory_policy": WASM_MEMORY_POLICY,
    }


def expected_layout() -> list[str]:
    return [
        "manifest.json",
        "web/canvaskit.js",
        "web/canvaskit.wasm",
        *(f"web/{name}" for name in BRIDGE_FILES),
        "licenses/",
    ]


def load_config(path: Path) -> dict[str, Any]:
    config = foundation.load_config(path)
    target = foundation.select_target(config, TARGET)
    if target.get("kind") != "canvaskit":
        raise foundation.SkiaToolError(f"{TARGET!r} is not a CanvasKit target")
    common_features = {
        "raster_fallback": True,
        "paragraph": True,
        "unicode": "icu",
        "svg": "backend-neutral-lowering-until-svgdom-is-proven",
        "pdf": False,
        "codecs": ["jpeg", "png", "webp"],
    }
    for name in CANVASKIT_PROFILES:
        profile = foundation.select_profile(config, name)
        if profile.get("kind") != "canvaskit":
            raise foundation.SkiaToolError(f"{name!r} is not a CanvasKit profile")
        if profile.get("qualified") is not False:
            raise foundation.SkiaToolError("CanvasKit foundation profiles must remain unqualified")
        if profile.get("build_recipe") != "available":
            raise foundation.SkiaToolError(f"{name!r} does not have an available build recipe")
        if profile.get("layout") != expected_layout():
            raise foundation.SkiaToolError(
                f"{name!r} layout does not match the Fission Web runtime contract"
            )
        required = foundation.require_string_list(
            profile.get("required_licenses"),
            f"profiles.{name}.required_licenses",
        )
        if required != CANVASKIT_REQUIRED_LICENSES:
            raise foundation.SkiaToolError(
                f"{name!r} notices do not match its pinned dependency profile"
            )
        expected_features = {
            "gpu": "webgl" if name == PRODUCTION_PROFILE else "none",
            **common_features,
        }
        if profile.get("features") != expected_features:
            raise foundation.SkiaToolError(f"{name!r} features do not match its pinned lane")
    return config


def web_recipe(
    config: Mapping[str, Any],
    profile_name: str = PRODUCTION_PROFILE,
) -> dict[str, Any]:
    profile = foundation.select_profile(config, profile_name)
    target = foundation.select_target(config, TARGET)
    return {
        "schema_version": BUILD_PLAN_SCHEMA_VERSION,
        "profile": profile_name,
        "target": TARGET,
        "lane": profile_lane(profile_name),
        "qualified": False,
        "skia_revision": config["source"]["revision"],
        "bridge_abi_version": config["bridge"]["abi_version"],
        "emsdk_revision": EMSDK_REVISION,
        "emscripten_version": EMSCRIPTEN_VERSION,
        "gn_args": dict(sorted(profile_gn_args(profile_name).items())),
        "ninja_targets": ["canvaskit.js"],
        "outputs": ["canvaskit.js", "canvaskit.wasm"],
        "features": profile["features"],
        "browser": browser_contract(profile_name, target["platform"]),
    }


def validate_recipe(
    raw: Any,
    config: Mapping[str, Any],
    expected_profile: str | None = None,
) -> dict[str, Any]:
    recipe = foundation.require_object(raw, "CanvasKit build receipt recipe")
    profile = recipe.get("profile")
    if profile not in CANVASKIT_PROFILES:
        raise foundation.SkiaToolError("CanvasKit build receipt has an unsupported profile")
    if expected_profile is not None and profile != expected_profile:
        raise foundation.SkiaToolError(
            f"CanvasKit build receipt profile mismatch: expected {expected_profile!r}"
        )
    expected = web_recipe(config, profile)
    if recipe != expected:
        raise foundation.SkiaToolError(
            f"CanvasKit build receipt recipe does not match pinned profile {profile!r}"
        )
    return recipe


def normalized_identifier(value: Any, context: str) -> str:
    text = foundation.require_string(value, context)
    return foundation.normalized_identity_text(text, context)


def verify_checkout(
    root: Path,
    *,
    repository: str,
    revision: str,
    receipt_name: str,
    description: str,
) -> dict[str, Any]:
    if not root.is_dir():
        raise foundation.SkiaToolError(f"{description} is not a directory: {root}")
    if (root / ".git").exists():
        actual = foundation.run_checked(["git", "-C", str(root), "rev-parse", "HEAD"])
        if actual != revision:
            raise foundation.SkiaToolError(
                f"{description} revision mismatch: expected {revision}, found {actual}"
            )
        dirty = foundation.run_checked(
            ["git", "-C", str(root), "status", "--porcelain", "--untracked-files=no"]
        )
        if dirty:
            raise foundation.SkiaToolError(f"{description} contains tracked modifications")
        kind = "local-git-checkout"
    else:
        receipt = root / receipt_name
        if not receipt.is_file():
            raise foundation.SkiaToolError(
                f"vendored {description} must contain {receipt_name}"
            )
        actual = receipt.read_text(encoding="utf-8").strip()
        if actual != revision:
            raise foundation.SkiaToolError(
                f"{description} revision mismatch: expected {revision}, found {actual or '<empty>'}"
            )
        kind = "local-vendored-source"
    return {
        "kind": kind,
        "qualified": False,
        "repository": repository,
        "revision": revision,
    }


def emsdk_tool_paths(root: Path) -> dict[str, Path]:
    suffix = ".bat" if os.name == "nt" else ""
    tools = root / "upstream" / "emscripten"
    return {
        "emcc": tools / f"emcc{suffix}",
        "emxx": tools / f"em++{suffix}",
        "emar": tools / f"emar{suffix}",
    }


def verified_tool(
    path: Path,
    expected_sha256: str,
    name: str,
    *,
    require_emscripten_version: bool = False,
) -> dict[str, str]:
    actual = foundation.verify_tool_digest(path, expected_sha256, name)
    identity = foundation.capture_tool_identity(path, expected_sha256, actual)
    if require_emscripten_version and EMSCRIPTEN_VERSION not in identity["version"]:
        raise foundation.SkiaToolError(
            f"{name} version does not identify pinned Emscripten {EMSCRIPTEN_VERSION}"
        )
    return identity


def tool_identities(
    source_dir: Path,
    emsdk_dir: Path,
    args: argparse.Namespace,
) -> tuple[dict[str, Path], dict[str, dict[str, str]]]:
    gn = Path(args.gn).expanduser().resolve() if args.gn else source_dir / "bin" / "gn"
    ninja = (
        Path(args.ninja).expanduser().resolve()
        if args.ninja
        else source_dir / "third_party" / "ninja" / ("ninja.exe" if os.name == "nt" else "ninja")
    )
    paths = {"gn": gn, "ninja": ninja, **emsdk_tool_paths(emsdk_dir)}
    identities = {
        "gn": verified_tool(gn, args.gn_sha256, "GN"),
        "ninja": verified_tool(ninja, args.ninja_sha256, "Ninja"),
        "emcc": verified_tool(
            paths["emcc"],
            args.emcc_sha256,
            "emcc",
            require_emscripten_version=True,
        ),
        "emxx": verified_tool(
            paths["emxx"],
            args.emxx_sha256,
            "em++",
            require_emscripten_version=True,
        ),
        "emar": verified_tool(paths["emar"], args.emar_sha256, "emar"),
    }
    return paths, identities


def source_identity(config: Mapping[str, Any], source_dir: Path) -> dict[str, Any]:
    return verify_checkout(
        source_dir,
        repository=config["source"]["repository"],
        revision=config["source"]["revision"],
        receipt_name=foundation.SOURCE_RECEIPT,
        description="Skia source directory",
    )


def emsdk_identity(emsdk_dir: Path) -> dict[str, Any]:
    identity = verify_checkout(
        emsdk_dir,
        repository=EMSDK_REPOSITORY,
        revision=EMSDK_REVISION,
        receipt_name=EMSDK_RECEIPT,
        description="emsdk directory",
    )
    identity["emscripten_version"] = EMSCRIPTEN_VERSION
    return identity


def build_plan(
    config: Mapping[str, Any],
    source: Mapping[str, Any],
    emsdk: Mapping[str, Any],
    toolchain_id: str,
    tools: Mapping[str, Any],
    profile: str = PRODUCTION_PROFILE,
) -> dict[str, Any]:
    return {
        "schema_version": BUILD_PLAN_SCHEMA_VERSION,
        "recipe": web_recipe(config, profile),
        "source": dict(source),
        "emsdk": dict(emsdk),
        "toolchain_id": normalized_identifier(toolchain_id, "--toolchain-id"),
        "tools": dict(tools),
    }


def validate_source_identity(raw: Any, config: Mapping[str, Any]) -> dict[str, Any]:
    identity = foundation.require_object(raw, "CanvasKit build receipt source")
    expected = {
        "repository": config["source"]["repository"],
        "revision": config["source"]["revision"],
    }
    if set(identity) != {"kind", "qualified", *expected}:
        raise foundation.SkiaToolError("CanvasKit source identity has unknown or missing fields")
    if identity.get("kind") not in {"local-git-checkout", "local-vendored-source"}:
        raise foundation.SkiaToolError("CanvasKit source identity has an unsupported kind")
    if identity.get("qualified") is not False:
        raise foundation.SkiaToolError("local CanvasKit source cannot claim qualification")
    if any(identity.get(key) != value for key, value in expected.items()):
        raise foundation.SkiaToolError("CanvasKit source identity does not match the pin")
    return identity


def validate_emsdk_identity(raw: Any) -> dict[str, Any]:
    identity = foundation.require_object(raw, "CanvasKit build receipt emsdk")
    expected = {
        "repository": EMSDK_REPOSITORY,
        "revision": EMSDK_REVISION,
        "emscripten_version": EMSCRIPTEN_VERSION,
    }
    if set(identity) != {"kind", "qualified", *expected}:
        raise foundation.SkiaToolError("emsdk identity has unknown or missing fields")
    if identity.get("kind") not in {"local-git-checkout", "local-vendored-source"}:
        raise foundation.SkiaToolError("emsdk identity has an unsupported kind")
    if identity.get("qualified") is not False:
        raise foundation.SkiaToolError("local emsdk cannot claim qualification")
    if any(identity.get(key) != value for key, value in expected.items()):
        raise foundation.SkiaToolError("emsdk identity does not match the pin")
    return identity


def digest_record(path: Path, name: str, relative: str) -> dict[str, Any]:
    return foundation.regular_file_record(path, name=name, relative_path=relative)


def validate_build_receipt(
    raw: Any,
    config: Mapping[str, Any],
    expected_profile: str | None = None,
) -> dict[str, Any]:
    receipt = foundation.require_object(raw, "CanvasKit build receipt")
    if set(receipt) != {"schema_version", "result", "plan", "plan_sha256", "outputs"}:
        raise foundation.SkiaToolError("CanvasKit build receipt has unknown or missing fields")
    if receipt.get("schema_version") != BUILD_RECEIPT_SCHEMA_VERSION:
        raise foundation.SkiaToolError("unsupported CanvasKit build receipt schema")
    if receipt.get("result") != "complete":
        raise foundation.SkiaToolError("CanvasKit build receipt is not complete")
    plan = foundation.require_object(receipt.get("plan"), "CanvasKit build receipt plan")
    if set(plan) != {"schema_version", "recipe", "source", "emsdk", "toolchain_id", "tools"}:
        raise foundation.SkiaToolError("CanvasKit build plan has unknown or missing fields")
    if plan.get("schema_version") != BUILD_PLAN_SCHEMA_VERSION:
        raise foundation.SkiaToolError("unsupported CanvasKit build plan schema")
    validate_recipe(plan.get("recipe"), config, expected_profile)
    validate_source_identity(plan.get("source"), config)
    validate_emsdk_identity(plan.get("emsdk"))
    normalized_identifier(plan.get("toolchain_id"), "CanvasKit build plan toolchain_id")
    tools = foundation.require_object(plan.get("tools"), "CanvasKit build plan tools")
    expected_tools = {"gn", "ninja", "emcc", "emxx", "emar"}
    if set(tools) != expected_tools:
        raise foundation.SkiaToolError("CanvasKit build plan has an incomplete tool identity set")
    for name in sorted(expected_tools):
        identity = foundation.validate_tool_identity(tools.get(name), f"build plan tools.{name}")
        if name in {"emcc", "emxx"} and EMSCRIPTEN_VERSION not in identity["version"]:
            raise foundation.SkiaToolError(
                f"build plan tool {name} does not identify Emscripten {EMSCRIPTEN_VERSION}"
            )
    if receipt.get("plan_sha256") != foundation.sha256_json(plan):
        raise foundation.SkiaToolError("CanvasKit build plan digest is invalid")
    outputs = receipt.get("outputs")
    if not isinstance(outputs, list) or len(outputs) != 2:
        raise foundation.SkiaToolError("CanvasKit build receipt must contain exactly two outputs")
    expected = [
        ("canvaskit-js", "canvaskit.js"),
        ("canvaskit-wasm", "canvaskit.wasm"),
    ]
    validated = [
        foundation.validate_digest_record(value, f"CanvasKit build outputs[{index}]")
        for index, value in enumerate(outputs)
    ]
    if [(entry["name"], entry["path"]) for entry in validated] != expected:
        raise foundation.SkiaToolError("CanvasKit build outputs have the wrong names or order")
    return receipt


def build_canvaskit(args: argparse.Namespace, config: dict[str, Any]) -> None:
    source_dir = foundation.resolve_explicit_path(
        args.source_dir,
        "FISSION_SKIA_SOURCE_DIR",
        "pinned Skia source directory",
        must_exist=True,
    )
    emsdk_dir = foundation.resolve_explicit_path(
        args.emsdk_dir,
        "FISSION_CANVASKIT_EMSDK_DIR",
        "pinned activated emsdk directory",
        must_exist=True,
    )
    build_dir = foundation.resolve_explicit_path(
        args.build_dir,
        "FISSION_CANVASKIT_BUILD_DIR",
        "CanvasKit GN output directory",
        must_exist=False,
    )
    if build_dir in {source_dir, emsdk_dir} or build_dir in source_dir.parents or build_dir in emsdk_dir.parents:
        raise foundation.SkiaToolError("build output cannot be a source/toolchain directory or its parent")
    if not (source_dir / config["source"]["license_file"]).is_file():
        raise foundation.SkiaToolError("pinned Skia source is missing its licence")
    source = source_identity(config, source_dir)
    emsdk = emsdk_identity(emsdk_dir)
    paths, identities = tool_identities(source_dir, emsdk_dir, args)
    plan = build_plan(config, source, emsdk, args.toolchain_id, identities, args.profile)
    foundation.check_reusable_build_dir(build_dir, plan, BUILD_PLAN)
    foundation.write_json(build_dir / BUILD_PLAN, plan)

    rendered = profile_gn_args(args.profile)
    rendered["skia_emsdk_dir"] = str(emsdk_dir)
    rendered_args = " ".join(
        f"{name}={foundation.gn_literal(value)}" for name, value in sorted(rendered.items())
    )
    foundation.verify_tool_digest(paths["gn"], args.gn_sha256, "GN")
    foundation.run_checked(
        [
            str(paths["gn"]),
            "gen",
            str(build_dir),
            f"--args={rendered_args}",
            "--fail-on-unused-args",
        ],
        cwd=source_dir,
    )
    for name, digest, label in (
        ("ninja", args.ninja_sha256, "Ninja"),
        ("emcc", args.emcc_sha256, "emcc"),
        ("emxx", args.emxx_sha256, "em++"),
        ("emar", args.emar_sha256, "emar"),
    ):
        foundation.verify_tool_digest(paths[name], digest, label)
    foundation.run_checked(
        [str(paths["ninja"]), "-C", str(build_dir), "canvaskit.js"],
        cwd=source_dir,
    )
    outputs = [
        digest_record(build_dir / "canvaskit.js", "canvaskit-js", "canvaskit.js"),
        digest_record(build_dir / "canvaskit.wasm", "canvaskit-wasm", "canvaskit.wasm"),
    ]
    validate_wasm(build_dir / "canvaskit.wasm")
    receipt = {
        "schema_version": BUILD_RECEIPT_SCHEMA_VERSION,
        "result": "complete",
        "plan": plan,
        "plan_sha256": foundation.sha256_json(plan),
        "outputs": outputs,
    }
    foundation.write_json(build_dir / BUILD_METADATA, receipt)
    print(build_dir / BUILD_METADATA)


def protocol_version(path: Path) -> int:
    try:
        raw = path.read_text(encoding="utf-8")
    except (FileNotFoundError, UnicodeDecodeError) as error:
        raise foundation.SkiaToolError(f"Web bridge must be a UTF-8 regular file: {path}") from error
    matches = WEB_PROTOCOL_RE.findall(raw)
    if len(matches) != 1:
        raise foundation.SkiaToolError("Web bridge must export PROTOCOL_VERSION exactly once")
    version = int(matches[0])
    if version <= 0:
        raise foundation.SkiaToolError("Web bridge protocol version must be positive")
    return version


def validate_javascript(path: Path, description: str) -> None:
    try:
        data = path.read_bytes()
        data.decode("utf-8")
    except (FileNotFoundError, UnicodeDecodeError) as error:
        raise foundation.SkiaToolError(f"{description} must be a UTF-8 regular file") from error
    if not data or b"\0" in data:
        raise foundation.SkiaToolError(f"{description} is empty or contains a NUL byte")


def validate_wasm(path: Path) -> None:
    try:
        with path.open("rb") as source:
            header = source.read(8)
    except FileNotFoundError as error:
        raise foundation.SkiaToolError(f"CanvasKit Wasm does not exist: {path}") from error
    if header != b"\0asm\x01\0\0\0":
        raise foundation.SkiaToolError("CanvasKit Wasm has an invalid magic or module version")


def regular_input(path: str, description: str) -> Path:
    result = Path(path).expanduser().absolute()
    try:
        metadata = result.stat(follow_symlinks=False)
    except FileNotFoundError as error:
        raise foundation.SkiaToolError(f"{description} does not exist: {result}") from error
    if not stat.S_ISREG(metadata.st_mode):
        raise foundation.SkiaToolError(f"{description} must be a regular file, not a link: {result}")
    return result


def bridge_inputs(root_value: str) -> dict[str, Path]:
    root = Path(root_value).expanduser().absolute()
    try:
        metadata = root.stat(follow_symlinks=False)
    except FileNotFoundError as error:
        raise foundation.SkiaToolError(f"Fission bridge directory does not exist: {root}") from error
    if not stat.S_ISDIR(metadata.st_mode):
        raise foundation.SkiaToolError(
            f"Fission bridge directory must be a directory, not a link: {root}"
        )
    result: dict[str, Path] = {}
    contents: dict[str, str] = {}
    for name in BRIDGE_FILES:
        path = regular_input(str(root / name), f"Fission bridge module {name}")
        validate_javascript(path, f"Fission bridge module {name}")
        result[name] = path
        contents[name] = path.read_text(encoding="utf-8")
    for name, imports in REQUIRED_BRIDGE_IMPORTS.items():
        for imported in imports:
            if imported not in contents[name]:
                raise foundation.SkiaToolError(
                    f"Fission bridge module {name} does not import required module {imported}"
                )
    return result


def load_deployment(path: Path, receipt: Mapping[str, Any]) -> dict[str, Any]:
    metadata = foundation.require_object(foundation.load_json(path), str(path))
    if set(metadata) != {"toolchain", "deployment"}:
        raise foundation.SkiaToolError("Web deployment metadata has unknown or missing fields")
    toolchain = foundation.require_object(metadata["toolchain"], "Web deployment toolchain")
    if set(toolchain) != {"id", "compiler", "runtime_abi"}:
        raise foundation.SkiaToolError("Web deployment toolchain has unknown or missing fields")
    plan = receipt["plan"]
    expected_toolchain = {
        "id": plan["toolchain_id"],
        "compiler": plan["tools"]["emcc"]["version"],
        "runtime_abi": f"Emscripten {EMSCRIPTEN_VERSION} / wasm32",
    }
    if toolchain != expected_toolchain:
        raise foundation.SkiaToolError("Web deployment toolchain does not match the build receipt")
    deployment = foundation.require_object(metadata["deployment"], "Web deployment")
    expected_deployment = deployment_contract(receipt["plan"]["recipe"]["profile"])
    if deployment != expected_deployment:
        raise foundation.SkiaToolError("Web deployment does not match the CanvasKit profile")
    return {"toolchain": dict(toolchain), "deployment": dict(deployment)}


def compare_build_output(record: Mapping[str, Any], path: Path, description: str) -> None:
    actual = digest_record(path, record["name"], record["path"])
    if actual != record:
        raise foundation.SkiaToolError(f"{description} does not match the completed build receipt")


def asset_record(path: Path, relative: str, role: str, media_type: str) -> dict[str, Any]:
    payload = foundation.payload_record(path, relative)
    return {**payload, "role": role, "media_type": media_type}


def artifact_id(fission_version: str, profile: str, abi: int, protocol: int) -> str:
    version = normalized_identifier(fission_version, "--fission-version")
    if profile not in CANVASKIT_PROFILES:
        raise foundation.SkiaToolError(f"unsupported CanvasKit profile: {profile!r}")
    result = f"fission-canvaskit-{version}-{TARGET}-{profile}-abi{abi}-wire{protocol}"
    if not foundation.NAME_RE.fullmatch(result):
        raise foundation.SkiaToolError("Fission version produces an unsafe artifact identity")
    return result


def package_canvaskit(args: argparse.Namespace, config: dict[str, Any]) -> None:
    if args.profile not in CANVASKIT_PROFILES or args.target != TARGET:
        raise foundation.SkiaToolError(
            f"unsupported CanvasKit profile/target: {args.profile}/{args.target}"
        )
    build_path = regular_input(args.build_metadata, "CanvasKit build metadata")
    receipt = validate_build_receipt(
        foundation.load_json(build_path),
        config,
        expected_profile=args.profile,
    )
    canvaskit_js = regular_input(args.canvaskit_js, "CanvasKit JavaScript")
    canvaskit_wasm = regular_input(args.canvaskit_wasm, "CanvasKit Wasm")
    bridges = bridge_inputs(args.bridge_dir)
    validate_javascript(canvaskit_js, "CanvasKit JavaScript")
    validate_wasm(canvaskit_wasm)
    protocol = protocol_version(bridges["fission_skia_web.js"])
    compare_build_output(receipt["outputs"][0], canvaskit_js, "CanvasKit JavaScript")
    compare_build_output(receipt["outputs"][1], canvaskit_wasm, "CanvasKit Wasm")
    deployment = load_deployment(
        regular_input(args.deployment_metadata, "Web deployment metadata"),
        receipt,
    )
    licences = foundation.parse_named_paths(args.license, "licence")
    profile = foundation.select_profile(config, args.profile)
    expected_licences = set(
        foundation.require_string_list(profile.get("required_licenses"), "profile licences")
    )
    if set(licences) != expected_licences:
        raise foundation.SkiaToolError(
            "CanvasKit licence set does not match the profile; "
            f"missing={sorted(expected_licences - set(licences))}, "
            f"extra={sorted(set(licences) - expected_licences)}"
        )
    output = Path(args.output).expanduser().resolve()
    archive = Path(args.archive).expanduser().resolve() if args.archive else None
    if archive is not None and (archive == output or output in archive.parents):
        raise foundation.SkiaToolError("artifact archive must be outside the artifact directory")
    foundation.assert_empty_output(output)
    foundation.copy_file(canvaskit_js, output / "web" / "canvaskit.js")
    foundation.copy_file(canvaskit_wasm, output / "web" / "canvaskit.wasm")
    for name, source in bridges.items():
        foundation.copy_file(source, output / "web" / name)
    for name, source in sorted(licences.items()):
        foundation.copy_file(source, output / "licenses" / f"{name}.txt")

    identity = artifact_id(
        args.fission_version,
        args.profile,
        config["bridge"]["abi_version"],
        protocol,
    )
    assets = {
        "canvaskit_js": asset_record(
            output / "web" / "canvaskit.js",
            "web/canvaskit.js",
            "canvaskit-loader",
            "text/javascript",
        ),
        "canvaskit_wasm": asset_record(
            output / "web" / "canvaskit.wasm",
            "web/canvaskit.wasm",
            "canvaskit-module",
            "application/wasm",
        ),
    }
    for asset_name, (filename, role) in BRIDGE_ASSETS.items():
        assets[asset_name] = asset_record(
            output / "web" / filename,
            f"web/{filename}",
            role,
            "text/javascript",
        )
    manifest = {
        "schema_version": WEB_MANIFEST_SCHEMA_VERSION,
        "artifact_id": identity,
        "fission_version": normalized_identifier(args.fission_version, "--fission-version"),
        "origin": "local-build",
        "qualified": False,
        "profile": args.profile,
        "target": TARGET,
        "platform": "Web",
        "lane": profile_lane(args.profile),
        "source": {
            "repository": config["source"]["repository"],
            "revision": config["source"]["revision"],
        },
        "emsdk": {
            "repository": EMSDK_REPOSITORY,
            "revision": EMSDK_REVISION,
            "emscripten_version": EMSCRIPTEN_VERSION,
        },
        "abi": {
            "bridge_abi_version": config["bridge"]["abi_version"],
            "web_protocol_version": protocol,
        },
        "features": profile["features"],
        "browser": web_recipe(config, args.profile)["browser"],
        "toolchain": deployment["toolchain"],
        "deployment": deployment["deployment"],
        "assets": assets,
        "build_receipt": receipt,
        "files": foundation.listed_files(output),
    }
    foundation.write_json(output / foundation.MANIFEST, manifest)
    verify_artifact_directory(
        output,
        config,
        expected_profile=args.profile,
        expected_target=TARGET,
    )
    print(output / foundation.MANIFEST)
    if archive is not None:
        epoch_raw = args.source_date_epoch or os.environ.get("SOURCE_DATE_EPOCH")
        if epoch_raw is None or not re.fullmatch(r"[0-9]+", epoch_raw):
            raise foundation.SkiaToolError(
                "--source-date-epoch or numeric SOURCE_DATE_EPOCH is required for archives"
            )
        digest = foundation.create_archive_with_sidecar(
            output,
            archive,
            identity,
            int(epoch_raw),
        )
        print(f"{digest}  {archive}")


def validate_declared_files(
    root: Path,
    raw: Any,
    expected_paths: set[str],
) -> dict[str, dict[str, Any]]:
    if not isinstance(raw, list) or not raw:
        raise foundation.SkiaToolError("manifest.files must be a non-empty array")
    declared: dict[str, dict[str, Any]] = {}
    for index, value in enumerate(raw):
        record = foundation.require_object(value, f"manifest.files[{index}]")
        if set(record) != {"path", "sha256", "size"}:
            raise foundation.SkiaToolError(f"manifest.files[{index}] has unknown or missing fields")
        relative = foundation.validate_relative_path(
            record.get("path"),
            f"manifest.files[{index}].path",
        ).as_posix()
        digest = record.get("sha256")
        size = record.get("size")
        if relative in declared:
            raise foundation.SkiaToolError(f"artifact file is declared twice: {relative}")
        if not isinstance(digest, str) or not foundation.SHA256_RE.fullmatch(digest):
            raise foundation.SkiaToolError(f"invalid artifact digest for {relative}")
        if not isinstance(size, int) or isinstance(size, bool) or size < 0:
            raise foundation.SkiaToolError(f"invalid artifact size for {relative}")
        path = root.joinpath(*PurePosixPath(relative).parts)
        actual = foundation.payload_record(path, relative)
        if actual != record:
            raise foundation.SkiaToolError(f"artifact payload does not match manifest: {relative}")
        declared[relative] = record
    if set(declared) != expected_paths:
        raise foundation.SkiaToolError(
            "artifact payload set does not match the CanvasKit profile; "
            f"missing={sorted(expected_paths - set(declared))}, "
            f"extra={sorted(set(declared) - expected_paths)}"
        )
    return declared


def validate_binding(
    raw: Any,
    context: str,
    expected_path: str,
    declared: Mapping[str, Mapping[str, Any]],
    *,
    extra: Mapping[str, str] | None = None,
) -> dict[str, Any]:
    binding = foundation.require_object(raw, context)
    extras = dict(extra or {})
    if set(binding) != {"path", "sha256", "size", *extras}:
        raise foundation.SkiaToolError(f"{context} has unknown or missing fields")
    payload = {key: binding[key] for key in ("path", "sha256", "size")}
    foundation.validate_payload_binding(payload, context, expected_path, declared)
    for key, value in extras.items():
        if binding.get(key) != value:
            raise foundation.SkiaToolError(f"{context}.{key} does not match the profile")
    return binding


def validate_artifact_tree(root: Path) -> set[str]:
    entries = foundation.artifact_tree_entries(root)
    if len(entries) > MAX_ARTIFACT_FILES + 3:
        raise foundation.SkiaToolError("CanvasKit artifact contains too many entries")
    directories: set[str] = set()
    files: set[str] = set()
    total = 0
    for path in entries:
        relative = path.relative_to(root).as_posix()
        if path.is_dir():
            directories.add(relative)
            continue
        size = path.stat(follow_symlinks=False).st_size
        if size > MAX_SINGLE_FILE_BYTES:
            raise foundation.SkiaToolError(f"CanvasKit artifact file is too large: {relative}")
        total += size
        files.add(relative)
    if total > MAX_ARTIFACT_BYTES:
        raise foundation.SkiaToolError("CanvasKit artifact exceeds the unpacked size limit")
    if directories != {"licenses", "web"}:
        raise foundation.SkiaToolError("CanvasKit artifact has unexpected or missing directories")
    return files


def verify_artifact_directory(
    root: Path,
    config: Mapping[str, Any],
    *,
    expected_profile: str | None = None,
    expected_target: str | None = None,
    expected_manifest_sha256: str | None = None,
) -> dict[str, Any]:
    files = validate_artifact_tree(root)
    manifest_path = root / foundation.MANIFEST
    if expected_manifest_sha256 is not None:
        if not foundation.SHA256_RE.fullmatch(expected_manifest_sha256):
            raise foundation.SkiaToolError("expected manifest SHA-256 is invalid")
        actual = foundation.sha256_file(manifest_path)
        if actual != expected_manifest_sha256:
            raise foundation.SkiaToolError(
                f"manifest digest mismatch: expected {expected_manifest_sha256}, found {actual}"
            )
    manifest = foundation.require_object(foundation.load_json(manifest_path), str(manifest_path))
    expected_fields = {
        "schema_version",
        "artifact_id",
        "fission_version",
        "origin",
        "qualified",
        "profile",
        "target",
        "platform",
        "lane",
        "source",
        "emsdk",
        "abi",
        "features",
        "browser",
        "toolchain",
        "deployment",
        "assets",
        "build_receipt",
        "files",
    }
    if set(manifest) != expected_fields:
        raise foundation.SkiaToolError("CanvasKit manifest has unknown or missing fields")
    if manifest.get("schema_version") != WEB_MANIFEST_SCHEMA_VERSION:
        raise foundation.SkiaToolError("unsupported CanvasKit manifest schema")
    if manifest.get("origin") != "local-build" or manifest.get("qualified") is not False:
        raise foundation.SkiaToolError("foundation tooling accepts only local, unqualified artifacts")
    profile_name = manifest.get("profile")
    if profile_name not in CANVASKIT_PROFILES:
        raise foundation.SkiaToolError("CanvasKit artifact has an unsupported profile")
    if expected_profile is not None and profile_name != expected_profile:
        raise foundation.SkiaToolError("CanvasKit artifact profile mismatch")
    if manifest.get("target") != TARGET or (expected_target and expected_target != TARGET):
        raise foundation.SkiaToolError("CanvasKit artifact target mismatch")
    if manifest.get("platform") != "Web" or manifest.get("lane") != profile_lane(profile_name):
        raise foundation.SkiaToolError("CanvasKit artifact browser lane mismatch")
    source = foundation.require_object(manifest.get("source"), "manifest.source")
    if source != {
        "repository": config["source"]["repository"],
        "revision": config["source"]["revision"],
    }:
        raise foundation.SkiaToolError("CanvasKit artifact source does not match the pin")
    if manifest.get("emsdk") != {
        "repository": EMSDK_REPOSITORY,
        "revision": EMSDK_REVISION,
        "emscripten_version": EMSCRIPTEN_VERSION,
    }:
        raise foundation.SkiaToolError("CanvasKit artifact emsdk does not match the pin")
    abi = foundation.require_object(manifest.get("abi"), "manifest.abi")
    if set(abi) != {"bridge_abi_version", "web_protocol_version"}:
        raise foundation.SkiaToolError("CanvasKit artifact ABI has unknown or missing fields")
    if abi.get("bridge_abi_version") != config["bridge"]["abi_version"]:
        raise foundation.SkiaToolError("CanvasKit artifact bridge ABI does not match Fission")
    protocol = abi.get("web_protocol_version")
    if not isinstance(protocol, int) or isinstance(protocol, bool) or protocol <= 0:
        raise foundation.SkiaToolError("CanvasKit artifact protocol version is invalid")
    expected_id = artifact_id(
        manifest.get("fission_version"),
        profile_name,
        config["bridge"]["abi_version"],
        protocol,
    )
    if manifest.get("artifact_id") != expected_id:
        raise foundation.SkiaToolError("CanvasKit artifact identity is inconsistent")
    profile = foundation.select_profile(config, profile_name)
    if manifest.get("features") != profile["features"]:
        raise foundation.SkiaToolError("CanvasKit artifact features do not match the profile")
    if manifest.get("browser") != web_recipe(config, profile_name)["browser"]:
        raise foundation.SkiaToolError("CanvasKit browser contract does not match the profile")

    required_licences = set(
        foundation.require_string_list(profile.get("required_licenses"), "profile licences")
    )
    payload_paths = {
        "web/canvaskit.js",
        "web/canvaskit.wasm",
        *(f"web/{name}" for name in BRIDGE_FILES),
        *(f"licenses/{name}.txt" for name in required_licences),
    }
    if files != {foundation.MANIFEST, *payload_paths}:
        raise foundation.SkiaToolError("CanvasKit artifact contains undeclared files")
    declared = validate_declared_files(root, manifest.get("files"), payload_paths)
    assets = foundation.require_object(manifest.get("assets"), "manifest.assets")
    expected_assets = {"canvaskit_js", "canvaskit_wasm", *BRIDGE_ASSETS}
    if set(assets) != expected_assets:
        raise foundation.SkiaToolError("CanvasKit asset map has unknown or missing assets")
    validate_binding(
        assets["canvaskit_js"],
        "manifest.assets.canvaskit_js",
        "web/canvaskit.js",
        declared,
        extra={"role": "canvaskit-loader", "media_type": "text/javascript"},
    )
    validate_binding(
        assets["canvaskit_wasm"],
        "manifest.assets.canvaskit_wasm",
        "web/canvaskit.wasm",
        declared,
        extra={"role": "canvaskit-module", "media_type": "application/wasm"},
    )
    for asset_name, (filename, role) in BRIDGE_ASSETS.items():
        validate_binding(
            assets[asset_name],
            f"manifest.assets.{asset_name}",
            f"web/{filename}",
            declared,
            extra={"role": role, "media_type": "text/javascript"},
        )
    validate_javascript(root / "web" / "canvaskit.js", "CanvasKit JavaScript")
    bridge_inputs(str(root / "web"))
    validate_wasm(root / "web" / "canvaskit.wasm")
    if protocol_version(root / "web" / "fission_skia_web.js") != protocol:
        raise foundation.SkiaToolError("Web bridge protocol does not match the manifest ABI")

    receipt = validate_build_receipt(
        manifest.get("build_receipt"),
        config,
        expected_profile=profile_name,
    )
    if manifest.get("toolchain") != {
        "id": receipt["plan"]["toolchain_id"],
        "compiler": receipt["plan"]["tools"]["emcc"]["version"],
        "runtime_abi": f"Emscripten {EMSCRIPTEN_VERSION} / wasm32",
    }:
        raise foundation.SkiaToolError("CanvasKit manifest toolchain does not match its build")
    if manifest.get("deployment") != deployment_contract(profile_name):
        raise foundation.SkiaToolError("CanvasKit manifest deployment does not match its profile")
    compare_build_output(receipt["outputs"][0], root / "web" / "canvaskit.js", "CanvasKit JavaScript")
    compare_build_output(receipt["outputs"][1], root / "web" / "canvaskit.wasm", "CanvasKit Wasm")
    return manifest


def verify_archive(
    archive: Path,
    expected_sha256: str,
    config: Mapping[str, Any],
    expected_profile: str | None,
    expected_target: str | None,
) -> dict[str, Any]:
    if not foundation.SHA256_RE.fullmatch(expected_sha256):
        raise foundation.SkiaToolError("archive SHA-256 must be lowercase hexadecimal")
    with tempfile.TemporaryDirectory(prefix="fission-canvaskit-verify-") as temporary:
        destination = Path(temporary)
        snapshot = destination / "input.tar.gz"
        actual = foundation.copy_archive_once(archive, snapshot)
        if actual != expected_sha256:
            raise foundation.SkiaToolError(
                f"archive digest mismatch: expected {expected_sha256}, found {actual}"
            )
        try:
            with tarfile.open(snapshot, "r:gz") as source:
                _, members = foundation.validated_archive_members(source)
                if len(members) > MAX_ARTIFACT_FILES + 4:
                    raise foundation.SkiaToolError("CanvasKit archive contains too many entries")
                regular = [member for member in members if member.isfile()]
                if any(member.size > MAX_SINGLE_FILE_BYTES for member in regular):
                    raise foundation.SkiaToolError("CanvasKit archive contains an oversized file")
                if sum(member.size for member in regular) > MAX_ARTIFACT_BYTES:
                    raise foundation.SkiaToolError("CanvasKit archive exceeds the unpacked size limit")
                root_name = foundation.extract_validated_archive(source, destination)
        except (tarfile.TarError, EOFError) as error:
            raise foundation.SkiaToolError(f"invalid CanvasKit archive: {error}") from error
        return verify_artifact_directory(
            destination / root_name,
            config,
            expected_profile=expected_profile,
            expected_target=expected_target,
        )


def verify_command(args: argparse.Namespace, config: dict[str, Any]) -> None:
    if bool(args.artifact_dir) == bool(args.archive):
        raise foundation.SkiaToolError("verify requires exactly one of --artifact-dir or --archive")
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
            raise foundation.SkiaToolError("--sha256 is required when verifying an archive")
        manifest = verify_archive(
            Path(args.archive).expanduser().resolve(),
            args.sha256,
            config,
            args.profile,
            args.target,
        )
    print(
        f"verified {manifest['artifact_id']} "
        f"(profile={manifest['profile']}, lane={manifest['lane']}, qualified=false)"
    )


def show_plan(args: argparse.Namespace, config: dict[str, Any]) -> None:
    print(foundation.canonical_json(web_recipe(config, args.profile)), end="")


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--config", default=str(DEFAULT_CONFIG), help="pinned Skia configuration")
    commands = result.add_subparsers(dest="command", required=True)

    plan = commands.add_parser("show-plan", help="print a pinned CanvasKit recipe")
    plan.add_argument("--profile", choices=CANVASKIT_PROFILES, default=PRODUCTION_PROFILE)
    plan.set_defaults(action=show_plan)

    build = commands.add_parser("build", help="build CanvasKit from prepared local checkouts")
    build.add_argument("--source-dir")
    build.add_argument("--emsdk-dir")
    build.add_argument("--build-dir")
    build.add_argument("--profile", choices=CANVASKIT_PROFILES, required=True)
    build.add_argument("--toolchain-id", required=True)
    build.add_argument("--gn")
    build.add_argument("--ninja")
    build.add_argument("--gn-sha256", required=True)
    build.add_argument("--ninja-sha256", required=True)
    build.add_argument("--emcc-sha256", required=True)
    build.add_argument("--emxx-sha256", required=True)
    build.add_argument("--emar-sha256", required=True)
    build.set_defaults(action=build_canvaskit)

    package = commands.add_parser("package", help="assemble an unqualified CanvasKit artifact")
    package.add_argument("--profile", choices=CANVASKIT_PROFILES, required=True)
    package.add_argument("--target", default=TARGET)
    package.add_argument("--fission-version", required=True)
    package.add_argument("--build-metadata", required=True)
    package.add_argument("--canvaskit-js", required=True)
    package.add_argument("--canvaskit-wasm", required=True)
    package.add_argument(
        "--bridge-dir",
        required=True,
        help="directory containing the exact Fission CanvasKit runtime modules",
    )
    package.add_argument("--deployment-metadata", required=True)
    package.add_argument("--license", action="append", default=[])
    package.add_argument("--output", required=True)
    package.add_argument("--archive")
    package.add_argument("--source-date-epoch")
    package.set_defaults(action=package_canvaskit)

    verify = commands.add_parser("verify", help="strictly verify an artifact or archive")
    verify.add_argument("--artifact-dir")
    verify.add_argument("--archive")
    verify.add_argument("--sha256")
    verify.add_argument("--manifest-sha256")
    verify.add_argument("--profile")
    verify.add_argument("--target")
    verify.set_defaults(action=verify_command)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        config = load_config(Path(args.config).expanduser().resolve())
        args.action(args, config)
    except foundation.SkiaToolError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
