#!/usr/bin/env python3
"""CI orchestration for Fission's fail-closed Skia artifact pipeline.

This module deliberately delegates build, package, qualification, promotion,
and provenance policy to the existing authoritative tools.  It only supplies
the fixed release matrix and moves the resulting exact-byte artifacts between
those stages.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys
import tempfile
from typing import Any, Iterable, Mapping, Sequence


TOOL_DIR = Path(__file__).resolve().parent
REPOSITORY_ROOT = TOOL_DIR.parents[1]
QUALIFICATION_DIR = REPOSITORY_ROOT / "tools/backend-qualification"
if str(TOOL_DIR) not in sys.path:
    sys.path.insert(0, str(TOOL_DIR))
if str(QUALIFICATION_DIR) not in sys.path:
    sys.path.insert(0, str(QUALIFICATION_DIR))

import canvaskit  # noqa: E402
import promote as promotion  # noqa: E402
import skia as foundation  # noqa: E402
import qualification  # noqa: E402


DEFAULT_QUALIFICATION_MANIFEST = QUALIFICATION_DIR / "qualification-manifest.json"
DEFAULT_LOCK = (
    REPOSITORY_ROOT
    / "crates/rendering/fission-skia-artifacts/artifacts.lock.json"
)
DESCRIPTOR_SCHEMA_VERSION = 1
INDEX_SCHEMA_VERSION = 1
NATIVE_PROFILES = ("native-raster", "native-ganesh")
WEB_PROFILES = (
    canvaskit.PRODUCTION_PROFILE,
    canvaskit.SOFTWARE_PROFILE,
)
QUALIFICATION_PROFILE = {
    "native-raster": "standalone-software",
    "native-ganesh": "skia-only",
    canvaskit.PRODUCTION_PROFILE: "skia-only",
    canvaskit.SOFTWARE_PROFILE: "standalone-software",
}
RUNNERS = {
    "Linux": "ubuntu-24.04",
    "macOS": "macos-15",
    "Windows": "windows-2025",
    "Android": "ubuntu-24.04",
    "iOS": "macos-15",
}
TOOL_FAMILIES = {
    "Linux": "linux-x86_64",
    "macOS": "macos-arm64",
    "Windows": "windows-x86_64",
    "Android": "linux-x86_64",
    "iOS": "macos-arm64",
}
LICENSE_CANDIDATES: dict[str, tuple[str, ...]] = {
    "brotli": ("third_party/externals/brotli/LICENSE",),
    "cpu-features": (
        "third_party/cpu-features/LICENSE",
        "third_party/externals/cpu_features/LICENSE",
        "third_party/externals/cpu-features/LICENSE",
    ),
    "expat": (
        "third_party/externals/expat/COPYING",
        "third_party/externals/expat/LICENSE",
    ),
    "freetype": (
        "third_party/externals/freetype/LICENSE.TXT",
    ),
    "harfbuzz": (
        "third_party/externals/harfbuzz/COPYING",
        "third_party/externals/harfbuzz/LICENSE",
    ),
    "icu": (
        "third_party/externals/icu/LICENSE",
        "third_party/externals/icu/LICENSE.md",
    ),
    "libjpeg-turbo": (
        "third_party/externals/libjpeg-turbo/LICENSE.md",
        "third_party/externals/libjpeg-turbo/LICENSE.txt",
    ),
    "libpng": (
        "third_party/externals/libpng/LICENSE",
        "third_party/externals/libpng/LICENSE.md",
    ),
    "libwebp": ("third_party/externals/libwebp/COPYING",),
    "skia": ("LICENSE",),
    "vulkan-headers": (
        "third_party/externals/vulkan-headers/LICENSE.md",
    ),
    "vulkan-memory-allocator": (
        "third_party/externals/vulkanmemoryallocator/LICENSE.txt",
        "third_party/externals/vulkanmemoryallocator/LICENSE.md",
    ),
    "wuffs": ("third_party/externals/wuffs/LICENSE",),
    "zlib": (
        "third_party/externals/zlib/LICENSE",
    ),
}


class CiError(foundation.SkiaToolError):
    """A CI input failed the checked-in artifact contract."""


def load_qualification_manifest(path: Path) -> dict[str, Any]:
    try:
        return qualification.validate_manifest(qualification.load_json(path))
    except qualification.QualificationError as error:
        raise CiError(f"qualification manifest is invalid: {error}") from error


def qualification_targets(path: Path) -> list[dict[str, Any]]:
    manifest = load_qualification_manifest(path)
    return [dict(target) for target in manifest["targets"]]


def native_matrix(
    config: Mapping[str, Any], qualification_manifest_path: Path
) -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    for target in qualification_targets(qualification_manifest_path):
        if target["kind"] != "native":
            continue
        target_name = target["target"]
        platform = target["platform"]
        foundation.select_target(config, target_name)
        for profile_name in NATIVE_PROFILES:
            profile = foundation.select_profile(config, profile_name)
            foundation.select_profile_target_recipe(
                profile,
                profile_name,
                target_name,
            )
            rows.append(
                {
                    "id": f"{target['id']}-{profile_name}",
                    "kind": "native",
                    "profile": profile_name,
                    "target": target_name,
                    "qualification_target_id": target["id"],
                    "qualification_profile_id": QUALIFICATION_PROFILE[profile_name],
                    "platform": platform,
                    "runner": RUNNERS[platform],
                    "tool_family": TOOL_FAMILIES[platform],
                }
            )
    return rows


def web_matrix(
    config: Mapping[str, Any], qualification_manifest_path: Path
) -> list[dict[str, str]]:
    web_targets = [
        target
        for target in qualification_targets(qualification_manifest_path)
        if target["kind"] == "web"
    ]
    if not web_targets:
        raise CiError("the frozen qualification matrix has no interactive Web target")
    target_names = {target["target"] for target in web_targets}
    if target_names != {canvaskit.TARGET}:
        raise CiError("interactive Web targets do not use the CanvasKit target identity")
    primary = next(
        (target for target in web_targets if target["browser"] == "Chromium"),
        web_targets[0],
    )
    rows = []
    for profile_name in WEB_PROFILES:
        foundation.select_profile(config, profile_name)
        rows.append(
            {
                "id": f"web-{profile_name}",
                "kind": "canvaskit",
                "profile": profile_name,
                "target": canvaskit.TARGET,
                "qualification_target_id": primary["id"],
                "qualification_profile_id": QUALIFICATION_PROFILE[profile_name],
                "platform": "Web",
                "runner": "ubuntu-24.04",
                "tool_family": "linux-x86_64",
            }
        )
    return rows


def matrix_command(args: argparse.Namespace, config: Mapping[str, Any]) -> None:
    qualification_path = Path(args.qualification_manifest).expanduser().resolve()
    rows = (
        native_matrix(config, qualification_path)
        if args.kind == "native"
        else web_matrix(config, qualification_path)
    )
    print(json.dumps({"include": rows}, separators=(",", ":"), sort_keys=True))


def parse_key_values(values: Iterable[str], context: str) -> dict[str, str]:
    result: dict[str, str] = {}
    for raw in values:
        if "=" not in raw:
            raise CiError(f"{context} must use NAME=VALUE syntax")
        name, value = raw.split("=", 1)
        if not name or not value or name in result:
            raise CiError(f"{context} contains an empty or duplicate value")
        result[name] = value
    return result


def resolve_license(component: str, source_dir: Path) -> Path:
    if component == "fission":
        path = REPOSITORY_ROOT / "LICENSE"
        if path.is_file():
            return path
        raise CiError("the Fission repository licence is missing")
    candidates = LICENSE_CANDIDATES.get(component)
    if candidates is None:
        raise CiError(f"no reviewed licence path is declared for {component!r}")
    existing = [source_dir / relative for relative in candidates if (source_dir / relative).is_file()]
    if len(existing) != 1:
        rendered = ", ".join(str(source_dir / relative) for relative in candidates)
        raise CiError(
            f"expected exactly one reviewed licence path for {component!r}; checked {rendered}"
        )
    return existing[0]


def license_arguments(
    profile: Mapping[str, Any],
    target: Mapping[str, Any] | None,
    source_dir: Path,
    explicit: Mapping[str, Path] | None = None,
) -> list[str]:
    components = (
        foundation.required_native_licenses(profile, target)
        if target is not None
        else foundation.require_string_list(
            profile.get("required_licenses"), "profile required_licenses"
        )
    )
    explicit = dict(explicit or {})
    unexpected = set(explicit) - set(components)
    if unexpected:
        raise CiError(f"explicit licences are not required: {sorted(unexpected)}")
    return [
        f"{component}={explicit.get(component) or resolve_license(component, source_dir)}"
        for component in components
    ]


def package_native_command(args: argparse.Namespace, config: Mapping[str, Any]) -> None:
    source_dir = Path(args.source_dir).expanduser().resolve()
    build_dir = Path(args.build_dir).expanduser().resolve()
    profile = foundation.select_profile(config, args.profile)
    target = foundation.select_target(config, args.target)
    explicit_licenses = foundation.parse_named_paths(args.license, "target licence")
    build_receipt = foundation.validate_build_receipt(
        foundation.load_json(build_dir / foundation.BUILD_METADATA),
        dict(config),
        args.profile,
        args.target,
    )
    recipe = build_receipt["plan"]["recipe"]
    deployment = parse_key_values(args.deployment, "--deployment")
    output = Path(args.output).expanduser().resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="fission-skia-ci-", dir=output.parent) as raw:
        temporary = Path(raw)
        link_path = temporary / "link.json"
        deployment_path = temporary / "deployment.json"
        foundation.write_json(
            link_path,
            {
                "link_search_paths": ["lib"],
                "static_libraries": [
                    "fission_skia_bridge",
                    *recipe["upstream_libraries"],
                ],
                "system_libraries": recipe["system_libraries"],
                "frameworks": recipe["frameworks"],
            },
        )
        foundation.write_json(
            deployment_path,
            {
                "toolchain": {
                    "id": args.toolchain_id,
                    "compiler": args.compiler,
                    "runtime_abi": args.runtime_abi,
                },
                "deployment": deployment,
            },
        )
        libraries = [f"fission_skia_bridge={Path(args.bridge_library).resolve()}"]
        libraries.extend(
            f"{name}={build_dir / foundation.canonical_library_filename(name, args.target)}"
            for name in recipe["upstream_libraries"]
        )
        namespace = argparse.Namespace(
            profile=args.profile,
            target=args.target,
            fission_version=args.fission_version,
            build_metadata=str(build_dir / foundation.BUILD_METADATA),
            bridge_header=str(
                REPOSITORY_ROOT
                / "crates/rendering/fission-skia-sys/include/fission_skia.h"
            ),
            library=libraries,
            license=license_arguments(profile, target, source_dir, explicit_licenses),
            link_metadata=str(link_path),
            deployment_metadata=str(deployment_path),
            output=str(output),
            archive=str(Path(args.archive).expanduser().resolve()),
            source_date_epoch=args.source_date_epoch,
        )
        foundation.package_native(namespace, dict(config))


def package_canvaskit_command(args: argparse.Namespace, config: Mapping[str, Any]) -> None:
    source_dir = Path(args.source_dir).expanduser().resolve()
    build_dir = Path(args.build_dir).expanduser().resolve()
    profile = foundation.select_profile(config, args.profile)
    receipt = canvaskit.validate_build_receipt(
        foundation.load_json(build_dir / canvaskit.BUILD_METADATA),
        config,
        expected_profile=args.profile,
    )
    output = Path(args.output).expanduser().resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="fission-canvaskit-ci-", dir=output.parent) as raw:
        deployment_path = Path(raw) / "deployment.json"
        foundation.write_json(
            deployment_path,
            {
                "toolchain": {
                    "id": receipt["plan"]["toolchain_id"],
                    "compiler": receipt["plan"]["tools"]["emcc"]["version"],
                    "runtime_abi": f"Emscripten {canvaskit.EMSCRIPTEN_VERSION} / wasm32",
                },
                "deployment": canvaskit.deployment_contract(args.profile),
            },
        )
        namespace = argparse.Namespace(
            profile=args.profile,
            target=canvaskit.TARGET,
            fission_version=args.fission_version,
            build_metadata=str(build_dir / canvaskit.BUILD_METADATA),
            canvaskit_js=str(build_dir / "canvaskit.js"),
            canvaskit_wasm=str(build_dir / "canvaskit.wasm"),
            bridge_dir=str(
                REPOSITORY_ROOT / "crates/rendering/fission-skia-sys/web"
            ),
            deployment_metadata=str(deployment_path),
            license=license_arguments(profile, None, source_dir),
            output=str(output),
            archive=str(Path(args.archive).expanduser().resolve()),
            source_date_epoch=args.source_date_epoch,
        )
        canvaskit.package_canvaskit(namespace, dict(config))


def qualification_cell(
    qualification_manifest_path: Path,
    target: str,
    profile: str,
) -> tuple[str, str]:
    profile_id = QUALIFICATION_PROFILE.get(profile)
    if profile_id is None:
        raise CiError(f"artifact profile {profile!r} has no qualification mapping")
    candidates = [
        value
        for value in qualification_targets(qualification_manifest_path)
        if value["target"] == target
    ]
    if target == canvaskit.TARGET:
        chromium = [value for value in candidates if value["browser"] == "Chromium"]
        candidates = chromium or candidates
    if len(candidates) != 1:
        raise CiError(
            f"artifact {profile}/{target} has no unique qualification target mapping"
        )
    return candidates[0]["id"], profile_id


def descriptor_for_archive(
    archive: Path,
    kind: str,
    profile: str,
    target: str,
    config: Mapping[str, Any],
    qualification_manifest_path: Path,
) -> dict[str, Any]:
    digest = foundation.sha256_file(archive)
    if kind == "native":
        manifest = foundation.verify_archive(archive, digest, config, profile, target)
    elif kind == "canvaskit":
        manifest = canvaskit.verify_archive(archive, digest, config, profile, target)
    else:
        raise CiError(f"unsupported artifact kind: {kind!r}")
    target_id, profile_id = qualification_cell(
        qualification_manifest_path,
        target,
        profile,
    )
    return {
        "schema_version": DESCRIPTOR_SCHEMA_VERSION,
        "kind": kind,
        "profile": profile,
        "target": target,
        "artifact_id": manifest["artifact_id"],
        "archive": archive.name,
        "archive_sha256": digest,
        "qualification_target_id": target_id,
        "qualification_profile_id": profile_id,
    }


def describe_command(args: argparse.Namespace, config: Mapping[str, Any]) -> None:
    archive = Path(args.archive).expanduser().resolve()
    descriptor = descriptor_for_archive(
        archive,
        args.kind,
        args.profile,
        args.target,
        config,
        Path(args.qualification_manifest).expanduser().resolve(),
    )
    foundation.write_json(Path(args.output).expanduser().resolve(), descriptor)


def load_descriptor(path: Path) -> dict[str, Any]:
    descriptor = foundation.require_object(foundation.load_json(path), str(path))
    expected_fields = {
        "schema_version",
        "kind",
        "profile",
        "target",
        "artifact_id",
        "archive",
        "archive_sha256",
        "qualification_target_id",
        "qualification_profile_id",
    }
    if set(descriptor) != expected_fields or descriptor.get("schema_version") != DESCRIPTOR_SCHEMA_VERSION:
        raise CiError(f"artifact descriptor has unknown, missing, or unsupported fields: {path}")
    foundation.require_string(descriptor.get("artifact_id"), f"{path}.artifact_id")
    promotion.require_sha256(descriptor.get("archive_sha256"), f"{path}.archive_sha256")
    return descriptor


def expected_artifact_keys(
    config: Mapping[str, Any], qualification_manifest_path: Path
) -> set[tuple[str, str, str]]:
    rows = native_matrix(config, qualification_manifest_path)
    rows.extend(web_matrix(config, qualification_manifest_path))
    return {(row["kind"], row["profile"], row["target"]) for row in rows}


def verify_set(
    root: Path,
    config: Mapping[str, Any],
    qualification_manifest_path: Path,
) -> dict[str, Any]:
    descriptor_paths = sorted(root.rglob("*.artifact.json"))
    if not descriptor_paths:
        raise CiError("the build run contains no Skia artifact descriptors")
    entries = []
    keys: set[tuple[str, str, str]] = set()
    for path in descriptor_paths:
        descriptor = load_descriptor(path)
        key = (descriptor["kind"], descriptor["profile"], descriptor["target"])
        if key in keys:
            raise CiError(f"the build run contains duplicate artifact {key}")
        keys.add(key)
        archive = path.parent / descriptor["archive"]
        verified = descriptor_for_archive(
            archive,
            descriptor["kind"],
            descriptor["profile"],
            descriptor["target"],
            config,
            qualification_manifest_path,
        )
        if descriptor != verified:
            raise CiError(f"artifact descriptor does not bind its archive exactly: {path}")
        entries.append({**descriptor, "archive_path": str(archive)})
    expected = expected_artifact_keys(config, qualification_manifest_path)
    if keys != expected:
        raise CiError(
            "the build run does not contain the complete release matrix; "
            f"missing={sorted(expected - keys)}, extra={sorted(keys - expected)}"
        )
    entries.sort(key=lambda value: (value["kind"], value["target"], value["profile"]))
    return {"schema_version": INDEX_SCHEMA_VERSION, "artifacts": entries}


def verify_set_command(args: argparse.Namespace, config: Mapping[str, Any]) -> None:
    index = verify_set(
        Path(args.input_root).expanduser().resolve(),
        config,
        Path(args.qualification_manifest).expanduser().resolve(),
    )
    foundation.write_json(Path(args.output).expanduser().resolve(), index)


def load_index(path: Path) -> dict[str, Any]:
    index = foundation.require_object(foundation.load_json(path), str(path))
    if set(index) != {"schema_version", "artifacts"} or index.get("schema_version") != INDEX_SCHEMA_VERSION:
        raise CiError("artifact index has unknown, missing, or unsupported fields")
    artifacts = index.get("artifacts")
    if not isinstance(artifacts, list) or not artifacts:
        raise CiError("artifact index is empty")
    return index


def evidence_paths(root: Path) -> list[Path]:
    paths = sorted(root.glob("*.json"))
    if not paths:
        raise CiError("qualification run contains no raw evidence JSON files")
    return paths


def verify_provenance_set_command(
    args: argparse.Namespace, _config: Mapping[str, Any]
) -> None:
    index = load_index(Path(args.index).expanduser().resolve())
    for artifact in index["artifacts"]:
        promotion.verify_attestation(
            Path(artifact["archive_path"]),
            artifact["archive_sha256"],
            args.source_digest,
            None,
        )


def promote_set_command(args: argparse.Namespace, config: Mapping[str, Any]) -> None:
    index = load_index(Path(args.index).expanduser().resolve())
    qualification_manifest_path = Path(args.qualification_manifest).expanduser().resolve()
    evidence = evidence_paths(Path(args.evidence_root).expanduser().resolve())
    raw_manifest = qualification.load_json(qualification_manifest_path)
    raw_evidence = [qualification.load_json(path) for path in evidence]
    report = qualification.build_report(raw_manifest, raw_evidence)
    if report.get("qualified") is not True or report.get("issues") != []:
        raise CiError("the complete frozen backend matrix is not qualified")
    output = Path(args.output).expanduser().resolve()
    output.mkdir(parents=True, exist_ok=False)
    report_path = output / "qualification-report.json"
    foundation.write_json(report_path, report)
    promoted_entries = []
    for artifact in index["artifacts"]:
        archive = Path(artifact["archive_path"])
        promoted_archive = output / archive.name.replace(".tar.gz", "-qualified.tar.gz")
        namespace = argparse.Namespace(
            archive=str(archive),
            output=str(promoted_archive),
            sha256=artifact["archive_sha256"],
            kind=artifact["kind"],
            profile=artifact["profile"],
            target=artifact["target"],
            qualification_report=str(report_path),
            qualification_manifest=str(qualification_manifest_path),
            evidence=[str(path) for path in evidence],
            qualification_target_id=artifact["qualification_target_id"],
            qualification_profile_id=artifact["qualification_profile_id"],
            source_date_epoch=args.source_date_epoch,
        )
        promotion.promote_command(namespace, config)
        promoted_entries.append(
            {
                **{key: value for key, value in artifact.items() if key != "archive_path"},
                "archive": promoted_archive.name,
                "archive_path": str(promoted_archive),
                "archive_sha256": foundation.sha256_file(promoted_archive),
            }
        )
    foundation.write_json(
        output / "promoted-index.json",
        {"schema_version": INDEX_SCHEMA_VERSION, "artifacts": promoted_entries},
    )


def lock_set_command(args: argparse.Namespace, config: Mapping[str, Any]) -> None:
    index = load_index(Path(args.index).expanduser().resolve())
    lock = Path(args.lock).expanduser().resolve()
    for artifact in index["artifacts"]:
        archive = Path(artifact["archive_path"])
        if not archive.is_file():
            fallback = Path(args.index).expanduser().resolve().parent / artifact["archive"]
            if not fallback.is_file():
                raise CiError(f"promoted archive is missing: {archive}")
            archive = fallback
        url = (
            f"https://github.com/{promotion.REPOSITORY}/releases/download/"
            f"{args.release_tag}/{archive.name}"
        )
        namespace = argparse.Namespace(
            archive=str(archive),
            sha256=artifact["archive_sha256"],
            kind=artifact["kind"],
            profile=artifact["profile"],
            target=artifact["target"],
            url=url,
            source_digest=args.source_digest,
            bundle=None,
            lock=str(lock),
        )
        promotion.lock_command(namespace, config)


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--config", default=str(foundation.DEFAULT_CONFIG))
    commands = result.add_subparsers(dest="command", required=True)

    matrix = commands.add_parser("matrix", help="emit the frozen release build matrix")
    matrix.add_argument("--kind", choices=["native", "canvaskit"], required=True)
    matrix.add_argument(
        "--qualification-manifest", default=str(DEFAULT_QUALIFICATION_MANIFEST)
    )
    matrix.set_defaults(action=matrix_command)

    native = commands.add_parser("package-native", help="package one completed native CI build")
    native.add_argument("--source-dir", required=True)
    native.add_argument("--build-dir", required=True)
    native.add_argument("--bridge-library", required=True)
    native.add_argument("--profile", choices=NATIVE_PROFILES, required=True)
    native.add_argument("--target", required=True)
    native.add_argument("--fission-version", required=True)
    native.add_argument("--toolchain-id", required=True)
    native.add_argument("--compiler", required=True)
    native.add_argument("--runtime-abi", required=True)
    native.add_argument("--deployment", action="append", default=[])
    native.add_argument("--license", action="append", default=[])
    native.add_argument("--output", required=True)
    native.add_argument("--archive", required=True)
    native.add_argument("--source-date-epoch", required=True)
    native.set_defaults(action=package_native_command)

    web = commands.add_parser("package-canvaskit", help="package one completed CanvasKit CI build")
    web.add_argument("--source-dir", required=True)
    web.add_argument("--build-dir", required=True)
    web.add_argument("--profile", choices=WEB_PROFILES, required=True)
    web.add_argument("--fission-version", required=True)
    web.add_argument("--output", required=True)
    web.add_argument("--archive", required=True)
    web.add_argument("--source-date-epoch", required=True)
    web.set_defaults(action=package_canvaskit_command)

    describe = commands.add_parser("describe", help="verify and describe one unqualified archive")
    describe.add_argument("--kind", choices=["native", "canvaskit"], required=True)
    describe.add_argument("--archive", required=True)
    describe.add_argument("--profile", required=True)
    describe.add_argument("--target", required=True)
    describe.add_argument("--qualification-manifest", default=str(DEFAULT_QUALIFICATION_MANIFEST))
    describe.add_argument("--output", required=True)
    describe.set_defaults(action=describe_command)

    verify_set_parser = commands.add_parser("verify-set", help="verify a complete build-run matrix")
    verify_set_parser.add_argument("--input-root", required=True)
    verify_set_parser.add_argument("--qualification-manifest", default=str(DEFAULT_QUALIFICATION_MANIFEST))
    verify_set_parser.add_argument("--output", required=True)
    verify_set_parser.set_defaults(action=verify_set_command)

    provenance = commands.add_parser(
        "verify-provenance-set", help="verify every unqualified build attestation"
    )
    provenance.add_argument("--index", required=True)
    provenance.add_argument("--source-digest", required=True)
    provenance.set_defaults(action=verify_provenance_set_command)

    promote = commands.add_parser("promote-set", help="qualify and promote a complete artifact set")
    promote.add_argument("--index", required=True)
    promote.add_argument("--qualification-manifest", default=str(DEFAULT_QUALIFICATION_MANIFEST))
    promote.add_argument("--evidence-root", required=True)
    promote.add_argument("--source-date-epoch", required=True)
    promote.add_argument("--output", required=True)
    promote.set_defaults(action=promote_set_command)

    lock = commands.add_parser("lock-set", help="verify provenance and populate the bundled lock")
    lock.add_argument("--index", required=True)
    lock.add_argument("--lock", default=str(DEFAULT_LOCK))
    lock.add_argument("--release-tag", required=True)
    lock.add_argument("--source-digest", required=True)
    lock.set_defaults(action=lock_set_command)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        config = foundation.load_config(Path(args.config).expanduser().resolve())
        args.action(args, config)
        return 0
    except (CiError, foundation.SkiaToolError, qualification.QualificationError, OSError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
