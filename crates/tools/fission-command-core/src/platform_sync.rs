use super::*;

pub(super) fn apply_native_module_config(root: &Path, project: &FissionProject) -> Result<()> {
    if project.targets.contains(&Target::Android) {
        write_file(
            &root.join("platforms/android/native-modules.gradle"),
            &render_android_native_modules_gradle(project),
        )?;
        apply_android_settings_gradle_hardening(root, project)?;
        apply_android_native_manifest_entries(root, project)?;
    }
    if project.targets.contains(&Target::Ios) {
        write_file(
            &root.join("platforms/ios/NativeModules/Package.swift"),
            &render_ios_native_modules_package(project),
        )?;
        write_file(
            &root.join(
                "platforms/ios/NativeModules/Sources/FissionNativeModules/FissionNativeCapabilities.swift",
            ),
            render_ios_native_capabilities_swift(),
        )?;
        sync_ios_native_module_sources(root, project)?;
    }
    Ok(())
}

fn apply_android_native_manifest_entries(root: &Path, project: &FissionProject) -> Result<()> {
    let entries = render_android_native_application_entries(project);
    if entries.trim().is_empty() {
        return Ok(());
    }
    let path = root.join("platforms/android/AndroidManifest.xml");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let missing = entries
        .lines()
        .filter(|entry| !entry.trim().is_empty() && !existing.contains(entry.trim()))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return Ok(());
    }

    let insertion = format!("{}\n", missing.join("\n"));
    let marker =
        "        <activity\n            android:name=\"rs.fission.runtime.FissionActivity\"";
    let updated = if let Some(index) = existing.find(marker) {
        let mut updated = existing.clone();
        updated.insert_str(index, &insertion);
        updated
    } else if let Some(index) = existing.find("</application>") {
        let mut updated = existing.clone();
        updated.insert_str(index, &insertion);
        updated
    } else {
        existing
    };

    if updated != fs::read_to_string(&path)? {
        fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

pub(super) fn sync_ios_native_module_sources(root: &Path, project: &FissionProject) -> Result<()> {
    let generated_root = root.join("platforms/ios/NativeModules/Sources/FissionNativeModules");
    fs::create_dir_all(&generated_root)
        .with_context(|| format!("failed to create {}", generated_root.display()))?;

    for module in &project.native.modules {
        let module_dir = generated_root.join(swift_module_source_dir_name(&module.name));
        if module_dir.exists() {
            fs::remove_dir_all(&module_dir)
                .with_context(|| format!("failed to remove {}", module_dir.display()))?;
        }
        if module.ios.source_dirs.is_empty() {
            continue;
        }
        fs::create_dir_all(&module_dir)
            .with_context(|| format!("failed to create {}", module_dir.display()))?;
        for source_dir in &module.ios.source_dirs {
            let source_dir = source_dir.trim();
            if source_dir.is_empty() {
                continue;
            }
            let source = resolve_project_path(root, source_dir);
            copy_dir_contents(&source, &module_dir).with_context(|| {
                format!(
                    "failed to copy iOS native module source {} into {}",
                    source.display(),
                    module_dir.display()
                )
            })?;
        }
    }
    Ok(())
}

fn resolve_project_path(root: &Path, value: &str) -> PathBuf {
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn swift_module_source_dir_name(name: &str) -> String {
    let mut output = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch);
        } else if !output.ends_with('_') {
            output.push('_');
        }
    }
    let output = output.trim_matches('_');
    if output.is_empty() {
        "module".to_string()
    } else {
        output.to_string()
    }
}

fn copy_dir_contents(source: &Path, dest: &Path) -> Result<()> {
    if source.is_file() {
        let file_name = source
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("source file has no file name"))?;
        fs::create_dir_all(dest)?;
        fs::copy(source, dest.join(file_name))?;
        return Ok(());
    }
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(source)
        .with_context(|| format!("failed to read native source dir {}", source.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let target = dest.join(entry.file_name());
        if path.is_dir() {
            copy_dir_contents(&path, &target)?;
        } else if path.is_file() {
            fs::copy(&path, &target)
                .with_context(|| format!("failed to copy {}", path.display()))?;
        }
    }
    Ok(())
}

/// Stages the project's `assets` directory as an application resource tree.
///
/// Desktop run and package commands use the same `assets` layout on every
/// platform so applications can ship large resources without compiling them
/// into the executable. A project without an `assets` directory is valid.
pub fn stage_project_assets(
    project_dir: &Path,
    destination_root: &Path,
) -> Result<Option<PathBuf>> {
    let source = project_dir.join("assets");
    if !source.exists() {
        return Ok(None);
    }
    if !source.is_dir() {
        bail!(
            "project assets path {} must be a directory",
            source.display()
        );
    }
    let destination = destination_root.join("assets");
    if destination.exists() {
        fs::remove_dir_all(&destination).with_context(|| {
            format!(
                "failed to clear staged project assets {}",
                destination.display()
            )
        })?;
    }
    copy_dir_contents(&source, &destination).with_context(|| {
        format!(
            "failed to stage project assets from {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(Some(destination))
}

pub(super) fn apply_mobile_run_script_hardening(
    root: &Path,
    project: &FissionProject,
) -> Result<()> {
    if project.targets.contains(&Target::Ios) {
        apply_ios_run_script_hardening(root)?;
        apply_ios_package_script_hardening(root)?;
    }
    if project.targets.contains(&Target::Android) {
        apply_android_run_script_hardening(root)?;
        apply_android_package_script_hardening(root)?;
        apply_android_manifest_hardening(root)?;
        apply_android_root_build_gradle_hardening(root)?;
        apply_android_app_build_gradle_hardening(root)?;
        apply_android_gradle_properties_hardening(root)?;
    }
    Ok(())
}

fn apply_ios_run_script_hardening(root: &Path) -> Result<()> {
    let path = root.join("platforms/ios/run-sim.sh");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    if existing.contains("IOS_SIM_UNINSTALL_BEFORE_INSTALL") {
        return Ok(());
    }
    let marker = "xcrun simctl bootstatus \"$DEVICE_ID\" -b\n";
    let insertion = "xcrun simctl bootstatus \"$DEVICE_ID\" -b\nif [[ \"${IOS_SIM_UNINSTALL_BEFORE_INSTALL:-1}\" == \"1\" ]]; then\n  xcrun simctl uninstall \"$DEVICE_ID\" \"$BUNDLE_ID\" >/dev/null 2>&1 || true\nfi\n";
    let updated = existing.replacen(marker, insertion, 1);
    fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))
}

fn apply_ios_package_script_hardening(root: &Path) -> Result<()> {
    let path = root.join("platforms/ios/package-sim.sh");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut updated = existing.clone();
    if updated.contains("import plistlib") {
        let Some(start) = updated.find("python3 - <<'PY' \"$SCRIPT_DIR/Info.plist\"") else {
            return Ok(());
        };
        let Some(relative_end) = updated[start..].find("\nPY") else {
            return Ok(());
        };
        let end = start + relative_end + "\nPY\n".len();
        updated.replace_range(start..end, IOS_INFO_PLIST_PLUTIL_PATCH);
    }
    if !updated.contains("IOS_MARKETING_VERSION") {
        updated = updated.replacen(
            "BUNDLE_NAME=\"${IOS_BUNDLE_NAME:-$DISPLAY_NAME.app}\"\n",
            "BUNDLE_NAME=\"${IOS_BUNDLE_NAME:-$DISPLAY_NAME.app}\"\nIOS_MARKETING_VERSION=\"${IOS_MARKETING_VERSION:-0.1.0}\"\nIOS_BUILD_NUMBER=\"${IOS_BUILD_NUMBER:-1}\"\n",
            1,
        );
    }
    if updated != existing {
        fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

fn apply_android_run_script_hardening(root: &Path) -> Result<()> {
    let path = root.join("platforms/android/run-emulator.sh");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    if existing.contains(":app:assemble") {
        return Ok(());
    }
    let mut updated = existing.clone();
    let wait_function = android_wait_for_boot_function();
    if let Some(start) = updated.find("wait_for_android_boot() {") {
        let marker = "\n}\n\nANDROID_EMULATOR_API_LEVEL=";
        if let Some(relative_end) = updated[start..].find(marker) {
            let end = start + relative_end + "\n}\n\n".len();
            updated.replace_range(start..end, &format!("{wait_function}\n\n"));
        }
    } else {
        updated = updated.replacen(
            "\nANDROID_EMULATOR_API_LEVEL=",
            &format!("\n{wait_function}\n\nANDROID_EMULATOR_API_LEVEL="),
            1,
        );
    }
    updated =
        replace_android_boot_wait_after(updated, "  disown || true\n", "  wait_for_android_boot\n");
    updated = replace_android_boot_wait_after(
        updated,
        "  \"$EMULATOR_BIN\" \"${EMULATOR_ARGS[@]}\" >/tmp/fission-android-emulator.log 2>&1 &\n",
        "  wait_for_android_boot\n",
    );
    if !updated.contains(
        "printf 'Using existing emulator %s\\n' \"$RUNNING_EMULATOR\"\n  wait_for_android_boot\n",
    ) {
        updated = updated.replacen(
            "printf 'Using existing emulator %s\\n' \"$RUNNING_EMULATOR\"\n",
            "printf 'Using existing emulator %s\\n' \"$RUNNING_EMULATOR\"\n  wait_for_android_boot\n",
            1,
        );
    }
    while updated.contains("  wait_for_android_boot\n  wait_for_android_boot\n") {
        updated = updated.replace(
            "  wait_for_android_boot\n  wait_for_android_boot\n",
            "  wait_for_android_boot\n",
        );
    }
    updated = updated.replace(
        "\"$ADB\" install -r \"$APK\"",
        "read -r -a ADB_INSTALL_FLAGS <<< \"${ADB_INSTALL_FLAGS:---no-streaming -r}\"\n\"$ADB\" install \"${ADB_INSTALL_FLAGS[@]}\" \"$APK\"",
    );
    if updated != existing {
        fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

fn apply_android_package_script_hardening(root: &Path) -> Result<()> {
    let path = root.join("platforms/android/package-apk.sh");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut updated = existing.clone();
    if updated.contains("import re\nimport sys\n") && !updated.contains("import pathlib\n") {
        updated = updated.replace(
            "import re\nimport sys\n",
            "import pathlib\nimport re\nimport sys\n",
        );
    }
    let has_code_line = r#"has_code = "true" if pathlib.Path(dest).with_name("apk-root").joinpath("classes.dex").exists() else "false"
manifest = re.sub(r'android:hasCode="(?:true|false)"', f'android:hasCode="{has_code}"', manifest)
"#;
    if !updated.contains("android:hasCode=") || !updated.contains("with_name(\"apk-root\")") {
        updated = updated.replace(
            "manifest = re.sub(r'android:targetSdkVersion=\"\\d+\"', f'android:targetSdkVersion=\"{target_api}\"', manifest)\n",
            &format!(
                "manifest = re.sub(r'android:targetSdkVersion=\"\\d+\"', f'android:targetSdkVersion=\"{{target_api}}\"', manifest)\n{has_code_line}"
            ),
        );
    }
    if updated != existing {
        fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

fn apply_android_manifest_hardening(root: &Path) -> Result<()> {
    let path = root.join("platforms/android/AndroidManifest.xml");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    if existing.contains("rs.fission.runtime.FissionActivity") {
        return Ok(());
    }
    let updated = existing.replace(r#"android:hasCode="true""#, r#"android:hasCode="false""#);
    if updated != existing {
        fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

fn apply_android_root_build_gradle_hardening(root: &Path) -> Result<()> {
    let path = root.join("platforms/android/build.gradle.kts");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut updated = String::new();
    for line in existing.lines() {
        if line
            .trim_start()
            .starts_with("id(\"com.android.application\") version ")
        {
            let indent = line
                .chars()
                .take_while(|ch| ch.is_whitespace())
                .collect::<String>();
            updated.push_str(&format!(
                "{indent}id(\"com.android.application\") version \"{ANDROID_GRADLE_PLUGIN_VERSION}\" apply false\n"
            ));
        } else {
            updated.push_str(line);
            updated.push('\n');
        }
    }
    if updated != existing {
        fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

fn apply_android_app_build_gradle_hardening(root: &Path) -> Result<()> {
    let path = root.join("platforms/android/app/build.gradle.kts");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut updated = existing.replace("../native-modules.gradle.kts", "../native-modules.gradle");
    updated = updated.replace(
        "versionCode = 1",
        "versionCode = (System.getenv(\"ANDROID_VERSION_CODE\") ?: \"1\").toInt()",
    );
    updated = updated.replace(
        "versionName = \"0.1.0\"",
        "versionName = System.getenv(\"ANDROID_VERSION_NAME\") ?: \"0.1.0\"",
    );
    if !updated.contains("../native-modules.gradle") {
        updated.push_str("\napply(from = \"../native-modules.gradle\")\n");
    }
    if updated != existing {
        fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

fn apply_android_gradle_properties_hardening(root: &Path) -> Result<()> {
    let path = root.join("platforms/android/gradle.properties");
    if !path.exists() {
        return fs::write(&path, render_android_gradle_properties())
            .with_context(|| format!("failed to write {}", path.display()));
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut saw_androidx = false;
    let mut saw_jvmargs = false;
    let mut saw_compile_warning = false;
    let mut updated = String::new();
    for line in existing.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("android.useAndroidX=") {
            updated.push_str("android.useAndroidX=true\n");
            saw_androidx = true;
        } else if trimmed.starts_with("org.gradle.jvmargs=") {
            updated.push_str(line);
            updated.push('\n');
            saw_jvmargs = true;
        } else if trimmed.starts_with("android.javaCompile.suppressSourceTargetDeprecationWarning=")
        {
            updated.push_str(line);
            updated.push('\n');
            saw_compile_warning = true;
        } else {
            updated.push_str(line);
            updated.push('\n');
        }
    }
    if !saw_androidx {
        if !updated.ends_with('\n') {
            updated.push('\n');
        }
        updated.push_str("android.useAndroidX=true\n");
    }
    if !saw_jvmargs {
        updated.push_str("org.gradle.jvmargs=-Xmx2048m -Dfile.encoding=UTF-8\n");
    }
    if !saw_compile_warning {
        updated.push_str("android.javaCompile.suppressSourceTargetDeprecationWarning=true\n");
    }
    if updated != existing {
        fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

fn apply_android_settings_gradle_hardening(root: &Path, project: &FissionProject) -> Result<()> {
    let path = root.join("platforms/android/settings.gradle.kts");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let missing = android_dependency_repositories(project)
        .into_iter()
        .filter(|repository| !existing.contains(repository))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return Ok(());
    }
    let marker = "    repositories {\n";
    let Some(index) = existing.find(marker) else {
        return Ok(());
    };
    let mut insertion = String::new();
    for repository in missing {
        insertion.push_str("        ");
        insertion.push_str(&repository);
        insertion.push('\n');
    }
    let mut updated = existing;
    updated.insert_str(index + marker.len(), &insertion);
    fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))
}

fn android_wait_for_boot_function() -> &'static str {
    r#"wait_for_android_boot() {
  "$ADB" wait-for-device
  until "$ADB" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r' | grep -q '^1$'; do
    sleep 1
  done
  local deadline=$((SECONDS + 180))
  until "$ADB" shell cmd package list packages >/dev/null 2>&1; do
    if (( SECONDS > deadline )); then
      printf 'Android package manager did not become available. Restart the emulator with ANDROID_EMULATOR_RESTART=1 and try again.\n' >&2
      exit 1
    fi
    sleep 1
  done
}"#
}

fn replace_android_boot_wait_after(mut text: String, marker: &str, replacement: &str) -> String {
    let Some(start) = text.find(marker) else {
        return text;
    };
    let wait_start = start + marker.len();
    let old_wait = "  \"$ADB\" wait-for-device\n  until \"$ADB\" shell getprop sys.boot_completed 2>/dev/null | tr -d '\\r' | grep -q '^1$'; do\n    sleep 1\n  done\n";
    if text[wait_start..].starts_with(old_wait) {
        text.replace_range(wait_start..wait_start + old_wait.len(), replacement);
    }
    text
}

pub(super) const IOS_INFO_PLIST_PLUTIL_PATCH: &str = r#"cp "$SCRIPT_DIR/Info.plist" "$BUNDLE_DIR/Info.plist"
PLUTIL=$(xcrun --find plutil 2>/dev/null || command -v plutil || true)
if [[ -z "$PLUTIL" ]]; then
  printf 'plutil not found. Install Xcode command line tools to package the iOS simulator app.\n' >&2
  exit 1
fi
"$PLUTIL" -replace CFBundleIdentifier -string "$BUNDLE_ID" "$BUNDLE_DIR/Info.plist"
"$PLUTIL" -replace CFBundleDisplayName -string "$DISPLAY_NAME" "$BUNDLE_DIR/Info.plist"
"$PLUTIL" -replace CFBundleName -string "$DISPLAY_NAME" "$BUNDLE_DIR/Info.plist"
"$PLUTIL" -replace CFBundleExecutable -string "$EXECUTABLE_NAME" "$BUNDLE_DIR/Info.plist"
"$PLUTIL" -replace CFBundleShortVersionString -string "$IOS_MARKETING_VERSION" "$BUNDLE_DIR/Info.plist"
"$PLUTIL" -replace CFBundleVersion -string "$IOS_BUILD_NUMBER" "$BUNDLE_DIR/Info.plist"
"#;

pub(super) fn apply_platform_capability_config(
    root: &Path,
    project: &FissionProject,
) -> Result<()> {
    if project.capabilities.is_empty() {
        return Ok(());
    }
    if project.targets.contains(&Target::Android) {
        ensure_android_capability_helper(root)?;
        apply_android_capability_config(root, project)?;
    }
    if project.targets.contains(&Target::Ios) {
        apply_ios_capability_config(root, project)?;
    }
    Ok(())
}

fn ensure_android_capability_helper(root: &Path) -> Result<()> {
    write_file_with_policy(
        &root.join("platforms/android/java/rs/fission/runtime/FissionAndroidCapabilities.java"),
        render_android_capabilities_java(),
        WritePolicy::PreserveExisting,
    )
}

fn apply_android_capability_config(root: &Path, project: &FissionProject) -> Result<()> {
    let path = root.join("platforms/android/AndroidManifest.xml");
    if !path.exists() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut capabilities = String::new();
    if project.capabilities.contains(&PlatformCapability::Nfc)
        && !existing.contains("android.permission.NFC")
    {
        capabilities.push_str(&render_android_nfc_manifest_entries());
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Notifications)
        && !existing.contains("android.permission.POST_NOTIFICATIONS")
    {
        capabilities.push_str(&render_android_notifications_manifest_entries());
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Biometric)
        && !existing.contains("android.permission.USE_BIOMETRIC")
    {
        capabilities.push_str(&render_android_biometric_manifest_entries());
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Bluetooth)
    {
        capabilities.push_str(&render_missing_android_bluetooth_manifest_entries(
            &existing,
        ));
    }
    if project
        .capabilities
        .contains(&PlatformCapability::BarcodeScanner)
        && !project.capabilities.contains(&PlatformCapability::Camera)
        && !existing.contains("android.permission.CAMERA")
    {
        capabilities.push_str(&render_android_barcode_camera_manifest_entries());
    }
    if project.capabilities.contains(&PlatformCapability::Camera) {
        capabilities.push_str(&render_missing_android_camera_manifest_entries(&existing));
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Geolocation)
        && !existing.contains("android.permission.ACCESS_FINE_LOCATION")
    {
        capabilities.push_str(&render_android_geolocation_manifest_entries());
    }
    if project.capabilities.contains(&PlatformCapability::Haptics)
        && !existing.contains("android.permission.VIBRATE")
    {
        capabilities.push_str(&render_android_haptics_manifest_entries());
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Microphone)
        && !existing.contains("android.permission.RECORD_AUDIO")
    {
        capabilities.push_str(&render_android_microphone_manifest_entries());
    }
    if project.capabilities.contains(&PlatformCapability::Wifi) {
        capabilities.push_str(&render_missing_android_wifi_manifest_entries(&existing));
    }
    if project
        .capabilities
        .contains(&PlatformCapability::VolumeControl)
        && !existing.contains("android.permission.MODIFY_AUDIO_SETTINGS")
    {
        capabilities.push_str(&render_android_volume_manifest_entries());
    }
    if capabilities.is_empty() {
        return Ok(());
    }
    let marker = r#"    <uses-permission android:name="android.permission.INTERNET" />"#;
    let updated = if existing.contains(marker) {
        existing.replacen(marker, &format!("{marker}\n{capabilities}"), 1)
    } else {
        existing.replacen("<uses-sdk", &format!("{capabilities}\n    <uses-sdk"), 1)
    };
    fs::write(&path, updated).with_context(|| format!("failed to write {}", path.display()))
}

fn apply_ios_capability_config(root: &Path, project: &FissionProject) -> Result<()> {
    let info_path = root.join("platforms/ios/Info.plist");
    if info_path.exists() {
        let existing = fs::read_to_string(&info_path)
            .with_context(|| format!("failed to read {}", info_path.display()))?;
        if project.capabilities.contains(&PlatformCapability::Nfc)
            && !existing.contains("NFCReaderUsageDescription")
        {
            let entry = "  <key>NFCReaderUsageDescription</key>\n  <string>This app uses NFC to scan nearby tags when you request it.</string>\n";
            let updated = existing.replacen("</dict>", &format!("{entry}</dict>"), 1);
            fs::write(&info_path, updated)
                .with_context(|| format!("failed to write {}", info_path.display()))?;
        }
    }

    if project.capabilities.contains(&PlatformCapability::Nfc) {
        let entitlements_path = root.join("platforms/ios/Entitlements.plist");
        if entitlements_path.exists() {
            let existing = fs::read_to_string(&entitlements_path)
                .with_context(|| format!("failed to read {}", entitlements_path.display()))?;
            if !existing.contains("com.apple.developer.nfc.readersession.formats") {
                let entry = "  <key>com.apple.developer.nfc.readersession.formats</key>\n  <array>\n    <string>NDEF</string>\n  </array>\n";
                let updated = existing.replacen("</dict>", &format!("{entry}</dict>"), 1);
                fs::write(&entitlements_path, updated)
                    .with_context(|| format!("failed to write {}", entitlements_path.display()))?;
            }
        } else {
            write_file_with_policy(
                &entitlements_path,
                IOS_NFC_ENTITLEMENTS_PLIST,
                WritePolicy::PreserveExisting,
            )?;
        }
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Biometric)
        && info_path.exists()
    {
        let existing = fs::read_to_string(&info_path)
            .with_context(|| format!("failed to read {}", info_path.display()))?;
        if !existing.contains("NSFaceIDUsageDescription") {
            let entry = "  <key>NSFaceIDUsageDescription</key>\n  <string>This app uses biometrics to authenticate you when you request it.</string>\n";
            let updated = existing.replacen("</dict>", &format!("{entry}</dict>"), 1);
            fs::write(&info_path, updated)
                .with_context(|| format!("failed to write {}", info_path.display()))?;
        }
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Bluetooth)
        && info_path.exists()
    {
        let existing = fs::read_to_string(&info_path)
            .with_context(|| format!("failed to read {}", info_path.display()))?;
        if !existing.contains("NSBluetoothAlwaysUsageDescription") {
            let entry = "  <key>NSBluetoothAlwaysUsageDescription</key>\n  <string>This app uses Bluetooth when you request nearby-device features.</string>\n";
            let updated = existing.replacen("</dict>", &format!("{entry}</dict>"), 1);
            fs::write(&info_path, updated)
                .with_context(|| format!("failed to write {}", info_path.display()))?;
        }
    }
    if project
        .capabilities
        .contains(&PlatformCapability::BarcodeScanner)
        && info_path.exists()
    {
        let existing = fs::read_to_string(&info_path)
            .with_context(|| format!("failed to read {}", info_path.display()))?;
        if !existing.contains("NSCameraUsageDescription") {
            let entry = "  <key>NSCameraUsageDescription</key>\n  <string>This app uses the camera to scan barcodes when you request it.</string>\n";
            let updated = existing.replacen("</dict>", &format!("{entry}</dict>"), 1);
            fs::write(&info_path, updated)
                .with_context(|| format!("failed to write {}", info_path.display()))?;
        }
    }
    if project.capabilities.contains(&PlatformCapability::Camera) && info_path.exists() {
        let existing = fs::read_to_string(&info_path)
            .with_context(|| format!("failed to read {}", info_path.display()))?;
        if !existing.contains("NSCameraUsageDescription") {
            let entry = "  <key>NSCameraUsageDescription</key>\n  <string>This app uses the camera when you request camera features.</string>\n";
            let updated = existing.replacen("</dict>", &format!("{entry}</dict>"), 1);
            fs::write(&info_path, updated)
                .with_context(|| format!("failed to write {}", info_path.display()))?;
        }
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Geolocation)
        && info_path.exists()
    {
        let existing = fs::read_to_string(&info_path)
            .with_context(|| format!("failed to read {}", info_path.display()))?;
        if !existing.contains("NSLocationWhenInUseUsageDescription") {
            let entry = "  <key>NSLocationWhenInUseUsageDescription</key>\n  <string>This app uses your location when you request location-aware features.</string>\n";
            let updated = existing.replacen("</dict>", &format!("{entry}</dict>"), 1);
            fs::write(&info_path, updated)
                .with_context(|| format!("failed to write {}", info_path.display()))?;
        }
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Microphone)
        && info_path.exists()
    {
        let existing = fs::read_to_string(&info_path)
            .with_context(|| format!("failed to read {}", info_path.display()))?;
        if !existing.contains("NSMicrophoneUsageDescription") {
            let entry = "  <key>NSMicrophoneUsageDescription</key>\n  <string>This app uses the microphone when you request audio capture.</string>\n";
            let updated = existing.replacen("</dict>", &format!("{entry}</dict>"), 1);
            fs::write(&info_path, updated)
                .with_context(|| format!("failed to write {}", info_path.display()))?;
        }
    }
    if project.capabilities.contains(&PlatformCapability::Wifi) && info_path.exists() {
        let existing = fs::read_to_string(&info_path)
            .with_context(|| format!("failed to read {}", info_path.display()))?;
        if !existing.contains("NSLocationWhenInUseUsageDescription") {
            let entry = "  <key>NSLocationWhenInUseUsageDescription</key>\n  <string>This app uses location permission where the platform requires it for Wi-Fi information.</string>\n";
            let updated = existing.replacen("</dict>", &format!("{entry}</dict>"), 1);
            fs::write(&info_path, updated)
                .with_context(|| format!("failed to write {}", info_path.display()))?;
        }
    }
    if project.capabilities.contains(&PlatformCapability::Wifi) {
        let entitlements_path = root.join("platforms/ios/Entitlements.plist");
        apply_ios_wifi_entitlements(&entitlements_path)?;
    }
    Ok(())
}

fn apply_ios_wifi_entitlements(path: &Path) -> Result<()> {
    if path.exists() {
        let existing = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let mut entry = String::new();
        if !existing.contains("com.apple.developer.networking.wifi-info") {
            entry.push_str("  <key>com.apple.developer.networking.wifi-info</key>\n  <true/>\n");
        }
        if !existing.contains("com.apple.developer.networking.HotspotConfiguration") {
            entry.push_str(
                "  <key>com.apple.developer.networking.HotspotConfiguration</key>\n  <true/>\n",
            );
        }
        if entry.is_empty() {
            return Ok(());
        }
        let updated = existing.replacen("</dict>", &format!("{entry}</dict>"), 1);
        fs::write(path, updated).with_context(|| format!("failed to write {}", path.display()))?;
        return Ok(());
    }
    write_file_with_policy(
        path,
        IOS_WIFI_ENTITLEMENTS_PLIST,
        WritePolicy::PreserveExisting,
    )
}

pub(super) fn target_scaffold_dir_exists(project_dir: &Path, target: Target) -> bool {
    if target == Target::Site && project_dir.join("content").exists() {
        return true;
    }
    if target == Target::Site && project_dir.join("platforms/site").exists() {
        return true;
    }
    if target == Target::Server && project_dir.join("platforms/server").exists() {
        return true;
    }
    Path::new(target.scaffold_relative_path())
        .parent()
        .is_some_and(|relative| project_dir.join(relative).exists())
}
