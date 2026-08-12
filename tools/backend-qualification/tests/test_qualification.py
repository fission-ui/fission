from __future__ import annotations

import argparse
import copy
import hashlib
import importlib.util
from pathlib import Path
import tempfile
import unittest


TOOL = Path(__file__).resolve().parents[1] / "qualification.py"
CHECKED_IN_MANIFEST = TOOL.parent / "qualification-manifest.json"
SPEC = importlib.util.spec_from_file_location("fission_backend_qualification", TOOL)
assert SPEC is not None and SPEC.loader is not None
qualification = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(qualification)


class QualificationTests(unittest.TestCase):
    def complete_manifest(self, ceiling: float = 100.0) -> dict:
        manifest = qualification.manifest_template()
        manifest["budget_revision"] = "qualification-budget-v1"
        for target in manifest["targets"]:
            target_id = target["id"]
            manifest["environment_ids"][target_id] = f"environment-{target_id}"
            manifest["budgets"][target_id] = {
                "latency_ms": {
                    metric: {name: ceiling for name in qualification.PERCENTILES}
                    for metric in qualification.LATENCIES
                },
                "memory_bytes": {
                    metric: ceiling for metric in qualification.MEMORY
                },
                "size_bytes": {"raw": ceiling, "compressed": ceiling},
            }
        for workload in manifest["workloads"]:
            workload_id = workload["id"]
            manifest["workload_input_ids"][workload_id] = f"input-{workload_id}-v1"
        for target in manifest["targets"]:
            for profile in manifest["profiles"]:
                key = qualification.pair_key(target["id"], profile["id"])
                artifact_id = f"artifact-{target['id']}-{profile['id']}"
                manifest["identities"][key].update(
                    {
                        "build_id": f"build-{target['id']}-{profile['id']}",
                        "toolchain_id": f"toolchain-{target['id']}",
                        "artifact_id": artifact_id,
                        "artifact_sha256": hashlib.sha256(
                            artifact_id.encode("utf-8")
                        ).hexdigest(),
                    }
                )
        return manifest

    @staticmethod
    def passing_outcome(identifier: str) -> dict[str, str]:
        return {"status": "pass", "evidence_id": identifier}

    def run_evidence(
        self,
        manifest: dict,
        target_id: str,
        profile_id: str,
        *,
        samples: list[float] | None = None,
    ) -> dict:
        samples = list(samples or [1.0, 2.0, 3.0, 4.0])
        target = next(value for value in manifest["targets"] if value["id"] == target_id)
        identity = manifest["identities"][qualification.pair_key(target_id, profile_id)]
        workflows = {
            name: self.passing_outcome(f"workflow-{name}-{target_id}-{profile_id}")
            for name in qualification.WORKFLOWS
        }
        workloads = {}
        for workload in manifest["workloads"]:
            workload_id = workload["id"]
            workloads[workload_id] = {
                "input_id": manifest["workload_input_ids"][workload_id],
                "samples_ms": {
                    metric: list(samples) for metric in qualification.LATENCIES
                },
                "memory_bytes": {
                    "cold_bytes": 10,
                    "warm_bytes": 20,
                    "peak_bytes": 30,
                    "gpu_bytes": 5,
                },
                "suites": {
                    "semantic": self.passing_outcome(
                        f"semantic-{workload_id}-{target_id}-{profile_id}"
                    ),
                    "visual": self.passing_outcome(
                        f"visual-{workload_id}-{target_id}-{profile_id}"
                    ),
                },
            }
        if target["kind"] == "native":
            sizes = {
                "native_raw": 80,
                "native_compressed": 40,
                "web_raw": None,
                "web_compressed": None,
            }
        else:
            sizes = {
                "native_raw": None,
                "native_compressed": None,
                "web_raw": 80,
                "web_compressed": 40,
            }
        return {
            "schema_version": qualification.SCHEMA_VERSION,
            "matrix_revision": qualification.MATRIX_REVISION,
            "run_id": f"run-{target_id}-{profile_id}",
            "identity": {
                "target_id": target_id,
                "profile_id": profile_id,
                "environment_id": manifest["environment_ids"][target_id],
                "backend_ids": list(identity["backend_ids"]),
                "build_id": identity["build_id"],
                "toolchain_id": identity["toolchain_id"],
                "artifact_id": identity["artifact_id"],
                "artifact_sha256": identity["artifact_sha256"],
            },
            "workflows": workflows,
            "size_bytes": sizes,
            "workloads": workloads,
        }

    def all_runs(self, manifest: dict) -> list[dict]:
        return [
            self.run_evidence(manifest, target["id"], profile["id"])
            for target in manifest["targets"]
            for profile in manifest["profiles"]
        ]

    def test_percentiles_use_deterministic_linear_interpolation(self) -> None:
        values = [4.0, 1.0, 3.0, 2.0]
        self.assertEqual(
            qualification.percentiles(values),
            {"p50": 2.5, "p95": 3.85, "p99": 3.97},
        )
        self.assertEqual(qualification.percentiles([7.0]), {"p50": 7.0, "p95": 7.0, "p99": 7.0})

    def test_template_defines_the_complete_required_matrix_without_budgets(self) -> None:
        manifest = qualification.manifest_template()
        qualification.validate_manifest(manifest)
        self.assertEqual(len(manifest["targets"]), 8)
        self.assertEqual(len(manifest["profiles"]), 5)
        self.assertEqual(len(manifest["workloads"]), 22)
        self.assertEqual(
            {target["browser"] for target in manifest["targets"] if target["kind"] == "web"},
            {"Chromium", "Firefox", "WebKit"},
        )
        self.assertTrue(all(value is None for value in manifest["budgets"].values()))
        self.assertTrue(
            all(
                identity["artifact_sha256"] is None
                for identity in manifest["identities"].values()
            )
        )
        standalone = next(
            value for value in manifest["profiles"] if value["id"] == "standalone-software"
        )
        self.assertEqual(standalone["backend_ids"], ["fission-render-skia"])
        report = qualification.build_report(manifest, [])
        self.assertFalse(report["qualified"])
        self.assertIn("missing-budget", {value["code"] for value in report["issues"]})
        self.assertEqual(
            qualification.markdown_report(report).splitlines()[2],
            "Overall: **UNQUALIFIED**",
        )
        with tempfile.TemporaryDirectory() as raw:
            temporary = Path(raw)
            manifest_path = temporary / "manifest.json"
            machine_path = temporary / "report.json"
            markdown_path = temporary / "report.md"
            manifest_path.write_text(qualification.canonical_json(manifest), encoding="utf-8")
            exit_code = qualification.report_command(
                argparse.Namespace(
                    manifest=str(manifest_path),
                    evidence=[],
                    json_output=str(machine_path),
                    markdown_output=str(markdown_path),
                )
            )
            self.assertEqual(exit_code, 1)
            self.assertIn('"qualified": false', machine_path.read_text(encoding="utf-8"))
            self.assertIn("**UNQUALIFIED**", markdown_path.read_text(encoding="utf-8"))

    def test_checked_in_manifest_is_canonical_and_covers_the_required_matrix(self) -> None:
        raw = CHECKED_IN_MANIFEST.read_text(encoding="utf-8")
        manifest = qualification.load_json(CHECKED_IN_MANIFEST)
        qualification.validate_manifest(manifest)
        self.assertEqual(raw, qualification.canonical_json(manifest))
        self.assertEqual(
            {target["id"] for target in manifest["targets"]},
            {target["id"] for target in qualification.REQUIRED_TARGETS},
        )
        self.assertEqual(
            {profile["id"] for profile in manifest["profiles"]},
            {profile["id"] for profile in qualification.REQUIRED_PROFILES},
        )

    def test_manifest_readiness_requires_every_explicit_frozen_value(self) -> None:
        draft = qualification.manifest_readiness_report(qualification.manifest_template())
        self.assertFalse(draft["ready"])
        self.assertEqual(
            {
                "missing-budget-revision",
                "missing-environment",
                "missing-workload-input",
                "missing-frozen-identity",
                "missing-artifact-digest",
                "missing-budget",
            },
            {blocker["code"] for blocker in draft["blockers"]},
        )

        complete = qualification.manifest_readiness_report(self.complete_manifest())
        self.assertTrue(complete["ready"])
        self.assertEqual(complete["blockers"], [])

    def test_check_manifest_command_is_fail_closed_and_writes_blockers(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            temporary = Path(raw)
            manifest_path = temporary / "manifest.json"
            report_path = temporary / "manifest-readiness.json"
            manifest_path.write_text(
                qualification.canonical_json(qualification.manifest_template()),
                encoding="utf-8",
            )
            exit_code = qualification.check_manifest_command(
                argparse.Namespace(
                    manifest=str(manifest_path),
                    json_output=str(report_path),
                )
            )
            report = qualification.load_json(report_path)
            self.assertEqual(exit_code, 1)
            self.assertFalse(report["ready"])
            self.assertGreater(report["blocker_count"], 0)

    def test_narrowing_any_required_matrix_axis_is_rejected(self) -> None:
        cases = (
            ("targets", "web-webkit"),
            ("profiles", "standalone-software"),
            ("workloads", "external-surface"),
        )
        for axis, identifier in cases:
            with self.subTest(axis=axis):
                manifest = qualification.manifest_template()
                manifest[axis] = [value for value in manifest[axis] if value["id"] != identifier]
                with self.assertRaisesRegex(
                    qualification.QualificationError,
                    "narrows the required matrix",
                ):
                    qualification.validate_manifest(manifest)

    def test_missing_run_or_measurement_is_unqualified(self) -> None:
        manifest = self.complete_manifest()
        only = self.run_evidence(manifest, "linux-x86_64-gnu", "skia-only")
        del only["workloads"]["scroll"]["samples_ms"]["frame_ms"]
        report = qualification.build_report(manifest, [only])
        self.assertFalse(report["qualified"])
        codes = {value["code"] for value in report["issues"]}
        self.assertIn("missing-evidence", codes)
        self.assertIn("missing-measurement", codes)

    def test_budget_failure_blocks_an_otherwise_complete_matrix(self) -> None:
        manifest = self.complete_manifest(ceiling=100.0)
        runs = self.all_runs(manifest)
        failing = next(
            value
            for value in runs
            if value["identity"]["target_id"] == "web-firefox"
            and value["identity"]["profile_id"] == "skia-only"
        )
        failing["workloads"]["scroll"]["samples_ms"]["frame_ms"] = [200.0]
        report = qualification.build_report(manifest, runs)
        self.assertFalse(report["qualified"])
        failures = [
            value
            for value in report["issues"]
            if value["code"] == "budget-exceeded"
        ]
        self.assertTrue(
            any(
                value["scope"] == "web-firefox::skia-only.scroll"
                and "frame_ms.p95" in value["message"]
                for value in failures
            )
        )

    def test_diagnostic_profile_records_but_does_not_apply_budgets(self) -> None:
        manifest = self.complete_manifest(ceiling=1.0)
        run = self.run_evidence(
            manifest,
            "linux-x86_64-gnu",
            "both-backend-diagnostic",
            samples=[500.0],
        )
        run["workloads"]["scroll"]["memory_bytes"]["peak_bytes"] = 500
        report = qualification.build_report(manifest, [run])
        pair = next(
            value
            for value in report["comparisons"]
            if value["target_id"] == "linux-x86_64-gnu"
            and value["profile_id"] == "both-backend-diagnostic"
        )
        self.assertFalse(pair["comparison_eligible"])
        self.assertEqual(pair["budget_failures"], [])
        self.assertNotIn(
            "budget-exceeded",
            {
                value["code"]
                for value in report["issues"]
                if value["scope"].startswith(
                    "linux-x86_64-gnu::both-backend-diagnostic"
                )
            },
        )

    def test_unrevisioned_budget_is_never_used_for_comparison(self) -> None:
        manifest = self.complete_manifest(ceiling=1.0)
        manifest["budget_revision"] = None
        run = self.run_evidence(
            manifest,
            "linux-x86_64-gnu",
            "skia-only",
            samples=[500.0],
        )
        report = qualification.build_report(manifest, [run])
        pair = next(
            value
            for value in report["comparisons"]
            if value["target_id"] == "linux-x86_64-gnu"
            and value["profile_id"] == "skia-only"
        )
        self.assertFalse(pair["budget_applied"])
        self.assertIn("missing-budget-revision", {value["code"] for value in report["issues"]})
        self.assertNotIn("budget-exceeded", {value["code"] for value in report["issues"]})

    def test_backend_build_toolchain_and_artifact_identity_are_exact(self) -> None:
        manifest = self.complete_manifest()
        run = self.run_evidence(manifest, "macos-aarch64", "skia-only")
        run["identity"]["backend_ids"] = ["fission-render-vello"]
        run["identity"]["build_id"] = "different-build"
        run["identity"]["toolchain_id"] = "different-toolchain"
        run["identity"]["artifact_id"] = "different-artifact"
        run["identity"]["artifact_sha256"] = "f" * 64
        report = qualification.build_report(manifest, [run])
        mismatches = [
            value
            for value in report["issues"]
            if value["scope"] == "macos-aarch64::skia-only"
            and value["code"] == "identity-mismatch"
        ]
        self.assertEqual(len(mismatches), 5)
        self.assertFalse(report["qualified"])

    def test_artifact_digest_requires_exact_lowercase_sha256(self) -> None:
        for digest in ("a" * 63, "A" * 64, "g" * 64, "sha256:" + "a" * 64):
            with self.subTest(digest=digest):
                manifest = self.complete_manifest()
                key = qualification.pair_key("linux-x86_64-gnu", "skia-only")
                manifest["identities"][key]["artifact_sha256"] = digest
                with self.assertRaisesRegex(
                    qualification.QualificationError,
                    "exactly 64 lowercase hexadecimal characters",
                ):
                    qualification.validate_manifest(manifest)

    def test_evidence_cannot_omit_or_malform_artifact_digest(self) -> None:
        manifest = self.complete_manifest()
        omitted = self.run_evidence(manifest, "linux-x86_64-gnu", "skia-only")
        del omitted["identity"]["artifact_sha256"]
        omitted_report = qualification.build_report(manifest, [omitted])
        self.assertIn(
            "invalid-identity",
            {
                value["code"]
                for value in omitted_report["issues"]
                if value["scope"] == "linux-x86_64-gnu::skia-only"
            },
        )
        for digest in (None, "A" * 64, "a" * 63):
            with self.subTest(digest=digest):
                run = self.run_evidence(manifest, "linux-x86_64-gnu", "skia-only")
                run["identity"]["artifact_sha256"] = digest
                report = qualification.build_report(manifest, [run])
                pair_issues = [
                    value
                    for value in report["issues"]
                    if value["scope"] == "linux-x86_64-gnu::skia-only"
                ]
                self.assertIn(
                    "invalid-artifact-digest",
                    {value["code"] for value in pair_issues},
                )
                self.assertFalse(report["qualified"])

    def test_complete_passing_evidence_is_the_only_qualified_path(self) -> None:
        manifest = self.complete_manifest()
        report = qualification.build_report(manifest, self.all_runs(manifest))
        self.assertTrue(report["qualified"])
        self.assertEqual(report["issues"], [])
        self.assertTrue(all(value["qualified"] for value in report["comparisons"]))
        self.assertIn("Overall: **QUALIFIED**", qualification.markdown_report(report))
        native = next(value for value in report["comparisons"] if value["target_id"] == "linux-x86_64-gnu")
        web = next(value for value in report["comparisons"] if value["target_id"] == "web-chromium")
        self.assertEqual(
            native["identity"]["artifact_sha256"],
            manifest["identities"][
                qualification.pair_key(native["target_id"], native["profile_id"])
            ]["artifact_sha256"],
        )
        self.assertIn(native["identity"]["artifact_sha256"], qualification.markdown_report(report))
        self.assertEqual((native["size_bytes"]["native_raw"], native["size_bytes"]["web_raw"]), (80, None))
        self.assertEqual((web["size_bytes"]["native_raw"], web["size_bytes"]["web_raw"]), (None, 80))

    def test_required_workflow_and_visual_failures_block_qualification(self) -> None:
        manifest = self.complete_manifest()
        runs = self.all_runs(manifest)
        failing = next(
            value
            for value in runs
            if value["identity"]["target_id"] == "android-aarch64"
            and value["identity"]["profile_id"] == "skia-only"
        )
        failing["workflows"]["offline"] = {
            "status": "fail",
            "evidence_id": "offline-failure",
        }
        failing["workloads"]["accessibility-ime"]["suites"]["visual"] = {
            "status": "fail",
            "evidence_id": "visual-failure",
        }
        report = qualification.build_report(manifest, runs)
        failures = [value for value in report["issues"] if value["code"] == "required-failure"]
        self.assertEqual(len(failures), 2)
        self.assertFalse(report["qualified"])

    def test_machine_and_markdown_outputs_are_order_independent(self) -> None:
        manifest = self.complete_manifest()
        runs = self.all_runs(manifest)
        forward = qualification.build_report(manifest, runs)
        reverse = qualification.build_report(copy.deepcopy(manifest), list(reversed(runs)))
        self.assertEqual(qualification.canonical_json(forward), qualification.canonical_json(reverse))
        self.assertEqual(qualification.markdown_report(forward), qualification.markdown_report(reverse))


if __name__ == "__main__":
    unittest.main()
