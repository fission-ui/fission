#!/usr/bin/env python3
"""Fail-closed promotion of qualified Fission Skia release artifacts."""

from __future__ import annotations

import argparse
from contextlib import contextmanager
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys
import tarfile
import tempfile
from typing import Any, Iterator, Mapping, Sequence
from urllib.parse import urlparse


TOOL_DIR = Path(__file__).resolve().parent
REPOSITORY_ROOT = TOOL_DIR.parents[1]
DEFAULT_QUALIFICATION_MANIFEST = (
    REPOSITORY_ROOT / "tools/backend-qualification/qualification-manifest.json"
)
QUALIFICATION_TOOL = REPOSITORY_ROOT / "tools/backend-qualification/qualification.py"
if str(TOOL_DIR) not in sys.path:
    sys.path.insert(0, str(TOOL_DIR))

import canvaskit  # noqa: E402
import skia as foundation  # noqa: E402


REPOSITORY = "fission-ui/fission"
PREDICATE_TYPE = "https://slsa.dev/provenance/v1"
DEFAULT_SIGNER_WORKFLOW = (
    "github.com/fission-ui/fission/.github/workflows/skia-artifacts.yml"
)
PROMOTED_ORIGIN = "fission-release"
LOCK_SCHEMA_VERSION = 1
MAX_ARCHIVE_BYTES = 2 * 1024 * 1024 * 1024
MAX_EXPANDED_BYTES = 4 * 1024 * 1024 * 1024
MAX_ARCHIVE_MEMBERS = 8_192
GIT_DIGEST_RE = re.compile(r"^[0-9a-f]{40}(?:[0-9a-f]{24})?$")


def load_qualification_tool() -> Any:
    specification = importlib.util.spec_from_file_location(
        "fission_backend_qualification_for_promotion",
        QUALIFICATION_TOOL,
    )
    if specification is None or specification.loader is None:
        raise RuntimeError("cannot load the authoritative backend qualification tool")
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


qualification = load_qualification_tool()


class PromotionError(foundation.SkiaToolError):
    """A release artifact failed a promotion or trust gate."""


def require_sha256(value: str, context: str) -> str:
    if not foundation.SHA256_RE.fullmatch(value):
        raise PromotionError(f"{context} must be 64 lowercase hexadecimal characters")
    return value


def load_object(path: Path, context: str) -> dict[str, Any]:
    value = foundation.load_json(path)
    if not isinstance(value, dict):
        raise PromotionError(f"{context} must be a JSON object")
    return value


def qualification_report_digest(
    path: Path,
    qualification_manifest_path: Path,
    evidence_paths: Sequence[Path],
    artifact_id: str,
    artifact_sha256: str,
    target_id: str,
    profile_id: str,
) -> str:
    try:
        qualification_manifest = qualification.load_json(qualification_manifest_path)
        evidence = [qualification.load_json(evidence_path) for evidence_path in evidence_paths]
        authoritative = qualification.build_report(qualification_manifest, evidence)
    except qualification.QualificationError as error:
        raise PromotionError(f"backend qualification input is invalid: {error}") from error
    report = load_object(path, "qualification report")
    if report != authoritative:
        raise PromotionError(
            "qualification report was not produced from the supplied frozen matrix and evidence"
        )
    if report.get("schema_version") != qualification.SCHEMA_VERSION:
        raise PromotionError("qualification report schema is unsupported")
    if report.get("qualified") is not True or report.get("issues") != []:
        raise PromotionError("the complete frozen backend matrix is not qualified")
    matrix_revision = qualification.MATRIX_REVISION
    if (
        not isinstance(matrix_revision, str)
        or not matrix_revision
        or report.get("matrix_revision") != matrix_revision
    ):
        raise PromotionError("qualification report does not match the frozen matrix revision")
    manifest_sha256 = report.get("manifest_sha256")
    if manifest_sha256 != qualification.digest_json(
        qualification.validate_manifest(qualification_manifest)
    ):
        raise PromotionError("qualification report does not bind the frozen manifest")

    comparisons = report.get("comparisons")
    if not isinstance(comparisons, list):
        raise PromotionError("qualification report comparisons must be an array")
    matches = [
        comparison
        for comparison in comparisons
        if isinstance(comparison, dict)
        and comparison.get("target_id") == target_id
        and comparison.get("profile_id") == profile_id
    ]
    if len(matches) != 1:
        raise PromotionError(
            "qualification report must contain exactly one requested target/profile cell"
        )
    comparison = matches[0]
    identity = comparison.get("identity")
    if comparison.get("qualified") is not True or not isinstance(identity, dict):
        raise PromotionError("the requested qualification cell did not pass")
    if identity.get("artifact_id") != artifact_id:
        raise PromotionError(
            "qualification evidence is not bound to the artifact being promoted"
        )
    if identity.get("artifact_sha256") != artifact_sha256:
        raise PromotionError(
            "qualification evidence is not bound to the exact archive being promoted"
        )
    run_id = comparison.get("run_id")
    if not isinstance(run_id, str) or not run_id:
        raise PromotionError("the requested qualification cell has no run identity")
    return foundation.sha256_file(path)


def bounded_archive_members(
    source: tarfile.TarFile,
    *,
    kind: str,
) -> list[tarfile.TarInfo]:
    members: list[tarfile.TarInfo] = []
    expanded_bytes = 0
    while True:
        member = source.next()
        if member is None:
            return members
        if len(members) >= MAX_ARCHIVE_MEMBERS:
            raise PromotionError(f"{kind} archive contains too many entries")
        if member.isfile():
            if member.size < 0 or member.size > MAX_ARCHIVE_BYTES:
                raise PromotionError(f"{kind} archive contains an oversized file")
            expanded_bytes += member.size
            if expanded_bytes > MAX_EXPANDED_BYTES:
                raise PromotionError(f"{kind} archive exceeds the expanded-size limit")
        members.append(member)


@contextmanager
def extracted_archive(
    archive: Path,
    expected_sha256: str,
    kind: str,
) -> Iterator[tuple[Path, str, Path]]:
    require_sha256(expected_sha256, "archive SHA-256")
    with tempfile.TemporaryDirectory(prefix="fission-skia-promotion-") as raw:
        temporary = Path(raw)
        snapshot = temporary / "input.tar.gz"
        actual = foundation.copy_archive_once(
            archive,
            snapshot,
            max_bytes=MAX_ARCHIVE_BYTES,
        )
        if actual != expected_sha256:
            raise PromotionError(
                f"archive digest mismatch: expected {expected_sha256}, found {actual}"
            )
        try:
            with tarfile.open(snapshot, "r:gz") as source:
                bounded_archive_members(source, kind=kind)
                foundation.validated_archive_members(source)
                root_name = foundation.extract_validated_archive(source, temporary)
        except (tarfile.TarError, EOFError) as error:
            raise PromotionError(f"invalid {kind} archive: {error}") from error
        yield temporary / root_name, root_name, snapshot


def verify_unqualified(
    root: Path,
    kind: str,
    config: Mapping[str, Any],
    profile: str,
    target: str,
) -> dict[str, Any]:
    if kind == "native":
        return foundation.verify_artifact_directory(
            root,
            config,
            expected_profile=profile,
            expected_target=target,
        )
    if kind == "canvaskit":
        return canvaskit.verify_artifact_directory(
            root,
            config,
            expected_profile=profile,
            expected_target=target,
        )
    raise PromotionError(f"unsupported artifact kind: {kind}")


def promoted_manifest(
    root: Path,
    kind: str,
    config: Mapping[str, Any],
    profile: str,
    target: str,
) -> tuple[dict[str, Any], str]:
    path = root / foundation.MANIFEST
    manifest = load_object(path, "promoted artifact manifest")
    if manifest.get("origin") != PROMOTED_ORIGIN or manifest.get("qualified") is not True:
        raise PromotionError("artifact manifest is not release-qualified")

    # The foundation verifier remains intentionally incapable of accepting a
    # qualification claim. Verify an exact temporary demotion so every source,
    # ABI, payload, toolchain, and deployment invariant still comes from that
    # single strict verifier; only these two release-owned fields differ.
    unqualified = dict(manifest)
    unqualified["origin"] = "local-build"
    unqualified["qualified"] = False
    foundation.write_json(path, unqualified)
    try:
        verified = verify_unqualified(root, kind, config, profile, target)
    finally:
        foundation.write_json(path, manifest)
    if set(verified) != set(manifest):
        raise PromotionError("promotion changed fields outside origin and qualified")
    return manifest, foundation.sha256_file(path)


def promote_command(args: argparse.Namespace, config: Mapping[str, Any]) -> None:
    archive = Path(args.archive).expanduser().resolve()
    output = Path(args.output).expanduser().resolve()
    if archive == output:
        raise PromotionError("promoted archive output must differ from its input")
    with extracted_archive(archive, args.sha256, args.kind) as (root, root_name, _):
        manifest = verify_unqualified(root, args.kind, config, args.profile, args.target)
        report_sha256 = qualification_report_digest(
            Path(args.qualification_report).expanduser().resolve(),
            Path(args.qualification_manifest).expanduser().resolve(),
            [Path(path).expanduser().resolve() for path in args.evidence],
            manifest["artifact_id"],
            args.sha256,
            args.qualification_target_id,
            args.qualification_profile_id,
        )
        promoted = dict(manifest)
        promoted["origin"] = PROMOTED_ORIGIN
        promoted["qualified"] = True
        foundation.write_json(root / foundation.MANIFEST, promoted)
        checked, manifest_sha256 = promoted_manifest(
            root,
            args.kind,
            config,
            args.profile,
            args.target,
        )
        if checked["artifact_id"] != root_name:
            raise PromotionError("archive root does not match its artifact identity")
        epoch = args.source_date_epoch
        if epoch is None or not re.fullmatch(r"[0-9]+", epoch):
            raise PromotionError("--source-date-epoch must be a non-negative integer")
        digest = foundation.create_archive_with_sidecar(
            root,
            output,
            checked["artifact_id"],
            int(epoch),
        )
    print(
        foundation.canonical_json(
            {
                "archive": str(output),
                "archive_sha256": digest,
                "artifact_id": manifest["artifact_id"],
                "manifest_sha256": manifest_sha256,
                "qualification_report_sha256": report_sha256,
            }
        ),
        end="",
    )


def verify_attestation(
    archive: Path,
    archive_sha256: str,
    source_digest: str,
    bundle: Path | None,
) -> None:
    if not GIT_DIGEST_RE.fullmatch(source_digest):
        raise PromotionError("source digest must be a lowercase Git SHA-1 or SHA-256")
    command = [
        "gh",
        "attestation",
        "verify",
        str(archive),
        "--repo",
        REPOSITORY,
        "--predicate-type",
        PREDICATE_TYPE,
        "--signer-workflow",
        DEFAULT_SIGNER_WORKFLOW,
        "--source-digest",
        source_digest,
        "--deny-self-hosted-runners",
        "--format",
        "json",
    ]
    if bundle is not None:
        command.extend(["--bundle", str(bundle)])
    try:
        result = subprocess.run(command, check=True, capture_output=True, text=True)
    except FileNotFoundError as error:
        raise PromotionError("GitHub CLI is unavailable: gh") from error
    except subprocess.CalledProcessError as error:
        detail = (error.stderr or error.stdout or "attestation verification failed").strip()
        raise PromotionError(f"GitHub provenance verification failed: {detail}") from error
    try:
        verified = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise PromotionError("GitHub CLI returned invalid attestation JSON") from error
    if not isinstance(verified, list) or not verified:
        raise PromotionError("GitHub CLI returned no verified attestations")
    for index, record in enumerate(verified):
        result_object = record.get("verificationResult") if isinstance(record, dict) else None
        statement = result_object.get("statement") if isinstance(result_object, dict) else None
        timestamps = (
            result_object.get("verifiedTimestamps") if isinstance(result_object, dict) else None
        )
        if not isinstance(statement, dict) or statement.get("predicateType") != PREDICATE_TYPE:
            raise PromotionError(f"verified attestation {index} has the wrong predicate type")
        subjects = statement.get("subject")
        if not isinstance(subjects, list) or not any(
            isinstance(subject, dict)
            and isinstance(subject.get("digest"), dict)
            and subject["digest"].get("sha256") == archive_sha256
            for subject in subjects
        ):
            raise PromotionError(f"verified attestation {index} does not bind the archive digest")
        if not isinstance(timestamps, list) or not timestamps:
            raise PromotionError(f"verified attestation {index} has no trusted timestamp")


def canonical_release_url(url: str, archive_name: str) -> str:
    if any(character.isspace() for character in url):
        raise PromotionError("release URL must not contain whitespace")
    parsed = urlparse(url)
    prefix = "/fission-ui/fission/releases/download/"
    release_path = parsed.path[len(prefix) :] if parsed.path.startswith(prefix) else ""
    release_parts = release_path.split("/")
    if (
        parsed.scheme != "https"
        or parsed.netloc != "github.com"
        or not parsed.path.startswith(prefix)
        or len(release_parts) != 2
        or not all(release_parts)
        or parsed.query
        or parsed.fragment
        or Path(parsed.path).name != archive_name
    ):
        raise PromotionError("release URL is not the canonical GitHub release asset URL")
    return url


def append_lock_entry(
    lock_path: Path,
    manifest: Mapping[str, Any],
    kind: str,
    profile: str,
    target: str,
    url: str,
    archive: Path,
    archive_sha256: str,
    manifest_sha256: str,
) -> None:
    require_sha256(archive_sha256, "archive SHA-256")
    require_sha256(manifest_sha256, "manifest SHA-256")
    if manifest.get("origin") != PROMOTED_ORIGIN or manifest.get("qualified") is not True:
        raise PromotionError("only a verified release-qualified manifest can enter the lock")
    lock = load_object(lock_path, "artifact lock")
    if lock.get("schema_version") != LOCK_SCHEMA_VERSION:
        raise PromotionError("artifact lock schema is unsupported")
    provenance = lock.get("provenance")
    if provenance != {"repository": REPOSITORY, "predicate_type": PREDICATE_TYPE}:
        raise PromotionError("artifact lock provenance policy is not Fission's SLSA v1 policy")
    if (
        lock.get("fission_version") != manifest.get("fission_version")
        or lock.get("skia_revision")
        != (manifest.get("skia") or manifest.get("source") or {}).get("revision")
    ):
        raise PromotionError("artifact identity does not match the bundled lock release")
    abi = manifest.get("bridge_abi_version")
    web_protocol = None
    if kind == "canvaskit":
        web_abi = manifest.get("abi")
        if not isinstance(web_abi, dict):
            raise PromotionError("CanvasKit manifest has no ABI object")
        abi = web_abi.get("bridge_abi_version")
        web_protocol = web_abi.get("web_protocol_version")
    if lock.get("bridge_abi_version") != abi:
        raise PromotionError("artifact bridge ABI does not match the bundled lock")
    artifacts = lock.get("artifacts")
    if not isinstance(artifacts, list):
        raise PromotionError("artifact lock entries must be an array")
    if any(
        isinstance(entry, dict)
        and entry.get("kind") == kind
        and entry.get("target") == target
        and entry.get("profile") == profile
        for entry in artifacts
    ):
        raise PromotionError("artifact lock already contains this kind/target/profile")
    entry: dict[str, Any] = {
        "kind": kind,
        "artifact_id": manifest["artifact_id"],
        "target": target,
        "profile": profile,
        "qualified": True,
        "url": url,
        "archive_sha256": archive_sha256,
        "archive_size": archive.stat(follow_symlinks=False).st_size,
        "manifest_sha256": manifest_sha256,
    }
    if kind == "canvaskit":
        entry["web_protocol_version"] = web_protocol
    artifacts.append(entry)
    artifacts.sort(
        key=lambda value: (value.get("kind", ""), value.get("target", ""), value.get("profile", ""))
    )
    foundation.write_json(lock_path, lock)


def lock_command(args: argparse.Namespace, config: Mapping[str, Any]) -> None:
    archive = Path(args.archive).expanduser().resolve()
    with extracted_archive(archive, args.sha256, args.kind) as (
        root,
        root_name,
        snapshot,
    ):
        manifest, manifest_sha256 = promoted_manifest(
            root,
            args.kind,
            config,
            args.profile,
            args.target,
        )
        if manifest["artifact_id"] != root_name:
            raise PromotionError("archive root does not match its artifact identity")
        verify_attestation(
            snapshot,
            args.sha256,
            args.source_digest,
            Path(args.bundle).expanduser().resolve() if args.bundle else None,
        )
        url = canonical_release_url(args.url, archive.name)
        append_lock_entry(
            Path(args.lock).expanduser().resolve(),
            manifest,
            args.kind,
            args.profile,
            args.target,
            url,
            snapshot,
            args.sha256,
            manifest_sha256,
        )
    print(f"locked {manifest['artifact_id']} ({args.sha256})")


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--config", default=str(foundation.DEFAULT_CONFIG))
    commands = result.add_subparsers(dest="command", required=True)

    promote = commands.add_parser("promote", help="promote one qualified local artifact")
    promote.add_argument("--kind", choices=["native", "canvaskit"], required=True)
    promote.add_argument("--archive", required=True)
    promote.add_argument("--sha256", required=True)
    promote.add_argument("--profile", required=True)
    promote.add_argument("--target", required=True)
    promote.add_argument("--qualification-report", required=True)
    promote.add_argument(
        "--qualification-manifest",
        default=str(DEFAULT_QUALIFICATION_MANIFEST),
    )
    promote.add_argument(
        "--evidence",
        action="append",
        required=True,
        help="raw qualification evidence JSON; repeat for every frozen matrix cell",
    )
    promote.add_argument("--qualification-target-id", required=True)
    promote.add_argument("--qualification-profile-id", required=True)
    promote.add_argument("--source-date-epoch", required=True)
    promote.add_argument("--output", required=True)
    promote.set_defaults(action=promote_command)

    lock = commands.add_parser(
        "lock", help="verify GitHub provenance and add one immutable release asset to the lock"
    )
    lock.add_argument("--kind", choices=["native", "canvaskit"], required=True)
    lock.add_argument("--archive", required=True)
    lock.add_argument("--sha256", required=True)
    lock.add_argument("--profile", required=True)
    lock.add_argument("--target", required=True)
    lock.add_argument("--url", required=True)
    lock.add_argument("--source-digest", required=True)
    lock.add_argument("--bundle")
    lock.add_argument(
        "--lock",
        default=str(
            REPOSITORY_ROOT
            / "crates/rendering/fission-skia-artifacts/artifacts.lock.json"
        ),
    )
    lock.set_defaults(action=lock_command)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        config = foundation.load_config(Path(args.config).expanduser().resolve())
        args.action(args, config)
        return 0
    except (PromotionError, foundation.SkiaToolError, OSError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
