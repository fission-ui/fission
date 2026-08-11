use fontdue::{Font, FontSettings};
use std::sync::{Arc, Mutex, OnceLock};

static DEFAULT_FONT: OnceLock<Font> = OnceLock::new();
static PACKAGED_FONTS: OnceLock<Mutex<Vec<SoftwareFontFace>>> = OnceLock::new();

struct SoftwareFontFace {
    family: &'static str,
    weight: u16,
    style: fission_theme::PackagedFontStyle,
    data: &'static [u8],
    font: OnceLock<Option<Arc<Font>>>,
}

pub(crate) fn default_font() -> &'static Font {
    DEFAULT_FONT.get_or_init(|| {
        Font::from_bytes(
            fission_theme::fonts::default_font_bytes(),
            FontSettings::default(),
        )
        .expect("failed to load bundled UI font")
    })
}

/// Register packaged font descriptors for lazy use by software text rendering.
///
/// Font bytes are parsed only when a matching family is first rendered.
pub fn register_packaged_fonts(fonts: &'static [fission_theme::PackagedFont]) {
    let registry = PACKAGED_FONTS.get_or_init(|| Mutex::new(Vec::new()));
    let mut registry = registry.lock().unwrap();
    for face in fonts {
        let replacement = SoftwareFontFace {
            family: face.family,
            weight: face.weight,
            style: face.style,
            data: face.data,
            font: OnceLock::new(),
        };
        if let Some(existing) = registry.iter_mut().find(|existing| {
            existing.family.eq_ignore_ascii_case(face.family)
                && existing.weight == face.weight
                && existing.style == face.style
        }) {
            *existing = replacement;
        } else {
            registry.push(replacement);
        }
    }
}

pub(crate) fn packaged_font(
    family: Option<&str>,
    weight: u16,
    style: fission_ir::op::FontStyle,
) -> Option<Arc<Font>> {
    let family = family?;
    let desired_style = match style {
        fission_ir::op::FontStyle::Normal => fission_theme::PackagedFontStyle::Normal,
        fission_ir::op::FontStyle::Italic => fission_theme::PackagedFontStyle::Italic,
    };
    PACKAGED_FONTS
        .get()?
        .lock()
        .unwrap()
        .iter()
        .filter(|face| face.family.eq_ignore_ascii_case(family))
        .min_by_key(|face| {
            let style_penalty = u32::from(face.style != desired_style) * 10_000;
            style_penalty + u32::from(face.weight.abs_diff(weight))
        })
        .and_then(|face| {
            face.font
                .get_or_init(|| {
                    Font::from_bytes(face.data, FontSettings::default())
                        .ok()
                        .map(Arc::new)
                })
                .clone()
        })
}

#[cfg(test)]
mod tests {
    use super::{packaged_font, register_packaged_fonts, PACKAGED_FONTS};

    #[test]
    fn packaged_fonts_are_parsed_only_when_software_rendering_uses_them() {
        const FAMILY: &str = "Software Lazy Test Sans";
        let fonts = Box::leak(
            vec![fission_theme::PackagedFont {
                family: FAMILY,
                weight: 700,
                style: fission_theme::PackagedFontStyle::Normal,
                format: "truetype",
                data: fission_theme::fonts::default_font_bytes(),
                axes: &[],
            }]
            .into_boxed_slice(),
        );

        register_packaged_fonts(fonts);

        let registry = PACKAGED_FONTS.get().unwrap().lock().unwrap();
        let face = registry
            .iter()
            .find(|face| face.family == FAMILY)
            .expect("registered software font descriptor");
        assert!(
            face.font.get().is_none(),
            "registration must not parse fonts until software rendering uses them"
        );
        drop(registry);

        assert!(packaged_font(Some(FAMILY), 700, fission_ir::op::FontStyle::Normal).is_some());

        let registry = PACKAGED_FONTS.get().unwrap().lock().unwrap();
        let face = registry
            .iter()
            .find(|face| face.family == FAMILY)
            .expect("registered software font descriptor");
        assert!(face.font.get().is_some());
    }
}
