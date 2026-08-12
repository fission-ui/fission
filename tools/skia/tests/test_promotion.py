from __future__ import annotations

import argparse
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock


TOOL_DIR = Path(__file__).resolve().parents[1]
TOOL = TOOL_DIR / "promote.py"
SPEC = importlib.util.spec_from_file_location("fission_skia_promotion_tool", TOOL)
assert SPEC is not None and SPEC.loader is not None
promotion = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(promotion)

SKIA_TEST_SPEC = importlib.util.spec_from_file_location(
    "fission_skia_fixture_module",
    Path(__file__).with_name("test_skia.py"),
)
assert SKIA_TEST_SPEC is not None and SKIA_TEST_SPEC.loader is not None
skia_fixture_module = importlib.util.module_from_spec(SKIA_TEST_SPEC)
SKIA_TEST_SPEC.loader.exec_module(skia_fixture_module)


class SkiaPromotionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.config = promotion.foundation.load_config(TOOL_DIR / "config.json")

    @staticmethod
    def write_json(path: Path, value: object) -> None:
        path.write_text(promotion.foundation.canonical_json(value), encoding="utf-8")

    def qualification_fixture(
        self,
        temporary: Path,
        artifact_id: str = "fission-skia-fixture",
    ) -> tuple[Path, Path, str]:
        manifest = {
            "schema_version": 1,
            "matrix_revision": "fixture-matrix-v1",
            "frozen": True,
        }
        manifest_path = temporary / "qualification-manifest.json"
        self.write_json(manifest_path, manifest)
        report = {
            "schema_version": 1,
            "matrix_revision": manifest["matrix_revision"],
            "manifest_sha256": promotion.foundation.sha256_json(manifest),
            "qualified": True,
            "issues": [],
            "comparisons": [
                {
                    "target_id": "linux-x86_64-gnu",
                    "profile_id": "skia-only",
                    "run_id": "fixture-run",
                    "identity": {"artifact_id": artifact_id},
                    "qualified": True,
                }
            ],
        }
        report_path = temporary / "qualification-report.json"
        self.write_json(report_path, report)
        return manifest_path, report_path, artifact_id

    def test_promote_reverifies_and_repackages_a_native_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            temporary = Path(raw)
            fixture = skia_fixture_module.SkiaToolTests(methodName="runTest")
            fixture.setUp()
            _, _, archive = fixture.package_fixture(temporary)
            digest = promotion.foundation.sha256_file(archive)
            packaged = promotion.foundation.verify_archive(
                archive,
                digest,
                self.config,
                "native-raster",
                "x86_64-unknown-linux-gnu",
            )
            manifest_path, report_path, _ = self.qualification_fixture(
                temporary,
                packaged["artifact_id"],
            )
            output = temporary / "qualified.tar.gz"
            args = argparse.Namespace(
                archive=str(archive),
                output=str(output),
                sha256=digest,
                kind="native",
                profile="native-raster",
                target="x86_64-unknown-linux-gnu",
                qualification_report=str(report_path),
                qualification_manifest=str(manifest_path),
                qualification_target_id="linux-x86_64-gnu",
                qualification_profile_id="skia-only",
                source_date_epoch="1",
            )
            with contextlib.redirect_stdout(io.StringIO()):
                promotion.promote_command(args, self.config)
            promoted_digest = promotion.foundation.sha256_file(output)
            with promotion.extracted_archive(
                output,
                promoted_digest,
                "native",
            ) as (root, root_name, _):
                promoted, _ = promotion.promoted_manifest(
                    root,
                    "native",
                    self.config,
                    "native-raster",
                    "x86_64-unknown-linux-gnu",
                )
            self.assertEqual(root_name, packaged["artifact_id"])
            self.assertEqual(promoted["origin"], promotion.PROMOTED_ORIGIN)
            self.assertTrue(promoted["qualified"])

    def test_qualification_report_is_bound_to_frozen_matrix_and_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            temporary = Path(raw)
            manifest_path, report_path, artifact_id = self.qualification_fixture(temporary)
            digest = promotion.qualification_report_digest(
                report_path,
                manifest_path,
                artifact_id,
                "linux-x86_64-gnu",
                "skia-only",
            )
            self.assertEqual(digest, promotion.foundation.sha256_file(report_path))

            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["comparisons"][0]["identity"]["artifact_id"] = "substituted"
            self.write_json(report_path, report)
            with self.assertRaisesRegex(promotion.PromotionError, "not bound to the artifact"):
                promotion.qualification_report_digest(
                    report_path,
                    manifest_path,
                    artifact_id,
                    "linux-x86_64-gnu",
                    "skia-only",
                )

    def test_qualification_report_rejects_a_changed_frozen_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            temporary = Path(raw)
            manifest_path, report_path, artifact_id = self.qualification_fixture(temporary)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["matrix_revision"] = "fixture-matrix-v2"
            self.write_json(manifest_path, manifest)
            with self.assertRaisesRegex(promotion.PromotionError, "matrix revision"):
                promotion.qualification_report_digest(
                    report_path,
                    manifest_path,
                    artifact_id,
                    "linux-x86_64-gnu",
                    "skia-only",
                )

    def test_attestation_verification_enforces_digest_and_trusted_timestamp(self) -> None:
        digest = "a" * 64
        record = {
            "verificationResult": {
                "statement": {
                    "predicateType": promotion.PREDICATE_TYPE,
                    "subject": [{"name": "fixture", "digest": {"sha256": digest}}],
                },
                "verifiedTimestamps": [{"type": "rekor"}],
            }
        }
        completed = subprocess.CompletedProcess(
            args=[],
            returncode=0,
            stdout=json.dumps([record]),
            stderr="",
        )
        with mock.patch.object(promotion.subprocess, "run", return_value=completed) as run:
            promotion.verify_attestation(
                Path("artifact.tar.gz"),
                digest,
                "gh",
                "b" * 40,
                promotion.DEFAULT_SIGNER_WORKFLOW,
                None,
                None,
            )
        command = run.call_args.args[0]
        self.assertIn("--deny-self-hosted-runners", command)
        self.assertEqual(command[command.index("--source-digest") + 1], "b" * 40)
        self.assertEqual(
            command[command.index("--signer-workflow") + 1],
            promotion.DEFAULT_SIGNER_WORKFLOW,
        )

        record["verificationResult"]["verifiedTimestamps"] = []
        completed.stdout = json.dumps([record])
        with mock.patch.object(promotion.subprocess, "run", return_value=completed):
            with self.assertRaisesRegex(promotion.PromotionError, "trusted timestamp"):
                promotion.verify_attestation(
                    Path("artifact.tar.gz"),
                    digest,
                    "gh",
                    "b" * 40,
                    promotion.DEFAULT_SIGNER_WORKFLOW,
                    None,
                    None,
                )

    def test_release_url_must_be_the_exact_github_asset(self) -> None:
        valid = (
            "https://github.com/fission-ui/fission/releases/download/"
            "skia-0.10.1/fission-skia.tar.gz"
        )
        self.assertEqual(
            promotion.canonical_release_url(valid, "fission-skia.tar.gz"),
            valid,
        )
        for invalid in (
            valid + "?download=1",
            valid.replace("github.com", "example.com"),
            valid.replace("fission-skia.tar.gz", "other.tar.gz"),
        ):
            with self.subTest(url=invalid):
                with self.assertRaises(promotion.PromotionError):
                    promotion.canonical_release_url(invalid, "fission-skia.tar.gz")

    def test_lock_entry_is_written_only_for_matching_release_identity(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            temporary = Path(raw)
            archive = temporary / "fission-skia.tar.gz"
            archive.write_bytes(b"qualified archive")
            lock_path = temporary / "artifacts.lock.json"
            lock = {
                "schema_version": 1,
                "fission_version": "0.10.1",
                "skia_revision": self.config["source"]["revision"],
                "bridge_abi_version": self.config["bridge"]["abi_version"],
                "provenance": {
                    "repository": promotion.REPOSITORY,
                    "predicate_type": promotion.PREDICATE_TYPE,
                },
                "artifacts": [],
            }
            self.write_json(lock_path, lock)
            manifest = {
                "artifact_id": "fission-skia-fixture",
                "fission_version": "0.10.1",
                "origin": promotion.PROMOTED_ORIGIN,
                "qualified": True,
                "skia": {"revision": self.config["source"]["revision"]},
                "bridge_abi_version": self.config["bridge"]["abi_version"],
            }
            promotion.append_lock_entry(
                lock_path,
                manifest,
                "native",
                "native-raster",
                "x86_64-unknown-linux-gnu",
                (
                    "https://github.com/fission-ui/fission/releases/download/"
                    "skia-0.10.1/fission-skia.tar.gz"
                ),
                archive,
                "c" * 64,
                "d" * 64,
            )
            written = json.loads(lock_path.read_text(encoding="utf-8"))
            self.assertEqual(len(written["artifacts"]), 1)
            entry = written["artifacts"][0]
            self.assertTrue(entry["qualified"])
            self.assertEqual(entry["archive_size"], len(b"qualified archive"))
            self.assertEqual(entry["archive_sha256"], "c" * 64)

            with self.assertRaisesRegex(promotion.PromotionError, "already contains"):
                promotion.append_lock_entry(
                    lock_path,
                    manifest,
                    "native",
                    "native-raster",
                    "x86_64-unknown-linux-gnu",
                    entry["url"],
                    archive,
                    "c" * 64,
                    "d" * 64,
                )

    def test_main_fails_closed_without_a_qualification_report(self) -> None:
        def reject(_args: object, _config: object) -> None:
            raise promotion.PromotionError("fixture rejection")

        with mock.patch.object(promotion, "parser") as parser:
            parser.return_value.parse_args.return_value = argparse.Namespace(
                config=str(TOOL_DIR / "config.json"),
                action=reject,
            )
            with contextlib.redirect_stderr(io.StringIO()) as stderr:
                self.assertEqual(promotion.main([]), 2)
        self.assertIn("fixture rejection", stderr.getvalue())


if __name__ == "__main__":
    unittest.main()
