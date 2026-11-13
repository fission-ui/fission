//! Upgrade only known, unmodified generated Android capability bridges.
use anyhow::{bail, Context, Result};
use sha1::{Digest, Sha1};
use std::fs;
use std::path::Path;

const HELPER: &str = "platforms/android/java/rs/fission/runtime/FissionAndroidCapabilities.java";
const MARKER: &str = "platforms/android/.fission-capabilities.sha1";
const API_VERSION: &str = "public static final int FISSION_BRIDGE_API_VERSION = 1;";
// These identify public, unmodified generated templates, not a security boundary.
const LEGACY_GENERATED: &[&str] = &[
    "52c5592ac2fb0cbc1485a54b1a50d66f9882347d", // Before document pickers/printing.
    "198b2224fe75b1c0d98f80d24a8576d5dc43ebaa", // Initial picker/print template.
];

/// Refresh the Android bridge before packaging. Customised obsolete bridges are
/// preserved, with a candidate written next to them for an explicit merge.
pub fn prepare_android_capability_helper(root: &Path) -> Result<()> {
    prepare(
        root,
        super::render_android_capabilities_java(),
        LEGACY_GENERATED,
    )
}

fn fingerprint(source: &str) -> String {
    format!("{:x}", Sha1::digest(source.as_bytes()))
}

fn prepare(root: &Path, generated: &str, legacy: &[&str]) -> Result<()> {
    let path = root.join(HELPER);
    let previous = super::read_optional_string(&path)?;
    let marker = root.join(MARKER);
    let recorded = super::read_optional_string(&marker)?;
    let managed = previous.as_ref().is_none_or(|source| {
        let hash = fingerprint(source);
        source == generated
            || legacy.contains(&hash.as_str())
            || recorded.as_ref().is_some_and(|saved| saved.trim() == hash)
    });
    if managed {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        if previous.as_deref() != Some(generated) {
            fs::write(&path, generated)
                .with_context(|| format!("update managed Android bridge {}", path.display()))?;
        }
        fs::write(marker, fingerprint(generated))?;
        return Ok(());
    }
    let previous = previous.unwrap();
    let required = [
        API_VERSION,
        "public static void beginPickFile(",
        "public static int openPickedFileRead(",
        "public static int openPickedFileWrite(",
        "public static String printPdf(",
        "String pdfPath",
    ];
    if required.iter().all(|required| previous.contains(required)) {
        // Do not record a custom file as generated: future upgrades must never
        // silently replace these user changes.
        return Ok(());
    }
    let candidate = path.with_extension("java.fission-new");
    fs::write(&candidate, generated)?;
    bail!(
        "Android's customised capability bridge is incompatible with the current Fission picker/print API. \
         The original {} was preserved. Merge the updated implementation from {} into it, then rebuild; \
         do not merely change its version marker. Standard unmodified bridges upgrade automatically.",
        path.display(), candidate.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn root() -> std::path::PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let target = std::env::var_os("CARGO_TARGET_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let root = target.join("test-tmp").join(format!(
            "android-bridge-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join(Path::new(HELPER).parent().unwrap())).unwrap();
        root
    }

    #[test]
    fn generated_legacy_and_recorded_bridges_upgrade_but_custom_changes_do_not() {
        let root = root();
        let old = "old generated Java";
        fs::write(root.join(HELPER), old).unwrap();
        prepare(&root, "new generated Java", &[&fingerprint(old)]).unwrap();
        assert_eq!(
            fs::read_to_string(root.join(HELPER)).unwrap(),
            "new generated Java"
        );
        prepare(&root, "next generated Java", &[]).unwrap();
        fs::write(root.join(HELPER), "customised Java").unwrap();
        assert!(prepare(&root, "latest generated Java", &[]).is_err());
        assert_eq!(
            fs::read_to_string(root.join(HELPER)).unwrap(),
            "customised Java"
        );
        assert_eq!(
            fs::read_to_string(root.join(HELPER).with_extension("java.fission-new")).unwrap(),
            "latest generated Java"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn compatible_custom_bridges_remain_custom_and_missing_methods_fail_early() {
        let root = root();
        let generated = super::super::render_android_capabilities_java();
        let custom = format!("{generated}\n// App-specific change\n");
        fs::write(root.join(HELPER), &custom).unwrap();
        prepare(&root, generated, &[]).unwrap();
        assert_eq!(fs::read_to_string(root.join(HELPER)).unwrap(), custom);
        assert!(!root.join(MARKER).exists());
        fs::write(
            root.join(HELPER),
            custom.replace(
                "public static void beginPickFile(",
                "private static void renamedPickFile(",
            ),
        )
        .unwrap();
        assert!(prepare(&root, generated, &[]).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
