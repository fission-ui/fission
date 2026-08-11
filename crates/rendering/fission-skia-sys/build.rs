use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
#[cfg(feature = "skia-build-from-source")]
use std::process::Command;

use serde::Deserialize;
use sha2::{Digest, Sha256};

const ABI_VERSION: u32 = 7;
const SKIA_REVISION: &str = "cf5c36972b73698eb3939cda147ea47152670312";
const PREBUILT_STATIC_LIBRARIES: &[&str] = &[
    "fission_skia_bridge",
    "svg",
    "skparagraph",
    "skshaper",
    "skunicode",
    "skia",
];

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
    println!("cargo:rerun-if-changed=cpp/fission_skia_paragraph.cpp");
    println!("cargo:rerun-if-changed=cpp/fission_skia_paragraph_internal.h");
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
    if manifest.profile != profile {
        panic!(
            "Skia artifact profile {} does not match requested profile {profile}",
            manifest.profile
        );
    }
    if manifest
        .native
        .static_libraries
        .iter()
        .map(String::as_str)
        .ne(PREBUILT_STATIC_LIBRARIES.iter().copied())
    {
        panic!(
            "Skia artifact static libraries do not match the required consumer-before-dependency order"
        );
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
    let ninja = env::var("NINJA").unwrap_or_else(|_| "ninja".into());
    let status = Command::new(&ninja)
        .arg("-C")
        .arg(&build)
        .args(SOURCE_NINJA_TARGETS)
        .status()
        .unwrap_or_else(|error| panic!("failed to execute {ninja}: {error}"));
    if !status.success() {
        panic!("Ninja failed to build the pinned Skia native-raster libraries");
    }
    compile_bridge(&source, &build, "native-raster");
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
    compiler
        .cpp(true)
        .file("cpp/fission_skia.cpp")
        .file("cpp/fission_skia_paragraph.cpp")
        .include("include")
        .include(source)
        .include(build)
        .include(source.join("third_party/icu/source/common"))
        .define("FISSION_SKIA_BUILDING_BRIDGE", None)
        .define("FISSION_SKIA_REVISION", revision_define.as_str())
        .define("FISSION_SKIA_BUILD_PROFILE", profile_define.as_str())
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
