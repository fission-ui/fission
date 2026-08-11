from __future__ import annotations

import argparse
import contextlib
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
from unittest import mock


TOOL = Path(__file__).resolve().parents[1] / "skia.py"
SPEC = importlib.util.spec_from_file_location("fission_skia_tool", TOOL)
assert SPEC is not None and SPEC.loader is not None
skia = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(skia)


class SkiaToolTests(unittest.TestCase):
    def setUp(self) -> None:
        self.config = skia.load_config(Path(__file__).resolve().parents[1] / "config.json")

    def fixture(self, temporary: Path) -> tuple[argparse.Namespace, Path, Path]:
        inputs = temporary / "inputs"
        inputs.mkdir()
        header = inputs / "fission_skia.h"
        header.write_text("#define FISSION_SKIA_ABI_VERSION 3u\n", encoding="utf-8")

        profile = self.config["profiles"]["native-raster"]
        target = "x86_64-unknown-linux-gnu"
        library_paths: dict[str, Path] = {}
        for name in ["fission_skia_bridge", *profile["upstream_libraries"]]:
            path = inputs / skia.canonical_library_filename(name, target)
            path.write_bytes(f"fixture:{name}".encode("ascii"))
            library_paths[name] = path

        recipe = skia.resolve_build_plan(self.config, "native-raster", target, {})
        plan = {
            "schema_version": skia.BUILD_PLAN_SCHEMA_VERSION,
            "recipe": recipe,
            "source": {
                "kind": "local-vendored-source",
                "qualified": False,
                "repository": self.config["source"]["repository"],
                "revision": self.config["source"]["revision"],
            },
            "toolchain_id": "test-clang",
            "tools": {
                "gn": {
                    "expected_sha256": "1" * 64,
                    "actual_sha256": "1" * 64,
                    "version": "gn fixture",
                },
                "ninja": {
                    "expected_sha256": "2" * 64,
                    "actual_sha256": "2" * 64,
                    "version": "ninja fixture",
                },
            },
        }
        outputs = [
            skia.regular_file_record(
                library_paths[name],
                name=name,
                relative_path=skia.canonical_library_filename(name, target),
            )
            for name in profile["upstream_libraries"]
        ]
        build = inputs / skia.BUILD_METADATA
        build.write_text(
            skia.canonical_json(
                {
                    "schema_version": skia.BUILD_RECEIPT_SCHEMA_VERSION,
                    "result": "complete",
                    "plan": plan,
                    "plan_sha256": skia.sha256_json(plan),
                    "outputs": outputs,
                }
            ),
            encoding="utf-8",
        )

        links = inputs / "links.json"
        links.write_text(
            skia.canonical_json(
                {
                    "link_search_paths": ["lib"],
                    "static_libraries": [
                        "fission_skia_bridge",
                        *profile["upstream_libraries"],
                    ],
                    "system_libraries": ["dl"],
                    "frameworks": [],
                }
            ),
            encoding="utf-8",
        )
        deployment = inputs / "deployment.json"
        deployment.write_text(
            skia.canonical_json(
                {
                    "toolchain": {
                        "id": "test-clang",
                        "compiler": "fixture compiler",
                        "runtime_abi": "fixture cxx abi",
                    },
                    "deployment": {
                        "libc": "glibc",
                        "libc_version": "fixture",
                        "cxx_runtime": "fixture",
                    },
                }
            ),
            encoding="utf-8",
        )
        licences: list[str] = []
        for name in profile["required_licenses"]:
            path = inputs / f"{name}-LICENSE"
            path.write_text(f"{name} licence fixture\n", encoding="utf-8")
            licences.append(f"{name}={path}")

        output = temporary / "artifact"
        archive = temporary / "artifact.tar.gz"
        args = argparse.Namespace(
            profile="native-raster",
            target=target,
            fission_version="0.11.0-test",
            build_metadata=str(build),
            bridge_header=str(header),
            library=[f"{name}={path}" for name, path in library_paths.items()],
            license=licences,
            link_metadata=str(links),
            deployment_metadata=str(deployment),
            output=str(output),
            archive=str(archive),
            source_date_epoch="1",
        )
        return args, output, archive

    def package_fixture(self, temporary: Path) -> tuple[argparse.Namespace, Path, Path]:
        args, output, archive = self.fixture(temporary)
        with contextlib.redirect_stdout(io.StringIO()):
            skia.package_native(args, self.config)
        return args, output, archive

    def test_pin_and_all_local_profiles_are_explicitly_unqualified(self) -> None:
        self.assertRegex(self.config["source"]["revision"], r"^[0-9a-f]{40}$")
        self.assertEqual(self.config["source"]["qualification"], "unqualified")
        self.assertEqual(self.config["bridge"]["abi_version"], 3)
        lock = json.loads(
            (Path(__file__).resolve().parents[1] / "artifacts.lock.json").read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual(lock["bridge_abi_version"], self.config["bridge"]["abi_version"])
        self.assertEqual(lock["skia_revision"], self.config["source"]["revision"])
        self.assertEqual(lock["artifacts"], [])
        self.assertEqual(self.config["profiles"]["native-raster"]["build_recipe"], "available")
        for profile in self.config["profiles"].values():
            self.assertIs(profile["qualified"], False)
            self.assertTrue({"fission", "skia"}.issubset(profile["required_licenses"]))
        self.assertEqual(
            set(self.config["profiles"]["native-raster"]["required_licenses"]),
            {
                "cpu-features",
                "expat",
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
            },
        )

    def test_source_and_tool_identities_omit_host_paths(self) -> None:
        with tempfile.TemporaryDirectory() as raw_temporary:
            source = Path(raw_temporary)
            (source / skia.SOURCE_RECEIPT).write_text(
                self.config["source"]["revision"] + "\n",
                encoding="utf-8",
            )
            identity = skia.verify_source_checkout(
                source,
                self.config["source"]["revision"],
                self.config["source"]["repository"],
            )
            self.assertNotIn("path", identity)
            self.assertNotIn("origin", identity)
            self.assertIs(identity["qualified"], False)
        executable = Path(sys.executable)
        digest = skia.sha256_file(executable)
        tool = skia.verified_tool_identities(
            executable,
            digest,
            executable,
            digest,
        )["gn"]
        self.assertEqual(
            set(tool),
            {"expected_sha256", "actual_sha256", "version"},
        )
        self.assertEqual(tool["expected_sha256"], digest)
        self.assertEqual(tool["actual_sha256"], digest)
        self.assertEqual(tool["version"], " ".join(tool["version"].split()))

    def test_both_tool_digests_are_checked_before_either_executes(self) -> None:
        executable = Path(sys.executable)
        digest = skia.sha256_file(executable)
        with mock.patch.object(skia, "run_checked") as run_checked:
            with self.assertRaisesRegex(skia.SkiaToolError, "Ninja SHA-256 mismatch"):
                skia.verified_tool_identities(
                    executable,
                    digest,
                    executable,
                    "0" * 64,
                )
        run_checked.assert_not_called()

    def test_unimplemented_profile_fails_closed(self) -> None:
        with self.assertRaisesRegex(skia.SkiaToolError, "no artifact will be fabricated"):
            skia.resolve_build_plan(
                self.config,
                "native-ganesh",
                "x86_64-unknown-linux-gnu",
                {},
            )

    def test_gn_overrides_are_closed_to_the_target_allowlist(self) -> None:
        with self.assertRaisesRegex(skia.SkiaToolError, "not allowed"):
            skia.resolve_build_plan(
                self.config,
                "native-raster",
                "x86_64-unknown-linux-gnu",
                {"arbitrary_feature": True},
            )
        android = skia.resolve_build_plan(
            self.config,
            "native-raster",
            "aarch64-linux-android",
            {"ndk": "/explicit/ndk", "ndk_api": 26},
        )
        self.assertEqual(android["gn_args"]["ndk_api"], 26)

    def test_packaging_verifies_receipts_then_detects_tampering(self) -> None:
        with tempfile.TemporaryDirectory() as raw_temporary:
            temporary = Path(raw_temporary)
            _, output, archive = self.package_fixture(temporary)
            manifest = skia.verify_artifact_directory(
                output,
                self.config,
                expected_profile="native-raster",
                expected_target="x86_64-unknown-linux-gnu",
            )
            self.assertFalse(manifest["qualified"])
            digest = skia.sha256_file(archive)
            verified = skia.verify_archive(
                archive,
                digest,
                self.config,
                "native-raster",
                "x86_64-unknown-linux-gnu",
            )
            self.assertEqual(verified["artifact_id"], manifest["artifact_id"])
            self.assertTrue(archive.with_suffix(".gz.sha256").is_file())

            (output / "lib" / "libskia.a").write_bytes(b"tampered")
            with self.assertRaisesRegex(skia.SkiaToolError, "size mismatch|digest mismatch"):
                skia.verify_artifact_directory(output, self.config)

    def test_build_receipt_schema_and_plan_digest_are_exact(self) -> None:
        with tempfile.TemporaryDirectory() as raw_temporary:
            args, _, _ = self.fixture(Path(raw_temporary))
            path = Path(args.build_metadata)
            receipt = json.loads(path.read_text(encoding="utf-8"))
            receipt["unexpected"] = True
            with self.assertRaisesRegex(skia.SkiaToolError, "unknown or missing"):
                skia.validate_build_receipt(
                    receipt,
                    self.config,
                    args.profile,
                    args.target,
                )
            receipt.pop("unexpected")
            receipt["plan_sha256"] = "0" * 64
            with self.assertRaisesRegex(skia.SkiaToolError, "plan digest"):
                skia.validate_build_receipt(
                    receipt,
                    self.config,
                    args.profile,
                    args.target,
                )

    def test_packaging_requires_exact_profile_licences(self) -> None:
        with tempfile.TemporaryDirectory() as raw_temporary:
            args, _, _ = self.fixture(Path(raw_temporary))
            args.license = [value for value in args.license if not value.startswith("fission=")]
            with self.assertRaisesRegex(skia.SkiaToolError, "licence set"):
                skia.package_native(args, self.config)

    def test_packaging_rejects_bridge_header_abi_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as raw_temporary:
            args, _, _ = self.fixture(Path(raw_temporary))
            Path(args.bridge_header).write_text(
                "#define FISSION_SKIA_ABI_VERSION 1u\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(skia.SkiaToolError, "header ABI mismatch"):
                skia.package_native(args, self.config)

    def test_nested_manifest_is_not_excluded_from_file_set(self) -> None:
        with tempfile.TemporaryDirectory() as raw_temporary:
            _, output, _ = self.package_fixture(Path(raw_temporary))
            nested = output / "metadata" / skia.MANIFEST
            nested.write_text("{}\n", encoding="utf-8")
            with self.assertRaisesRegex(skia.SkiaToolError, "undeclared"):
                skia.verify_artifact_directory(output, self.config)

    @unittest.skipUnless(hasattr(os, "symlink"), "platform has no symlink support")
    def test_artifact_directory_rejects_every_symlink(self) -> None:
        with tempfile.TemporaryDirectory() as raw_temporary:
            _, output, _ = self.package_fixture(Path(raw_temporary))
            os.symlink(output / "licenses", output / "linked-licenses", target_is_directory=True)
            with self.assertRaisesRegex(skia.SkiaToolError, "symbolic links"):
                skia.verify_artifact_directory(output, self.config)

    def test_reproducible_archive_has_stable_digest(self) -> None:
        with tempfile.TemporaryDirectory() as raw_temporary:
            temporary = Path(raw_temporary)
            root = temporary / "root"
            (root / "nested").mkdir(parents=True)
            (root / "nested" / "payload").write_bytes(b"same bytes")
            first = temporary / "first.tar.gz"
            second = temporary / "second.tar.gz"
            skia.create_reproducible_archive(root, first, "artifact", 42)
            skia.create_reproducible_archive(root, second, "artifact", 42)
            self.assertEqual(skia.sha256_file(first), skia.sha256_file(second))

    def test_archive_pair_refuses_overwrite_without_partial_output(self) -> None:
        with tempfile.TemporaryDirectory() as raw_temporary:
            temporary = Path(raw_temporary)
            root = temporary / "root"
            root.mkdir()
            (root / skia.MANIFEST).write_text("{}\n", encoding="utf-8")
            archive = temporary / "artifact.tar.gz"
            sidecar = archive.with_suffix(".gz.sha256")
            sidecar.write_text("owned\n", encoding="ascii")
            with self.assertRaisesRegex(skia.SkiaToolError, "refusing to overwrite"):
                skia.create_archive_with_sidecar(root, archive, "artifact", 1)
            self.assertFalse(archive.exists())
            self.assertEqual(sidecar.read_text(encoding="ascii"), "owned\n")

    def test_archive_pair_rolls_back_if_sidecar_publication_fails(self) -> None:
        with tempfile.TemporaryDirectory() as raw_temporary:
            temporary = Path(raw_temporary)
            root = temporary / "root"
            root.mkdir()
            (root / skia.MANIFEST).write_text("{}\n", encoding="utf-8")
            archive = temporary / "artifact.tar.gz"
            original = skia.publish_file_no_replace
            calls = 0

            def fail_second(source: Path, destination: Path) -> tuple[int, int]:
                nonlocal calls
                calls += 1
                if calls == 2:
                    raise skia.SkiaToolError("fixture sidecar failure")
                return original(source, destination)

            with mock.patch.object(skia, "publish_file_no_replace", side_effect=fail_second):
                with self.assertRaisesRegex(skia.SkiaToolError, "fixture sidecar failure"):
                    skia.create_archive_with_sidecar(root, archive, "artifact", 1)
            self.assertFalse(archive.exists())
            self.assertFalse(archive.with_suffix(".gz.sha256").exists())

    def test_archive_verification_rejects_links_before_extraction(self) -> None:
        with tempfile.TemporaryDirectory() as raw_temporary:
            archive = Path(raw_temporary) / "malicious.tar.gz"
            with tarfile.open(archive, "w:gz") as output:
                manifest = tarfile.TarInfo("artifact/manifest.json")
                payload = b"{}\n"
                manifest.size = len(payload)
                output.addfile(manifest, io.BytesIO(payload))
                link = tarfile.TarInfo("artifact/link")
                link.type = tarfile.SYMTYPE
                link.linkname = "/tmp/escape"
                output.addfile(link)
            with self.assertRaisesRegex(skia.SkiaToolError, "regular files and directories"):
                skia.verify_archive(
                    archive,
                    skia.sha256_file(archive),
                    self.config,
                    None,
                    None,
                )

    def test_archive_cannot_be_written_inside_staged_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as raw_temporary:
            output = Path(raw_temporary) / "artifact"
            output.mkdir()
            archive = output / "artifact.tar.gz"

            with self.assertRaisesRegex(skia.SkiaToolError, "archive must be outside"):
                skia.package_native(
                    argparse.Namespace(
                        profile="native-raster",
                        target="x86_64-unknown-linux-gnu",
                        fission_version="0.11.0-test",
                        build_metadata=str(output / "missing-build.json"),
                        bridge_header=str(output / "missing-header.h"),
                        library=[],
                        license=[],
                        link_metadata=str(output / "missing-links.json"),
                        deployment_metadata=str(output / "missing-deployment.json"),
                        output=str(output),
                        archive=str(archive),
                        source_date_epoch="1",
                    ),
                    self.config,
                )


if __name__ == "__main__":
    unittest.main()
