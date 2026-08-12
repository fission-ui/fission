use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
#[cfg(feature = "skia-build-from-source")]
use std::process::Command;

use serde::Deserialize;
use sha2::{Digest, Sha256};

#[path = "build_support/artifact_resolver.rs"]
mod artifact_resolver;
#[path = "build_support/native_contract.rs"]
mod native_contract;

const ABI_VERSION: u32 = 13;
const SKIA_REVISION: &str = "cf5c36972b73698eb3939cda147ea47152670312";
const STATIC_LIBRARIES: &[&str] = &[
    "fission_skia_bridge",
    "svg",
    "skparagraph",
    "skshaper",
    "skunicode",
    "skia",
];
const GANESH_LINUX_SYSTEM_LIBRARIES: &[&str] = &["dl", "fontconfig", "vulkan"];
const GANESH_APPLE_SYSTEM_LIBRARIES: &[&str] = &["c++"];
const GANESH_WINDOWS_SYSTEM_LIBRARIES: &[&str] = &["d3d12", "dxgi", "user32", "kernel32"];
const GANESH_ANDROID_SYSTEM_LIBRARIES: &[&str] = &["android", "vulkan", "c++_shared"];
const GANESH_MACOS_FRAMEWORKS: &[&str] = &[
    "AppKit",
    "CoreFoundation",
    "CoreGraphics",
    "Foundation",
    "Metal",
    "QuartzCore",
];
const GANESH_IOS_FRAMEWORKS: &[&str] = &[
    "CoreFoundation",
    "CoreGraphics",
    "Foundation",
    "Metal",
    "QuartzCore",
    "UIKit",
];
const RASTER_BRIDGE_SOURCES: &[&str] = &[
    "cpp/fission_skia.cpp",
    "cpp/fission_skia_registry.cpp",
    "cpp/fission_skia_frame_validation.cpp",
    "cpp/fission_skia_frame_playback.cpp",
    "cpp/fission_skia_paragraph.cpp",
];
const GANESH_VULKAN_BRIDGE_SOURCES: &[&str] = &[
    "cpp/fission_skia.cpp",
    "cpp/fission_skia_registry.cpp",
    "cpp/fission_skia_frame_validation.cpp",
    "cpp/fission_skia_frame_playback.cpp",
    "cpp/fission_skia_paragraph.cpp",
    "cpp/fission_skia_ganesh_vulkan_context.cpp",
    "cpp/fission_skia_ganesh_vulkan_surface.cpp",
];
const GANESH_MACOS_BRIDGE_SOURCES: &[&str] = &[
    "cpp/fission_skia.cpp",
    "cpp/fission_skia_registry.cpp",
    "cpp/fission_skia_frame_validation.cpp",
    "cpp/fission_skia_frame_playback.cpp",
    "cpp/fission_skia_paragraph.cpp",
    "cpp/fission_skia_ganesh_metal_context.mm",
    "cpp/fission_skia_ganesh_metal_surface.mm",
];
const GANESH_IOS_BRIDGE_SOURCES: &[&str] = &[
    "cpp/fission_skia.cpp",
    "cpp/fission_skia_registry.cpp",
    "cpp/fission_skia_frame_validation.cpp",
    "cpp/fission_skia_frame_playback.cpp",
    "cpp/fission_skia_paragraph.cpp",
    "cpp/fission_skia_ganesh_ios_metal_context.mm",
    "cpp/fission_skia_ganesh_ios_metal_surface.mm",
];
const GANESH_D3D12_BRIDGE_SOURCES: &[&str] = &[
    "cpp/fission_skia.cpp",
    "cpp/fission_skia_registry.cpp",
    "cpp/fission_skia_frame_validation.cpp",
    "cpp/fission_skia_frame_playback.cpp",
    "cpp/fission_skia_paragraph.cpp",
    "cpp/fission_skia_ganesh_d3d_context.cpp",
    "cpp/fission_skia_ganesh_d3d_surface.cpp",
];
const GANESH_ANDROID_VULKAN_BRIDGE_SOURCES: &[&str] = &[
    "cpp/fission_skia.cpp",
    "cpp/fission_skia_registry.cpp",
    "cpp/fission_skia_frame_validation.cpp",
    "cpp/fission_skia_frame_playback.cpp",
    "cpp/fission_skia_paragraph.cpp",
    "cpp/fission_skia_ganesh_android_vulkan_context.cpp",
    "cpp/fission_skia_ganesh_android_vulkan_surface.cpp",
];

#[derive(Clone, Copy)]
enum GaneshBackend {
    Vulkan,
    MacOSMetal,
    IosMetal,
    D3D12,
    AndroidVulkan,
}

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
    artifact_id: String,
    fission_version: String,
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
    let target = env::var("TARGET").expect("Cargo must set TARGET");
    if target.starts_with("wasm32") {
        if !prebuilt || source || test_shim {
            panic!(
                "the wasm32 CanvasKit transport requires the skia-prebuilt profile only; native \
                 source and test-shim build modes cannot be linked into browser Wasm"
            );
        }
        println!("cargo:rustc-check-cfg=cfg(fission_skia_canvaskit)");
        println!("cargo:rustc-cfg=fission_skia_canvaskit");
        return;
    }
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
    println!("cargo:rerun-if-changed=cpp/fission_skia_ganesh_metal.h");
    println!("cargo:rerun-if-changed=cpp/fission_skia_ganesh_metal_internal.h");
    println!("cargo:rerun-if-changed=cpp/fission_skia_ganesh_metal_context.mm");
    println!("cargo:rerun-if-changed=cpp/fission_skia_ganesh_metal_surface.mm");
    println!("cargo:rerun-if-changed=cpp/fission_skia_ganesh_ios_metal.h");
    println!("cargo:rerun-if-changed=cpp/fission_skia_ganesh_ios_metal_internal.h");
    println!("cargo:rerun-if-changed=cpp/fission_skia_ganesh_ios_metal_context.mm");
    println!("cargo:rerun-if-changed=cpp/fission_skia_ganesh_ios_metal_surface.mm");
    println!("cargo:rerun-if-changed=cpp/fission_skia_ganesh_d3d.h");
    println!("cargo:rerun-if-changed=cpp/fission_skia_ganesh_d3d_internal.h");
    println!("cargo:rerun-if-changed=cpp/fission_skia_ganesh_d3d_context.cpp");
    println!("cargo:rerun-if-changed=cpp/fission_skia_ganesh_d3d_surface.cpp");
    println!("cargo:rerun-if-changed=cpp/fission_skia_ganesh_android_vulkan.h");
    println!("cargo:rerun-if-changed=cpp/fission_skia_ganesh_android_vulkan_internal.h");
    println!("cargo:rerun-if-changed=cpp/fission_skia_ganesh_android_vulkan_context.cpp");
    println!("cargo:rerun-if-changed=cpp/fission_skia_ganesh_android_vulkan_surface.cpp");
    println!("cargo:rerun-if-changed=cpp/test_shim.cpp");
    println!("cargo:rerun-if-changed=cpp/test_shim_paragraph.cpp");
    println!("cargo:rerun-if-changed=skia_revision.txt");
    println!("cargo:rerun-if-changed=artifacts.lock.json");
    for variable in [
        "FISSION_SKIA_ARTIFACT_DIR",
        "FISSION_SKIA_CACHE_DIR",
        "FISSION_SKIA_OFFLINE",
        "FISSION_SKIA_SOURCE_DIR",
        "FISSION_SKIA_BUILD_DIR",
        "FISSION_SKIA_PROFILE",
        "FISSION_SKIA_LINK_LIBS",
        "FISSION_SKIA_ALLOW_UNQUALIFIED_ARTIFACT",
        "CARGO_HOME",
        "CARGO_NET_OFFLINE",
        "HOME",
        "USERPROFILE",
        "NINJA",
    ] {
        println!("cargo:rerun-if-env-changed={variable}");
    }
}

fn configure_prebuilt() {
    let target = env::var("TARGET").expect("Cargo must set TARGET");
    let profile = env::var("FISSION_SKIA_PROFILE").unwrap_or_else(|_| "native-raster".into());
    validate_profile_target(&profile, &target);
    let local_override = env::var_os("FISSION_SKIA_ARTIFACT_DIR").is_some();
    let allow_unqualified = optional_boolean_environment("FISSION_SKIA_ALLOW_UNQUALIFIED_ARTIFACT");
    if allow_unqualified && !local_override {
        panic!(
            "FISSION_SKIA_ALLOW_UNQUALIFIED_ARTIFACT applies only with an explicit \
             FISSION_SKIA_ARTIFACT_DIR"
        );
    }
    let root = if local_override {
        required_real_dir("FISSION_SKIA_ARTIFACT_DIR")
    } else {
        let cache_root = artifact_resolver::cache_root_from_environment()
            .unwrap_or_else(|error| panic!("failed to select the Skia artifact cache: {error}"));
        let offline = artifact_resolver::offline_from_environment()
            .unwrap_or_else(|error| panic!("failed to read Skia offline configuration: {error}"));
        artifact_resolver::resolve(artifact_resolver::ResolveRequest {
            lock_json: include_bytes!("artifacts.lock.json"),
            fission_version: env!("CARGO_PKG_VERSION"),
            skia_revision: SKIA_REVISION,
            bridge_abi_version: ABI_VERSION,
            target: &target,
            profile: &profile,
            cache_root: &cache_root,
            offline,
        })
        .unwrap_or_else(|error| panic!("failed to resolve the Skia artifact: {error}"))
    };
    let artifact_files = native_contract::inspect_artifact_tree(&root)
        .unwrap_or_else(|error| panic!("failed to verify the Skia artifact tree: {error}"));
    let manifest_path = root.join("manifest.json");
    println!("cargo:rerun-if-changed={}", manifest_path.display());
    let raw = fs::read(&manifest_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", manifest_path.display()));
    let manifest: ArtifactManifest = serde_json::from_slice(&raw)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", manifest_path.display()));
    validate_manifest(&manifest);
    if manifest.fission_version != env!("CARGO_PKG_VERSION") {
        panic!(
            "Skia artifact is for Fission {}, not {}",
            manifest.fission_version,
            env!("CARGO_PKG_VERSION")
        );
    }
    if !manifest.qualified && !allow_unqualified {
        panic!(
            "Skia artifact is not production-qualified; use a qualified release artifact or set \
             FISSION_SKIA_ALLOW_UNQUALIFIED_ARTIFACT=1 with an explicit \
             FISSION_SKIA_ARTIFACT_DIR for local development"
        );
    }
    verify_files(&root, &manifest.files, artifact_files);
    let header = root.join("include/fission_skia.h");
    if !header.is_file() {
        panic!("verified artifact does not contain {}", header.display());
    }
    emit_native_link(&root, &manifest.native, local_override);
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
    let expected_id =
        native_contract::artifact_id(env!("CARGO_PKG_VERSION"), ABI_VERSION, &profile, &target);
    if manifest.artifact_id != expected_id {
        panic!(
            "Skia artifact id {} does not match expected {expected_id}",
            manifest.artifact_id
        );
    }
    if manifest.profile != profile {
        panic!(
            "Skia artifact profile {} does not match requested profile {profile}",
            manifest.profile
        );
    }
    if manifest.native.link_search_paths != ["lib"] {
        panic!("Skia artifact native.link_search_paths must be exactly [\"lib\"]");
    }
    validate_bridge_recipe(
        &profile,
        &target,
        &manifest.bridge.sources,
        &manifest.bridge.defines,
    );
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
    if profile == "native-ganesh" {
        let (system_libraries, frameworks) = ganesh_link_contract(&target);
        if manifest
            .native
            .system_libraries
            .iter()
            .map(String::as_str)
            .ne(system_libraries.iter().copied())
            || manifest
                .native
                .frameworks
                .iter()
                .map(String::as_str)
                .ne(frameworks.iter().copied())
        {
            panic!(
                "Skia native-ganesh artifact does not match the pinned {target} system-link contract"
            );
        }
    }
    for library in &manifest.native.system_libraries {
        native_contract::validate_link_name(library, "Skia system library")
            .unwrap_or_else(|error| panic!("{error}"));
    }
    for framework in &manifest.native.frameworks {
        native_contract::validate_link_name(framework, "Skia framework")
            .unwrap_or_else(|error| panic!("{error}"));
    }
}

fn ganesh_backend(target: &str) -> Option<GaneshBackend> {
    match target {
        "x86_64-unknown-linux-gnu" | "aarch64-unknown-linux-gnu" => Some(GaneshBackend::Vulkan),
        "x86_64-apple-darwin" | "aarch64-apple-darwin" => Some(GaneshBackend::MacOSMetal),
        "aarch64-apple-ios" | "aarch64-apple-ios-sim" | "x86_64-apple-ios" => {
            Some(GaneshBackend::IosMetal)
        }
        "x86_64-pc-windows-msvc" | "aarch64-pc-windows-msvc" => Some(GaneshBackend::D3D12),
        "aarch64-linux-android"
        | "armv7-linux-androideabi"
        | "x86_64-linux-android"
        | "i686-linux-android" => Some(GaneshBackend::AndroidVulkan),
        _ => None,
    }
}

fn bridge_sources(profile: &str, target: &str) -> &'static [&'static str] {
    match profile {
        "native-raster" => RASTER_BRIDGE_SOURCES,
        "native-ganesh" => match ganesh_backend(target)
            .expect("profile target was validated before selecting bridge sources")
        {
            GaneshBackend::Vulkan => GANESH_VULKAN_BRIDGE_SOURCES,
            GaneshBackend::MacOSMetal => GANESH_MACOS_BRIDGE_SOURCES,
            GaneshBackend::IosMetal => GANESH_IOS_BRIDGE_SOURCES,
            GaneshBackend::D3D12 => GANESH_D3D12_BRIDGE_SOURCES,
            GaneshBackend::AndroidVulkan => GANESH_ANDROID_VULKAN_BRIDGE_SOURCES,
        },
        _ => unreachable!("profile was validated before selecting bridge sources"),
    }
}

fn bridge_defines(profile: &str, target: &str) -> BTreeMap<String, String> {
    match (profile, ganesh_backend(target)) {
        ("native-ganesh", Some(GaneshBackend::Vulkan)) => BTreeMap::from([(
            "FISSION_SKIA_ENABLE_GANESH_VULKAN".to_owned(),
            "1".to_owned(),
        )]),
        ("native-ganesh", Some(GaneshBackend::MacOSMetal)) => BTreeMap::from([(
            "FISSION_SKIA_ENABLE_GANESH_METAL".to_owned(),
            "1".to_owned(),
        )]),
        ("native-ganesh", Some(GaneshBackend::IosMetal)) => BTreeMap::from([(
            "FISSION_SKIA_ENABLE_GANESH_IOS_METAL".to_owned(),
            "1".to_owned(),
        )]),
        ("native-ganesh", Some(GaneshBackend::D3D12)) => {
            BTreeMap::from([("FISSION_SKIA_ENABLE_GANESH_D3D".to_owned(), "1".to_owned())])
        }
        ("native-ganesh", Some(GaneshBackend::AndroidVulkan)) => BTreeMap::from([(
            "FISSION_SKIA_ENABLE_GANESH_ANDROID_VULKAN".to_owned(),
            "1".to_owned(),
        )]),
        ("native-raster", _) => BTreeMap::new(),
        _ => unreachable!("profile target was validated before selecting bridge defines"),
    }
}

fn ganesh_link_contract(target: &str) -> (&'static [&'static str], &'static [&'static str]) {
    match ganesh_backend(target)
        .expect("profile target was validated before selecting the link contract")
    {
        GaneshBackend::Vulkan => (GANESH_LINUX_SYSTEM_LIBRARIES, &[]),
        GaneshBackend::MacOSMetal => (GANESH_APPLE_SYSTEM_LIBRARIES, GANESH_MACOS_FRAMEWORKS),
        GaneshBackend::IosMetal => (GANESH_APPLE_SYSTEM_LIBRARIES, GANESH_IOS_FRAMEWORKS),
        GaneshBackend::D3D12 => (GANESH_WINDOWS_SYSTEM_LIBRARIES, &[]),
        GaneshBackend::AndroidVulkan => (GANESH_ANDROID_SYSTEM_LIBRARIES, &[]),
    }
}

fn validate_bridge_recipe(
    profile: &str,
    target: &str,
    sources: &[String],
    defines: &BTreeMap<String, String>,
) {
    if sources
        .iter()
        .map(String::as_str)
        .ne(bridge_sources(profile, target).iter().copied())
    {
        panic!("Skia artifact bridge sources do not match the requested {profile}/{target} recipe");
    }
    if *defines != bridge_defines(profile, target) {
        panic!("Skia artifact bridge defines do not match the requested {profile}/{target} recipe");
    }
}

fn validate_profile_target(profile: &str, target: &str) {
    match profile {
        "native-raster" if native_contract::supports_native_raster(target) => {}
        "native-raster" => panic!(
            "Skia profile native-raster is not available for {target}; select one of the \
             native targets declared by the Fission Skia artifact matrix"
        ),
        "native-ganesh" if ganesh_backend(target).is_some() => {}
        "native-ganesh" => panic!(
            "Skia profile native-ganesh is not available for {target}; supported targets are \
             Linux GNU x86_64/arm64, macOS x86_64/arm64, iOS device/simulator arm64/x86_64, \
             Windows MSVC x86_64/arm64, and Android arm64/armv7/x86_64/x86"
        ),
        _ => panic!(
            "unsupported FISSION_SKIA_PROFILE={profile:?}; select native-raster or native-ganesh"
        ),
    }
}

fn verify_files(root: &Path, files: &[ArtifactFile], actual: std::collections::BTreeSet<String>) {
    let declared =
        native_contract::declared_file_set(files.iter().map(|entry| entry.path.as_str()))
            .unwrap_or_else(|error| panic!("{error}"));
    let mut declared_bytes = 0u64;
    for entry in files {
        validate_sha256(&entry.sha256, "Skia artifact payload");
        declared_bytes = declared_bytes
            .checked_add(entry.size)
            .unwrap_or_else(|| panic!("Skia artifact payload size overflowed u64"));
        if declared_bytes > native_contract::MAX_EXPANDED_BYTES {
            panic!(
                "Skia artifact payload exceeds the {}-byte expanded limit",
                native_contract::MAX_EXPANDED_BYTES
            );
        }
        let relative =
            native_contract::safe_relative(&entry.path).unwrap_or_else(|error| panic!("{error}"));
        let path = root.join(&relative);
        println!("cargo:rerun-if-changed={}", path.display());
        let metadata = fs::symlink_metadata(&path).unwrap_or_else(|error| {
            panic!(
                "failed to inspect artifact file {}: {error}",
                path.display()
            )
        });
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            panic!("artifact payload is not a regular file: {}", path.display());
        }
        if metadata.len() != entry.size {
            panic!("artifact file {} has the wrong size", path.display());
        }
        let actual_digest = sha256_file(&path);
        if actual_digest != entry.sha256 {
            panic!(
                "artifact file {} failed SHA-256 verification",
                path.display()
            );
        }
    }
    native_contract::verify_payload_set(actual, &declared)
        .unwrap_or_else(|error| panic!("{error}"));
}

fn validate_sha256(value: &str, description: &str) {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        panic!("{description} SHA-256 must be 64 lowercase hexadecimal characters");
    }
}

fn sha256_file(path: &Path) -> String {
    let mut file = File::open(path)
        .unwrap_or_else(|error| panic!("failed to open {}: {error}", path.display()));
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .unwrap_or_else(|error| panic!("failed to hash {}: {error}", path.display()));
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    format!("{:x}", digest.finalize())
}

fn emit_native_link(root: &Path, native: &NativeLink, local_override: bool) {
    for relative in &native.link_search_paths {
        let relative =
            native_contract::safe_relative(relative).unwrap_or_else(|error| panic!("{error}"));
        println!(
            "cargo:rustc-link-search=native={}",
            root.join(relative).display()
        );
    }
    let override_libraries = env::var("FISSION_SKIA_LINK_LIBS").ok();
    if override_libraries.is_some() && !local_override {
        panic!(
            "FISSION_SKIA_LINK_LIBS may override only an explicit local \
             FISSION_SKIA_ARTIFACT_DIR; downloaded release artifacts use their locked link contract"
        );
    }
    let libraries: Vec<&str> = override_libraries
        .as_deref()
        .map(|value| {
            native_contract::parse_link_override(value).unwrap_or_else(|error| panic!("{error}"))
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

fn required_dir(variable: &str) -> PathBuf {
    let value = env::var_os(variable).unwrap_or_else(|| panic!("{variable} must be set"));
    if value.is_empty() {
        panic!("{variable} must not be empty");
    }
    let path = PathBuf::from(value);
    if !path.is_dir() {
        panic!("{variable}={} is not a directory", path.display());
    }
    path
}

fn required_real_dir(variable: &str) -> PathBuf {
    let path = required_dir(variable);
    let metadata = fs::symlink_metadata(&path)
        .unwrap_or_else(|error| panic!("failed to inspect {variable}={}: {error}", path.display()));
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        panic!("{variable}={} is not a real directory", path.display());
    }
    path
}

fn optional_boolean_environment(variable: &str) -> bool {
    env::var_os(variable)
        .map(|value| {
            let value = value
                .to_str()
                .unwrap_or_else(|| panic!("{variable} is not valid UTF-8"));
            native_contract::parse_boolean(value, variable)
                .unwrap_or_else(|error| panic!("{error}"))
        })
        .unwrap_or(false)
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
    compile_bridge(&source, &build, &profile, &target);
    println!("cargo:rustc-link-search=native={}", build.display());
    let links = env::var("FISSION_SKIA_LINK_LIBS")
        .unwrap_or_else(|_| "svg,skparagraph,skshaper,skunicode,skia".into());
    let libraries =
        native_contract::parse_link_override(&links).unwrap_or_else(|error| panic!("{error}"));
    for library in libraries {
        println!("cargo:rustc-link-lib=static={library}");
    }
    if profile == "native-ganesh" {
        let (system_libraries, frameworks) = ganesh_link_contract(&target);
        for library in system_libraries {
            println!("cargo:rustc-link-lib={library}");
        }
        for framework in frameworks {
            println!("cargo:rustc-link-lib=framework={framework}");
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
        .ne(bridge_sources(profile, target).iter().copied().map(Some))
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
    if actual_defines != bridge_defines(profile, target) {
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
        ("skia_use_gl", false),
    ] {
        if gn_args.get(name).and_then(serde_json::Value::as_bool) != Some(expected) {
            panic!("{} does not pin {name}={expected}", path.display());
        }
    }
    let (expected_os, expected_cpu, metal, vulkan, direct3d) = match target {
        "x86_64-unknown-linux-gnu" => ("linux", "x64", false, true, false),
        "aarch64-unknown-linux-gnu" => ("linux", "arm64", false, true, false),
        "x86_64-apple-darwin" => ("mac", "x64", true, false, false),
        "aarch64-apple-darwin" => ("mac", "arm64", true, false, false),
        "aarch64-apple-ios" | "aarch64-apple-ios-sim" => ("ios", "arm64", true, false, false),
        "x86_64-apple-ios" => ("ios", "x64", true, false, false),
        "x86_64-pc-windows-msvc" => ("win", "x64", false, false, true),
        "aarch64-pc-windows-msvc" => ("win", "arm64", false, false, true),
        "aarch64-linux-android" => ("android", "arm64", false, true, false),
        "armv7-linux-androideabi" => ("android", "arm", false, true, false),
        "x86_64-linux-android" => ("android", "x64", false, true, false),
        "i686-linux-android" => ("android", "x86", false, true, false),
        _ => unreachable!("native-ganesh target was validated"),
    };
    for (name, expected) in [
        ("skia_use_metal", metal),
        ("skia_use_vulkan", vulkan),
        ("skia_use_direct3d", direct3d),
    ] {
        if gn_args.get(name).and_then(serde_json::Value::as_bool) != Some(expected) {
            panic!("{} does not pin {name}={expected}", path.display());
        }
    }
    if gn_args.get("target_os").and_then(serde_json::Value::as_str) != Some(expected_os) {
        panic!("{} does not target {expected_os}", path.display());
    }
    if gn_args
        .get("target_cpu")
        .and_then(serde_json::Value::as_str)
        != Some(expected_cpu)
    {
        panic!("{} does not target CPU {expected_cpu}", path.display());
    }
    match ganesh_backend(target).expect("native-ganesh target was validated") {
        GaneshBackend::Vulkan => {
            for (name, expected) in [("skia_use_vma", true), ("skia_use_x11", false)] {
                if gn_args.get(name).and_then(serde_json::Value::as_bool) != Some(expected) {
                    panic!("{} does not pin {name}={expected}", path.display());
                }
            }
        }
        GaneshBackend::MacOSMetal => {}
        GaneshBackend::D3D12 => {}
        GaneshBackend::AndroidVulkan => {
            if gn_args
                .get("skia_use_vma")
                .and_then(serde_json::Value::as_bool)
                != Some(true)
            {
                panic!("{} does not pin skia_use_vma=true", path.display());
            }
            if !matches!(
                gn_args.get("ndk").and_then(serde_json::Value::as_str),
                Some(ndk) if !ndk.is_empty()
            ) {
                panic!("{} does not record an Android NDK path", path.display());
            }
            if !matches!(
                gn_args.get("ndk_api").and_then(serde_json::Value::as_u64),
                Some(api) if api >= 24
            ) {
                panic!(
                    "{} does not select the Android API 24 Vulkan baseline",
                    path.display()
                );
            }
        }
        GaneshBackend::IosMetal => {
            let simulator = target != "aarch64-apple-ios";
            if gn_args
                .get("ios_use_simulator")
                .and_then(serde_json::Value::as_bool)
                != Some(simulator)
            {
                panic!(
                    "{} does not pin ios_use_simulator={simulator}",
                    path.display()
                );
            }
            if gn_args
                .get("ios_min_target")
                .and_then(serde_json::Value::as_str)
                .is_none()
            {
                panic!("{} does not record an iOS minimum target", path.display());
            }
        }
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
fn compile_bridge(source: &Path, build: &Path, profile: &str, target: &str) {
    let revision_define = format!("\"{SKIA_REVISION}\"");
    let profile_define = format!("\"{profile}\"");
    let mut compiler = cc::Build::new();
    compiler.cpp(true);
    if matches!(ganesh_backend(target), Some(GaneshBackend::AndroidVulkan)) {
        // The exact Android C++ runtime is emitted through the pinned native
        // link contract below; do not let cc infer a second runtime choice.
        compiler.cpp_link_stdlib(None::<&str>);
    }
    for bridge_source in bridge_sources(profile, target) {
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
    for (name, value) in bridge_defines(profile, target) {
        compiler.define(&name, Some(value.as_str()));
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
