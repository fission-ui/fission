use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
#[cfg(feature = "skia-build-from-source")]
use std::process::Command;

use serde::Deserialize;
use sha2::{Digest, Sha256};

const ABI_VERSION: u32 = 9;
const SKIA_REVISION: &str = "cf5c36972b73698eb3939cda147ea47152670312";
const STATIC_LIBRARIES: &[&str] = &[
    "fission_skia_bridge",
    "svg",
    "skparagraph",
    "skshaper",
    "skunicode",
    "skia",
];
const GANESH_LINUX_TARGETS: &[&str] = &["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"];
const GANESH_LINUX_SYSTEM_LIBRARIES: &[&str] = &["dl", "fontconfig", "vulkan"];
const RASTER_BRIDGE_SOURCES: &[&str] = &[
    "cpp/fission_skia.cpp",
    "cpp/fission_skia_registry.cpp",
    "cpp/fission_skia_frame_validation.cpp",
    "cpp/fission_skia_frame_playback.cpp",
    "cpp/fission_skia_paragraph.cpp",
];
const GANESH_BRIDGE_SOURCES: &[&str] = &[
    "cpp/fission_skia.cpp",
    "cpp/fission_skia_registry.cpp",
    "cpp/fission_skia_frame_validation.cpp",
    "cpp/fission_skia_frame_playback.cpp",
    "cpp/fission_skia_paragraph.cpp",
    "cpp/fission_skia_ganesh_vulkan_context.cpp",
    "cpp/fission_skia_ganesh_vulkan_surface.cpp",
];
const GANESH_BRIDGE_DEFINE: &str = "FISSION_SKIA_ENABLE_GANESH_VULKAN";

#[cfg(feature = "skia-build-from-source")]
const SOURCE_NINJA_TARGETS: &[&str] = &[
    "skia",
    "modules/skparagraph:skparagraph",
    "modules/skshaper:skshaper",
    "modules/skunicode:skunicode",
    "modules/svg:svg",
];

#[derive(Debug, Deserialize)]
struct ArtifactManifest {
    schema_version: u32,
    skia: ArtifactSkia,
    bridge_abi_version: u32,
    target: String,
    profile: String,
    #[serde(default)]
    qualified: bool,
    files: Vec<ArtifactFile>,
    bridge: ArtifactBridge,
    native: NativeLink,
}

#[derive(Debug, Deserialize)]
struct ArtifactSkia {
    revision: String,
}

#[derive(Debug, Deserialize)]
struct ArtifactFile {
    path: String,
    sha256: String,
    size: u64,
}

#[derive(Debug, Deserialize)]
struct ArtifactBridge {
    sources: Vec<String>,
    defines: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct NativeLink {
    link_search_paths: Vec<String>,
    static_libraries: Vec<String>,
    #[serde(default)]
    system_libraries: Vec<String>,
    #[serde(default)]
    frameworks: Vec<String>,
}

fn main() {
    emit_inputs();
    let prebuilt = env::var_os("CARGO_FEATURE_SKIA_PREBUILT").is_some();
    let source = env::var_os("CARGO_FEATURE_SKIA_BUILD_FROM_SOURCE").is_some();
    let test_shim = env::var_os("CARGO_FEATURE_TEST_SHIM").is_some();
    if [prebuilt, source, test_shim]
        .into_iter()
        .filter(|enabled| *enabled)
        .count()
        != 1
    {
        panic!(
            "fission-skia-sys requires exactly one build mode: skia-prebuilt, \
             skia-build-from-source, or test-shim"
        );
    }
    if env::var("TARGET")
        .expect("Cargo must set TARGET")
        .starts_with("wasm32")
    {
        panic!(
            "the native fission-skia-sys ABI does not target wasm32; CanvasKit uses \
             the separately versioned Web transport"
        );
    }

    if prebuilt {
        configure_prebuilt();
    } else if source {
        #[cfg(feature = "skia-build-from-source")]
        configure_source();
        #[cfg(not(feature = "skia-build-from-source"))]
        unreachable!();
    } else {
        #[cfg(feature = "test-shim")]
        configure_test_shim();
        #[cfg(not(feature = "test-shim"))]
        unreachable!();
    }
}

fn emit_inputs() {
    println!("cargo:rerun-if-changed=include/fission_skia.h");
    println!("cargo:rerun-if-changed=cpp/fission_skia.cpp");
    println!("cargo:rerun-if-changed=cpp/fission_skia_internal.h");
    println!("cargo:rerun-if-changed=cpp/fission_skia_registry.cpp");
    println!("cargo:rerun-if-changed=cpp/fission_skia_frame_validation.cpp");
    println!("cargo:rerun-if-changed=cpp/fission_skia_frame_playback.cpp");
    println!("cargo:rerun-if-changed=cpp/fission_skia_paragraph.cpp");
    println!("cargo:rerun-if-changed=cpp/fission_skia_paragraph_internal.h");
    println!("cargo:rerun-if-changed=cpp/fission_skia_ganesh_vulkan.h");
    println!("cargo:rerun-if-changed=cpp/fission_skia_ganesh_vulkan_internal.h");
    println!("cargo:rerun-if-changed=cpp/fission_skia_ganesh_vulkan_context.cpp");
    println!("cargo:rerun-if-changed=cpp/fission_skia_ganesh_vulkan_surface.cpp");
    println!("cargo:rerun-if-changed=cpp/test_shim.cpp");
    println!("cargo:rerun-if-changed=cpp/test_shim_paragraph.cpp");
    println!("cargo:rerun-if-changed=skia_revision.txt");
    for variable in [
        "FISSION_SKIA_ARTIFACT_DIR",
        "FISSION_SKIA_SOURCE_DIR",
        "FISSION_SKIA_BUILD_DIR",
        "FISSION_SKIA_PROFILE",
        "FISSION_SKIA_LINK_LIBS",
        "FISSION_SKIA_ALLOW_UNQUALIFIED_ARTIFACT",
        "NINJA",
    ] {
        println!("cargo:rerun-if-env-changed={variable}");
    }
}

fn configure_prebuilt() {
    let root = required_dir("FISSION_SKIA_ARTIFACT_DIR");
    let manifest_path = root.join("manifest.json");
    println!("cargo:rerun-if-changed={}", manifest_path.display());
    let raw = fs::read(&manifest_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", manifest_path.display()));
    let manifest: ArtifactManifest = serde_json::from_slice(&raw)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", manifest_path.display()));
    validate_manifest(&manifest);
    if !manifest.qualified && env::var_os("FISSION_SKIA_ALLOW_UNQUALIFIED_ARTIFACT").is_none() {
        panic!(
            "Skia artifact is not production-qualified; use a qualified release artifact or set \
             FISSION_SKIA_ALLOW_UNQUALIFIED_ARTIFACT=1 for explicit local development"
        );
    }
    verify_files(&root, &manifest.files);
    let header = root.join("include/fission_skia.h");
    if !header.is_file() {
        panic!("verified artifact does not contain {}", header.display());
    }
    emit_native_link(&root, &manifest.native);
}

fn validate_manifest(manifest: &ArtifactManifest) {
    if manifest.schema_version != 1 {
        panic!(
            "unsupported Skia artifact schema {}",
            manifest.schema_version
        );
    }
    if manifest.skia.revision != SKIA_REVISION {
        panic!(
            "Skia artifact revision {} does not match pinned revision {SKIA_REVISION}",
            manifest.skia.revision
        );
    }
    if manifest.bridge_abi_version != ABI_VERSION {
        panic!(
            "Skia bridge ABI {} does not match required ABI {ABI_VERSION}",
            manifest.bridge_abi_version
        );
    }
    let target = env::var("TARGET").expect("Cargo must set TARGET");
    if manifest.target != target {
        panic!(
            "Skia artifact target {} does not match {target}",
            manifest.target
        );
    }
    let profile = env::var("FISSION_SKIA_PROFILE").unwrap_or_else(|_| "native-raster".into());
    validate_profile_target(&profile, &target);
    if manifest.profile != profile {
        panic!(
            "Skia artifact profile {} does not match requested profile {profile}",
            manifest.profile
        );
    }
    validate_bridge_recipe(&profile, &manifest.bridge.sources, &manifest.bridge.defines);
    if manifest
        .native
        .static_libraries
        .iter()
        .map(String::as_str)
        .ne(STATIC_LIBRARIES.iter().copied())
    {
        panic!(
            "Skia artifact static libraries do not match the required consumer-before-dependency order"
        );
    }
    if profile == "native-ganesh"
        && (manifest
            .native
            .system_libraries
            .iter()
            .map(String::as_str)
            .ne(GANESH_LINUX_SYSTEM_LIBRARIES.iter().copied())
            || !manifest.native.frameworks.is_empty())
    {
        panic!(
            "Skia native-ganesh artifact does not match the pinned Linux Vulkan system-link contract"
        );
    }
}

fn bridge_sources(profile: &str) -> &'static [&'static str] {
    match profile {
        "native-raster" => RASTER_BRIDGE_SOURCES,
        "native-ganesh" => GANESH_BRIDGE_SOURCES,
        _ => unreachable!("profile was validated before selecting bridge sources"),
    }
}

fn bridge_defines(profile: &str) -> BTreeMap<String, String> {
    if profile == "native-ganesh" {
        BTreeMap::from([(GANESH_BRIDGE_DEFINE.to_owned(), "1".to_owned())])
    } else {
        BTreeMap::new()
    }
}

fn validate_bridge_recipe(profile: &str, sources: &[String], defines: &BTreeMap<String, String>) {
    if sources
        .iter()
        .map(String::as_str)
        .ne(bridge_sources(profile).iter().copied())
    {
        panic!("Skia artifact bridge sources do not match the requested {profile} profile");
    }
    if *defines != bridge_defines(profile) {
        panic!("Skia artifact bridge defines do not match the requested {profile} profile");
    }
}

fn validate_profile_target(profile: &str, target: &str) {
    match profile {
        "native-raster" => {}
        "native-ganesh" if GANESH_LINUX_TARGETS.contains(&target) => {}
        "native-ganesh" => panic!(
            "Skia profile native-ganesh is not available for {target}; the current foundation "
            "supports only x86_64-unknown-linux-gnu and aarch64-unknown-linux-gnu"
        ),
        _ => panic!(
            "unsupported FISSION_SKIA_PROFILE={profile:?}; select native-raster or native-ganesh"
        ),
    }
}

fn verify_files(root: &Path, files: &[ArtifactFile]) {
    if files.is_empty() {
        panic!("Skia artifact manifest has no files");
    }
    for entry in files {
        let relative = safe_relative(&entry.path);
        let path = root.join(relative);
        println!("cargo:rerun-if-changed={}", path.display());
        let bytes = fs::read(&path).unwrap_or_else(|error| {
            panic!("failed to read artifact file {}: {error}", path.display())
        });
        if bytes.len() as u64 != entry.size {
            panic!("artifact file {} has the wrong size", path.display());
        }
        let actual = format!("{:x}", Sha256::digest(&bytes));
        if !actual.eq_ignore_ascii_case(&entry.sha256) {
            panic!(
                "artifact file {} failed SHA-256 verification",
                path.display()
            );
        }
    }
}

fn emit_native_link(root: &Path, native: &NativeLink) {
    for relative in &native.link_search_paths {
        println!(
            "cargo:rustc-link-search=native={}",
            root.join(safe_relative(relative)).display()
        );
    }
    let override_libraries = env::var("FISSION_SKIA_LINK_LIBS").ok();
    let libraries: Vec<&str> = override_libraries
        .as_deref()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .collect()
        })
        .unwrap_or_else(|| native.static_libraries.iter().map(String::as_str).collect());
    if libraries.first().copied() != Some("fission_skia_bridge") {
        panic!("Skia artifact must link fission_skia_bridge first");
    }
    for library in libraries {
        println!("cargo:rustc-link-lib=static={library}");
    }
    for library in &native.system_libraries {
        println!("cargo:rustc-link-lib={library}");
    }
    for framework in &native.frameworks {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
}

fn safe_relative(value: &str) -> &Path {
    let path = Path::new(value);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        panic!("artifact manifest contains unsafe relative path {value:?}");
    }
    path
}

fn required_dir(variable: &str) -> PathBuf {
    let value = env::var_os(variable).unwrap_or_else(|| panic!("{variable} must be set"));
    let path = PathBuf::from(value);
    if !path.is_dir() {
        panic!("{variable}={} is not a directory", path.display());
    }
    path
}

#[cfg(feature = "skia-build-from-source")]
fn configure_source() {
    let source = required_dir("FISSION_SKIA_SOURCE_DIR");
    let build = required_dir("FISSION_SKIA_BUILD_DIR");
    verify_source_revision(&source);
    let target = env::var("TARGET").expect("Cargo must set TARGET");
    let profile = env::var("FISSION_SKIA_PROFILE").unwrap_or_else(|_| "native-raster".into());
    validate_profile_target(&profile, &target);
    verify_bridge_source_plan(&build, &profile, &target);
    if profile == "native-ganesh" {
        verify_ganesh_source_plan(&build, &target);
    }
    let ninja = env::var("NINJA").unwrap_or_else(|_| "ninja".into());
    let status = Command::new(&ninja)
        .arg("-C")
        .arg(&build)
        .args(SOURCE_NINJA_TARGETS)
        .status()
        .unwrap_or_else(|error| panic!("failed to execute {ninja}: {error}"));
    if !status.success() {
        panic!("Ninja failed to build the pinned Skia {profile} libraries");
    }
    compile_bridge(&source, &build, &profile);
    println!("cargo:rustc-link-search=native={}", build.display());
    let links = env::var("FISSION_SKIA_LINK_LIBS")
        .unwrap_or_else(|_| "svg,skparagraph,skshaper,skunicode,skia".into());
    for library in links
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        println!("cargo:rustc-link-lib=static={library}");
    }
    if profile == "native-ganesh" {
        for library in GANESH_LINUX_SYSTEM_LIBRARIES {
            println!("cargo:rustc-link-lib={library}");
        }
    }
}

#[cfg(feature = "skia-build-from-source")]
fn verify_bridge_source_plan(build: &Path, profile: &str, target: &str) {
    let path = build.join("fission-skia-build-plan.json");
    let raw = fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "{profile} source builds require the pinned tool plan {}: {error}",
            path.display()
        )
    });
    let plan: serde_json::Value = serde_json::from_slice(&raw)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()));
    let recipe = plan
        .get("recipe")
        .and_then(serde_json::Value::as_object)
        .unwrap_or_else(|| panic!("{} has no build recipe", path.display()));
    if recipe.get("profile").and_then(serde_json::Value::as_str) != Some(profile)
        || recipe.get("target").and_then(serde_json::Value::as_str) != Some(target)
    {
        panic!("{} does not select {profile} for {target}", path.display());
    }
    let sources = recipe
        .get("bridge_sources")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{} has no bridge source list", path.display()));
    if sources
        .iter()
        .map(serde_json::Value::as_str)
        .ne(bridge_sources(profile).iter().copied().map(Some))
    {
        panic!(
            "{} has the wrong {profile} bridge source list",
            path.display()
        );
    }
    let defines = recipe
        .get("bridge_defines")
        .and_then(serde_json::Value::as_object)
        .unwrap_or_else(|| panic!("{} has no bridge define map", path.display()));
    let actual_defines = defines
        .iter()
        .map(|(name, value)| {
            value
                .as_str()
                .map(|value| (name.clone(), value.to_owned()))
                .unwrap_or_else(|| {
                    panic!("{} has a non-string bridge define {name}", path.display())
                })
        })
        .collect::<BTreeMap<_, _>>();
    if actual_defines != bridge_defines(profile) {
        panic!(
            "{} has the wrong {profile} bridge define map",
            path.display()
        );
    }
}

#[cfg(feature = "skia-build-from-source")]
fn verify_ganesh_source_plan(build: &Path, target: &str) {
    let path = build.join("fission-skia-build-plan.json");
    let raw = fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "native-ganesh source builds require the pinned tool plan {}: {error}",
            path.display()
        )
    });
    let plan: serde_json::Value = serde_json::from_slice(&raw)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()));
    let recipe = plan
        .get("recipe")
        .and_then(serde_json::Value::as_object)
        .unwrap_or_else(|| panic!("{} has no build recipe", path.display()));
    for (name, expected) in [
        ("skia_revision", SKIA_REVISION),
        ("profile", "native-ganesh"),
        ("target", target),
    ] {
        if recipe.get(name).and_then(serde_json::Value::as_str) != Some(expected) {
            panic!(
                "{} does not select the required {name}={expected}",
                path.display()
            );
        }
    }
    if recipe
        .get("bridge_abi_version")
        .and_then(serde_json::Value::as_u64)
        != Some(u64::from(ABI_VERSION))
    {
        panic!(
            "{} does not select bridge ABI {ABI_VERSION}",
            path.display()
        );
    }
    let gn_args = recipe
        .get("gn_args")
        .and_then(serde_json::Value::as_object)
        .unwrap_or_else(|| panic!("{} has no GN argument map", path.display()));
    for (name, expected) in [
        ("skia_enable_ganesh", true),
        ("skia_enable_graphite", false),
        ("skia_use_dawn", false),
        ("skia_use_direct3d", false),
        ("skia_use_gl", false),
        ("skia_use_metal", false),
        ("skia_use_vulkan", true),
        ("skia_use_vma", true),
        ("skia_use_x11", false),
    ] {
        if gn_args.get(name).and_then(serde_json::Value::as_bool) != Some(expected) {
            panic!("{} does not pin {name}={expected}", path.display());
        }
    }
    if gn_args.get("target_os").and_then(serde_json::Value::as_str) != Some("linux") {
        panic!("{} does not target Linux", path.display());
    }
    let expected_cpu = if target == "x86_64-unknown-linux-gnu" {
        "x64"
    } else {
        "arm64"
    };
    if gn_args
        .get("target_cpu")
        .and_then(serde_json::Value::as_str)
        != Some(expected_cpu)
    {
        panic!("{} does not target CPU {expected_cpu}", path.display());
    }
    let expected_upstream = &STATIC_LIBRARIES[1..];
    let upstream = recipe
        .get("upstream_libraries")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{} has no upstream library order", path.display()));
    if upstream
        .iter()
        .map(|value| value.as_str())
        .ne(expected_upstream.iter().copied().map(Some))
    {
        panic!("{} has the wrong upstream library order", path.display());
    }
    let ninja_targets = recipe
        .get("ninja_targets")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{} has no Ninja target list", path.display()));
    if ninja_targets
        .iter()
        .map(|value| value.as_str())
        .ne(SOURCE_NINJA_TARGETS.iter().copied().map(Some))
    {
        panic!("{} has the wrong Ninja target list", path.display());
    }
}

#[cfg(feature = "skia-build-from-source")]
fn verify_source_revision(source: &Path) {
    let output = Command::new("git")
        .arg("-C")
        .arg(source)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap_or_else(|error| panic!("failed to inspect Skia source revision: {error}"));
    if !output.status.success() {
        panic!("FISSION_SKIA_SOURCE_DIR is not an inspectable Git checkout");
    }
    let revision = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if revision != SKIA_REVISION {
        panic!("Skia source revision {revision} does not match pinned revision {SKIA_REVISION}");
    }
}

#[cfg(feature = "skia-build-from-source")]
fn compile_bridge(source: &Path, build: &Path, profile: &str) {
    let revision_define = format!("\"{SKIA_REVISION}\"");
    let profile_define = format!("\"{profile}\"");
    let mut compiler = cc::Build::new();
    compiler.cpp(true);
    for bridge_source in bridge_sources(profile) {
        compiler.file(bridge_source);
    }
    compiler
        .include("include")
        .include(source)
        .include(build)
        .include(source.join("third_party/icu/source/common"))
        .define("FISSION_SKIA_BUILDING_BRIDGE", None)
        .define("FISSION_SKIA_REVISION", revision_define.as_str())
        .define("FISSION_SKIA_BUILD_PROFILE", profile_define.as_str());
    if profile == "native-ganesh" {
        compiler.define(GANESH_BRIDGE_DEFINE, Some("1"));
    }
    compiler
        .flag_if_supported("-std=c++20")
        .flag_if_supported("-fno-exceptions")
        .flag_if_supported("-fno-rtti")
        .flag_if_supported("/std:c++20")
        .flag_if_supported("/EHs-c-")
        .compile("fission_skia_bridge");
}

#[cfg(feature = "test-shim")]
fn configure_test_shim() {
    let revision_define = format!("\"{SKIA_REVISION}\"");
    cc::Build::new()
        .cpp(true)
        .file("cpp/test_shim.cpp")
        .file("cpp/test_shim_paragraph.cpp")
        .include("include")
        .define("FISSION_SKIA_TEST_SHIM", None)
        .define("FISSION_SKIA_REVISION", revision_define.as_str())
        .flag_if_supported("-std=c++20")
        .flag_if_supported("/std:c++20")
        .compile("fission_skia_bridge");
}
