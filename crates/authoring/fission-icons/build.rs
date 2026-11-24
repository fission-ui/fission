use heck::ToSnakeCase;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

// Structure: Category -> IconName -> Variant -> Path (relative to crate root)
type IconMap = BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>>;
type IconOffsets = BTreeMap<String, [usize; 2]>;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("material_icons.rs");
    println!("cargo:rerun-if-env-changed=FISSION_MATERIAL_ICONS_DIR");

    // CARGO_MANIFEST_DIR points to crates/authoring/fission-icons
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_dir = Path::new(&manifest_dir);
    let submodule_root = manifest_dir.join("material-design-icons/src");
    let vendor_root = manifest_dir.join("material-icons-vendor");
    let vendor_index = vendor_root.join("index.json");
    let vendor_src = vendor_root.join("src");

    let override_root = env::var("FISSION_MATERIAL_ICONS_DIR")
        .ok()
        .map(PathBuf::from)
        .filter(|p| p.exists());

    let mut vendor_bundle = None;
    let icons = if let Some(root) = override_root {
        println!("cargo:rerun-if-changed={}", root.display());
        scan_icons_root(&root, manifest_dir)
    } else if vendor_index.exists() {
        println!("cargo:rerun-if-changed=material-icons-vendor/index.json");
        match load_icons_index(&vendor_index) {
            Ok(map) => {
                let packed_asset = vendor_root.join("material-icons.bin");
                let offsets_path = vendor_root.join("offsets.json");
                if packed_asset.is_file() && offsets_path.is_file() {
                    println!("cargo:rerun-if-changed={}", packed_asset.display());
                    println!("cargo:rerun-if-changed={}", offsets_path.display());
                    vendor_bundle = Some((packed_asset, offsets_path));
                }
                map
            }
            Err(err) => {
                println!("cargo:warning=Failed to parse {:?}: {}", vendor_index, err);
                fs::write(&dest_path, "// Material Icons not found\n").unwrap();
                return;
            }
        }
    } else {
        let src_root = resolve_icons_root(&submodule_root, &vendor_src);
        let Some(src_root) = src_root else {
            println!(
                "cargo:warning=Material Icons sources not found. Expected {:?} or {:?}.",
                submodule_root, vendor_src
            );
            fs::write(&dest_path, "// Material Icons not found\n").unwrap();
            return;
        };

        if src_root == submodule_root {
            // Avoid reruns on the huge submodule unless explicitly using it.
            println!("cargo:rerun-if-changed=material-design-icons/src");
        } else {
            println!("cargo:rerun-if-changed=material-icons-vendor/src");
        }

        scan_icons_root(&src_root, manifest_dir)
    };

    let sources = if let Some((packed_asset, offsets_path)) = vendor_bundle {
        match load_vendor_sources(&icons, &packed_asset, &offsets_path) {
            Some(sources) => sources,
            None => {
                println!(
                    "cargo:warning=Material icon packed offsets are invalid; reading indexed SVGs."
                );
                load_icon_sources(&icons, manifest_dir)
            }
        }
    } else {
        load_icon_sources(&icons, manifest_dir)
    };

    let mut code = String::new();
    code.push_str("// Generated Material Icons\n\n");
    // The pack is a build-time input, not runtime storage. Independent literals
    // let the linker discard unused SVGs; slices into one static pack retain the
    // entire catalogue even when an app uses a single icon. Debug formatting
    // escapes each valid UTF-8 source as a Rust string literal without unsafe code.

    let reflection_enabled = env::var("CARGO_FEATURE_REFLECTION").is_ok();
    let mut reflection_entries = Vec::new();

    for (category, icon_map) in &icons {
        let mod_name = sanitize_keyword(&category.to_snake_case());
        code.push_str(&format!("pub mod {} {{\n", mod_name));

        for (icon_name, variants) in icon_map {
            let icon_mod = sanitize_keyword(&icon_name.to_snake_case());
            if icon_mod.chars().next().unwrap().is_numeric() {
                continue;
            }

            code.push_str(&format!("    pub mod {} {{\n", icon_mod));

            for (variant, path) in variants {
                let source = sources
                    .get(path)
                    .unwrap_or_else(|| panic!("No source for Material Icon {path}"));
                code.push_str(&format!(
                    "        pub const fn {}() -> &'static str {{ {:?} }}\n",
                    variant, source
                ));

                if reflection_enabled {
                    reflection_entries.push(format!(
                        "(\"{}\", \"{}\", \"{}\", {}::{}::{})",
                        category, icon_name, variant, mod_name, icon_mod, variant
                    ));
                }
            }
            code.push_str("    }\n");
        }
        code.push_str("}\n");
    }

    if reflection_enabled {
        code.push_str("\n#[cfg(feature = \"reflection\")]\n");
        code.push_str("pub fn all_icons() -> Vec<(&'static str, &'static str, &'static str, fn() -> &'static str)> {\n");
        code.push_str("    vec![\n");
        for entry in reflection_entries {
            code.push_str(&format!("        {},\n", entry));
        }
        code.push_str("    ]\n");
        code.push_str("}\n");
    }

    fs::write(&dest_path, code).unwrap();
}

fn load_vendor_sources(
    icons: &IconMap,
    packed_asset: &Path,
    offsets_path: &Path,
) -> Option<BTreeMap<String, String>> {
    let serialized = fs::read_to_string(offsets_path).ok()?;
    let offsets_by_digest: IconOffsets = serde_json::from_str(&serialized).ok()?;
    let packed = fs::read(packed_asset).ok()?;
    let mut sources = BTreeMap::new();

    for path in icons
        .values()
        .flat_map(|icons| icons.values())
        .flat_map(|variants| variants.values())
    {
        if sources.contains_key(path) {
            continue;
        }
        let digest = Path::new(path).file_stem()?.to_str()?;
        let range = *offsets_by_digest.get(digest)?;
        let end = range[0].checked_add(range[1])?;
        let bytes = packed.get(range[0]..end)?;
        sources.insert(path.clone(), std::str::from_utf8(bytes).ok()?.to_owned());
    }

    Some(sources)
}

fn load_icon_sources(icons: &IconMap, manifest_dir: &Path) -> BTreeMap<String, String> {
    let mut sources = BTreeMap::new();

    for path in icons
        .values()
        .flat_map(|icons| icons.values())
        .flat_map(|variants| variants.values())
    {
        if sources.contains_key(path) {
            continue;
        }

        let source_path = Path::new(path);
        let source_path = if source_path.is_absolute() {
            source_path.to_path_buf()
        } else {
            manifest_dir.join(source_path)
        };
        println!("cargo:rerun-if-changed={}", source_path.display());
        let source = fs::read_to_string(&source_path).unwrap_or_else(|error| {
            panic!("Failed to read Material Icon {:?}: {error}", source_path)
        });
        sources.insert(path.clone(), source);
    }

    sources
}

fn load_icons_index(path: &Path) -> Result<IconMap, serde_json::Error> {
    let data = fs::read_to_string(path).unwrap_or_default();
    serde_json::from_str(&data)
}

fn scan_icons_root(src_root: &Path, manifest_dir: &Path) -> IconMap {
    let mut icons: IconMap = BTreeMap::new();

    for entry in WalkDir::new(src_root)
        .min_depth(3)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_dir() {
            continue;
        }

        // entry is .../category/icon_name/variant
        let variant_path = entry.path();
        let icon_name_path = variant_path.parent().unwrap();
        let category_path = icon_name_path.parent().unwrap();

        let variant_str = variant_path.file_name().unwrap().to_string_lossy();
        let icon_name = icon_name_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let category = category_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();

        let svg_path_24 = variant_path.join("24px.svg");

        if svg_path_24.exists() {
            // Map folder names to rust identifiers
            let variant_key = match variant_str.as_ref() {
                "materialicons" => "regular",
                "materialiconsoutlined" => "outlined",
                "materialiconsround" => "round",
                "materialiconssharp" => "sharp",
                "materialiconstwotone" => "two_tone",
                _ => continue, // Skip unknown variants
            };

            // Calculate path relative to CARGO_MANIFEST_DIR for include_str!
            let rel_path = svg_path_24.strip_prefix(manifest_dir).unwrap();
            let rel_path_str = rel_path.to_string_lossy().to_string();

            icons
                .entry(category)
                .or_default()
                .entry(icon_name)
                .or_default()
                .insert(variant_key.to_string(), rel_path_str);
        }
    }

    icons
}

fn resolve_icons_root(submodule_root: &Path, vendor_root: &Path) -> Option<PathBuf> {
    // Prefer the vendored tree to avoid traversing the large submodule.
    if vendor_root.exists() {
        return Some(vendor_root.to_path_buf());
    }
    if submodule_root.exists() {
        return Some(submodule_root.to_path_buf());
    }

    None
}

fn sanitize_keyword(name: &str) -> String {
    match name {
        "type" => "r#type".to_string(),
        "struct" => "r#struct".to_string(),
        "enum" => "r#enum".to_string(),
        "mod" => "r#mod".to_string(),
        "use" => "r#use".to_string(),
        "fn" => "r#fn".to_string(),
        "if" => "r#if".to_string(),
        "else" => "r#else".to_string(),
        "match" => "r#match".to_string(),
        "loop" => "r#loop".to_string(),
        "while" => "r#while".to_string(),
        "for" => "r#for".to_string(),
        "return" => "r#return".to_string(),
        "break" => "r#break".to_string(),
        "continue" => "r#continue".to_string(),
        "box" => "r#box".to_string(),
        "crate" => "r#crate".to_string(),
        "false" => "r#false".to_string(),
        "true" => "r#true".to_string(),
        "impl" => "r#impl".to_string(),
        "in" => "r#in".to_string(),
        "let" => "r#let".to_string(),
        "move" => "r#move".to_string(),
        "mut" => "r#mut".to_string(),
        "pub" => "r#pub".to_string(),
        "ref" => "r#ref".to_string(),
        "self" => "r#self".to_string(),
        "static" => "r#static".to_string(),
        "super" => "r#super".to_string(),
        "trait" => "r#trait".to_string(),
        "unsafe" => "r#unsafe".to_string(),
        "where" => "r#where".to_string(),
        "try" => "r#try".to_string(),
        _ => name.to_string(),
    }
}
