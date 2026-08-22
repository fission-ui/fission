from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock


TOOL_DIR = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("fission_skia_ci_tool", TOOL_DIR / "ci.py")
assert SPEC is not None and SPEC.loader is not None
ci = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ci)


class SkiaCiTests(unittest.TestCase):
    def setUp(self) -> None:
        self.config = ci.foundation.load_config(TOOL_DIR / "config.json")
        self.qualification_manifest = (
            TOOL_DIR.parent / "backend-qualification/qualification-manifest.json"
        )

    def test_matrix_is_derived_from_every_frozen_release_target(self) -> None:
        native = ci.native_matrix(self.config, self.qualification_manifest)
        web = ci.web_matrix(self.config, self.qualification_manifest)

        self.assertEqual(len(native), 10)
        self.assertEqual(len(web), 2)
        self.assertEqual(
            {row["qualification_target_id"] for row in native},
            {
                "linux-x86_64-gnu",
                "macos-aarch64",
                "windows-x86_64-msvc",
                "android-aarch64",
                "ios-aarch64",
            },
        )
        for target_id in {row["qualification_target_id"] for row in native}:
            rows = [row for row in native if row["qualification_target_id"] == target_id]
            self.assertEqual(
                {(row["profile"], row["qualification_profile_id"]) for row in rows},
                {
                    ("native-raster", "standalone-software"),
                    ("native-ganesh", "skia-only"),
                },
            )
        self.assertEqual(
            {(row["profile"], row["qualification_profile_id"]) for row in web},
            {
                ("canvaskit-production", "skia-only"),
                ("canvaskit-software-qualification", "standalone-software"),
            },
        )
        self.assertTrue(all(row["qualification_target_id"] == "web-chromium" for row in web))

    def test_expected_set_contains_every_matrix_artifact_once(self) -> None:
        keys = ci.expected_artifact_keys(self.config, self.qualification_manifest)
        self.assertEqual(len(keys), 12)
        self.assertIn(
            ("native", "native-ganesh", "aarch64-linux-android"),
            keys,
        )
        self.assertIn(
            ("canvaskit", "canvaskit-production", "wasm32-unknown-unknown"),
            keys,
        )

    def test_every_native_ci_recipe_binds_packaging_inputs(self) -> None:
        for row in ci.native_matrix(self.config, self.qualification_manifest):
            overrides: dict[str, object] = {}
            if row["platform"] == "Android":
                overrides = {"ndk": "/pinned/ndk", "ndk_api": 24}
            elif row["platform"] == "iOS":
                overrides = {"ios_min_target": "13.0"}
            recipe = ci.foundation.resolve_build_plan(
                self.config,
                row["profile"],
                row["target"],
                overrides,
            )
            with self.subTest(profile=row["profile"], target=row["target"]):
                self.assertIsInstance(recipe["required_licenses"], list)
                self.assertIsInstance(recipe["system_libraries"], list)
                self.assertIsInstance(recipe["frameworks"], list)

    def test_key_value_parser_rejects_ambiguous_metadata(self) -> None:
        self.assertEqual(
            ci.parse_key_values(["libc=glibc", "libc_version=2.39"], "fixture"),
            {"libc": "glibc", "libc_version": "2.39"},
        )
        for values in (["missing-separator"], ["name="], ["a=one", "a=two"]):
            with self.subTest(values=values):
                with self.assertRaises(ci.CiError):
                    ci.parse_key_values(values, "fixture")

    def test_license_resolution_accepts_only_one_reviewed_path(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            source = Path(raw)
            first = source / ci.LICENSE_CANDIDATES["expat"][0]
            first.parent.mkdir(parents=True)
            first.write_text("licence", encoding="utf-8")
            self.assertEqual(ci.resolve_license("expat", source), first)

            second = source / ci.LICENSE_CANDIDATES["expat"][1]
            second.write_text("second", encoding="utf-8")
            with self.assertRaisesRegex(ci.CiError, "exactly one"):
                ci.resolve_license("expat", source)

    def test_native_target_licenses_require_explicit_external_notice(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            notice = root / "NOTICE"
            notice.write_text("Android cpu features notice", encoding="utf-8")
            profile = self.config["profiles"]["native-raster"]
            target = self.config["targets"]["aarch64-linux-android"]
            required = ci.foundation.required_native_licenses(profile, target)
            with mock.patch.object(ci, "resolve_license", return_value=root / "common"):
                arguments = ci.license_arguments(
                    required,
                    root,
                    {"cpu-features": notice},
                )
            self.assertIn(f"cpu-features={notice}", arguments)

    def test_native_profile_target_licenses_follow_the_selected_gpu_recipe(self) -> None:
        profile = self.config["profiles"]["native-ganesh"]
        linux = self.config["targets"]["x86_64-unknown-linux-gnu"]
        macos = self.config["targets"]["aarch64-apple-darwin"]
        linux_recipe = ci.foundation.select_profile_target_recipe(
            profile, "native-ganesh", "x86_64-unknown-linux-gnu"
        )
        macos_recipe = ci.foundation.select_profile_target_recipe(
            profile, "native-ganesh", "aarch64-apple-darwin"
        )

        linux_licenses = ci.foundation.required_native_licenses(
            profile, linux, linux_recipe
        )
        macos_licenses = ci.foundation.required_native_licenses(
            profile, macos, macos_recipe
        )

        self.assertIn("vulkan-headers", linux_licenses)
        self.assertIn("vulkan-memory-allocator", linux_licenses)
        self.assertNotIn("vulkan-headers", macos_licenses)
        self.assertNotIn("vulkan-memory-allocator", macos_licenses)

    def test_provenance_set_verifies_every_archive_against_one_source(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            temporary = Path(raw)
            index_path = temporary / "index.json"
            index_path.write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "artifacts": [
                            {
                                "archive_path": str(temporary / "first.tar.gz"),
                                "archive_sha256": "a" * 64,
                            },
                            {
                                "archive_path": str(temporary / "second.tar.gz"),
                                "archive_sha256": "b" * 64,
                            },
                        ],
                    }
                ),
                encoding="utf-8",
            )
            args = mock.Mock(index=str(index_path), source_digest="c" * 40)
            with mock.patch.object(ci.promotion, "verify_attestation") as verify:
                ci.verify_provenance_set_command(args, self.config)
            self.assertEqual(verify.call_count, 2)
            self.assertEqual(
                {call.args[2] for call in verify.call_args_list},
                {"c" * 40},
            )

    def test_evidence_collection_does_not_recurse_into_reports(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            (root / "run.json").write_text("{}", encoding="utf-8")
            nested = root / "nested"
            nested.mkdir()
            (nested / "report.json").write_text("{}", encoding="utf-8")
            self.assertEqual(ci.evidence_paths(root), [root / "run.json"])

    def test_partial_artifact_set_is_not_promotable(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            descriptor = {
                "schema_version": 1,
                "kind": "native",
                "profile": "native-raster",
                "target": "x86_64-unknown-linux-gnu",
                "artifact_id": "fixture",
                "archive": "fixture.tar.gz",
                "archive_sha256": "a" * 64,
                "qualification_target_id": "linux-x86_64-gnu",
                "qualification_profile_id": "standalone-software",
            }
            (root / "fixture.artifact.json").write_text(
                json.dumps(descriptor), encoding="utf-8"
            )
            (root / "fixture.tar.gz").write_bytes(b"fixture")
            with mock.patch.object(
                ci,
                "descriptor_for_archive",
                return_value=descriptor,
            ):
                with self.assertRaisesRegex(ci.CiError, "complete release matrix"):
                    ci.verify_set(root, self.config, self.qualification_manifest)

    def test_workflow_keeps_build_qualification_and_promotion_separate(self) -> None:
        workflow = (
            ci.REPOSITORY_ROOT / ".github/workflows/skia-artifacts.yml"
        ).read_text(encoding="utf-8")
        self.assertEqual(
            ci.promotion.DEFAULT_SIGNER_WORKFLOW,
            "github.com/fission-ui/fission/.github/workflows/skia-artifacts.yml",
        )
        self.assertIn("- build\n", workflow)
        self.assertIn("- qualify\n", workflow)
        self.assertIn("- promote\n", workflow)
        self.assertIn("uses: actions/attest@v4", workflow)
        self.assertIn("tools/backend-qualification/collect.py", workflow)
        self.assertIn("cargo test --locked -p fission-skia-artifacts", workflow)
        self.assertIn("--no-default-features --features test-shim", workflow)
        self.assertIn("web/protocol_fixture.mjs", workflow)
        self.assertIn("web/paragraph_fixture.mjs", workflow)
        self.assertIn("web/executor_fixture.mjs", workflow)
        self.assertNotIn("continue-on-error", workflow)


if __name__ == "__main__":
    unittest.main()
