use std::env;
use std::fs;
use std::path::Path;
use std::collections::{HashMap, BTreeMap};
use walkdir::WalkDir;
use heck::ToSnakeCase;

// Structure: Category -> IconName -> Variant -> Path (relative to crate root)
type IconMap = BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>>;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("material_icons.rs");
    
    // Path to the submodule source
    // CARGO_MANIFEST_DIR points to crates/authoring/fission-icons
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let src_root = Path::new(&manifest_dir).join("material-design-icons/src");
    
    if !src_root.exists() {
        println!("cargo:warning=Material Icons submodule not found at {:?}. Skipping generation.", src_root);
        fs::write(&dest_path, "// Material Icons not found").unwrap();
        return;
    }

    // Tell cargo to rerun if the directory changes (though it's huge, maybe just check if it exists?)
    println!("cargo:rerun-if-changed=material-design-icons/src");

    let mut icons: IconMap = BTreeMap::new();

    for entry in WalkDir::new(&src_root).min_depth(3).max_depth(3).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_dir() { continue; } 
        
        // entry is .../category/icon_name/variant
        let variant_path = entry.path();
        let icon_name_path = variant_path.parent().unwrap();
        let category_path = icon_name_path.parent().unwrap();

        let variant_str = variant_path.file_name().unwrap().to_string_lossy();
        let icon_name = icon_name_path.file_name().unwrap().to_string_lossy().to_string();
        let category = category_path.file_name().unwrap().to_string_lossy().to_string();

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
            // We want "material-design-icons/src/"
            let rel_path = svg_path_24.strip_prefix(&manifest_dir).unwrap();
            let rel_path_str = rel_path.to_string_lossy().to_string();

            icons.entry(category)
                .or_default()
                .entry(icon_name)
                .or_default()
                .insert(variant_key.to_string(), rel_path_str);
        }
    }

    let mut code = String::new();
    code.push_str("// Generated Material Icons\n\n");

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
                code.push_str(&format!("        pub const fn {}() -> &'static str {{\n", variant));
                code.push_str(&format!("            include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/{}\"))\n", path));
                code.push_str("        }\n");

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
