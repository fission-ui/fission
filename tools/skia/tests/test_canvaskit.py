from __future__ import annotations

import argparse
import contextlib
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest


TOOL_DIR = Path(__file__).resolve().parents[1]
if str(TOOL_DIR) not in sys.path:
    sys.path.insert(0, str(TOOL_DIR))
TOOL = TOOL_DIR / "canvaskit.py"
SPEC = importlib.util.spec_from_file_location("fission_canvaskit_tool", TOOL)
assert SPEC is not None and SPEC.loader is not None
canvaskit = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(canvaskit)


class CanvasKitToolTests(unittest.TestCase):
    def setUp(self) -> None:
        self.config = canvaskit.load_config(TOOL_DIR / "config.json")

    @staticmethod
    def tool_identity(digit: str, version: str) -> dict[str, str]:
        return {
            "expected_sha256": digit * 64,
            "actual_sha256": digit * 64,
            "version": version,
        }

    def completed_build(
        self,
        temporary: Path,
        *,
        profile: str = canvaskit.PRODUCTION_PROFILE,
    ) -> tuple[Path, Path, Path, dict[str, object]]:
        inputs = temporary / "inputs"
        inputs.mkdir()
        javascript = inputs / "canvaskit.js"
        javascript.write_text(
            "function CanvasKitInit(options) { return Promise.resolve(options); }\n",
            encoding="utf-8",
        )
        wasm = inputs / "canvaskit.wasm"
        wasm.write_bytes(b"\0asm\x01\0\0\0fixture")
        bridge = inputs / "bridges"
        bridge.mkdir()
        bridge_sources = {
            "fission_skia_web.js": (
                "export const PROTOCOL_VERSION = 1;\n"
                "export function accept(packet) { return packet; }\n"
            ),
            "fission_skia_commands.js": (
                'import "./fission_skia_web.js";\nexport const COMMAND_VERSION = 1;\n'
            ),
            "fission_skia_executor.js": (
                'import "./fission_skia_web.js";\n'
                'import "./fission_skia_commands.js";\n'
                "export function execute(packet) { return packet; }\n"
            ),
            "fission_skia_paragraph_wire.js": "export const PARAGRAPH_VERSION = 1;\n",
            "fission_skia_paragraph_unicode.js": (
                "export function resolveParagraphDirection() { return 0; }\n"
            ),
            "fission_skia_paragraph.js": (
                'import "./fission_skia_paragraph_wire.js";\n'
                'import "./fission_skia_paragraph_unicode.js";\n'
                "export function layoutParagraph(packet) { return packet; }\n"
            ),
        }
        for name, source in bridge_sources.items():
            (bridge / name).write_text(source, encoding="utf-8")
        source = {
            "kind": "local-vendored-source",
            "qualified": False,
            "repository": self.config["source"]["repository"],
            "revision": self.config["source"]["revision"],
        }
        emsdk = {
            "kind": "local-vendored-source",
            "qualified": False,
            "repository": canvaskit.EMSDK_REPOSITORY,
            "revision": canvaskit.EMSDK_REVISION,
            "emscripten_version": canvaskit.EMSCRIPTEN_VERSION,
        }
        tools = {
            "gn": self.tool_identity("1", "gn 1"),
            "ninja": self.tool_identity("2", "1.12.1"),
            "emcc": self.tool_identity("3", "emcc 4.0.7 fixture"),
            "emxx": self.tool_identity("4", "em++ 4.0.7 fixture"),
            "emar": self.tool_identity("5", "LLVM ar fixture"),
        }
        plan = canvaskit.build_plan(
            self.config,
            source,
            emsdk,
            "emscripten-4.0.7-test",
            tools,
            profile,
        )
        receipt: dict[str, object] = {
            "schema_version": canvaskit.BUILD_RECEIPT_SCHEMA_VERSION,
            "result": "complete",
            "plan": plan,
            "plan_sha256": canvaskit.foundation.sha256_json(plan),
            "outputs": [
                canvaskit.digest_record(javascript, "canvaskit-js", "canvaskit.js"),
                canvaskit.digest_record(wasm, "canvaskit-wasm", "canvaskit.wasm"),
            ],
        }
        metadata = inputs / canvaskit.BUILD_METADATA
        metadata.write_text(
            canvaskit.foundation.canonical_json(receipt),
            encoding="utf-8",
        )
        return javascript, wasm, bridge, receipt

    def package_args(
        self,
        temporary: Path,
        *,
        output_name: str = "artifact",
        archive_name: str | None = "artifact.tar.gz",
        profile: str = canvaskit.PRODUCTION_PROFILE,
    ) -> tuple[argparse.Namespace, Path, Path | None]:
        javascript, wasm, bridge, receipt = self.completed_build(temporary, profile=profile)
        deployment = temporary / "inputs" / "deployment.json"
        deployment.write_text(
            canvaskit.foundation.canonical_json(
                {
                    "toolchain": {
                        "id": receipt["plan"]["toolchain_id"],  # type: ignore[index]
                        "compiler": receipt["plan"]["tools"]["emcc"]["version"],  # type: ignore[index]
                        "runtime_abi": "Emscripten 4.0.7 / wasm32",
                    },
                    "deployment": canvaskit.deployment_contract(profile),
                }
            ),
            encoding="utf-8",
        )
        licences: list[str] = []
        for name in self.config["profiles"][profile]["required_licenses"]:
            path = temporary / "inputs" / f"{name}-LICENSE"
            path.write_text(f"{name} licence fixture\n", encoding="utf-8")
            licences.append(f"{name}={path}")
        output = temporary / output_name
        archive = temporary / archive_name if archive_name is not None else None
        args = argparse.Namespace(
            profile=profile,
            target=canvaskit.TARGET,
            fission_version="0.11.0-test",
            build_metadata=str(temporary / "inputs" / canvaskit.BUILD_METADATA),
            canvaskit_js=str(javascript),
            canvaskit_wasm=str(wasm),
            bridge_dir=str(bridge),
            deployment_metadata=str(deployment),
            license=licences,
            output=str(output),
            archive=str(archive) if archive else None,
            source_date_epoch="1786406400",
        )
        return args, output, archive

    def package_fixture(
        self,
        temporary: Path,
        *,
        output_name: str = "artifact",
        archive_name: str | None = "artifact.tar.gz",
        profile: str = canvaskit.PRODUCTION_PROFILE,
    ) -> tuple[argparse.Namespace, Path, Path | None]:
        args, output, archive = self.package_args(
            temporary,
            output_name=output_name,
            archive_name=archive_name,
            profile=profile,
        )
        with contextlib.redirect_stdout(io.StringIO()):
            canvaskit.package_canvaskit(args, self.config)
        return args, output, archive

    def test_plans_pin_distinct_webgl_and_software_lanes(self) -> None:
        recipe = canvaskit.web_recipe(self.config, canvaskit.PRODUCTION_PROFILE)
        self.assertEqual(recipe["profile"], "canvaskit-production")
        self.assertEqual(recipe["target"], "wasm32-unknown-unknown")
        self.assertEqual(recipe["lane"], "webgl-ganesh")
        self.assertIs(recipe["qualified"], False)
        self.assertEqual(recipe["skia_revision"], self.config["source"]["revision"])
        self.assertEqual(recipe["bridge_abi_version"], self.config["bridge"]["abi_version"])
        self.assertEqual(recipe["emsdk_revision"], canvaskit.EMSDK_REVISION)
        self.assertEqual(recipe["emscripten_version"], "4.0.7")
        self.assertEqual(recipe["ninja_targets"], ["canvaskit.js"])
        self.assertEqual(recipe["outputs"], ["canvaskit.js", "canvaskit.wasm"])
        self.assertIs(recipe["gn_args"]["skia_enable_ganesh"], True)
        self.assertIs(recipe["gn_args"]["skia_enable_graphite"], False)
        self.assertIs(recipe["gn_args"]["skia_use_webgl"], True)
        self.assertIs(recipe["gn_args"]["skia_use_webgpu"], False)
        self.assertIs(recipe["gn_args"]["skia_canvaskit_enable_webgl"], True)
        self.assertIs(recipe["gn_args"]["skia_canvaskit_enable_embedded_font"], False)
        self.assertEqual(recipe["browser"]["gpu_api"], "WebGL 2")
        self.assertIs(recipe["browser"]["software_only"], False)
        self.assertEqual(recipe["browser"]["wasm_memory_policy"], canvaskit.WASM_MEMORY_POLICY)

        software = canvaskit.web_recipe(self.config, canvaskit.SOFTWARE_PROFILE)
        self.assertEqual(software["lane"], "software-raster")
        self.assertIs(software["gn_args"]["skia_enable_ganesh"], False)
        self.assertIs(software["gn_args"]["skia_use_webgl"], False)
        self.assertIs(software["gn_args"]["skia_canvaskit_enable_webgl"], False)
        self.assertIs(software["gn_args"]["skia_enable_graphite"], False)
        self.assertIs(software["gn_args"]["skia_use_webgpu"], False)
        self.assertEqual(software["browser"]["graphics_backend"], "Skia Raster")
        self.assertEqual(software["browser"]["gpu_api"], "none")
        self.assertIs(software["browser"]["software_only"], True)
        for profile in canvaskit.CANVASKIT_PROFILES:
            self.assertIs(self.config["profiles"][profile]["qualified"], False)
            self.assertEqual(self.config["profiles"][profile]["build_recipe"], "available")
            self.assertEqual(
                self.config["profiles"][profile]["required_licenses"],
                canvaskit.CANVASKIT_REQUIRED_LICENSES,
            )
            self.assertEqual(self.config["profiles"][profile]["layout"], canvaskit.expected_layout())

    def test_package_and_verify_bind_every_browser_asset_for_both_profiles(self) -> None:
        for profile in canvaskit.CANVASKIT_PROFILES:
            with self.subTest(profile=profile), tempfile.TemporaryDirectory() as raw:
                _, output, archive = self.package_fixture(Path(raw), profile=profile)
                self.assertIsNotNone(archive)
                manifest = canvaskit.verify_artifact_directory(
                    output,
                    self.config,
                    expected_profile=profile,
                    expected_target=canvaskit.TARGET,
                )
                self.assertFalse(manifest["qualified"])
                self.assertEqual(manifest["origin"], "local-build")
                self.assertEqual(manifest["abi"]["web_protocol_version"], 1)
                self.assertEqual(
                    set(manifest["assets"]),
                    {"canvaskit_js", "canvaskit_wasm", *canvaskit.BRIDGE_ASSETS},
                )
                digest = canvaskit.foundation.sha256_file(archive)
                archived = canvaskit.verify_archive(
                    archive,
                    digest,
                    self.config,
                    profile,
                    canvaskit.TARGET,
                )
                self.assertEqual(archived["artifact_id"], manifest["artifact_id"])

    def test_archives_are_byte_reproducible(self) -> None:
        with tempfile.TemporaryDirectory() as first_raw, tempfile.TemporaryDirectory() as second_raw:
            _, _, first = self.package_fixture(Path(first_raw))
            _, _, second = self.package_fixture(Path(second_raw))
            self.assertIsNotNone(first)
            self.assertIsNotNone(second)
            self.assertEqual(first.read_bytes(), second.read_bytes())
            self.assertEqual(
                first.with_suffix(first.suffix + ".sha256").read_text(encoding="ascii").split()[0],
                hashlib.sha256(first.read_bytes()).hexdigest(),
            )

    def test_packaging_rejects_a_substituted_build_output(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            temporary = Path(raw)
            args, _, _ = self.package_args(temporary)
            Path(args.canvaskit_js).write_text("substituted\n", encoding="utf-8")
            with self.assertRaisesRegex(
                canvaskit.foundation.SkiaToolError,
                "does not match the completed build receipt",
            ):
                canvaskit.package_canvaskit(args, self.config)

    def test_packaging_rejects_an_invalid_wire_protocol_export(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            temporary = Path(raw)
            args, _, _ = self.package_args(temporary)
            (Path(args.bridge_dir) / "fission_skia_web.js").write_text(
                "export const PROTOCOL_VERSION = 0;\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(
                canvaskit.foundation.SkiaToolError,
                "protocol version must be positive",
            ):
                canvaskit.package_canvaskit(args, self.config)

    def test_packaging_rejects_an_incomplete_bridge_bundle(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            temporary = Path(raw)
            args, _, _ = self.package_args(temporary)
            (Path(args.bridge_dir) / "fission_skia_executor.js").unlink()
            with self.assertRaisesRegex(
                canvaskit.foundation.SkiaToolError,
                "fission_skia_executor.js.*does not exist",
            ):
                canvaskit.package_canvaskit(args, self.config)

    def test_packaging_rejects_a_bridge_with_a_missing_runtime_import(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            temporary = Path(raw)
            args, _, _ = self.package_args(temporary)
            (Path(args.bridge_dir) / "fission_skia_paragraph.js").write_text(
                "export function layoutParagraph(packet) { return packet; }\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(
                canvaskit.foundation.SkiaToolError,
                "does not import required module",
            ):
                canvaskit.package_canvaskit(args, self.config)

    def test_verifier_rejects_payload_tampering_and_extra_files(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            _, output, _ = self.package_fixture(Path(raw), archive_name=None)
            (output / "web" / "canvaskit.js").write_text("tampered\n", encoding="utf-8")
            with self.assertRaisesRegex(
                canvaskit.foundation.SkiaToolError,
                "does not match manifest",
            ):
                canvaskit.verify_artifact_directory(output, self.config)

        with tempfile.TemporaryDirectory() as raw:
            _, output, _ = self.package_fixture(Path(raw), archive_name=None)
            (output / "web" / "undeclared.js").write_text("extra\n", encoding="utf-8")
            with self.assertRaisesRegex(
                canvaskit.foundation.SkiaToolError,
                "undeclared files",
            ):
                canvaskit.verify_artifact_directory(output, self.config)

    def test_verifier_rejects_qualification_claim_and_symlink(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            _, output, _ = self.package_fixture(Path(raw), archive_name=None)
            manifest_path = output / "manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["qualified"] = True
            manifest_path.write_text(
                canvaskit.foundation.canonical_json(manifest),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(
                canvaskit.foundation.SkiaToolError,
                "local, unqualified",
            ):
                canvaskit.verify_artifact_directory(output, self.config)

        with tempfile.TemporaryDirectory() as raw:
            _, output, _ = self.package_fixture(Path(raw), archive_name=None)
            (output / "web" / "alias.js").symlink_to(output / "web" / "canvaskit.js")
            with self.assertRaisesRegex(
                canvaskit.foundation.SkiaToolError,
                "symbolic links",
            ):
                canvaskit.verify_artifact_directory(output, self.config)

    def test_build_receipt_cannot_relabel_the_recipe(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            _, _, _, receipt = self.completed_build(Path(raw))
            receipt["plan"]["recipe"]["gn_args"]["skia_use_webgl"] = False  # type: ignore[index]
            receipt["plan_sha256"] = canvaskit.foundation.sha256_json(receipt["plan"])
            with self.assertRaisesRegex(
                canvaskit.foundation.SkiaToolError,
                "does not match pinned profile",
            ):
                canvaskit.validate_build_receipt(receipt, self.config)

    def test_build_receipt_cannot_cross_profile_boundaries(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            _, _, _, receipt = self.completed_build(
                Path(raw),
                profile=canvaskit.SOFTWARE_PROFILE,
            )
            with self.assertRaisesRegex(
                canvaskit.foundation.SkiaToolError,
                "profile mismatch",
            ):
                canvaskit.validate_build_receipt(
                    receipt,
                    self.config,
                    expected_profile=canvaskit.PRODUCTION_PROFILE,
                )

    def test_archive_rejects_parent_traversal_before_extraction(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            temporary = Path(raw)
            archive = temporary / "unsafe.tar.gz"
            with tarfile.open(archive, "w:gz") as output:
                manifest = b"{}\n"
                valid = tarfile.TarInfo("artifact/manifest.json")
                valid.size = len(manifest)
                output.addfile(valid, io.BytesIO(manifest))
                bad = tarfile.TarInfo("artifact/../escape")
                bad.size = 1
                output.addfile(bad, io.BytesIO(b"x"))
            digest = canvaskit.foundation.sha256_file(archive)
            with self.assertRaisesRegex(
                canvaskit.foundation.SkiaToolError,
                "unsafe or non-canonical path",
            ):
                canvaskit.verify_archive(archive, digest, self.config, None, None)
            self.assertFalse((temporary / "escape").exists())

    def test_artifact_size_is_bounded_before_manifest_parsing(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw) / "artifact"
            for name in ("licenses", "web"):
                (root / name).mkdir(parents=True, exist_ok=True)
            (root / "manifest.json").write_text("{}\n", encoding="utf-8")
            with (root / "web" / "canvaskit.wasm").open("wb") as output:
                output.truncate(canvaskit.MAX_SINGLE_FILE_BYTES + 1)
            with self.assertRaisesRegex(
                canvaskit.foundation.SkiaToolError,
                "file is too large",
            ):
                canvaskit.verify_artifact_directory(root, self.config)

    def test_build_uses_only_prepared_checkouts_and_pinned_tools(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            temporary = Path(raw)
            source = temporary / "skia"
            emsdk = temporary / "emsdk"
            (source / "bin").mkdir(parents=True)
            (source / "third_party" / "ninja").mkdir(parents=True)
            (emsdk / "upstream" / "emscripten").mkdir(parents=True)
            (source / canvaskit.foundation.SOURCE_RECEIPT).write_text(
                self.config["source"]["revision"] + "\n",
                encoding="utf-8",
            )
            (source / self.config["source"]["license_file"]).write_text(
                "Skia fixture licence\n",
                encoding="utf-8",
            )
            (emsdk / canvaskit.EMSDK_RECEIPT).write_text(
                canvaskit.EMSDK_REVISION + "\n",
                encoding="utf-8",
            )

            gn = source / "bin" / "gn"
            gn.write_text(
                "#!/bin/sh\n"
                "if [ \"$1\" = \"--version\" ]; then echo 'gn fixture'; exit 0; fi\n"
                "printf '%s\\n' \"$3\" > \"$2/observed-gn-args.txt\"\n",
                encoding="utf-8",
            )
            ninja = source / "third_party" / "ninja" / "ninja"
            ninja.write_text(
                "#!/bin/sh\n"
                "if [ \"$1\" = \"--version\" ]; then echo 'ninja fixture'; exit 0; fi\n"
                "printf 'function CanvasKitInit() {}\\n' > \"$2/canvaskit.js\"\n"
                "printf '\\000asm\\001\\000\\000\\000fixture' > \"$2/canvaskit.wasm\"\n",
                encoding="utf-8",
            )
            em_tools: dict[str, Path] = {}
            for name, version in (
                ("emcc", "emcc 4.0.7 fixture"),
                ("em++", "em++ 4.0.7 fixture"),
                ("emar", "LLVM ar fixture"),
            ):
                path = emsdk / "upstream" / "emscripten" / name
                path.write_text(f"#!/bin/sh\necho '{version}'\n", encoding="utf-8")
                em_tools[name] = path
            for path in (gn, ninja, *em_tools.values()):
                path.chmod(0o755)

            for profile in canvaskit.CANVASKIT_PROFILES:
                with self.subTest(profile=profile):
                    build = temporary / profile
                    args = argparse.Namespace(
                        source_dir=str(source),
                        emsdk_dir=str(emsdk),
                        build_dir=str(build),
                        profile=profile,
                        toolchain_id="fixture-emscripten",
                        gn=None,
                        ninja=None,
                        gn_sha256=canvaskit.foundation.sha256_file(gn),
                        ninja_sha256=canvaskit.foundation.sha256_file(ninja),
                        emcc_sha256=canvaskit.foundation.sha256_file(em_tools["emcc"]),
                        emxx_sha256=canvaskit.foundation.sha256_file(em_tools["em++"]),
                        emar_sha256=canvaskit.foundation.sha256_file(em_tools["emar"]),
                    )
                    with contextlib.redirect_stdout(io.StringIO()):
                        canvaskit.build_canvaskit(args, self.config)
                        canvaskit.build_canvaskit(args, self.config)
                    receipt = canvaskit.validate_build_receipt(
                        json.loads(
                            (build / canvaskit.BUILD_METADATA).read_text(encoding="utf-8")
                        ),
                        self.config,
                        expected_profile=profile,
                    )
                    self.assertEqual(
                        receipt["plan"]["emsdk"]["revision"],
                        canvaskit.EMSDK_REVISION,
                    )
                    self.assertEqual(receipt["plan"]["recipe"]["profile"], profile)
                    self.assertIs(
                        receipt["plan"]["recipe"]["gn_args"][
                            "skia_enable_fontmgr_custom_embedded"
                        ],
                        True,
                    )
                    expected_webgl = profile == canvaskit.PRODUCTION_PROFILE
                    self.assertIs(
                        receipt["plan"]["recipe"]["gn_args"]["skia_use_webgl"],
                        expected_webgl,
                    )
                    observed = (build / "observed-gn-args.txt").read_text(encoding="utf-8")
                    self.assertIn(f"skia_use_webgl={str(expected_webgl).lower()}", observed)
                    self.assertIn(
                        f"skia_enable_ganesh={str(expected_webgl).lower()}",
                        observed,
                    )
                    self.assertEqual(
                        [entry["path"] for entry in receipt["outputs"]],
                        ["canvaskit.js", "canvaskit.wasm"],
                    )


if __name__ == "__main__":
    unittest.main()
