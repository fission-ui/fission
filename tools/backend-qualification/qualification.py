#!/usr/bin/env python3
"""Fail-closed backend qualification manifest, evidence, and report tooling."""

from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
import math
from pathlib import Path
import re
import sys
import tempfile
from typing import Any, Iterable, Mapping, Sequence


SCHEMA_VERSION = 1
MATRIX_REVISION = "rfc-multi-backend-phase-0-v1"
ID_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.:+/-]*$")
PERCENTILES = ("p50", "p95", "p99")
LATENCIES = ("startup_ms", "frame_ms", "input_ms", "recovery_ms")
MEMORY = ("cold_bytes", "warm_bytes", "peak_bytes", "gpu_bytes")
SIZES = ("native_raw", "native_compressed", "web_raw", "web_compressed")
WORKFLOWS = (
    "build",
    "development",
    "packaging",
    "ci",
    "release",
    "offline",
    "source-build",
    "prebuilt",
)


REQUIRED_TARGETS = (
    {
        "id": "linux-x86_64-gnu",
        "platform": "Linux",
        "target": "x86_64-unknown-linux-gnu",
        "kind": "native",
        "browser": None,
    },
    {
        "id": "macos-aarch64",
        "platform": "macOS",
        "target": "aarch64-apple-darwin",
        "kind": "native",
        "browser": None,
    },
    {
        "id": "windows-x86_64-msvc",
        "platform": "Windows",
        "target": "x86_64-pc-windows-msvc",
        "kind": "native",
        "browser": None,
    },
    {
        "id": "android-aarch64",
        "platform": "Android",
        "target": "aarch64-linux-android",
        "kind": "native",
        "browser": None,
    },
    {
        "id": "ios-aarch64",
        "platform": "iOS",
        "target": "aarch64-apple-ios",
        "kind": "native",
        "browser": None,
    },
    {
        "id": "web-chromium",
        "platform": "Web",
        "target": "wasm32-unknown-unknown",
        "kind": "web",
        "browser": "Chromium",
    },
    {
        "id": "web-firefox",
        "platform": "Web",
        "target": "wasm32-unknown-unknown",
        "kind": "web",
        "browser": "Firefox",
    },
    {
        "id": "web-webkit",
        "platform": "Web",
        "target": "wasm32-unknown-unknown",
        "kind": "web",
        "browser": "WebKit",
    },
)

REQUIRED_PROFILES = (
    {
        "id": "vello-only",
        "backend_ids": ["fission-render-vello"],
        "single_2d_backend": True,
        "comparison_eligible": True,
    },
    {
        "id": "skia-only",
        "backend_ids": ["fission-render-skia"],
        "single_2d_backend": True,
        "comparison_eligible": True,
    },
    {
        "id": "both-backend-diagnostic",
        "backend_ids": ["fission-render-skia", "fission-render-vello"],
        "single_2d_backend": False,
        "comparison_eligible": False,
    },
    {
        "id": "standalone-software",
        "backend_ids": ["fission-render-skia"],
        "single_2d_backend": True,
        "comparison_eligible": True,
    },
    {
        "id": "skia-plus-3d",
        "backend_ids": ["fission-render-skia", "fission-render-wgpu-3d"],
        "single_2d_backend": True,
        "comparison_eligible": True,
    },
)

REQUIRED_WORKLOADS = (
    {"id": "representative-application", "category": "application"},
    {"id": "widget-gallery", "category": "widgets"},
    {"id": "text-editing", "category": "text"},
    {"id": "accessibility-ime", "category": "accessibility"},
    {"id": "scroll", "category": "interaction"},
    {"id": "resize", "category": "interaction"},
    {"id": "animation", "category": "rendering"},
    {"id": "opacity", "category": "rendering"},
    {"id": "filters", "category": "rendering"},
    {"id": "images", "category": "resources"},
    {"id": "svg", "category": "resources"},
    {"id": "large-list", "category": "application"},
    {"id": "editor-document", "category": "application"},
    {"id": "charts", "category": "application"},
    {"id": "complex-product-site", "category": "application"},
    {"id": "video", "category": "external-content"},
    {"id": "web-content", "category": "external-content"},
    {"id": "external-surface", "category": "external-content"},
    {"id": "three-d", "category": "external-content"},
    {"id": "mobile-lifecycle", "category": "mobile"},
    {"id": "lifecycle-recovery", "category": "lifecycle"},
    {"id": "screenshot-readback", "category": "readback"},
)


class QualificationError(RuntimeError):
    """An actionable manifest, evidence, or output error."""


def reject_constant(value: str) -> None:
    raise QualificationError(f"non-finite JSON number is forbidden: {value}")


def unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise QualificationError(f"duplicate JSON object key: {key}")
        result[key] = value
    return result


def load_json(path: Path) -> Any:
    try:
        with path.open("r", encoding="utf-8") as source:
            return json.load(
                source,
                parse_constant=reject_constant,
                object_pairs_hook=unique_object,
            )
    except FileNotFoundError as error:
        raise QualificationError(f"JSON input does not exist: {path}") from error
    except json.JSONDecodeError as error:
        raise QualificationError(f"invalid JSON in {path}: {error}") from error


def canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=True, indent=2, sort_keys=True) + "\n"


def digest_json(value: Any) -> str:
    return hashlib.sha256(canonical_json(value).encode("utf-8")).hexdigest()


def require_object(value: Any, context: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise QualificationError(f"{context} must be an object")
    return value


def require_array(value: Any, context: str) -> list[Any]:
    if not isinstance(value, list):
        raise QualificationError(f"{context} must be an array")
    return value


def require_string(value: Any, context: str) -> str:
    if not isinstance(value, str) or not value or not ID_RE.fullmatch(value):
        raise QualificationError(f"{context} must be a non-empty stable identifier")
    return value


def require_exact_fields(value: Mapping[str, Any], fields: set[str], context: str) -> None:
    if set(value) != fields:
        raise QualificationError(
            f"{context} has unknown or missing fields; "
            f"missing={sorted(fields - set(value))}, extra={sorted(set(value) - fields)}"
        )


def indexed_entries(
    raw: Any,
    context: str,
    fields: set[str],
) -> dict[str, dict[str, Any]]:
    result: dict[str, dict[str, Any]] = {}
    for index, value in enumerate(require_array(raw, context)):
        entry = require_object(value, f"{context}[{index}]")
        require_exact_fields(entry, fields, f"{context}[{index}]")
        identifier = require_string(entry.get("id"), f"{context}[{index}].id")
        if identifier in result:
            raise QualificationError(f"duplicate {context} identifier: {identifier}")
        result[identifier] = entry
    return result


def require_definitions(
    actual: Mapping[str, Mapping[str, Any]],
    required: Iterable[Mapping[str, Any]],
    context: str,
) -> None:
    for expected in required:
        identifier = expected["id"]
        if identifier not in actual:
            raise QualificationError(f"{context} narrows the required matrix: missing {identifier}")
        if actual[identifier] != expected:
            raise QualificationError(f"{context} changes the required definition for {identifier}")


def pair_key(target_id: str, profile_id: str) -> str:
    return f"{target_id}::{profile_id}"


def manifest_template() -> dict[str, Any]:
    targets = [dict(value) for value in REQUIRED_TARGETS]
    profiles = [
        {**value, "backend_ids": list(value["backend_ids"])}
        for value in REQUIRED_PROFILES
    ]
    workloads = [dict(value) for value in REQUIRED_WORKLOADS]
    return {
        "schema_version": SCHEMA_VERSION,
        "matrix_revision": MATRIX_REVISION,
        "frozen": True,
        "budget_revision": None,
        "targets": targets,
        "profiles": profiles,
        "workloads": workloads,
        "environment_ids": {value["id"]: None for value in targets},
        "workload_input_ids": {value["id"]: None for value in workloads},
        "identities": {
            pair_key(target["id"], profile["id"]): {
                "backend_ids": list(profile["backend_ids"]),
                "build_id": None,
                "toolchain_id": None,
                "artifact_id": None,
            }
            for target in targets
            for profile in profiles
        },
        # Numeric budgets are intentionally absent. Qualification remains
        # fail-closed until reviewed target-specific values are populated.
        "budgets": {value["id"]: None for value in targets},
    }


def validate_target(entry: Mapping[str, Any], context: str) -> None:
    for field in ("id", "platform", "target", "kind"):
        require_string(entry.get(field), f"{context}.{field}")
    kind = entry.get("kind")
    platform = entry.get("platform")
    browser = entry.get("browser")
    if platform not in {"Linux", "macOS", "Windows", "Android", "iOS", "Web"}:
        raise QualificationError(f"{context}.platform is not a Fission interactive platform")
    if kind == "native":
        if platform == "Web":
            raise QualificationError(f"{context} cannot classify Web as native")
        if browser is not None:
            raise QualificationError(f"{context}.browser must be null for native targets")
    elif kind == "web":
        if platform != "Web":
            raise QualificationError(f"{context} Web targets must use platform Web")
        if browser not in {"Chromium", "Firefox", "WebKit"}:
            raise QualificationError(f"{context}.browser must be Chromium, Firefox, or WebKit")
    else:
        raise QualificationError(f"{context}.kind must be native or web")


def validate_profile(entry: Mapping[str, Any], context: str) -> None:
    require_string(entry.get("id"), f"{context}.id")
    backends = require_array(entry.get("backend_ids"), f"{context}.backend_ids")
    if not backends or len(set(backends)) != len(backends):
        raise QualificationError(f"{context}.backend_ids must be a non-empty unique array")
    for index, backend in enumerate(backends):
        require_string(backend, f"{context}.backend_ids[{index}]")
    for field in ("single_2d_backend", "comparison_eligible"):
        if not isinstance(entry.get(field), bool):
            raise QualificationError(f"{context}.{field} must be boolean")
    if entry.get("single_2d_backend") and sum("render-" in value for value in backends) < 1:
        raise QualificationError(f"{context} does not identify its 2D backend")


def validate_manifest(raw: Any) -> dict[str, Any]:
    manifest = require_object(raw, "qualification manifest")
    fields = {
        "schema_version",
        "matrix_revision",
        "frozen",
        "budget_revision",
        "targets",
        "profiles",
        "workloads",
        "environment_ids",
        "workload_input_ids",
        "identities",
        "budgets",
    }
    require_exact_fields(manifest, fields, "qualification manifest")
    if manifest.get("schema_version") != SCHEMA_VERSION:
        raise QualificationError("unsupported qualification manifest schema")
    if manifest.get("matrix_revision") != MATRIX_REVISION:
        raise QualificationError("qualification manifest matrix revision is unsupported")
    if manifest.get("frozen") is not True:
        raise QualificationError("qualification manifest must be explicitly frozen")
    budget_revision = manifest.get("budget_revision")
    if budget_revision is not None:
        require_string(budget_revision, "qualification manifest budget_revision")

    targets = indexed_entries(
        manifest.get("targets"),
        "targets",
        {"id", "platform", "target", "kind", "browser"},
    )
    profiles = indexed_entries(
        manifest.get("profiles"),
        "profiles",
        {"id", "backend_ids", "single_2d_backend", "comparison_eligible"},
    )
    workloads = indexed_entries(
        manifest.get("workloads"),
        "workloads",
        {"id", "category"},
    )
    require_definitions(targets, REQUIRED_TARGETS, "targets")
    require_definitions(profiles, REQUIRED_PROFILES, "profiles")
    require_definitions(workloads, REQUIRED_WORKLOADS, "workloads")
    for identifier, entry in targets.items():
        validate_target(entry, f"targets.{identifier}")
    for identifier, entry in profiles.items():
        validate_profile(entry, f"profiles.{identifier}")
    for identifier, entry in workloads.items():
        require_string(entry.get("category"), f"workloads.{identifier}.category")

    environment_ids = require_object(manifest.get("environment_ids"), "environment_ids")
    if set(environment_ids) != set(targets):
        raise QualificationError("environment_ids must classify every target exactly once")
    for target_id, value in environment_ids.items():
        if value is not None:
            require_string(value, f"environment_ids.{target_id}")

    input_ids = require_object(manifest.get("workload_input_ids"), "workload_input_ids")
    if set(input_ids) != set(workloads):
        raise QualificationError("workload_input_ids must classify every workload exactly once")
    for workload_id, value in input_ids.items():
        if value is not None:
            require_string(value, f"workload_input_ids.{workload_id}")

    identities = require_object(manifest.get("identities"), "identities")
    expected_pairs = {
        pair_key(target_id, profile_id)
        for target_id in targets
        for profile_id in profiles
    }
    if set(identities) != expected_pairs:
        raise QualificationError("identities must classify every target/profile pair exactly once")
    for target_id in targets:
        for profile_id, profile in profiles.items():
            key = pair_key(target_id, profile_id)
            identity = require_object(identities[key], f"identities.{key}")
            require_exact_fields(
                identity,
                {"backend_ids", "build_id", "toolchain_id", "artifact_id"},
                f"identities.{key}",
            )
            if identity.get("backend_ids") != profile["backend_ids"]:
                raise QualificationError(f"identities.{key}.backend_ids changes the profile identity")
            for field in ("build_id", "toolchain_id", "artifact_id"):
                if identity.get(field) is not None:
                    require_string(identity[field], f"identities.{key}.{field}")

    budgets = require_object(manifest.get("budgets"), "budgets")
    if set(budgets) != set(targets):
        raise QualificationError("budgets must classify every target exactly once")
    for target_id, budget in budgets.items():
        if budget is not None:
            validate_budget_shape(budget, f"budgets.{target_id}")
    return manifest


def number(value: Any) -> int | float | None:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return None
    return value if math.isfinite(float(value)) and value >= 0 else None


def validate_budget_shape(raw: Any, context: str) -> None:
    budget = require_object(raw, context)
    require_exact_fields(budget, {"latency_ms", "memory_bytes", "size_bytes"}, context)
    latency = require_object(budget.get("latency_ms"), f"{context}.latency_ms")
    require_exact_fields(latency, set(LATENCIES), f"{context}.latency_ms")
    for metric in LATENCIES:
        limits = require_object(latency.get(metric), f"{context}.latency_ms.{metric}")
        require_exact_fields(limits, set(PERCENTILES), f"{context}.latency_ms.{metric}")
        if any(number(limits.get(percentile)) is None for percentile in PERCENTILES):
            raise QualificationError(f"{context}.latency_ms.{metric} has an invalid ceiling")
        if not limits["p50"] <= limits["p95"] <= limits["p99"]:
            raise QualificationError(f"{context}.latency_ms.{metric} ceilings are not monotonic")
    memory = require_object(budget.get("memory_bytes"), f"{context}.memory_bytes")
    require_exact_fields(memory, set(MEMORY), f"{context}.memory_bytes")
    if any(number(memory.get(metric)) is None for metric in MEMORY):
        raise QualificationError(f"{context}.memory_bytes has an invalid ceiling")
    if memory["peak_bytes"] < max(memory["cold_bytes"], memory["warm_bytes"]):
        raise QualificationError(f"{context}.memory_bytes peak ceiling is below cold or warm")
    size = require_object(budget.get("size_bytes"), f"{context}.size_bytes")
    require_exact_fields(size, {"raw", "compressed"}, f"{context}.size_bytes")
    if any(number(size.get(metric)) is None for metric in ("raw", "compressed")):
        raise QualificationError(f"{context}.size_bytes has an invalid ceiling")


def percentile(values: Sequence[float], probability: float) -> float:
    if not values:
        raise QualificationError("cannot compute a percentile from no samples")
    if not 0 <= probability <= 1:
        raise QualificationError("percentile probability must be between zero and one")
    validated = [number(value) for value in values]
    if any(value is None for value in validated):
        raise QualificationError("percentile samples must be finite non-negative numbers")
    ordered = sorted(float(value) for value in validated if value is not None)
    position = (len(ordered) - 1) * probability
    lower = math.floor(position)
    upper = math.ceil(position)
    if lower == upper:
        return round(ordered[lower], 6)
    weight = position - lower
    return round(ordered[lower] * (1 - weight) + ordered[upper] * weight, 6)


def percentiles(values: Sequence[float]) -> dict[str, float]:
    return {
        "p50": percentile(values, 0.50),
        "p95": percentile(values, 0.95),
        "p99": percentile(values, 0.99),
    }


def issue(issues: list[dict[str, str]], scope: str, code: str, message: str) -> None:
    issues.append({"scope": scope, "code": code, "message": message})


def valid_samples(
    raw: Any,
    scope: str,
    issues: list[dict[str, str]],
) -> list[float] | None:
    if not isinstance(raw, list) or not raw:
        issue(issues, scope, "missing-measurement", "sample array is missing or empty")
        return None
    values = [number(value) for value in raw]
    if any(value is None for value in values):
        issue(issues, scope, "invalid-measurement", "samples must be finite non-negative numbers")
        return None
    return [value for value in values if value is not None]


def validate_outcome(
    raw: Any,
    scope: str,
    issues: list[dict[str, str]],
) -> dict[str, Any] | None:
    if not isinstance(raw, dict) or set(raw) != {"status", "evidence_id"}:
        issue(issues, scope, "missing-outcome", "outcome must contain status and evidence_id")
        return None
    status = raw.get("status")
    evidence_id = raw.get("evidence_id")
    if status not in {"pass", "fail"} or not isinstance(evidence_id, str) or not ID_RE.fullmatch(evidence_id):
        issue(issues, scope, "invalid-outcome", "outcome status or evidence identifier is invalid")
        return None
    if status != "pass":
        issue(issues, scope, "required-failure", f"required outcome failed ({evidence_id})")
    return {"status": status, "evidence_id": evidence_id}


def expected_ids(
    manifest: Mapping[str, Any],
    target_id: str,
    profile_id: str,
    scope: str,
    issues: list[dict[str, str]],
) -> dict[str, Any]:
    expected = manifest["identities"][pair_key(target_id, profile_id)]
    missing = [
        field
        for field in ("build_id", "toolchain_id", "artifact_id")
        if expected.get(field) is None
    ]
    if missing:
        issue(
            issues,
            scope,
            "missing-frozen-identity",
            "manifest has no frozen " + ", ".join(missing),
        )
    return expected


def evaluate_workload(
    raw: Any,
    workload_id: str,
    expected_input_id: str | None,
    budget: Mapping[str, Any] | None,
    comparison_eligible: bool,
    scope: str,
    issues: list[dict[str, str]],
) -> dict[str, Any]:
    result: dict[str, Any] = {
        "workload_id": workload_id,
        "input_id": None,
        "percentiles_ms": {metric: None for metric in LATENCIES},
        "memory_bytes": {metric: None for metric in MEMORY},
        "suites": {"semantic": None, "visual": None},
        "budget_failures": [],
    }
    if not isinstance(raw, dict):
        issue(issues, scope, "missing-workload", "required workload evidence is missing")
        return result
    expected_fields = {"input_id", "samples_ms", "memory_bytes", "suites"}
    if set(raw) != expected_fields:
        issue(issues, scope, "invalid-schema", "workload evidence has unknown or missing fields")
    input_id = raw.get("input_id")
    result["input_id"] = input_id if isinstance(input_id, str) else None
    if expected_input_id is None:
        issue(issues, scope, "missing-frozen-input", "manifest has no frozen workload input ID")
    elif input_id != expected_input_id:
        issue(issues, scope, "identity-mismatch", "workload input ID does not match the manifest")

    samples = raw.get("samples_ms")
    if not isinstance(samples, dict) or set(samples) != set(LATENCIES):
        issue(issues, scope, "invalid-schema", "samples_ms must contain every latency metric")
        samples = {}
    for metric in LATENCIES:
        values = valid_samples(samples.get(metric), f"{scope}.{metric}", issues)
        if values is None:
            continue
        measured = percentiles(values)
        result["percentiles_ms"][metric] = measured
        if comparison_eligible and budget is not None:
            limits = budget["latency_ms"][metric]
            for name in PERCENTILES:
                if measured[name] > limits[name]:
                    failure = {
                        "metric": f"{metric}.{name}",
                        "observed": measured[name],
                        "ceiling": limits[name],
                    }
                    result["budget_failures"].append(failure)
                    issue(
                        issues,
                        scope,
                        "budget-exceeded",
                        f"{metric}.{name}={measured[name]} exceeds {limits[name]}",
                    )

    memory = raw.get("memory_bytes")
    if not isinstance(memory, dict) or set(memory) != set(MEMORY):
        issue(issues, scope, "invalid-schema", "memory_bytes must contain cold, warm, peak, and GPU")
        memory = {}
    for metric in MEMORY:
        measured = number(memory.get(metric))
        if measured is None:
            issue(issues, f"{scope}.{metric}", "missing-measurement", "memory value is missing")
            continue
        result["memory_bytes"][metric] = measured
        if comparison_eligible and budget is not None and measured > budget["memory_bytes"][metric]:
            failure = {
                "metric": f"memory_bytes.{metric}",
                "observed": measured,
                "ceiling": budget["memory_bytes"][metric],
            }
            result["budget_failures"].append(failure)
            issue(
                issues,
                scope,
                "budget-exceeded",
                f"memory {metric}={measured} exceeds {budget['memory_bytes'][metric]}",
            )
    measured_memory = result["memory_bytes"]
    if (
        measured_memory["peak_bytes"] is not None
        and measured_memory["cold_bytes"] is not None
        and measured_memory["peak_bytes"] < measured_memory["cold_bytes"]
    ) or (
        measured_memory["peak_bytes"] is not None
        and measured_memory["warm_bytes"] is not None
        and measured_memory["peak_bytes"] < measured_memory["warm_bytes"]
    ):
        issue(issues, scope, "invalid-measurement", "peak memory is below cold or warm memory")

    suites = raw.get("suites")
    if not isinstance(suites, dict) or set(suites) != {"semantic", "visual"}:
        issue(issues, scope, "invalid-schema", "suites must contain semantic and visual outcomes")
        suites = {}
    for name in ("semantic", "visual"):
        result["suites"][name] = validate_outcome(
            suites.get(name),
            f"{scope}.{name}",
            issues,
        )
    result["budget_failures"].sort(key=lambda value: value["metric"])
    return result


def evaluate_size(
    raw: Any,
    target: Mapping[str, Any],
    budget: Mapping[str, Any] | None,
    comparison_eligible: bool,
    scope: str,
    issues: list[dict[str, str]],
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    result = {metric: None for metric in SIZES}
    failures: list[dict[str, Any]] = []
    if not isinstance(raw, dict) or set(raw) != set(SIZES):
        issue(issues, scope, "invalid-schema", "size_bytes must contain native and Web raw/compressed fields")
        raw = {}
    relevant = (
        ("native_raw", "native_compressed")
        if target["kind"] == "native"
        else ("web_raw", "web_compressed")
    )
    irrelevant = set(SIZES) - set(relevant)
    for metric in relevant:
        measured = number(raw.get(metric))
        if measured is None:
            issue(issues, f"{scope}.{metric}", "missing-measurement", "required size is missing")
        else:
            result[metric] = measured
    for metric in irrelevant:
        if raw.get(metric) is not None:
            issue(issues, f"{scope}.{metric}", "invalid-measurement", "inapplicable size must be null")
    if comparison_eligible and budget is not None:
        for metric, budget_name in zip(relevant, ("raw", "compressed"), strict=True):
            measured = result[metric]
            if measured is not None and measured > budget["size_bytes"][budget_name]:
                failure = {
                    "metric": f"size_bytes.{metric}",
                    "observed": measured,
                    "ceiling": budget["size_bytes"][budget_name],
                }
                failures.append(failure)
                issue(
                    issues,
                    scope,
                    "budget-exceeded",
                    f"size {metric}={measured} exceeds {budget['size_bytes'][budget_name]}",
                )
    return result, failures


def evaluate_run(
    run: Any,
    manifest: Mapping[str, Any],
    target: Mapping[str, Any],
    profile: Mapping[str, Any],
    issues: list[dict[str, str]],
) -> dict[str, Any]:
    target_id = target["id"]
    profile_id = profile["id"]
    scope = pair_key(target_id, profile_id)
    declared_budget = manifest["budgets"].get(target_id)
    budget = declared_budget if manifest.get("budget_revision") is not None else None
    environment_id = manifest["environment_ids"].get(target_id)
    frozen_identity = manifest["identities"][scope]
    comparison: dict[str, Any] = {
        "target_id": target_id,
        "platform": target["platform"],
        "target": target["target"],
        "browser": target["browser"],
        "profile_id": profile_id,
        "comparison_eligible": profile["comparison_eligible"],
        "run_id": None,
        "identity": {
            "environment_id": environment_id,
            "backend_ids": list(frozen_identity["backend_ids"]),
            "build_id": frozen_identity["build_id"],
            "toolchain_id": frozen_identity["toolchain_id"],
            "artifact_id": frozen_identity["artifact_id"],
        },
        "qualified": False,
        "workflows": {name: None for name in WORKFLOWS},
        "size_bytes": {metric: None for metric in SIZES},
        "workloads": [],
        "budget_failures": [],
        "budget": declared_budget,
        "budget_applied": budget is not None,
    }
    if manifest.get("budget_revision") is None:
        issue(issues, scope, "missing-budget-revision", "manifest has no frozen budget revision")
    if declared_budget is None:
        issue(issues, scope, "missing-budget", "target has no populated frozen budget")
    if environment_id is None:
        issue(issues, scope, "missing-environment", "target has no frozen device/driver environment ID")
    expected_identity = expected_ids(manifest, target_id, profile_id, scope, issues)
    if not isinstance(run, dict):
        issue(issues, scope, "missing-evidence", "required target/profile run evidence is missing")
        for workload_id in sorted(manifest["workload_input_ids"]):
            comparison["workloads"].append(
                evaluate_workload(
                    None,
                    workload_id,
                    manifest["workload_input_ids"][workload_id],
                    budget,
                    profile["comparison_eligible"],
                    f"{scope}.{workload_id}",
                    issues,
                )
            )
        return comparison

    expected_fields = {
        "schema_version",
        "matrix_revision",
        "run_id",
        "identity",
        "workflows",
        "size_bytes",
        "workloads",
    }
    if set(run) != expected_fields:
        issue(issues, scope, "invalid-schema", "run evidence has unknown or missing fields")
    if run.get("schema_version") != SCHEMA_VERSION:
        issue(issues, scope, "invalid-schema", "run evidence schema version is unsupported")
    if run.get("matrix_revision") != MATRIX_REVISION:
        issue(issues, scope, "identity-mismatch", "run matrix revision does not match the manifest")
    run_id = run.get("run_id")
    if isinstance(run_id, str) and ID_RE.fullmatch(run_id):
        comparison["run_id"] = run_id
    else:
        issue(issues, scope, "invalid-identity", "run_id is missing or invalid")

    identity = run.get("identity")
    identity_fields = {
        "target_id",
        "profile_id",
        "environment_id",
        "backend_ids",
        "build_id",
        "toolchain_id",
        "artifact_id",
    }
    if not isinstance(identity, dict) or set(identity) != identity_fields:
        issue(issues, scope, "invalid-identity", "run identity has unknown or missing fields")
        identity = {}
    actual_fixed = {
        "target_id": identity.get("target_id"),
        "profile_id": identity.get("profile_id"),
        "environment_id": identity.get("environment_id"),
    }
    expected_fixed = {
        "target_id": target_id,
        "profile_id": profile_id,
        "environment_id": environment_id,
    }
    if actual_fixed != expected_fixed:
        issue(issues, scope, "identity-mismatch", "target, profile, or environment identity differs")
    if identity.get("backend_ids") != expected_identity["backend_ids"]:
        issue(issues, scope, "identity-mismatch", "backend identity differs from the profile")
    for field in ("build_id", "toolchain_id", "artifact_id"):
        frozen = expected_identity.get(field)
        if frozen is not None and identity.get(field) != frozen:
            issue(issues, scope, "identity-mismatch", f"{field} differs from the frozen identity")

    workflows = run.get("workflows")
    if not isinstance(workflows, dict) or set(workflows) != set(WORKFLOWS):
        issue(issues, scope, "invalid-schema", "workflows must contain every required workflow")
        workflows = {}
    for name in WORKFLOWS:
        comparison["workflows"][name] = validate_outcome(
            workflows.get(name),
            f"{scope}.workflow.{name}",
            issues,
        )
    size, size_failures = evaluate_size(
        run.get("size_bytes"),
        target,
        budget,
        profile["comparison_eligible"],
        f"{scope}.size",
        issues,
    )
    comparison["size_bytes"] = size
    comparison["budget_failures"].extend(size_failures)

    workloads = run.get("workloads")
    if not isinstance(workloads, dict):
        issue(issues, scope, "invalid-schema", "workloads must be an object")
        workloads = {}
    expected_workloads = set(manifest["workload_input_ids"])
    if set(workloads) != expected_workloads:
        issue(
            issues,
            scope,
            "invalid-schema",
            "run workloads do not match the frozen workload matrix",
        )
    for workload_id in sorted(expected_workloads):
        evaluated = evaluate_workload(
            workloads.get(workload_id),
            workload_id,
            manifest["workload_input_ids"][workload_id],
            budget,
            profile["comparison_eligible"],
            f"{scope}.{workload_id}",
            issues,
        )
        comparison["workloads"].append(evaluated)
        comparison["budget_failures"].extend(evaluated["budget_failures"])
    comparison["budget_failures"].sort(
        key=lambda value: (value["metric"], value["observed"], value["ceiling"])
    )
    return comparison


def evidence_index(
    raw_runs: Iterable[Any],
    manifest: Mapping[str, Any],
    issues: list[dict[str, str]],
) -> dict[str, Any]:
    targets = {entry["id"] for entry in manifest["targets"]}
    profiles = {entry["id"] for entry in manifest["profiles"]}
    result: dict[str, Any] = {}
    run_ids: set[str] = set()
    for index, run in enumerate(raw_runs):
        scope = f"evidence[{index}]"
        if not isinstance(run, dict):
            issue(issues, scope, "invalid-schema", "run evidence must be an object")
            continue
        identity = run.get("identity")
        if not isinstance(identity, dict):
            issue(issues, scope, "invalid-identity", "run evidence has no identity object")
            continue
        target_id = identity.get("target_id")
        profile_id = identity.get("profile_id")
        if target_id not in targets or profile_id not in profiles:
            issue(issues, scope, "unknown-matrix-cell", "run identifies an unknown target/profile")
            continue
        key = pair_key(target_id, profile_id)
        if key in result:
            issue(issues, key, "duplicate-evidence", "multiple runs claim the same matrix cell")
            continue
        run_id = run.get("run_id")
        if isinstance(run_id, str) and run_id in run_ids:
            issue(issues, key, "duplicate-evidence", "run_id is reused by another matrix cell")
            continue
        if isinstance(run_id, str):
            run_ids.add(run_id)
        result[key] = run
    return result


def aggregate_comparison(comparison: Mapping[str, Any]) -> dict[str, Any]:
    latency: dict[str, dict[str, float] | None] = {}
    for metric in LATENCIES:
        rows = [
            workload["percentiles_ms"][metric]
            for workload in comparison["workloads"]
            if workload["percentiles_ms"][metric] is not None
        ]
        latency[metric] = (
            {name: max(row[name] for row in rows) for name in PERCENTILES}
            if rows
            else None
        )
    memory: dict[str, float | None] = {}
    for metric in MEMORY:
        values = [
            workload["memory_bytes"][metric]
            for workload in comparison["workloads"]
            if workload["memory_bytes"][metric] is not None
        ]
        memory[metric] = max(values) if values else None
    return {"worst_percentiles_ms": latency, "maximum_memory_bytes": memory}


def build_report(manifest_raw: Any, raw_runs: Iterable[Any]) -> dict[str, Any]:
    manifest = validate_manifest(manifest_raw)
    issues: list[dict[str, str]] = []
    indexed = evidence_index(raw_runs, manifest, issues)
    targets = {entry["id"]: entry for entry in manifest["targets"]}
    profiles = {entry["id"]: entry for entry in manifest["profiles"]}
    comparisons: list[dict[str, Any]] = []
    for target_id in sorted(targets):
        for profile_id in sorted(profiles):
            key = pair_key(target_id, profile_id)
            before = len(issues)
            comparison = evaluate_run(
                indexed.get(key),
                manifest,
                targets[target_id],
                profiles[profile_id],
                issues,
            )
            comparison["aggregate"] = aggregate_comparison(comparison)
            comparison["qualified"] = len(issues) == before
            comparisons.append(comparison)
    issues.sort(key=lambda value: (value["scope"], value["code"], value["message"]))
    qualified = not issues and all(value["qualified"] for value in comparisons)
    return {
        "schema_version": SCHEMA_VERSION,
        "matrix_revision": MATRIX_REVISION,
        "manifest_sha256": digest_json(manifest),
        "budget_revision": manifest.get("budget_revision"),
        "qualified": qualified,
        "summary": {
            "targets": len(targets),
            "profiles": len(profiles),
            "workloads": len(manifest["workloads"]),
            "required_runs": len(targets) * len(profiles),
            "received_runs": len(indexed),
            "issue_count": len(issues),
        },
        "issues": issues,
        "comparisons": comparisons,
    }


def format_number(value: float | int | None) -> str:
    if value is None:
        return "—"
    return f"{value:g}"


def markdown_report(report: Mapping[str, Any]) -> str:
    status = "QUALIFIED" if report["qualified"] else "UNQUALIFIED"
    lines = [
        "# Fission backend qualification",
        "",
        f"Overall: **{status}**",
        "",
        f"Matrix: `{report['matrix_revision']}`  ",
        f"Budget revision: `{report['budget_revision'] or 'missing'}`  ",
        f"Evidence: {report['summary']['received_runs']}/{report['summary']['required_runs']} runs  ",
        f"Issues: {report['summary']['issue_count']}",
        "",
        "| Target | Profile | Run | Startup p95 | Frame p95 | Input p95 | "
        "Recovery p95 | Peak bytes | GPU bytes | Raw bytes | Compressed bytes | Result |",
        "|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|",
    ]
    for comparison in report["comparisons"]:
        aggregate = comparison["aggregate"]
        latency = aggregate["worst_percentiles_ms"]
        memory = aggregate["maximum_memory_bytes"]
        size = comparison["size_bytes"]
        raw_size = size["native_raw"] if size["native_raw"] is not None else size["web_raw"]
        compressed = (
            size["native_compressed"]
            if size["native_compressed"] is not None
            else size["web_compressed"]
        )

        def p95(metric: str) -> str:
            value = latency[metric]
            return format_number(value["p95"] if value else None)

        result = "pass" if comparison["qualified"] else "fail"
        lines.append(
            "| "
            + " | ".join(
                [
                    comparison["target_id"],
                    comparison["profile_id"],
                    comparison["run_id"] or "—",
                    p95("startup_ms"),
                    p95("frame_ms"),
                    p95("input_ms"),
                    p95("recovery_ms"),
                    format_number(memory["peak_bytes"]),
                    format_number(memory["gpu_bytes"]),
                    format_number(raw_size),
                    format_number(compressed),
                    result,
                ]
            )
            + " |"
        )
    counts = Counter(value["code"] for value in report["issues"])
    if counts:
        lines.extend(["", "## Blocking evidence", ""])
        lines.extend(f"- `{name}`: {counts[name]}" for name in sorted(counts))
    return "\n".join(lines) + "\n"


def write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        mode="w",
        encoding="utf-8",
        dir=path.parent,
        prefix=f".{path.name}.",
        delete=False,
    ) as output:
        temporary = Path(output.name)
        output.write(content)
        output.flush()
    temporary.replace(path)


def template_command(args: argparse.Namespace) -> int:
    content = canonical_json(manifest_template())
    if args.output:
        write_text(Path(args.output).expanduser().resolve(), content)
    else:
        print(content, end="")
    return 0


def report_command(args: argparse.Namespace) -> int:
    manifest = load_json(Path(args.manifest).expanduser().resolve())
    runs = [load_json(Path(path).expanduser().resolve()) for path in args.evidence]
    report = build_report(manifest, runs)
    machine = canonical_json(report)
    markdown = markdown_report(report)
    if args.json_output:
        write_text(Path(args.json_output).expanduser().resolve(), machine)
    else:
        print(machine, end="")
    if args.markdown_output:
        write_text(Path(args.markdown_output).expanduser().resolve(), markdown)
    return 0 if report["qualified"] else 1


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    commands = result.add_subparsers(dest="command", required=True)
    template = commands.add_parser(
        "template",
        help="emit the complete matrix with deliberately unpopulated identities and budgets",
    )
    template.add_argument("--output")
    template.set_defaults(action=template_command)
    report = commands.add_parser("report", help="evaluate frozen manifest and real run evidence")
    report.add_argument("--manifest", required=True)
    report.add_argument("--evidence", action="append", default=[])
    report.add_argument("--json-output")
    report.add_argument("--markdown-output")
    report.set_defaults(action=report_command)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        return args.action(args)
    except QualificationError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
