use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Config {
    pub dsp_path: PathBuf,
    pub out_file: String,
    pub type_name: String,
    pub crate_path: String,
    /// Whether this design system must define every recipe the generators read.
    ///
    /// True for the design systems Fission ships, which are the complete
    /// authority for an application that selects one. False for an application
    /// design system, which customizes the components it cares about and is
    /// meant to inherit the rest.
    pub require_complete_components: bool,
}

impl Config {
    pub fn new(dsp_path: impl Into<PathBuf>) -> Self {
        Self {
            dsp_path: dsp_path.into(),
            out_file: "app_design_system.rs".into(),
            type_name: "AppDesignSystem".into(),
            crate_path: "fission_theme".into(),
            require_complete_components: false,
        }
    }

    /// Marks this as a design system Fission supplies, so an omitted recipe
    /// fails the build instead of silently falling back to inlined geometry.
    pub fn require_complete_components(mut self) -> Self {
        self.require_complete_components = true;
        self
    }
}

pub fn generate(config: Config) -> Result<PathBuf> {
    let out_dir = std::env::var_os("OUT_DIR").ok_or_else(|| anyhow!("OUT_DIR is not set"))?;
    let out_path = PathBuf::from(out_dir).join(&config.out_file);
    let package = Package::load(&config.dsp_path)?;
    if config.require_complete_components {
        package.check_required_components()?;
    }
    println!("cargo:rerun-if-changed={}", package.dsp_path.display());
    println!("cargo:rerun-if-changed={}", package.tokens_path.display());
    for font_path in package.font_paths()? {
        println!("cargo:rerun-if-changed={}", font_path.display());
    }
    let code = package.generate(&config)?;
    fs::write(&out_path, code)
        .with_context(|| format!("failed to write {}", out_path.display()))?;
    Ok(out_path)
}

/// Component recipes every supplied design system must define.
///
/// Keep this in step with the `/components/<name>` pointers the generators
/// read. Adding a generator that consults a new recipe means adding the recipe
/// to every design system under `crates/core/fission-theme/design/`, and adding
/// the name here so the next one cannot be forgotten.
const REQUIRED_COMPONENT_RECIPES: &[&str] = &[
    "alert",
    "avatar",
    "avatar_group",
    "badge",
    "button",
    "card",
    "code",
    "empty_state",
    "feature_icon",
    "input",
    "menu",
    "modal",
    "pagination",
    "progress_bar",
    "select",
    "tabs",
    "tooltip",
];

#[derive(Debug, Clone)]
struct Package {
    dsp_path: PathBuf,
    tokens_path: PathBuf,
    dsp: Value,
    tokens: TokenStore,
}

impl Package {
    fn load(dsp_path: &Path) -> Result<Self> {
        let dsp_path = dsp_path.to_path_buf();
        let dsp_text = fs::read_to_string(&dsp_path)
            .with_context(|| format!("failed to read DSP manifest {}", dsp_path.display()))?;
        let dsp: Value = serde_json::from_str(&dsp_text)
            .with_context(|| format!("invalid JSON in {}", dsp_path.display()))?;
        let dsp_dir = dsp_path
            .parent()
            .ok_or_else(|| anyhow!("DSP path has no parent: {}", dsp_path.display()))?;
        let token_ref = dsp
            .pointer("/tokens/$ref")
            .and_then(Value::as_str)
            .unwrap_or("tokens.json");
        let tokens_path = dsp_dir.join(token_ref);
        let tokens_text = fs::read_to_string(&tokens_path)
            .with_context(|| format!("failed to read token file {}", tokens_path.display()))?;
        let raw_tokens: Value = serde_json::from_str(&tokens_text)
            .with_context(|| format!("invalid JSON in {}", tokens_path.display()))?;
        let tokens = TokenStore::from_value(&raw_tokens)?;
        Ok(Self {
            dsp_path,
            tokens_path,
            dsp,
            tokens,
        })
    }

    /// Fails the build when a design system omits a component recipe that the
    /// generated theme resolves.
    ///
    /// Without this, a missing recipe falls back to geometry inlined in this
    /// crate. Colour, radius and typography still resolve through the design
    /// system's own tokens, so the result is not visibly foreign and nobody
    /// notices; but the control sizes, line heights and insets are then this
    /// crate's rather than the design system's, and the design system has no
    /// way to say otherwise because there is no recipe for it to say it in.
    ///
    /// Widgets should look like the design system that is loaded. If a recipe
    /// is genuinely optional, read it with an explicit fallback and drop it
    /// from this list.
    fn check_required_components(&self) -> Result<()> {
        let present = self
            .dsp
            .pointer("/components")
            .and_then(Value::as_object)
            .ok_or_else(|| anyhow!("{} has no components object", self.dsp_path.display()))?;
        let missing = REQUIRED_COMPONENT_RECIPES
            .iter()
            .filter(|name| !present.contains_key(**name))
            .copied()
            .collect::<Vec<_>>();
        if missing.is_empty() {
            return Ok(());
        }
        Err(anyhow!(
            "{} is missing component {}: {}.\n\
             The generated theme resolves {} for every design system, so an \
             omitted one silently falls back to fission-design-system-codegen's \
             own geometry instead of this design system's.",
            self.dsp_path.display(),
            if missing.len() == 1 {
                "recipe"
            } else {
                "recipes"
            },
            missing.join(", "),
            if missing.len() == 1 { "it" } else { "them" },
        ))
    }

    fn generate(&self, config: &Config) -> Result<String> {
        let krate = &config.crate_path;
        let type_name = &config.type_name;
        let info = self.info(krate);
        let light = self.theme_expr(krate, Mode::Light)?;
        let dark = self.theme_expr(krate, Mode::Dark)?;
        let design_tokens = self.design_tokens_expr(krate)?;
        let components = self.components_expr(krate)?;
        let patterns = self.patterns_expr(krate)?;
        let assets = self.assets_expr(krate)?;
        let font_faces = self.font_faces_expr(krate)?;

        Ok(format!(
            r#"// @generated by fission-design-system-codegen. Do not edit by hand.
#[allow(clippy::all)]
#[allow(dead_code)]
pub struct {type_name};

static DESIGN_INFO: ::std::sync::OnceLock<{krate}::DesignSystemInfo> = ::std::sync::OnceLock::new();
static DESIGN_TOKENS: ::std::sync::OnceLock<{krate}::DesignTokenSet> = ::std::sync::OnceLock::new();
static DESIGN_COMPONENTS: ::std::sync::OnceLock<Vec<{krate}::DesignComponentSpec>> = ::std::sync::OnceLock::new();
static DESIGN_PATTERNS: ::std::sync::OnceLock<Vec<{krate}::DesignPatternSpec>> = ::std::sync::OnceLock::new();
static DESIGN_ASSETS: ::std::sync::OnceLock<{krate}::DesignAssetManifest> = ::std::sync::OnceLock::new();
static LIGHT_THEME: ::std::sync::OnceLock<{krate}::Theme> = ::std::sync::OnceLock::new();
static DARK_THEME: ::std::sync::OnceLock<{krate}::Theme> = ::std::sync::OnceLock::new();

impl {krate}::DesignSystem for {type_name} {{
    fn info() -> &'static {krate}::DesignSystemInfo {{
        DESIGN_INFO.get_or_init(|| {info})
    }}

    fn tokens() -> &'static {krate}::DesignTokenSet {{
        DESIGN_TOKENS.get_or_init(|| {design_tokens})
    }}

    fn components() -> &'static [{krate}::DesignComponentSpec] {{
        DESIGN_COMPONENTS.get_or_init(|| {components}).as_slice()
    }}

    fn patterns() -> &'static [{krate}::DesignPatternSpec] {{
        DESIGN_PATTERNS.get_or_init(|| {patterns}).as_slice()
    }}

    fn assets() -> &'static {krate}::DesignAssetManifest {{
        DESIGN_ASSETS.get_or_init(|| {assets})
    }}

    fn font_faces() -> &'static [{krate}::PackagedFont] {{
        {font_faces}
    }}

    fn theme_ref(mode: {krate}::DesignMode) -> &'static {krate}::Theme {{
        match mode {{
            {krate}::DesignMode::Light => LIGHT_THEME.get_or_init(|| {light}),
            {krate}::DesignMode::Dark => DARK_THEME.get_or_init(|| {dark}),
        }}
    }}
}}
"#
        ))
    }

    fn info(&self, krate: &str) -> String {
        let package = self.dsp.get("$package").unwrap_or(&Value::Null);
        let name = package
            .get("name")
            .and_then(Value::as_str)
            .or_else(|| self.dsp.pointer("/brand/name").and_then(Value::as_str))
            .unwrap_or("design-system");
        let version = package
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("0.0.0");
        let description = package
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("");
        format!(
            "{krate}::DesignSystemInfo {{ name: {}, version: {}, description: {}, source: {} }}",
            rust_string(name),
            rust_string(version),
            rust_string(description),
            rust_string(&self.dsp_path.display().to_string()),
        )
    }

    fn theme_expr(&self, krate: &str, mode: Mode) -> Result<String> {
        let mode_name = mode.as_str();
        let colors = self.color_tokens_expr(krate, mode)?;
        let spacing = self.spacing_tokens_expr(krate)?;
        let typography = self.typography_tokens_expr(krate)?;
        let radii = self.radius_tokens_expr(krate)?;
        let elevations = self.elevation_tokens_expr(krate)?;
        let motion = self.motion_tokens_expr(krate)?;
        let data_visualization = self.data_visualization_tokens_expr(krate, mode)?;
        let components = self.component_theme_expr(krate, mode)?;
        Ok(format!(
            r#"{krate}::Theme {{
                tokens: {krate}::Tokens {{
                    colors: {colors},
                    spacing: {spacing},
                    typography: {typography},
                    radii: {radii},
                    elevations: {elevations},
                    motion: {motion},
                    data_visualization: {data_visualization},
                }},
                components: {components},
                design_system: {krate}::ResolvedDesignSystem {{
                    mode: {krate}::DesignMode::{mode_name},
                    info: <{type_placeholder} as {krate}::DesignSystem>::info().clone(),
                    tokens: <{type_placeholder} as {krate}::DesignSystem>::tokens().clone(),
                    components: <{type_placeholder} as {krate}::DesignSystem>::components().to_vec(),
                    patterns: <{type_placeholder} as {krate}::DesignSystem>::patterns().to_vec(),
                    assets: <{type_placeholder} as {krate}::DesignSystem>::assets().clone(),
                }},
            }}"#,
            type_placeholder = "Self"
        ))
    }

    fn color_tokens_expr(&self, krate: &str, mode: Mode) -> Result<String> {
        let prefix = match mode {
            Mode::Light => "color.light",
            Mode::Dark => "color.dark",
        };
        let fallback_on_error = match mode {
            Mode::Light => "#FFFFFF",
            Mode::Dark => "#020617",
        };
        let c =
            |name: &str| -> Result<String> { self.color_expr(krate, &format!("{prefix}.{name}")) };
        let c_or = |name: &str, fallback: &str| -> Result<String> {
            self.color_expr_optional(krate, &format!("{prefix}.{name}"), fallback)
        };
        Ok(format!(
            r#"{krate}::ColorTokens {{
                primary: {primary},
                on_primary: {on_primary},
                primary_hover: {primary_hover},
                primary_subtle: {primary_subtle},
                secondary: {secondary},
                on_secondary: {on_secondary},
                surface: {surface},
                on_surface: {on_surface},
                surface_raised: {surface_raised},
                surface_sunken: {surface_sunken},
                background: {background},
                on_background: {on_background},
                error: {error},
                on_error: {on_error},
                success: {success},
                warning: {warning},
                info: {info},
                border: {border},
                border_strong: {border_strong},
                divider: {divider},
                text_primary: {text_primary},
                text_secondary: {text_secondary},
                text_muted: {text_muted},
                text_link: {text_link},
                heading: {heading},
                focus_ring: {focus_ring},
            }}"#,
            primary = c("primary")?,
            on_primary = c("on_primary")?,
            primary_hover = c_or(
                "primary_hover",
                &self.resolve_token_string(&format!("{prefix}.primary"))?
            )?,
            primary_subtle = c_or(
                "primary_subtle",
                &self.resolve_token_string(&format!("{prefix}.surface"))?
            )?,
            secondary = c("secondary")?,
            on_secondary = c("on_secondary")?,
            surface = c("surface")?,
            on_surface = c_or(
                "on_surface",
                &self.resolve_token_string(&format!("{prefix}.text_primary"))?
            )?,
            surface_raised = c_or(
                "surface_raised",
                &self.resolve_token_string(&format!("{prefix}.surface"))?
            )?,
            surface_sunken = c_or(
                "surface_sunken",
                &self.resolve_token_string(&format!("{prefix}.background"))?
            )?,
            background = c("background")?,
            on_background = c_or(
                "on_background",
                &self.resolve_token_string(&format!("{prefix}.text_primary"))?
            )?,
            error = c("error")?,
            on_error = self.color_literal_expr(krate, fallback_on_error)?,
            success = c_or("success", "#10B981")?,
            warning = c_or("warning", "#F59E0B")?,
            info = c_or("info", "#0EA5E9")?,
            border = c("border")?,
            border_strong = c_or(
                "border_strong",
                &self.resolve_token_string(&format!("{prefix}.border"))?
            )?,
            divider = c_or(
                "divider",
                &self.resolve_token_string(&format!("{prefix}.border"))?
            )?,
            text_primary = c("text_primary")?,
            text_secondary = c("text_secondary")?,
            text_muted = c_or(
                "text_muted",
                &self.resolve_token_string(&format!("{prefix}.text_secondary"))?
            )?,
            text_link = c_or(
                "text_link",
                &self.resolve_token_string(&format!("{prefix}.primary"))?
            )?,
            heading = c_or(
                "heading",
                &self.resolve_token_string(&format!("{prefix}.text_primary"))?
            )?,
            focus_ring = c_or(
                "focus_ring",
                &self.resolve_token_string(&format!("{prefix}.primary"))?
            )?,
        ))
    }

    fn spacing_tokens_expr(&self, krate: &str) -> Result<String> {
        Ok(format!(
            "{krate}::SpacingTokens {{ none: {}, xs: {}, s: {}, m: {}, l: {}, xl: {}, xxl: {}, xxxl: {}, xxxxl: {} }}",
            f32_lit(self.dimension("spacing.none")?),
            f32_lit(self.dimension("spacing.xs")?),
            f32_lit(self.dimension("spacing.s")?),
            f32_lit(self.dimension("spacing.m")?),
            f32_lit(self.dimension("spacing.l")?),
            f32_lit(self.dimension("spacing.xl")?),
            f32_lit(self.dimension_optional("spacing.2xl", 48.0)?),
            f32_lit(self.dimension_optional("spacing.3xl", 64.0)?),
            f32_lit(self.dimension_optional("spacing.4xl", 96.0)?),
        ))
    }

    fn typography_tokens_expr(&self, krate: &str) -> Result<String> {
        Ok(format!(
            r#"{krate}::TypographyTokens {{
                font_family_sans: {},
                font_family_serif: {},
                font_family_mono: {},
                font_weight_regular: {},
                font_weight_medium: {},
                font_weight_semibold: {},
                font_weight_bold: {},
                font_size_xs: {},
                font_size_sm: {},
                font_size_base: {},
                label_large_size: {},
                body_medium_size: {},
                body_large_size: {},
                font_size_lg: {},
                font_size_xl: {},
                heading_size: {},
                heading2_size: {},
                heading1_size: {},
                display_sm_size: {},
                display_md_size: {},
                line_height_display: {},
                line_height_heading: {},
                line_height_snug: {},
                line_height_normal: {},
                line_height_relaxed: {},
                letter_spacing_tight: {},
                letter_spacing_normal: {},
                letter_spacing_label: {},
                letter_spacing_kicker: {},
            }}"#,
            rust_string(&self.string_token_optional("typography.font_family.sans", "Inter")?),
            rust_string(&self.string_token_optional("typography.font_family.serif", "Georgia")?),
            rust_string(&self.string_token_optional("typography.font_family.mono", "monospace")?),
            self.number_optional("typography.font_weight.regular", 400.0)? as u16,
            self.number_optional("typography.font_weight.medium", 500.0)? as u16,
            self.number_optional("typography.font_weight.semibold", 600.0)? as u16,
            self.number_optional("typography.font_weight.bold", 700.0)? as u16,
            f32_lit(self.dimension_optional("typography.font_size.xs", 12.0)?),
            f32_lit(self.dimension_optional("typography.font_size.sm", 13.0)?),
            f32_lit(self.dimension_optional("typography.font_size.base", 14.0)?),
            f32_lit(self.dimension("typography.font_size.label_large")?),
            f32_lit(self.dimension("typography.font_size.body")?),
            f32_lit(self.dimension("typography.font_size.body_large")?),
            f32_lit(self.dimension_optional("typography.font_size.lg", 20.0)?),
            f32_lit(self.dimension_optional("typography.font_size.xl", 24.0)?),
            f32_lit(self.dimension("typography.font_size.h3")?),
            f32_lit(self.dimension_optional("typography.font_size.h2", 36.0)?),
            f32_lit(self.dimension_optional("typography.font_size.h1", 48.0)?),
            f32_lit(self.dimension_optional("typography.font_size.display_sm", 60.0)?),
            f32_lit(self.dimension_optional("typography.font_size.display_md", 72.0)?),
            f32_lit(self.number_optional("typography.line_height.display", 0.98)?),
            f32_lit(self.number_optional("typography.line_height.heading", 1.05)?),
            f32_lit(self.number_optional("typography.line_height.snug", 1.4)?),
            f32_lit(self.number_optional("typography.line_height.normal", 1.6)?),
            f32_lit(self.number_optional("typography.line_height.relaxed", 1.68)?),
            f32_lit(self.dimension_optional("typography.letter_spacing.tight", -0.01)?),
            f32_lit(self.dimension_optional("typography.letter_spacing.normal", 0.0)?),
            f32_lit(self.dimension_optional("typography.letter_spacing.label", 0.1)?),
            f32_lit(self.dimension_optional("typography.letter_spacing.kicker", 0.14)?),
        ))
    }

    fn radius_tokens_expr(&self, krate: &str) -> Result<String> {
        Ok(format!(
            "{krate}::RadiusTokens {{ none: {}, small: {}, medium: {}, large: {}, xl: {}, xxl: {}, full: {} }}",
            f32_lit(self.dimension_optional("radius.none", 0.0)?),
            f32_lit(self.dimension("radius.small")?),
            f32_lit(self.dimension("radius.medium")?),
            f32_lit(self.dimension("radius.large")?),
            f32_lit(self.dimension_optional("radius.xl", 16.0)?),
            f32_lit(self.dimension_optional("radius.2xl", 24.0)?),
            f32_lit(self.dimension("radius.full")?),
        ))
    }

    fn elevation_tokens_expr(&self, krate: &str) -> Result<String> {
        Ok(format!(
            "{krate}::ElevationTokens {{ level0: {}, level1: {}, level2: {}, level3: {}, level4: {}, level5: {}, focus: {} }}",
            self.shadow_option_expr(krate, "elevation.level0")?,
            self.shadow_option_expr(krate, "elevation.level1")?,
            self.shadow_option_expr(krate, "elevation.level2")?,
            self.shadow_option_expr(krate, "elevation.level3")?,
            self.shadow_option_expr(krate, "elevation.level4")?,
            self.shadow_option_expr(krate, "elevation.level5")?,
            self.shadow_option_expr(krate, "elevation.focus")?,
        ))
    }

    fn motion_tokens_expr(&self, krate: &str) -> Result<String> {
        Ok(format!(
            "{krate}::MotionTokens {{ duration_instant_ms: {}, duration_micro_ms: {}, duration_fast_ms: {}, duration_normal_ms: {}, duration_slow_ms: {}, duration_deliberate_ms: {}, easing_linear: {}, easing_standard: {}, easing_in: {}, easing_out: {}, easing_ease: {} }}",
            self.duration_ms_optional("motion.duration.instant", 0)?,
            self.duration_ms_optional("motion.duration.micro", 120)?,
            self.duration_ms_optional("motion.duration.fast", 160)?,
            self.duration_ms_optional("motion.duration.normal", 200)?,
            self.duration_ms_optional("motion.duration.slow", 300)?,
            self.duration_ms_optional("motion.duration.deliberate", 480)?,
            self.easing_expr(krate, "motion.easing.linear")?,
            self.easing_expr(krate, "motion.easing.standard")?,
            self.easing_expr(krate, "motion.easing.in")?,
            self.easing_expr(krate, "motion.easing.out")?,
            self.easing_expr(krate, "motion.easing.ease")?,
        ))
    }

    fn data_visualization_tokens_expr(&self, krate: &str, mode: Mode) -> Result<String> {
        let palette = self.palette_expr(krate, mode)?;
        Ok(format!(
            "{krate}::DataVisualizationTokens {{ palette: vec![{palette}] }}"
        ))
    }

    fn component_theme_expr(&self, krate: &str, mode: Mode) -> Result<String> {
        let button = self.button_theme_expr(krate, mode)?;
        let text_input = self.text_input_theme_expr(krate, mode)?;
        let select = self.select_theme_expr(krate, mode)?;
        let menu = self.menu_theme_expr(krate, mode)?;
        let alert = self.alert_theme_expr(krate, mode)?;
        let avatar = self.avatar_theme_expr(krate, mode)?;
        let avatar_group = self.avatar_group_theme_expr(krate, mode)?;
        let pagination = self.pagination_theme_expr(krate, mode)?;
        let badge = self.badge_theme_expr(krate, mode)?;
        let tabs = self.tabs_theme_expr(krate, mode)?;
        let modal = self.modal_theme_expr(krate, mode)?;
        let progress = self.progress_theme_expr(krate, mode)?;
        let tooltip = self.tooltip_theme_expr(krate, mode)?;
        let card = self.card_theme_expr(krate, mode)?;
        let code = self.code_theme_expr(krate, mode)?;
        let empty_state = self.empty_state_theme_expr(krate, mode)?;
        let feature_icon = self.feature_icon_theme_expr(krate, mode)?;
        let colors_prefix = match mode {
            Mode::Light => "color.light",
            Mode::Dark => "color.dark",
        };
        Ok(format!(
            r#"{krate}::ComponentTheme {{
                button: {button},
                text_input: {text_input},
                select: {select},
                menu: {menu},
                calendar: {krate}::CalendarTheme {{ bg_color: {surface}, border_color: {border}, radius: {radius_medium}, selected_bg: {primary}, selected_text: {on_primary}, today_outline: {secondary} }},
                pagination: {pagination},
                timeline: {krate}::TimelineTheme {{ dot_size: 12.0, line_width: 2.0, dot_color: {primary}, line_color: {border} }},
                segmented_control: {krate}::SegmentedControlTheme {{ bg_color: {surface}, border_color: {border}, radius: {radius_full}, active_bg: {primary}, active_text: {on_primary} }},
                alert: {alert},
                avatar: {avatar},
                avatar_group: {avatar_group},
                badge: {badge},
                tabs: {tabs},
                modal: {modal},
                tree_view: {krate}::TreeViewTheme {{ indent: 16.0, selected_bg: {primary}.with_alpha(52), hover_bg: {surface} }},
                progress: {progress},
                tooltip: {tooltip},
                card: {card},
                code: {code},
                empty_state: {empty_state},
                feature_icon: {feature_icon},
            }}"#,
            surface = self.color_expr(krate, &format!("{colors_prefix}.surface"))?,
            border = self.color_expr(krate, &format!("{colors_prefix}.border"))?,
            primary = self.color_expr(krate, &format!("{colors_prefix}.primary"))?,
            on_primary = self.color_expr(krate, &format!("{colors_prefix}.on_primary"))?,
            secondary = self.color_expr(krate, &format!("{colors_prefix}.secondary"))?,
            radius_medium = f32_lit(self.dimension("radius.medium")?),
            radius_full = f32_lit(self.dimension("radius.full")?),
        ))
    }

    fn button_theme_expr(&self, krate: &str, mode: Mode) -> Result<String> {
        let colors_prefix = match mode {
            Mode::Light => "color.light",
            Mode::Dark => "color.dark",
        };
        let height = self.dsp_dimension_optional(
            "/components/button/sizes/md/height",
            self.dimension_optional("component.button.height", 42.0)?,
        )?;
        let padding_h = self.dsp_dimension_optional(
            "/components/button/sizes/md/padding_x",
            self.dimension_optional("component.button.padding_horizontal", 16.0)?,
        )?;
        let padding_v = self.dimension_optional("component.button.padding_vertical", 8.0)?;
        let radius = self.dsp_dimension_optional(
            "/components/button/radius",
            self.dimension_optional("component.button.radius", 9999.0)?,
        )?;
        let text_size = self.dsp_dimension_optional(
            "/components/button/sizes/md/font_size",
            self.dimension_optional("component.button.text_size", 15.0)?,
        )?;
        Ok(format!(
            r#"{krate}::ButtonTheme {{
                height: {height},
                padding_horizontal: {padding_h},
                padding_vertical: {padding_v},
                radius: {radius},
                text_size: {text_size},
                elevation_rest: {elevation_rest},
                elevation_hover: {elevation_hover},
                elevation_pressed: {elevation_pressed},
                focus_stroke: Some({krate}::Stroke {{ fill: {krate}::Fill::Solid({focus}), width: 2.0, dash_array: None, line_cap: {krate}::LineCap::Round, line_join: {krate}::LineJoin::Round }}),
                icon_size: {icon_size},
                font_weight: {font_weight},
                line_height: {line_height},
                transition: {transition},
                sizes: vec![{sizes}],
                hierarchies: vec![{hierarchies}],
            }}"#,
            height = f32_lit(height),
            padding_h = f32_lit(padding_h),
            padding_v = f32_lit(padding_v),
            radius = f32_lit(radius),
            text_size = f32_lit(text_size),
            elevation_rest = self.shadow_option_expr(krate, "component.button.elevation_rest")?,
            elevation_hover = self.shadow_option_expr(krate, "component.button.elevation_hover")?,
            elevation_pressed =
                self.shadow_option_expr(krate, "component.button.elevation_pressed")?,
            focus = self.color_expr_optional(
                krate,
                &format!("{colors_prefix}.focus_ring"),
                &self.resolve_token_string(&format!("{colors_prefix}.primary"))?
            )?,
            icon_size = f32_lit(self.style_dimension_optional(
                mode,
                self.dsp.pointer("/components/button/icon_size"),
                20.0,
            )?),
            font_weight = self.style_u16_optional(
                mode,
                self.dsp.pointer("/components/button/font_weight"),
                600,
            )?,
            line_height = f32_lit(
                self.dsp_dimension_optional("/components/button/sizes/md/line_height", 20.0,)?
            ),
            transition = self.motion_option_expr(
                krate,
                mode,
                self.dsp.pointer("/components/button/transition"),
            )?,
            sizes = self.size_styles_expr(krate, mode, "/components/button/sizes")?,
            hierarchies = self.enum_state_styles_expr(
                krate,
                mode,
                "/components/button/hierarchies",
                button_hierarchy_variant,
            )?,
        ))
    }

    fn text_input_theme_expr(&self, krate: &str, mode: Mode) -> Result<String> {
        let colors_prefix = match mode {
            Mode::Light => "color.light",
            Mode::Dark => "color.dark",
        };
        let height = self.dsp_dimension_optional(
            "/components/input/sizes/md/height",
            self.dimension_optional("component.text_input.height", 40.0)?,
        )?;
        let padding_h = self.dsp_dimension_optional(
            "/components/input/sizes/md/padding_x",
            self.dimension_optional("component.text_input.padding_h", 16.0)?,
        )?;
        let radius = self.dsp_dimension_optional(
            "/components/input/radius",
            self.dimension_optional("component.text_input.radius", 4.0)?,
        )?;
        let font_size = self.dsp_dimension_optional(
            "/components/input/font_size",
            self.dimension_optional("component.text_input.font_size", 17.0)?,
        )?;
        Ok(format!(
            r#"{krate}::TextInputTheme {{
                height: {height},
                padding_h: {padding_h},
                radius: {radius},
                font_size: {font_size},
                border_color: {border},
                border_width: {border_width},
                focus_color: {focus},
                text_color: {text},
                placeholder_color: {placeholder},
                line_height: {line_height},
                font_weight: {font_weight},
                sizes: vec![{sizes}],
                states: {states},
                placeholder_style: {placeholder_style},
                label_style: {label_style},
                helper_style: {helper_style},
            }}"#,
            height = f32_lit(height),
            padding_h = f32_lit(padding_h),
            radius = f32_lit(radius),
            font_size = f32_lit(font_size),
            border = self.color_expr(krate, &format!("{colors_prefix}.border"))?,
            border_width =
                f32_lit(self.dimension_optional("component.text_input.border_width", 1.0)?),
            focus = self.color_expr_optional(
                krate,
                &format!("{colors_prefix}.focus_ring"),
                &self.resolve_token_string(&format!("{colors_prefix}.primary"))?
            )?,
            text = self.color_expr(krate, &format!("{colors_prefix}.text_primary"))?,
            placeholder = self.color_expr_optional(
                krate,
                &format!("{colors_prefix}.text_muted"),
                &self.resolve_token_string(&format!("{colors_prefix}.text_secondary"))?
            )?,
            line_height =
                f32_lit(self.dsp_dimension_optional("/components/input/line_height", 24.0)?),
            font_weight = self.style_u16_optional(
                mode,
                self.dsp.pointer("/components/input/font_weight"),
                400,
            )?,
            sizes = self.size_styles_expr(krate, mode, "/components/input/sizes")?,
            states =
                self.state_styles_expr(krate, mode, self.dsp.pointer("/components/input/states"))?,
            placeholder_style = self.style_expr(
                krate,
                mode,
                self.dsp.pointer("/components/input/states/placeholder")
            )?,
            label_style =
                self.style_expr(krate, mode, self.dsp.pointer("/components/input/label"))?,
            helper_style = self.style_expr(
                krate,
                mode,
                self.dsp.pointer("/components/input/helper_text")
            )?,
        ))
    }

    fn select_theme_expr(&self, krate: &str, mode: Mode) -> Result<String> {
        let fallback = serde_json::json!({
            "sizes": {
                "sm": {
                    "height": "28px",
                    "radius": "{radius.small}",
                    "padding": "3px 8px 3px 10px",
                    "gap": "6px",
                    "font_size": "{typography.font_size.base}",
                    "font_weight": "{typography.font_weight.regular}",
                    "line_height": "20px",
                    "icon_size": "16px"
                },
                "md": {
                    "height": "32px",
                    "radius": "{radius.medium}",
                    "padding": "5px 8px 5px 10px",
                    "gap": "6px",
                    "font_size": "{typography.font_size.base}",
                    "font_weight": "{typography.font_weight.regular}",
                    "line_height": "20px",
                    "icon_size": "16px"
                }
            },
            "trigger_states": {
                "default": {
                    "background": "transparent",
                    "color": "{color.light.text_primary}",
                    "border": "1px solid {color.light.border}"
                },
                "hover": { "background": "{color.light.surface_sunken}" },
                "focus": {
                    "border": "1px solid {color.light.focus_ring}",
                    "box_shadow": "{elevation.focus}"
                },
                "disabled": { "opacity": 0.5 },
                "error": {
                    "border": "1px solid {color.light.error}",
                    "box_shadow": "0 0 0 3px rgba(225,29,72,0.2)"
                }
            },
            "placeholder": { "color": "{color.light.text_secondary}" },
            "indicator": {
                "color": "{color.light.text_secondary}",
                "icon_size": "16px",
                "inset_end": "8px"
            }
        });
        let select = self.dsp.pointer("/components/select").unwrap_or(&fallback);
        Ok(format!(
            r#"{krate}::SelectTheme {{
                sizes: vec![{sizes}],
                trigger_states: {trigger_states},
                placeholder_style: {placeholder_style},
                indicator_style: {indicator_style},
            }}"#,
            sizes = self.size_styles_expr_from_value(krate, mode, select.get("sizes"))?,
            trigger_states = self.state_styles_expr(
                krate,
                mode,
                select
                    .get("trigger_states")
                    .or_else(|| select.get("states")),
            )?,
            placeholder_style = self.style_expr(krate, mode, select.get("placeholder"))?,
            indicator_style = self.style_expr(krate, mode, select.get("indicator"))?,
        ))
    }

    fn menu_theme_expr(&self, krate: &str, mode: Mode) -> Result<String> {
        fn merge_recipe(base: &mut Value, overlay: &Value) {
            match (base, overlay) {
                (Value::Object(base), Value::Object(overlay)) => {
                    for (key, value) in overlay {
                        if let Some(existing) = base.get_mut(key) {
                            merge_recipe(existing, value);
                        } else {
                            base.insert(key.clone(), value.clone());
                        }
                    }
                }
                (base, overlay) => *base = overlay.clone(),
            }
        }

        let mut menu = serde_json::json!({
            "trigger": {
                "sizes": {
                    "sm": { "height": "28px", "radius": "{radius.small}", "padding": "0px 6px 0px 10px", "gap": "4px", "font_size": "12.8px", "font_weight": 500, "line_height": "19.2px", "icon_size": "14px" },
                    "md": { "height": "32px", "radius": "{radius.medium}", "padding": "0px 8px 0px 10px", "gap": "6px", "font_size": "14px", "font_weight": 500, "line_height": "20px", "icon_size": "16px" }
                },
                "states": {
                    "default": { "background": "transparent", "color": "{color.light.text_primary}", "border": "1px solid {color.light.border}", "box_shadow": "none" },
                    "hover": { "background": "{color.light.surface_sunken}", "color": "{color.light.text_primary}" },
                    "active": { "background": "{color.light.surface_sunken}", "translate_y": "1px" },
                    "focus": { "border": "1px solid {color.light.focus_ring}", "box_shadow": "{elevation.focus}" },
                    "disabled": { "opacity": 0.5 },
                    "selected": { "background": "{color.light.surface_sunken}", "color": "{color.light.text_primary}" }
                }
            },
            "surface": {
                "background": "{color.light.surface}",
                "border": "1px solid {color.light.border}",
                "radius": "{radius.medium}",
                "width": "208px",
                "padding": "4px",
                "gap": "2px",
                "box_shadow": "{elevation.level2}"
            },
            "item": {
                "states": {
                    "default": {
                        "background": "transparent",
                        "color": "{color.light.text_primary}",
                        "radius": "{radius.small}",
                        "height": "32px",
                        "padding_x": "8px",
                        "padding_y": "6px",
                        "gap": "8px",
                        "font_size": "{typography.font_size.base}",
                        "font_weight": "{typography.font_weight.regular}",
                        "line_height": "20px",
                        "icon_size": "16px"
                    },
                    "hover": { "background": "{color.light.surface_sunken}" },
                    "active": { "background": "{color.light.surface_sunken}" },
                    "focus": { "background": "{color.light.surface_sunken}" },
                    "disabled": { "background": "transparent", "color": "{color.light.text_muted}" },
                    "selected": { "background": "{color.light.surface_sunken}", "font_weight": "{typography.font_weight.medium}" }
                },
                "destructive_states": {
                    "default": { "color": "{color.light.error}" },
                    "hover": { "background": "rgba(225, 29, 72, 0.1)" },
                    "active": { "background": "rgba(225, 29, 72, 0.2)" },
                    "focus": { "background": "rgba(225, 29, 72, 0.1)" },
                    "disabled": { "background": "transparent", "color": "{color.light.text_muted}" },
                    "selected": { "background": "rgba(225, 29, 72, 0.1)" }
                },
                "description": { "color": "{color.light.text_muted}", "font_size": "13px", "font_weight": 400, "line_height": "18px", "gap": "2px" },
                "shortcut": { "color": "{color.light.text_muted}", "font_size": "12px", "font_weight": 400, "line_height": "16px", "letter_spacing": "0.4px" },
                "metadata": { "color": "{color.light.text_muted}", "font_size": "13px", "font_weight": 400, "line_height": "18px" },
                "indicator": { "color": "{color.light.text_primary}", "icon_size": "16px" }
            },
            "group_label": { "color": "{color.light.text_muted}", "height": "28px", "padding_x": "8px", "padding_y": "6px", "font_size": "12px", "font_weight": 500, "line_height": "16px" },
            "separator": { "border": "1px solid {color.light.divider}", "height": "9px", "padding_y": "4px", "margin": "0px -4px" }
        });
        if let Some(outline) = self.dsp.pointer("/components/button/hierarchies/outline") {
            let mut outline = outline.clone();
            let hover = outline.get("hover").cloned();
            if let (Some(states), Some(hover)) = (outline.as_object_mut(), hover) {
                states.insert("selected".into(), hover);
            }
            if let Some(states) = menu.pointer_mut("/trigger/states") {
                *states = outline;
            }
        }
        if let Some(configured) = self.dsp.pointer("/components/menu") {
            merge_recipe(&mut menu, configured);
        }
        let item = menu.get("item");
        let trigger = menu.get("trigger");

        Ok(format!(
            r#"{krate}::MenuTheme {{
                trigger_sizes: vec![{trigger_sizes}],
                trigger_states: {trigger_states},
                surface_style: {surface},
                item_states: {item_states},
                destructive_item_states: {destructive_states},
                description_style: {description},
                shortcut_style: {shortcut},
                metadata_style: {metadata},
                group_label_style: {group_label},
                separator_style: {separator},
                indicator_style: {indicator},
            }}"#,
            trigger_sizes = self.size_styles_expr_from_value(
                krate,
                mode,
                trigger.and_then(|value| value.get("sizes")),
            )?,
            trigger_states = self.state_styles_expr(
                krate,
                mode,
                trigger.and_then(|value| value.get("states")),
            )?,
            surface = self.style_expr(krate, mode, menu.get("surface"))?,
            item_states =
                self.state_styles_expr(krate, mode, item.and_then(|value| value.get("states")),)?,
            destructive_states = self.state_styles_expr(
                krate,
                mode,
                item.and_then(|value| value.get("destructive_states")),
            )?,
            description =
                self.style_expr(krate, mode, item.and_then(|value| value.get("description")),)?,
            shortcut =
                self.style_expr(krate, mode, item.and_then(|value| value.get("shortcut")),)?,
            metadata =
                self.style_expr(krate, mode, item.and_then(|value| value.get("metadata")),)?,
            group_label = self.style_expr(krate, mode, menu.get("group_label"))?,
            separator = self.style_expr(krate, mode, menu.get("separator"))?,
            indicator =
                self.style_expr(krate, mode, item.and_then(|value| value.get("indicator")),)?,
        ))
    }

    fn alert_theme_expr(&self, krate: &str, mode: Mode) -> Result<String> {
        fn merge_recipe(base: &mut Value, overlay: &Value) {
            match (base, overlay) {
                (Value::Object(base), Value::Object(overlay)) => {
                    for (key, value) in overlay {
                        if let Some(existing) = base.get_mut(key) {
                            merge_recipe(existing, value);
                        } else {
                            base.insert(key.clone(), value.clone());
                        }
                    }
                }
                (base, overlay) => *base = overlay.clone(),
            }
        }

        let mut alert = serde_json::json!({
            "surface": {
                "background": "{color.light.surface}",
                "border": "1px solid {color.light.border}",
                "radius": "{radius.medium}",
                "min_height": "60px",
                "padding": "8px 10px",
                "gap": "8px"
            },
            "icon": { "icon_size": "16px", "inset_top": "2px", "translate_y": "2px" },
            "content": { "gap": "2px" },
            "title": {
                "color": "{color.light.text_primary}",
                "font_size": "{typography.font_size.base}",
                "font_weight": "{typography.font_weight.medium}",
                "line_height": "20px"
            },
            "description": {
                "color": "{color.light.text_secondary}",
                "font_size": "{typography.font_size.base}",
                "font_weight": "{typography.font_weight.regular}",
                "line_height": "20px"
            },
            "action": { "width": "64px", "inset_top": "8px", "inset_end": "8px" },
            "tones": {
                "info": { "background": "{color.light.surface}", "color": "{color.light.info}" },
                "warning": { "background": "{color.light.surface}", "color": "{color.light.warning}" },
                "error": { "background": "{color.light.surface}", "color": "{color.light.error}" },
                "success": { "background": "{color.light.surface}", "color": "{color.light.success}" }
            }
        });
        if let Some(configured) = self.dsp.pointer("/components/alert") {
            merge_recipe(&mut alert, configured);
        }

        let colors_prefix = format!("color.{}", mode.color_name());
        let surface_color = self.color_expr(krate, &format!("{colors_prefix}.surface"))?;
        let radius = self.style_dimension_optional(
            mode,
            alert.pointer("/surface/radius"),
            self.dimension("radius.medium")?,
        )?;
        let tone = |name: &str| alert.pointer(&format!("/tones/{name}"));

        Ok(format!(
            r#"{krate}::AlertTheme {{
                surface_style: {surface},
                icon_style: {icon},
                content_style: {content},
                title_style: {title},
                description_style: {description},
                action_style: {action},
                info_style: {info},
                warning_style: {warning},
                error_style: {error},
                success_style: {success},
                info_bg: {surface_color},
                warning_bg: {surface_color},
                error_bg: {surface_color},
                success_bg: {surface_color},
                radius: {radius},
            }}"#,
            surface = self.style_expr(krate, mode, alert.get("surface"))?,
            icon = self.style_expr(krate, mode, alert.get("icon"))?,
            content = self.style_expr(krate, mode, alert.get("content"))?,
            title = self.style_expr(krate, mode, alert.get("title"))?,
            description = self.style_expr(krate, mode, alert.get("description"))?,
            action = self.style_expr(krate, mode, alert.get("action"))?,
            info = self.style_expr(krate, mode, tone("info"))?,
            warning = self.style_expr(krate, mode, tone("warning"))?,
            error = self.style_expr(krate, mode, tone("error"))?,
            success = self.style_expr(krate, mode, tone("success"))?,
            radius = f32_lit(radius),
        ))
    }

    fn pagination_theme_expr(&self, krate: &str, mode: Mode) -> Result<String> {
        let fallback_item = serde_json::json!({
            "color": "{color.light.text_primary}",
            "size": "32px",
            "padding": "0px",
            "radius": "{radius.medium}",
            "font_size": "{typography.font_size.base}",
            "font_weight": "{typography.font_weight.regular}",
            "line_height": "20px",
            "icon_size": "16px",
            "box_shadow": "none"
        });
        let fallback_selected = serde_json::json!({
            "background": "{color.light.surface}",
            "border": "1px solid {color.light.border}",
            "color": "{color.light.text_primary}",
            "font_weight": "{typography.font_weight.medium}",
            "box_shadow": "none"
        });
        let fallback_ellipsis = serde_json::json!({
            "color": "{color.light.text_muted}",
            "size": "32px",
            "font_size": "{typography.font_size.base}",
            "font_weight": "{typography.font_weight.regular}",
            "line_height": "20px"
        });
        let item = self
            .dsp
            .pointer("/components/pagination/item")
            .or(Some(&fallback_item));
        let selected = self
            .dsp
            .pointer("/components/pagination/selected")
            .or(Some(&fallback_selected));
        let ellipsis = self
            .dsp
            .pointer("/components/pagination/ellipsis")
            .or(Some(&fallback_ellipsis));
        let spacing = self.style_dimension_optional(
            mode,
            self.dsp.pointer("/components/pagination/gap"),
            self.dimension("spacing.xs")?,
        )?;
        let colors_prefix = format!("color.{}", mode.color_name());
        let surface = self.color_expr(krate, &format!("{colors_prefix}.surface"))?;
        let text = self.color_expr(krate, &format!("{colors_prefix}.text_primary"))?;

        Ok(format!(
            r#"{krate}::PaginationTheme {{
                item_style: {item_style},
                selected_style: {selected_style},
                ellipsis_style: {ellipsis_style},
                spacing: {spacing},
                active_bg: {surface},
                active_text: {text},
            }}"#,
            item_style = self.style_expr(krate, mode, item)?,
            selected_style = self.style_expr(krate, mode, selected)?,
            ellipsis_style = self.style_expr(krate, mode, ellipsis)?,
            spacing = f32_lit(spacing),
        ))
    }

    fn badge_theme_expr(&self, krate: &str, mode: Mode) -> Result<String> {
        let radius = self.style_dimension_optional(
            mode,
            self.dsp.pointer("/components/badge/radius"),
            self.dimension_optional("component.badge.radius", self.dimension("radius.full")?)?,
        )?;
        let font_size = self.dsp_dimension_optional(
            "/components/badge/sizes/md/font_size",
            self.dimension_optional("component.badge.font_size", 10.0)?,
        )?;
        Ok(format!(
            r#"{krate}::BadgeTheme {{
                radius: {radius},
                font_size: {font_size},
                font_weight: {font_weight},
                sizes: vec![{sizes}],
                tones: vec![{tones}],
            }}"#,
            radius = f32_lit(radius),
            font_size = f32_lit(font_size),
            font_weight = self.style_u16_optional(
                mode,
                self.dsp.pointer("/components/badge/font_weight"),
                500
            )?,
            sizes = self.size_styles_expr(krate, mode, "/components/badge/sizes")?,
            tones =
                self.enum_styles_expr(krate, mode, "/components/badge/colors", badge_tone_variant)?,
        ))
    }

    fn avatar_theme_expr(&self, krate: &str, mode: Mode) -> Result<String> {
        let fallback = serde_json::json!({
            "background": "{color.light.primary}",
            "color": "{color.light.on_primary}"
        });
        Ok(format!(
            r#"{krate}::AvatarTheme {{
                fallback_style: {fallback_style},
            }}"#,
            fallback_style = self.style_expr(
                krate,
                mode,
                self.dsp
                    .pointer("/components/avatar/fallback")
                    .or(Some(&fallback)),
            )?,
        ))
    }

    fn avatar_group_theme_expr(&self, krate: &str, mode: Mode) -> Result<String> {
        let fallback = serde_json::json!({
            "avatar": {
                "width": "32px",
                "height": "32px",
                "radius": "{radius.full}",
                "border": "2px solid {color.light.surface}"
            },
            "overflow": {
                "background": "{color.light.surface_sunken}",
                "color": "{color.light.text_secondary}",
                "width": "32px",
                "height": "32px",
                "radius": "{radius.full}",
                "border": "2px solid {color.light.surface}",
                "font_size": "{typography.font_size.xs}",
                "font_weight": "{typography.font_weight.medium}",
                "line_height": "16px"
            },
            "overlap": "10px",
            "max_visible": 4
        });
        let group = self
            .dsp
            .pointer("/components/avatar_group")
            .unwrap_or(&fallback);
        let overlap = self.style_dimension_optional(mode, group.get("overlap"), 10.0)?;
        let max_visible = group
            .get("max_visible")
            .and_then(Value::as_u64)
            .unwrap_or(4) as usize;
        Ok(format!(
            r#"{krate}::AvatarGroupTheme {{
                avatar_style: {avatar_style},
                overflow_style: {overflow_style},
                overlap: {overlap},
                max_visible: {max_visible},
            }}"#,
            avatar_style = self.style_expr(krate, mode, group.get("avatar"))?,
            overflow_style = self.style_expr(krate, mode, group.get("overflow"))?,
            overlap = f32_lit(overlap),
        ))
    }

    fn tabs_theme_expr(&self, krate: &str, mode: Mode) -> Result<String> {
        let colors_prefix = match mode {
            Mode::Light => "color.light",
            Mode::Dark => "color.dark",
        };
        let active = self.color_expr(krate, &format!("{colors_prefix}.primary"))?;
        let inactive = self.color_expr(krate, &format!("{colors_prefix}.text_secondary"))?;
        let background = self.color_expr(krate, &format!("{colors_prefix}.background"))?;
        let divider = self.color_expr_optional(
            krate,
            &format!("{colors_prefix}.divider"),
            &self.resolve_token_string(&format!("{colors_prefix}.border"))?,
        )?;
        let presentations = if self
            .dsp
            .pointer("/components/tabs/variants/underline")
            .is_some()
        {
            format!(
                "({krate}::TabPresentation::Underline, {krate}::TabPresentationTheme {{ indicator_height: {}, sizes: vec![{}], states: {}, track_style: {} }})",
                f32_lit(self.dsp_dimension_optional(
                    "/components/tabs/variants/underline/sizes/md/indicator_height",
                    2.0,
                )?),
                self.size_styles_expr(
                    krate,
                    mode,
                    "/components/tabs/variants/underline/sizes",
                )?,
                self.state_styles_expr(
                    krate,
                    mode,
                    self.dsp.pointer("/components/tabs/variants/underline/states"),
                )?,
                self.style_expr(
                    krate,
                    mode,
                    self.dsp.pointer("/components/tabs/variants/underline/track"),
                )?,
            )
        } else {
            String::new()
        };
        Ok(format!(
            r#"{krate}::TabsTheme {{
                active_color: {active},
                inactive_color: {inactive},
                indicator_height: {indicator_height},
                background: {background},
                divider_color: {divider}.with_alpha(120),
                sizes: vec![{sizes}],
                states: {states},
                track_style: {track},
                presentations: vec![{presentations}],
            }}"#,
            indicator_height = f32_lit(self.dsp_dimension_optional(
                "/components/tabs/sizes/md/indicator_height",
                self.dimension_optional("component.tabs.indicator_height", 3.0)?
            )?),
            sizes = self.size_styles_expr(krate, mode, "/components/tabs/sizes")?,
            states =
                self.state_styles_expr(krate, mode, self.dsp.pointer("/components/tabs/states"))?,
            track = self.style_expr(krate, mode, self.dsp.pointer("/components/tabs/track"))?,
            presentations = presentations,
        ))
    }

    fn modal_theme_expr(&self, krate: &str, mode: Mode) -> Result<String> {
        let bg = self
            .style_fill_color_expr(
                krate,
                mode,
                self.dsp.pointer("/components/modal/background"),
            )?
            .unwrap_or(self.color_expr(krate, &format!("color.{}.surface", mode.color_name()))?);
        let radius = self.style_dimension_optional(
            mode,
            self.dsp.pointer("/components/modal/radius"),
            self.dimension_optional("component.modal.radius", self.dimension("radius.large")?)?,
        )?;
        let max_width = self.style_dimension_optional(
            mode,
            self.dsp.pointer("/components/modal/max_width"),
            self.dimension_optional("component.modal.max_width", 600.0)?,
        )?;
        let shadow = self.shadow_option_expr(krate, "component.modal.shadow")?;
        let container_style = self.style_expr_for_component(
            krate,
            mode,
            "/components/modal",
            &[
                "background",
                "border",
                "radius",
                "padding",
                "gap",
                "max_width",
                "box_shadow",
            ],
        )?;
        let scrim_style =
            self.style_expr(krate, mode, self.dsp.pointer("/components/modal/scrim"))?;
        let scrim_blur = self
            .dsp
            .pointer("/components/modal/scrim/backdrop_filter")
            .and_then(Value::as_str)
            .and_then(|value| {
                value
                    .strip_prefix("blur(")
                    .and_then(|v| v.strip_suffix(')'))
            })
            .and_then(|value| parse_dimension(value).ok())
            .unwrap_or(4.0);
        let fallback_header = serde_json::json!({ "gap": "8px" });
        let fallback_title = serde_json::json!({
            "color": "{color.light.text_primary}",
            "font_size": "16px",
            "font_weight": 500,
            "line_height": "16px"
        });
        let fallback_description = serde_json::json!({
            "color": "{color.light.text_secondary}",
            "font_size": "14px",
            "font_weight": 400,
            "line_height": "20px"
        });
        let fallback_body = serde_json::json!({});
        let fallback_footer = serde_json::json!({
            "background": "{color.light.surface_sunken}",
            "border": "1px solid {color.light.border}",
            "padding": "16px",
            "margin": "0px -16px -16px",
            "gap": "8px"
        });
        let fallback_close = serde_json::json!({
            "size": "28px",
            "padding": "0px",
            "icon_size": "16px",
            "inset_top": "8px",
            "inset_end": "8px"
        });
        let viewport_margin = self.style_dimension_optional(
            mode,
            self.dsp.pointer("/components/modal/viewport_margin"),
            16.0,
        )?;
        let action_stack_breakpoint = self.style_dimension_optional(
            mode,
            self.dsp
                .pointer("/components/modal/action_stack_breakpoint"),
            640.0,
        )?;
        let motion_duration_ms = self.style_duration_optional(
            mode,
            self.dsp.pointer("/components/modal/motion_duration"),
            100,
        )?;
        let motion_initial_scale = self.style_number_optional(
            mode,
            self.dsp.pointer("/components/modal/motion_initial_scale"),
            0.95,
        )?;
        Ok(format!(
            r#"{krate}::ModalTheme {{
                bg_color: {bg},
                radius: {radius},
                shadow: {shadow},
                max_width: {max_width},
                container_style: {container_style},
                scrim_style: {scrim_style},
                scrim_blur: {scrim_blur},
                header_style: {header_style},
                title_style: {title_style},
                description_style: {description_style},
                content_style: {content_style},
                footer_style: {footer_style},
                close_button_style: {close_button_style},
                viewport_margin: {viewport_margin},
                action_stack_breakpoint: {action_stack_breakpoint},
                motion_duration_ms: {motion_duration_ms},
                motion_initial_scale: {motion_initial_scale},
            }}"#,
            radius = f32_lit(radius),
            max_width = f32_lit(max_width),
            scrim_blur = f32_lit(scrim_blur),
            header_style = self.style_expr(
                krate,
                mode,
                self.dsp
                    .pointer("/components/modal/header")
                    .or(Some(&fallback_header)),
            )?,
            title_style = self.style_expr(
                krate,
                mode,
                self.dsp
                    .pointer("/components/modal/title")
                    .or(Some(&fallback_title)),
            )?,
            description_style = self.style_expr(
                krate,
                mode,
                self.dsp
                    .pointer("/components/modal/description")
                    .or(Some(&fallback_description)),
            )?,
            content_style = self.style_expr(
                krate,
                mode,
                self.dsp
                    .pointer("/components/modal/body")
                    .or(Some(&fallback_body)),
            )?,
            footer_style = self.style_expr(
                krate,
                mode,
                self.dsp
                    .pointer("/components/modal/footer")
                    .or(Some(&fallback_footer)),
            )?,
            close_button_style = self.style_expr(
                krate,
                mode,
                self.dsp
                    .pointer("/components/modal/close")
                    .or(Some(&fallback_close)),
            )?,
            viewport_margin = f32_lit(viewport_margin),
            action_stack_breakpoint = f32_lit(action_stack_breakpoint),
            motion_initial_scale = f32_lit(motion_initial_scale),
        ))
    }

    fn progress_theme_expr(&self, krate: &str, mode: Mode) -> Result<String> {
        let height = self.style_dimension_optional(
            mode,
            self.dsp.pointer("/components/progress_bar/height"),
            self.dimension_optional("component.progress.height", 8.0)?,
        )?;
        let radius = self.style_dimension_optional(
            mode,
            self.dsp.pointer("/components/progress_bar/radius"),
            self.dimension("radius.full")?,
        )?;
        let track_style = self.style_expr(
            krate,
            mode,
            self.dsp.pointer("/components/progress_bar/track"),
        )?;
        let fill_style = self.style_expr(
            krate,
            mode,
            self.dsp.pointer("/components/progress_bar/fill"),
        )?;
        let track_color = self
            .style_fill_color_expr(
                krate,
                mode,
                self.dsp
                    .pointer("/components/progress_bar/track/background"),
            )?
            .unwrap_or_else(|| {
                self.color_expr(krate, &format!("color.{}.border", mode.color_name()))
                    .unwrap()
            });
        let fill_color = self
            .style_fill_color_expr(
                krate,
                mode,
                self.dsp.pointer("/components/progress_bar/fill/background"),
            )?
            .unwrap_or_else(|| {
                self.color_expr(krate, &format!("color.{}.primary", mode.color_name()))
                    .unwrap()
            });
        Ok(format!(
            r#"{krate}::ProgressTheme {{
                height: {height},
                track_color: {track_color},
                bar_color: {fill_color},
                radius: {radius},
                track_style: {track_style},
                fill_style: {fill_style},
            }}"#,
            height = f32_lit(height),
            radius = f32_lit(radius),
        ))
    }

    fn tooltip_theme_expr(&self, krate: &str, mode: Mode) -> Result<String> {
        let style = self.style_expr_for_component(
            krate,
            mode,
            "/components/tooltip",
            &[
                "background",
                "color",
                "radius",
                "font_size",
                "padding",
                "max_width",
                "box_shadow",
            ],
        )?;
        let bg = self
            .style_fill_color_expr(
                krate,
                mode,
                self.dsp.pointer("/components/tooltip/background"),
            )?
            .unwrap_or(self.color_literal_expr(krate, "#323232")?);
        let text = self
            .style_fill_color_expr(krate, mode, self.dsp.pointer("/components/tooltip/color"))?
            .unwrap_or(self.color_literal_expr(krate, "#FFFFFF")?);
        let radius = self.style_dimension_optional(
            mode,
            self.dsp.pointer("/components/tooltip/radius"),
            self.dimension_optional("component.tooltip.radius", self.dimension("radius.small")?)?,
        )?;
        let font_size = self.style_dimension_optional(
            mode,
            self.dsp.pointer("/components/tooltip/font_size"),
            self.dimension_optional("component.tooltip.font_size", 12.0)?,
        )?;
        let (padding_x, padding_y) = self
            .dsp
            .pointer("/components/tooltip/padding")
            .and_then(Value::as_str)
            .and_then(|raw| self.resolve_refs_in_string_for_mode(raw, mode).ok())
            .and_then(|raw| parse_padding(&raw).ok())
            .map(|p| (p[0], p[2]))
            .unwrap_or((10.0, 8.0));
        let max_width = self.style_dimension_optional(
            mode,
            self.dsp.pointer("/components/tooltip/max_width"),
            240.0,
        )?;
        Ok(format!(
            r#"{krate}::TooltipTheme {{
                bg_color: {bg},
                text_color: {text},
                radius: {radius},
                font_size: {font_size},
                padding_x: {padding_x},
                padding_y: {padding_y},
                max_width: {max_width},
                style: {style},
            }}"#,
            radius = f32_lit(radius),
            font_size = f32_lit(font_size),
            padding_x = f32_lit(padding_x),
            padding_y = f32_lit(padding_y),
            max_width = f32_lit(max_width),
        ))
    }

    fn card_theme_expr(&self, krate: &str, mode: Mode) -> Result<String> {
        let padding = self.style_dimension_optional(
            mode,
            self.dsp.pointer("/components/card/padding"),
            self.dimension_optional("component.card.padding", self.dimension("spacing.l")?)?,
        )?;
        let radius = self.style_dimension_optional(
            mode,
            self.dsp.pointer("/components/card/radius"),
            self.dimension_optional("component.card.radius", self.dimension("radius.large")?)?,
        )?;
        let patterns = self.enum_styles_expr(
            krate,
            mode,
            "/components/card/patterns",
            card_pattern_variant,
        )?;
        let hover = self.style_expr(
            krate,
            mode,
            self.dsp.pointer("/components/card/interaction/hover"),
        )?;
        let fallback_selected = serde_json::json!({
            "border": "2px solid {color.light.primary}"
        });
        let selected = self.style_expr(
            krate,
            mode,
            self.dsp
                .pointer("/components/card/interaction/selected")
                .or(Some(&fallback_selected)),
        )?;
        let selected_indicator = self.style_expr(
            krate,
            mode,
            self.dsp
                .pointer("/components/card/interaction/selected_indicator"),
        )?;
        let fallback_header = serde_json::json!({ "gap": "4px" });
        let fallback_content = serde_json::json!({});
        let fallback_footer = serde_json::json!({
            "background": "{color.light.surface_sunken}",
            "gap": "8px"
        });
        let fallback_title = serde_json::json!({
            "color": "{color.light.text_primary}",
            "font_size": "16px",
            "font_weight": 500,
            "line_height": "24px"
        });
        let fallback_description = serde_json::json!({
            "color": "{color.light.text_secondary}",
            "font_size": "14px",
            "font_weight": 400,
            "line_height": "20px"
        });
        let fallback_separator = serde_json::json!({
            "border": "1px solid {color.light.border}"
        });
        let sizes = if self.dsp.pointer("/components/card/sizes").is_some() {
            self.size_styles_expr(krate, mode, "/components/card/sizes")?
        } else {
            format!(
                "({krate}::ComponentSize::Sm, {krate}::ResolvedComponentStyle {{ padding: Some([12.0; 4]), gap: Some(12.0), font_size: Some(14.0), line_height: Some(20.0), ..{krate}::ResolvedComponentStyle::default() }}),({krate}::ComponentSize::Md, {krate}::ResolvedComponentStyle {{ padding: Some([16.0; 4]), gap: Some(16.0), font_size: Some(16.0), line_height: Some(24.0), ..{krate}::ResolvedComponentStyle::default() }})"
            )
        };
        Ok(format!(
            r#"{krate}::CardTheme {{
                padding: {padding},
                radius: {radius},
                default_pattern: {krate}::CardPattern::Raised,
                patterns: vec![{patterns}],
                hover_style: {hover},
                selected_style: {selected},
                selected_indicator_style: {selected_indicator},
                sizes: vec![{sizes}],
                header_style: {header},
                content_style: {content},
                footer_style: {footer},
                title_style: {title},
                description_style: {description},
                separator_style: {separator},
            }}"#,
            padding = f32_lit(padding),
            radius = f32_lit(radius),
            sizes = sizes,
            selected = selected,
            selected_indicator = selected_indicator,
            header = self.style_expr(
                krate,
                mode,
                self.dsp
                    .pointer("/components/card/header")
                    .or(Some(&fallback_header))
            )?,
            content = self.style_expr(
                krate,
                mode,
                self.dsp
                    .pointer("/components/card/content")
                    .or(Some(&fallback_content))
            )?,
            footer = self.style_expr(
                krate,
                mode,
                self.dsp
                    .pointer("/components/card/footer")
                    .or(Some(&fallback_footer))
            )?,
            title = self.style_expr(
                krate,
                mode,
                self.dsp
                    .pointer("/components/card/title")
                    .or(Some(&fallback_title))
            )?,
            description = self.style_expr(
                krate,
                mode,
                self.dsp
                    .pointer("/components/card/description")
                    .or(Some(&fallback_description)),
            )?,
            separator = self.style_expr(
                krate,
                mode,
                self.dsp
                    .pointer("/components/card/separator")
                    .or(Some(&fallback_separator)),
            )?,
        ))
    }

    fn code_theme_expr(&self, krate: &str, mode: Mode) -> Result<String> {
        let fallback = serde_json::json!({
            "background": "{color.light.surface_sunken}",
            "color": "{color.light.text_primary}",
            "radius": "{radius.small}",
            "padding_x": "{spacing.xs}",
            "padding_y": "2px",
            "font_size": "{typography.font_size.xs}",
            "font_family": "{typography.font_family.mono}"
        });
        let style = self.style_expr(
            krate,
            mode,
            self.dsp.pointer("/components/code").or(Some(&fallback)),
        )?;
        Ok(format!("{krate}::CodeTheme {{ style: {style} }}"))
    }

    fn empty_state_theme_expr(&self, krate: &str, mode: Mode) -> Result<String> {
        let fallback = serde_json::json!({
            "surface": {
                "border": "1px solid {color.light.border}",
                "border_dash": "4px 4px",
                "radius": "{radius.large}",
                "min_height": "160px",
                "padding": "{spacing.l}",
                "gap": "{spacing.m}"
            },
            "narrow_breakpoint": "640px",
            "narrow_surface": { "min_height": "128px" },
            "header": { "max_width": "384px", "gap": "{spacing.s}" },
            "icon": {
                "background": "{color.light.surface_sunken}",
                "size": "32px",
                "radius": "{radius.medium}",
                "icon_size": "16px",
                "margin_bottom": "{spacing.s}"
            },
            "title": {
                "color": "{color.light.text_primary}",
                "font_size": "{typography.font_size.base}",
                "font_weight": "{typography.font_weight.medium}",
                "line_height": "19.6px"
            },
            "description": {
                "color": "{color.light.text_secondary}",
                "font_size": "{typography.font_size.base}",
                "font_weight": "{typography.font_weight.regular}",
                "line_height": "23.52px"
            },
            "action": { "max_width": "384px", "gap": "10px" }
        });
        let empty_state = self
            .dsp
            .pointer("/components/empty_state")
            .unwrap_or(&fallback);
        Ok(format!(
            r#"{krate}::EmptyStateTheme {{
                surface_style: {surface_style},
                narrow_surface_style: {narrow_surface_style},
                narrow_breakpoint: {narrow_breakpoint},
                header_style: {header_style},
                icon_style: {icon_style},
                title_style: {title_style},
                description_style: {description_style},
                action_style: {action_style},
            }}"#,
            surface_style = self.style_expr(krate, mode, empty_state.get("surface"))?,
            narrow_surface_style =
                self.style_expr(krate, mode, empty_state.get("narrow_surface"))?,
            narrow_breakpoint = f32_lit(
                self.dsp_dimension_optional("/components/empty_state/narrow_breakpoint", 640.0,)?
            ),
            header_style = self.style_expr(krate, mode, empty_state.get("header"))?,
            icon_style = self.style_expr(krate, mode, empty_state.get("icon"))?,
            title_style = self.style_expr(krate, mode, empty_state.get("title"))?,
            description_style = self.style_expr(krate, mode, empty_state.get("description"))?,
            action_style = self.style_expr(krate, mode, empty_state.get("action"))?,
        ))
    }

    fn feature_icon_theme_expr(&self, krate: &str, mode: Mode) -> Result<String> {
        Ok(format!(
            r#"{krate}::FeatureIconTheme {{
                sizes: vec![{sizes}],
                tones: vec![{tones}],
                shadow: {shadow},
            }}"#,
            sizes = self.size_styles_expr(krate, mode, "/components/feature_icon/sizes")?,
            tones = self.enum_styles_expr(
                krate,
                mode,
                "/components/feature_icon/tones",
                feature_icon_tone_variant
            )?,
            shadow = self.shadow_option_from_value_expr(
                krate,
                mode,
                self.dsp.pointer("/components/feature_icon/box_shadow"),
            )?,
        ))
    }

    fn palette_expr(&self, krate: &str, mode: Mode) -> Result<String> {
        let mut colors = Vec::new();
        if let Some(items) = self
            .dsp
            .pointer("/data_visualization/palette")
            .or_else(|| self.dsp.pointer("/charts/palette"))
            .and_then(Value::as_array)
        {
            for item in items {
                if let Some(color) = self.style_fill_color_expr(krate, mode, Some(item))? {
                    colors.push(color);
                }
            }
        }
        if colors.is_empty() {
            let token_paths = [
                "color.teal.700",
                "color.brand.blue.600",
                "color.semantic.warning",
                "color.semantic.error",
                "color.semantic.success",
                "color.semantic.info",
                "color.brand.orange.600",
                "color.teal.500",
            ];
            for path in token_paths {
                if self.tokens.contains(path) {
                    colors.push(self.color_expr(krate, path)?);
                }
            }
        }
        if colors.is_empty() {
            colors = vec![
                self.color_literal_expr(krate, "#14B8A6")?,
                self.color_literal_expr(krate, "#4DA6E0")?,
                self.color_literal_expr(krate, "#F59E0B")?,
                self.color_literal_expr(krate, "#F43F5E")?,
            ];
        }
        Ok(colors.join(","))
    }

    fn size_styles_expr(&self, krate: &str, mode: Mode, pointer: &str) -> Result<String> {
        self.size_styles_expr_from_value(krate, mode, self.dsp.pointer(pointer))
    }

    fn size_styles_expr_from_value(
        &self,
        krate: &str,
        mode: Mode,
        value: Option<&Value>,
    ) -> Result<String> {
        let Some(obj) = value.and_then(Value::as_object) else {
            return Ok(String::new());
        };
        let mut items = Vec::new();
        for (name, value) in obj {
            if let Some(variant) = component_size_variant(krate, name) {
                items.push(format!(
                    "({variant}, {})",
                    self.style_expr(krate, mode, Some(value))?
                ));
            }
        }
        Ok(items.join(","))
    }

    fn enum_styles_expr(
        &self,
        krate: &str,
        mode: Mode,
        pointer: &str,
        variant: fn(&str, &str) -> Option<String>,
    ) -> Result<String> {
        let Some(obj) = self.dsp.pointer(pointer).and_then(Value::as_object) else {
            return Ok(String::new());
        };
        let mut items = Vec::new();
        for (name, value) in obj {
            if let Some(variant_expr) = variant(krate, name) {
                items.push(format!(
                    "({variant_expr}, {})",
                    self.style_expr(krate, mode, Some(value))?
                ));
            }
        }
        Ok(items.join(","))
    }

    fn enum_state_styles_expr(
        &self,
        krate: &str,
        mode: Mode,
        pointer: &str,
        variant: fn(&str, &str) -> Option<String>,
    ) -> Result<String> {
        let Some(obj) = self.dsp.pointer(pointer).and_then(Value::as_object) else {
            return Ok(String::new());
        };
        let mut items = Vec::new();
        for (name, value) in obj {
            if let Some(variant_expr) = variant(krate, name) {
                items.push(format!(
                    "({variant_expr}, {})",
                    self.state_styles_expr(krate, mode, Some(value))?
                ));
            }
        }
        Ok(items.join(","))
    }

    fn state_styles_expr(&self, krate: &str, mode: Mode, value: Option<&Value>) -> Result<String> {
        let state = |name: &str| -> Result<String> {
            let expr = value
                .and_then(|v| v.get(name))
                .map(|v| self.style_expr(krate, mode, Some(v)))
                .transpose()?;
            Ok(expr
                .map(|expr| format!("Some({expr})"))
                .unwrap_or_else(|| "None".into()))
        };
        let default = value
            .and_then(|v| v.get("default"))
            .map(|v| self.style_expr(krate, mode, Some(v)))
            .transpose()?
            .unwrap_or_else(|| format!("{krate}::ResolvedComponentStyle::default()"));
        Ok(format!(
            r#"{krate}::ComponentStateStyles {{
                default: {default},
                hover: {hover},
                active: {active},
                focus: {focus},
                disabled: {disabled},
                error: {error},
                selected: {selected},
            }}"#,
            hover = state("hover")?,
            active = state("active")?,
            focus = state("focus")?,
            disabled = state("disabled")?,
            error = state("error")?,
            selected = state("selected")?,
        ))
    }

    fn style_expr_for_component(
        &self,
        krate: &str,
        mode: Mode,
        pointer: &str,
        keys: &[&str],
    ) -> Result<String> {
        let Some(source) = self.dsp.pointer(pointer).and_then(Value::as_object) else {
            return self.style_expr(krate, mode, None);
        };
        let mut obj = serde_json::Map::new();
        for key in keys {
            if let Some(value) = source.get(*key) {
                obj.insert((*key).to_string(), value.clone());
            }
        }
        self.style_expr(krate, mode, Some(&Value::Object(obj)))
    }

    fn style_expr(&self, krate: &str, mode: Mode, value: Option<&Value>) -> Result<String> {
        let Some(value) = value else {
            return Ok(format!("{krate}::ResolvedComponentStyle::default()"));
        };
        let background = self.fill_option_expr(krate, mode, field(value, "background"))?;
        let text_color = self
            .style_fill_color_expr(krate, mode, field(value, "color"))?
            .map(|expr| format!("Some({expr})"))
            .unwrap_or_else(|| "None".into());
        let border = self.border_option_expr(
            krate,
            mode,
            field(value, "border").or_else(|| field(value, "border_bottom")),
        )?;
        let shadows = self.shadow_layers_expr(
            krate,
            mode,
            field(value, "box_shadow").or_else(|| field(value, "shadow")),
        )?;
        let radius = self.style_dimension_option_expr(mode, field(value, "radius"))?;
        let height = self.style_dimension_option_expr(mode, field(value, "height"))?;
        let min_height = self.style_dimension_option_expr(mode, field(value, "min_height"))?;
        let width = self.style_dimension_option_expr(mode, field(value, "width"))?;
        let size = self.style_dimension_option_expr(mode, field(value, "size"))?;
        let width = if width == "None" { size.clone() } else { width };
        let height = if height == "None" { size } else { height };
        let padding_x = self.style_dimension_option_expr(mode, field(value, "padding_x"))?;
        let padding_y = self.style_dimension_option_expr(mode, field(value, "padding_y"))?;
        let padding = self.padding_option_expr(mode, field(value, "padding"))?;
        let margin = self.padding_option_expr(mode, field(value, "margin"))?;
        let gap = self.style_dimension_option_expr(mode, field(value, "gap"))?;
        let font_size = self.style_dimension_option_expr(mode, field(value, "font_size"))?;
        let font_family = self
            .resolved_value_string(mode, field(value, "font_family"))?
            .map(|family| format!("Some({}.to_string())", rust_string(&family)))
            .unwrap_or_else(|| "None".into());
        let font_weight = self.style_u16_option_expr(mode, field(value, "font_weight"))?;
        let line_height = self.style_dimension_option_expr(mode, field(value, "line_height"))?;
        let letter_spacing =
            self.style_dimension_option_expr(mode, field(value, "letter_spacing"))?;
        let icon_size = self.style_dimension_option_expr(mode, field(value, "icon_size"))?;
        let max_width = self.style_dimension_option_expr(mode, field(value, "max_width"))?;
        let margin_bottom =
            self.style_dimension_option_expr(mode, field(value, "margin_bottom"))?;
        let inset_top = self.style_dimension_option_expr(mode, field(value, "inset_top"))?;
        let inset_end = self.style_dimension_option_expr(mode, field(value, "inset_end"))?;
        let opacity = self.style_number_option_expr(mode, field(value, "opacity"))?;
        let translate_y = self.style_dimension_option_expr(mode, field(value, "translate_y"))?;
        let border_dash = self.dimension_list_option_expr(mode, field(value, "border_dash"))?;
        let transition = self.motion_option_expr(krate, mode, field(value, "transition"))?;
        Ok(format!(
            r#"{krate}::ResolvedComponentStyle {{
                background: {background},
                text_color: {text_color},
                border: {border},
                radius: {radius},
                height: {height},
                min_height: {min_height},
                width: {width},
                padding_x: {padding_x},
                padding_y: {padding_y},
                padding: {padding},
                margin: {margin},
                gap: {gap},
                font_size: {font_size},
                font_family: {font_family},
                font_weight: {font_weight},
                line_height: {line_height},
                letter_spacing: {letter_spacing},
                icon_size: {icon_size},
                max_width: {max_width},
                margin_bottom: {margin_bottom},
                inset_top: {inset_top},
                inset_end: {inset_end},
                opacity: {opacity},
                translate_y: {translate_y},
                border_dash: {border_dash},
                shadows: {shadows},
                transition: {transition},
            }}"#
        ))
    }

    fn fill_option_expr(&self, krate: &str, mode: Mode, value: Option<&Value>) -> Result<String> {
        let Some(fill) = self.fill_expr(krate, mode, value)? else {
            return Ok("None".into());
        };
        Ok(format!("Some({fill})"))
    }

    fn fill_expr(&self, krate: &str, mode: Mode, value: Option<&Value>) -> Result<Option<String>> {
        let Some(raw) = self.resolved_value_string(mode, value)? else {
            return Ok(None);
        };
        let raw = raw.trim();
        if raw.eq_ignore_ascii_case("none") || raw.eq_ignore_ascii_case("auto") {
            return Ok(None);
        }
        if raw.eq_ignore_ascii_case("transparent") {
            return Ok(Some(format!(
                "{krate}::Fill::Solid({})",
                color_expr(krate, 0, 0, 0, 0)
            )));
        }
        if raw.starts_with("linear-gradient(") {
            let stops = gradient_stops(raw)
                .into_iter()
                .enumerate()
                .map(|(idx, color)| {
                    let position = if idx == 0 { 0.0 } else { 1.0 };
                    Ok(format!(
                        "({}, {})",
                        f32_lit(position),
                        self.color_literal_expr(krate, color)?
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            return Ok(Some(format!(
                "{krate}::Fill::LinearGradient {{ start: (0.0, 0.0), end: (1.0, 1.0), stops: vec![{}] }}",
                stops.join(",")
            )));
        }
        if raw.starts_with("radial-gradient(") {
            let colors = gradient_stops(raw);
            let mut stops = Vec::new();
            for (idx, color) in colors.iter().enumerate() {
                let position = if colors.len() <= 1 {
                    0.0
                } else {
                    idx as f32 / (colors.len() - 1) as f32
                };
                stops.push(format!(
                    "({}, {})",
                    f32_lit(position),
                    self.color_literal_expr(krate, color)?
                ));
            }
            return Ok(Some(format!(
                "{krate}::Fill::RadialGradient {{ center: (0.5, 0.5), radius: 1.0, stops: vec![{}] }}",
                stops.join(",")
            )));
        }
        if parse_color(raw).is_ok() {
            return Ok(Some(format!(
                "{krate}::Fill::Solid({})",
                self.color_literal_expr(krate, raw)?
            )));
        }
        Ok(None)
    }

    fn style_fill_color_expr(
        &self,
        krate: &str,
        mode: Mode,
        value: Option<&Value>,
    ) -> Result<Option<String>> {
        let Some(raw) = self.resolved_value_string(mode, value)? else {
            return Ok(None);
        };
        let raw = raw.trim();
        if raw.eq_ignore_ascii_case("transparent") {
            return Ok(Some(color_expr(krate, 0, 0, 0, 0)));
        }
        if parse_color(raw).is_ok() {
            return Ok(Some(self.color_literal_expr(krate, raw)?));
        }
        Ok(None)
    }

    fn border_option_expr(&self, krate: &str, mode: Mode, value: Option<&Value>) -> Result<String> {
        let Some(raw) = self.resolved_value_string(mode, value)? else {
            return Ok("None".into());
        };
        let raw = raw.trim();
        if raw.eq_ignore_ascii_case("none") || raw.eq_ignore_ascii_case("transparent") {
            return Ok("None".into());
        }
        let Some((width, color)) = parse_border(raw) else {
            return Ok("None".into());
        };
        Ok(format!(
            "Some({krate}::ComponentBorder {{ fill: {krate}::Fill::Solid({}), width: {} }})",
            self.color_literal_expr(krate, color)?,
            f32_lit(width)
        ))
    }

    fn shadow_layers_expr(&self, krate: &str, mode: Mode, value: Option<&Value>) -> Result<String> {
        let Some(raw) = self.resolved_value_string(mode, value)? else {
            return Ok("Vec::new()".into());
        };
        let layers = parse_shadow_layers(&raw);
        let exprs = layers
            .iter()
            .map(|layer| shadow_layer_expr(krate, layer))
            .collect::<Vec<_>>();
        Ok(format!("vec![{}]", exprs.join(",")))
    }

    fn shadow_option_from_value_expr(
        &self,
        krate: &str,
        mode: Mode,
        value: Option<&Value>,
    ) -> Result<String> {
        let Some(raw) = self.resolved_value_string(mode, value)? else {
            return Ok("None".into());
        };
        let layers = parse_shadow_layers(&raw);
        if let Some(layer) = layers.iter().find(|layer| !layer.inset) {
            Ok(format!("Some({})", box_shadow_expr(krate, layer)))
        } else {
            Ok("None".into())
        }
    }

    fn style_dimension_optional(
        &self,
        mode: Mode,
        value: Option<&Value>,
        fallback: f32,
    ) -> Result<f32> {
        let Some(raw) = self.resolved_value_string(mode, value)? else {
            return Ok(fallback);
        };
        parse_dimension(&raw)
            .or_else(|_| raw.parse::<f32>())
            .or(Ok(fallback))
    }

    fn style_number_optional(
        &self,
        mode: Mode,
        value: Option<&Value>,
        fallback: f32,
    ) -> Result<f32> {
        let Some(raw) = self.resolved_value_string(mode, value)? else {
            return Ok(fallback);
        };
        Ok(raw.parse::<f32>().unwrap_or(fallback))
    }

    fn style_duration_optional(
        &self,
        mode: Mode,
        value: Option<&Value>,
        fallback: u64,
    ) -> Result<u64> {
        let Some(raw) = self.resolved_value_string(mode, value)? else {
            return Ok(fallback);
        };
        Ok(parse_duration_ms(&raw).unwrap_or(fallback))
    }

    fn style_dimension_option_expr(&self, mode: Mode, value: Option<&Value>) -> Result<String> {
        let Some(raw) = self.resolved_value_string(mode, value)? else {
            return Ok("None".into());
        };
        if raw == "auto" {
            return Ok("None".into());
        }
        let Ok(dimension) = parse_dimension(&raw).or_else(|_| raw.parse::<f32>()) else {
            return Ok("None".into());
        };
        Ok(format!("Some({})", f32_lit(dimension)))
    }

    fn style_number_option_expr(&self, mode: Mode, value: Option<&Value>) -> Result<String> {
        let Some(raw) = self.resolved_value_string(mode, value)? else {
            return Ok("None".into());
        };
        let Ok(number) = raw.parse::<f32>() else {
            return Ok("None".into());
        };
        Ok(format!("Some({})", f32_lit(number)))
    }

    fn dimension_list_option_expr(&self, mode: Mode, value: Option<&Value>) -> Result<String> {
        let Some(value) = value else {
            return Ok("None".into());
        };
        let values = match value {
            Value::Array(values) => {
                let mut resolved = Vec::new();
                for value in values {
                    if let Some(raw) = self.resolved_value_string(mode, Some(value))? {
                        resolved.push(raw);
                    }
                }
                resolved
            }
            _ => self
                .resolved_value_string(mode, Some(value))?
                .map(|raw| raw.split_whitespace().map(str::to_owned).collect())
                .unwrap_or_default(),
        };
        let dimensions = values
            .iter()
            .filter_map(|raw| parse_dimension(raw).or_else(|_| raw.parse::<f32>()).ok())
            .map(f32_lit)
            .collect::<Vec<_>>();
        if dimensions.is_empty() {
            Ok("None".into())
        } else {
            Ok(format!("Some(vec![{}])", dimensions.join(",")))
        }
    }

    fn style_u16_optional(&self, mode: Mode, value: Option<&Value>, fallback: u16) -> Result<u16> {
        let Some(raw) = self.resolved_value_string(mode, value)? else {
            return Ok(fallback);
        };
        Ok(raw.parse::<u16>().unwrap_or(fallback))
    }

    fn style_u16_option_expr(&self, mode: Mode, value: Option<&Value>) -> Result<String> {
        let Some(raw) = self.resolved_value_string(mode, value)? else {
            return Ok("None".into());
        };
        Ok(raw
            .parse::<u16>()
            .map(|value| format!("Some({value})"))
            .unwrap_or_else(|_| "None".into()))
    }

    fn padding_option_expr(&self, mode: Mode, value: Option<&Value>) -> Result<String> {
        let Some(raw) = self.resolved_value_string(mode, value)? else {
            return Ok("None".into());
        };
        let Ok(padding) = parse_padding(&raw) else {
            return Ok("None".into());
        };
        Ok(format!(
            "Some([{}, {}, {}, {}])",
            f32_lit(padding[0]),
            f32_lit(padding[1]),
            f32_lit(padding[2]),
            f32_lit(padding[3])
        ))
    }

    fn motion_option_expr(&self, krate: &str, mode: Mode, value: Option<&Value>) -> Result<String> {
        let Some(raw) = self.resolved_value_string(mode, value)? else {
            return Ok("None".into());
        };
        let duration = raw
            .split_whitespace()
            .find_map(|part| parse_duration_ms(part).ok())
            .unwrap_or(self.duration_ms_optional("motion.duration.fast", 160)?);
        let easing = if raw.contains("cubic-bezier") {
            let start = raw.find("cubic-bezier").unwrap_or(0);
            let value = raw[start..].split_whitespace().next().unwrap_or("linear");
            easing_expr(krate, value.trim_end_matches(','))
        } else if raw.contains("ease") {
            format!("{krate}::EasingCurve::Ease")
        } else {
            self.easing_expr(krate, "motion.easing.standard")?
        };
        Ok(format!(
            "Some({krate}::ComponentMotion {{ duration_ms: {duration}, easing: {easing} }})"
        ))
    }

    fn resolved_value_string(&self, mode: Mode, value: Option<&Value>) -> Result<Option<String>> {
        let Some(value) = value else {
            return Ok(None);
        };
        match value {
            Value::String(raw) => Ok(Some(self.resolve_refs_in_string_for_mode(raw, mode)?)),
            Value::Number(number) => Ok(Some(number.to_string())),
            _ => Ok(None),
        }
    }

    fn design_tokens_expr(&self, krate: &str) -> Result<String> {
        let mut items = Vec::new();
        for path in self.tokens.paths() {
            let token = self.tokens.get_raw(path).unwrap();
            let kind = token.kind.clone().unwrap_or_else(|| "custom".into());
            let resolved = self.resolve_token_string(path)?;
            let value_expr = self.design_value_expr(krate, &kind, &resolved)?;
            items.push(format!(
                "{krate}::DesignToken {{ path: {}, kind: {}, value: {value_expr} }}",
                rust_string(path),
                rust_string(&kind),
            ));
        }
        Ok(format!(
            "{krate}::DesignTokenSet {{ tokens: vec![{}] }}",
            items.join(",")
        ))
    }

    fn components_expr(&self, krate: &str) -> Result<String> {
        let Some(obj) = self.dsp.get("components").and_then(Value::as_object) else {
            return Ok("Vec::new()".into());
        };
        let mut components = Vec::new();
        for (name, value) in obj {
            components.push(self.component_spec_expr(krate, name, value)?);
        }
        Ok(format!("vec![{}]", components.join(",")))
    }

    fn component_spec_expr(&self, krate: &str, name: &str, value: &Value) -> Result<String> {
        let description = value
            .get("$description")
            .and_then(Value::as_str)
            .unwrap_or("");
        let anatomy = value
            .get("anatomy")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(rust_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let props = self.object_properties_expr(krate, value)?;
        Ok(format!(
            "{krate}::DesignComponentSpec {{ name: {}, description: {}, anatomy: vec![{}], properties: {props} }}",
            rust_string(name), rust_string(description), anatomy.join(",")
        ))
    }

    fn patterns_expr(&self, krate: &str) -> Result<String> {
        let Some(obj) = self.dsp.get("patterns").and_then(Value::as_object) else {
            return Ok("Vec::new()".into());
        };
        let mut patterns = Vec::new();
        for (name, value) in obj {
            let description = value
                .get("$description")
                .and_then(Value::as_str)
                .unwrap_or("");
            let props = self.object_properties_expr(krate, value)?;
            patterns.push(format!(
                "{krate}::DesignPatternSpec {{ name: {}, description: {}, properties: {props} }}",
                rust_string(name),
                rust_string(description)
            ));
        }
        Ok(format!("vec![{}]", patterns.join(",")))
    }

    fn assets_expr(&self, krate: &str) -> Result<String> {
        let assets = self.dsp.get("assets").unwrap_or(&Value::Null);
        let mut logos = Vec::new();
        if let Some(arr) = assets.get("logos").and_then(Value::as_array) {
            for item in arr {
                logos.push(asset_expr(krate, item));
            }
        }
        let mut fonts = Vec::new();
        if let Some(arr) = assets.get("fonts").and_then(Value::as_array) {
            for item in arr {
                fonts.push(font_asset_expr(krate, item)?);
            }
        }
        Ok(format!(
            "{krate}::DesignAssetManifest {{ logos: vec![{}], fonts: vec![{}] }}",
            logos.join(","),
            fonts.join(",")
        ))
    }

    fn font_faces_expr(&self, krate: &str) -> Result<String> {
        let Some(fonts) = self.dsp.pointer("/assets/fonts").and_then(Value::as_array) else {
            return Ok("&[]".into());
        };
        let root = self
            .dsp_path
            .parent()
            .ok_or_else(|| anyhow!("DSP path has no parent: {}", self.dsp_path.display()))?;
        let faces = fonts
            .iter()
            .map(|font| packaged_font_expr(krate, root, font))
            .collect::<Result<Vec<_>>>()?;
        Ok(format!("&[{}]", faces.join(",")))
    }

    fn font_paths(&self) -> Result<Vec<PathBuf>> {
        let Some(fonts) = self.dsp.pointer("/assets/fonts").and_then(Value::as_array) else {
            return Ok(Vec::new());
        };
        let root = self
            .dsp_path
            .parent()
            .ok_or_else(|| anyhow!("DSP path has no parent: {}", self.dsp_path.display()))?;
        fonts
            .iter()
            .map(|font| resolve_font_path(root, font))
            .collect()
    }

    fn object_properties_expr(&self, krate: &str, value: &Value) -> Result<String> {
        let Some(obj) = value.as_object() else {
            return Ok("Vec::new()".into());
        };
        let mut props = Vec::new();
        for (key, value) in obj {
            if key.starts_with('$') || key == "anatomy" || key == "tab_anatomy" {
                continue;
            }
            let value_expr = self.any_design_value_expr(krate, value)?;
            props.push(format!(
                "{krate}::DesignProperty {{ name: {}, value: {value_expr} }}",
                rust_string(key)
            ));
        }
        Ok(format!("vec![{}]", props.join(",")))
    }

    fn any_design_value_expr(&self, krate: &str, value: &Value) -> Result<String> {
        match value {
            Value::String(s) => {
                let resolved = self.resolve_refs_in_string(s)?;
                self.design_value_expr(krate, "custom", &resolved)
            }
            Value::Number(n) => Ok(format!(
                "{krate}::DesignValue::Number({})",
                f32_lit(n.as_f64().unwrap_or_default() as f32)
            )),
            Value::Bool(b) => Ok(format!("{krate}::DesignValue::Bool({b})")),
            Value::Array(arr) => {
                let values = arr
                    .iter()
                    .map(|v| self.any_design_value_expr(krate, v))
                    .collect::<Result<Vec<_>>>()?;
                Ok(format!(
                    "{krate}::DesignValue::List(vec![{}])",
                    values.join(",")
                ))
            }
            Value::Object(obj) => {
                if let Some(v) = obj.get("value").or_else(|| obj.get("$value")) {
                    return self.any_design_value_expr(krate, v);
                }
                let mut props = Vec::new();
                for (key, child) in obj {
                    if key.starts_with('$') {
                        continue;
                    }
                    let value_expr = self.any_design_value_expr(krate, child)?;
                    props.push(format!(
                        "{krate}::DesignProperty {{ name: {}, value: {value_expr} }}",
                        rust_string(key)
                    ));
                }
                Ok(format!(
                    "{krate}::DesignValue::Object(vec![{}])",
                    props.join(",")
                ))
            }
            Value::Null => Ok(format!("{krate}::DesignValue::None")),
        }
    }

    fn design_value_expr(&self, krate: &str, kind: &str, value: &str) -> Result<String> {
        if kind == "color" || value.trim_start().starts_with('#') || value.starts_with("rgb") {
            if let Ok((r, g, b, a)) = parse_color(value) {
                return Ok(format!(
                    "{krate}::DesignValue::Color({})",
                    color_expr(krate, r, g, b, a)
                ));
            }
        }
        if kind == "dimension" || kind == "size" || value.trim_end().ends_with("px") {
            if let Ok(px) = parse_dimension(value) {
                return Ok(format!("{krate}::DesignValue::Dimension({})", f32_lit(px)));
            }
        }
        if kind == "duration" || value.trim_end().ends_with("ms") {
            if let Ok(ms) = parse_duration_ms(value) {
                return Ok(format!("{krate}::DesignValue::DurationMs({ms})"));
            }
        }
        if kind == "shadow" || value.contains("rgba(") || value == "none" {
            let layers = parse_shadow_layers(value);
            let exprs = layers
                .iter()
                .map(|layer| shadow_layer_expr(krate, layer))
                .collect::<Vec<_>>();
            return Ok(format!(
                "{krate}::DesignValue::Shadow(vec![{}])",
                exprs.join(",")
            ));
        }
        if kind == "cubicBezier"
            || value.starts_with("cubic-bezier")
            || matches!(value, "linear" | "ease")
        {
            return Ok(format!(
                "{krate}::DesignValue::Easing({})",
                easing_expr(krate, value)
            ));
        }
        if let Ok(num) = value.parse::<f32>() {
            return Ok(format!("{krate}::DesignValue::Number({})", f32_lit(num)));
        }
        Ok(format!(
            "{krate}::DesignValue::Text({})",
            rust_string(value)
        ))
    }

    fn color_expr(&self, krate: &str, path: &str) -> Result<String> {
        let value = self.resolve_token_string(path)?;
        self.color_literal_expr(krate, &value)
    }

    fn color_expr_optional(&self, krate: &str, path: &str, fallback: &str) -> Result<String> {
        if self.tokens.contains(path) {
            self.color_expr(krate, path)
        } else {
            self.color_literal_expr(krate, fallback)
        }
    }

    fn color_literal_expr(&self, krate: &str, value: &str) -> Result<String> {
        let (r, g, b, a) = parse_color(value).with_context(|| format!("invalid color {value}"))?;
        Ok(color_expr(krate, r, g, b, a))
    }

    fn dimension(&self, path: &str) -> Result<f32> {
        let value = self.resolve_token_string(path)?;
        parse_dimension(&value).with_context(|| format!("invalid dimension token {path} = {value}"))
    }

    fn dimension_optional(&self, path: &str, fallback: f32) -> Result<f32> {
        if self.tokens.contains(path) {
            self.dimension(path)
        } else {
            Ok(fallback)
        }
    }

    fn number_optional(&self, path: &str, fallback: f32) -> Result<f32> {
        if !self.tokens.contains(path) {
            return Ok(fallback);
        }
        let value = self.resolve_token_string(path)?;
        value
            .parse::<f32>()
            .or_else(|_| parse_dimension(&value))
            .with_context(|| format!("invalid number token {path} = {value}"))
    }

    fn duration_ms_optional(&self, path: &str, fallback: u64) -> Result<u64> {
        if !self.tokens.contains(path) {
            return Ok(fallback);
        }
        let value = self.resolve_token_string(path)?;
        parse_duration_ms(&value)
            .with_context(|| format!("invalid duration token {path} = {value}"))
    }

    fn string_token_optional(&self, path: &str, fallback: &str) -> Result<String> {
        if !self.tokens.contains(path) {
            return Ok(fallback.to_string());
        }
        self.resolve_token_string(path)
    }

    fn shadow_option_expr(&self, krate: &str, path: &str) -> Result<String> {
        let value = if self.tokens.contains(path) {
            self.resolve_token_string(path)?
        } else {
            return Ok("None".into());
        };
        let layers = parse_shadow_layers(&value);
        if let Some(layer) = layers.iter().find(|layer| !layer.inset) {
            Ok(format!("Some({})", box_shadow_expr(krate, layer)))
        } else {
            Ok("None".into())
        }
    }

    fn easing_expr(&self, krate: &str, path: &str) -> Result<String> {
        let value = if self.tokens.contains(path) {
            self.resolve_token_string(path)?
        } else {
            "linear".into()
        };
        Ok(easing_expr(krate, &value))
    }

    fn dsp_dimension_optional(&self, pointer: &str, fallback: f32) -> Result<f32> {
        let Some(value) = self.dsp.pointer(pointer) else {
            return Ok(fallback);
        };
        let raw = match value {
            Value::String(s) => self.resolve_refs_in_string(s)?,
            Value::Number(n) => n.to_string(),
            _ => return Ok(fallback),
        };
        parse_dimension(&raw)
            .or_else(|_| raw.parse::<f32>())
            .or(Ok(fallback))
    }

    fn resolve_token_string(&self, path: &str) -> Result<String> {
        self.tokens.resolve(path)
    }

    fn resolve_refs_in_string(&self, raw: &str) -> Result<String> {
        let mut out = String::new();
        let mut rest = raw;
        while let Some(start) = rest.find('{') {
            let (before, after_start) = rest.split_at(start);
            out.push_str(before);
            let after_start = &after_start[1..];
            let Some(end) = after_start.find('}') else {
                bail!("unclosed token reference in {raw}");
            };
            let token = &after_start[..end];
            if self.tokens.contains(token) {
                out.push_str(&self.resolve_token_string(token)?);
            } else {
                out.push('{');
                out.push_str(token);
                out.push('}');
            }
            rest = &after_start[end + 1..];
        }
        out.push_str(rest);
        Ok(out)
    }

    fn resolve_refs_in_string_for_mode(&self, raw: &str, mode: Mode) -> Result<String> {
        let mut out = String::new();
        let mut rest = raw;
        while let Some(start) = rest.find('{') {
            let (before, after_start) = rest.split_at(start);
            out.push_str(before);
            let after_start = &after_start[1..];
            let Some(end) = after_start.find('}') else {
                bail!("unclosed token reference in {raw}");
            };
            let mut token = after_start[..end].to_string();
            match mode {
                Mode::Light => {}
                Mode::Dark => {
                    if let Some(suffix) = token.strip_prefix("color.light.") {
                        let dark = format!("color.dark.{suffix}");
                        if self.tokens.contains(&dark) {
                            token = dark;
                        }
                    }
                }
            }
            if self.tokens.contains(&token) {
                out.push_str(&self.resolve_token_string(&token)?);
            } else {
                out.push('{');
                out.push_str(&token);
                out.push('}');
            }
            rest = &after_start[end + 1..];
        }
        out.push_str(rest);
        Ok(out)
    }
}

#[derive(Debug, Clone, Copy)]
enum Mode {
    Light,
    Dark,
}
impl Mode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    fn color_name(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

#[derive(Debug, Clone)]
struct Token {
    value: String,
    kind: Option<String>,
}

#[derive(Debug, Clone)]
struct TokenStore {
    tokens: BTreeMap<String, Token>,
}

impl TokenStore {
    fn from_value(value: &Value) -> Result<Self> {
        let mut tokens = BTreeMap::new();
        flatten_tokens(value, String::new(), &mut tokens)?;
        Ok(Self { tokens })
    }

    fn contains(&self, path: &str) -> bool {
        self.tokens.contains_key(path)
    }
    fn paths(&self) -> impl Iterator<Item = &str> {
        self.tokens.keys().map(String::as_str)
    }
    fn get_raw(&self, path: &str) -> Option<&Token> {
        self.tokens.get(path)
    }

    fn resolve(&self, path: &str) -> Result<String> {
        self.resolve_inner(path, &mut BTreeSet::new())
    }

    fn resolve_inner(&self, path: &str, seen: &mut BTreeSet<String>) -> Result<String> {
        if !seen.insert(path.to_string()) {
            bail!("cyclic token reference involving {path}");
        }
        let token = self
            .tokens
            .get(path)
            .ok_or_else(|| anyhow!("unknown token reference {{{path}}}"))?;
        let value = token.value.trim();
        if let Some(inner) = value.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
            let resolved = self.resolve_inner(inner, seen)?;
            seen.remove(path);
            return Ok(resolved);
        }
        let mut out = String::new();
        let mut rest = value;
        while let Some(start) = rest.find('{') {
            let (before, after_start) = rest.split_at(start);
            out.push_str(before);
            let after_start = &after_start[1..];
            let Some(end) = after_start.find('}') else {
                bail!("unclosed token reference in {value}");
            };
            let inner = &after_start[..end];
            out.push_str(&self.resolve_inner(inner, seen)?);
            rest = &after_start[end + 1..];
        }
        out.push_str(rest);
        seen.remove(path);
        Ok(out)
    }
}

fn flatten_tokens(value: &Value, prefix: String, out: &mut BTreeMap<String, Token>) -> Result<()> {
    let Some(obj) = value.as_object() else {
        return Ok(());
    };
    for (key, child) in obj {
        if key.starts_with('$') {
            continue;
        }
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        if let Some(child_obj) = child.as_object() {
            if let Some(raw_value) = token_object_value(child_obj) {
                let token_value = match raw_value {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    other => other.to_string(),
                };
                let kind = child_obj
                    .get("type")
                    .or_else(|| child_obj.get("$type"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
                out.insert(
                    path,
                    Token {
                        value: token_value,
                        kind,
                    },
                );
            } else {
                flatten_tokens(child, path, out)?;
            }
        }
    }
    Ok(())
}

fn token_object_value(obj: &serde_json::Map<String, Value>) -> Option<&Value> {
    let value_field = obj.get("value").or_else(|| obj.get("$value"))?;
    if obj.contains_key("type") || obj.contains_key("$type") {
        return Some(value_field);
    }
    if value_field
        .as_object()
        .is_some_and(looks_like_nested_token_object)
    {
        return None;
    }
    Some(value_field)
}

fn looks_like_nested_token_object(obj: &serde_json::Map<String, Value>) -> bool {
    obj.contains_key("value")
        || obj.contains_key("$value")
        || obj.contains_key("type")
        || obj.contains_key("$type")
}

#[derive(Debug, Clone)]
struct ShadowLayer {
    color: (u8, u8, u8, u8),
    offset_x: f32,
    offset_y: f32,
    blur_radius: f32,
    spread_radius: f32,
    inset: bool,
}

fn parse_shadow_layers(value: &str) -> Vec<ShadowLayer> {
    if value.trim() == "none" {
        return Vec::new();
    }
    split_css_layers(value)
        .into_iter()
        .filter_map(|layer| parse_shadow_layer(layer.trim()).ok())
        .collect()
}

fn parse_shadow_layer(layer: &str) -> Result<ShadowLayer> {
    let inset = layer.contains("inset");
    let color_start = layer
        .find("rgba(")
        .or_else(|| layer.find("rgb("))
        .ok_or_else(|| anyhow!("shadow has no rgb/rgba color: {layer}"))?;
    let color_end = layer[color_start..]
        .find(')')
        .ok_or_else(|| anyhow!("unterminated rgb/rgba in shadow: {layer}"))?
        + color_start;
    let color_raw = &layer[color_start..=color_end];
    let color = parse_rgb_color(color_raw)?;
    let nums = layer[..color_start]
        .replace("inset", "")
        .split_whitespace()
        .filter_map(|part| parse_dimension(part).ok())
        .collect::<Vec<_>>();
    Ok(ShadowLayer {
        color,
        offset_x: *nums.get(0).unwrap_or(&0.0),
        offset_y: *nums.get(1).unwrap_or(&0.0),
        blur_radius: *nums.get(2).unwrap_or(&0.0),
        spread_radius: *nums.get(3).unwrap_or(&0.0),
        inset,
    })
}

fn split_css_layers(value: &str) -> Vec<&str> {
    let mut layers = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (idx, ch) in value.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                layers.push(&value[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    layers.push(&value[start..]);
    layers
}

fn parse_hex_color(value: &str) -> Result<(u8, u8, u8, u8)> {
    let hex = value
        .trim()
        .strip_prefix('#')
        .ok_or_else(|| anyhow!("not a hex color: {value}"))?;
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16)?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16)?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16)?;
            Ok((r, g, b, 255))
        }
        6 | 8 => {
            let r = u8::from_str_radix(&hex[0..2], 16)?;
            let g = u8::from_str_radix(&hex[2..4], 16)?;
            let b = u8::from_str_radix(&hex[4..6], 16)?;
            let a = if hex.len() == 8 {
                u8::from_str_radix(&hex[6..8], 16)?
            } else {
                255
            };
            Ok((r, g, b, a))
        }
        _ => bail!("invalid hex color length: {value}"),
    }
}

fn parse_color(value: &str) -> Result<(u8, u8, u8, u8)> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("transparent") {
        return Ok((0, 0, 0, 0));
    }
    if value.starts_with('#') {
        return parse_hex_color(value);
    }
    if value.starts_with("rgb(") || value.starts_with("rgba(") {
        return parse_rgb_color(value);
    }
    bail!("unsupported color value: {value}")
}

fn parse_rgb_color(value: &str) -> Result<(u8, u8, u8, u8)> {
    let inner = value
        .trim()
        .trim_start_matches("rgba(")
        .trim_start_matches("rgb(")
        .trim_end_matches(')');
    let parts = inner.split(',').map(str::trim).collect::<Vec<_>>();
    let r = parts
        .get(0)
        .ok_or_else(|| anyhow!("missing red in {value}"))?
        .parse::<u8>()?;
    let g = parts
        .get(1)
        .ok_or_else(|| anyhow!("missing green in {value}"))?
        .parse::<u8>()?;
    let b = parts
        .get(2)
        .ok_or_else(|| anyhow!("missing blue in {value}"))?
        .parse::<u8>()?;
    let a = if let Some(alpha) = parts.get(3) {
        let alpha = alpha.parse::<f32>()?.clamp(0.0, 1.0);
        (alpha * 255.0).round() as u8
    } else {
        255
    };
    Ok((r, g, b, a))
}

fn parse_border(value: &str) -> Option<(f32, &str)> {
    let mut width = None;
    let mut color_start = None;
    for part in value.split_whitespace() {
        if width.is_none() {
            if let Ok(px) = parse_dimension(part) {
                width = Some(px);
                continue;
            }
        }
        if part.starts_with('#') || part.starts_with("rgb") || part == "transparent" {
            color_start = value.find(part);
            break;
        }
    }
    match (width, color_start) {
        (Some(width), Some(start)) => Some((width, value[start..].trim())),
        _ => None,
    }
}

fn parse_padding(value: &str) -> Result<[f32; 4]> {
    let parts = value
        .split_whitespace()
        .map(parse_dimension)
        .collect::<Result<Vec<_>>>()?;
    let (top, right, bottom, left) = match parts.as_slice() {
        [all] => (*all, *all, *all, *all),
        [vertical, horizontal] => (*vertical, *horizontal, *vertical, *horizontal),
        [top, horizontal, bottom] => (*top, *horizontal, *bottom, *horizontal),
        [top, right, bottom, left, ..] => (*top, *right, *bottom, *left),
        _ => (0.0, 0.0, 0.0, 0.0),
    };
    Ok([left, right, top, bottom])
}

fn gradient_stops(value: &str) -> Vec<&str> {
    let inner = value
        .split_once('(')
        .and_then(|(_, rest)| rest.strip_suffix(')'))
        .unwrap_or(value);
    split_css_layers(inner)
        .into_iter()
        .map(str::trim)
        .filter(|part| part.starts_with('#') || part.starts_with("rgb") || *part == "transparent")
        .map(|part| part.split_whitespace().next().unwrap_or(part))
        .collect()
}

fn parse_dimension(value: &str) -> Result<f32> {
    let trimmed = value.trim().trim_matches('"');
    if let Some(px) = trimmed.strip_suffix("px") {
        return Ok(px.trim().parse()?);
    }
    if let Some(em) = trimmed.strip_suffix("em") {
        return Ok(em.trim().parse()?);
    }
    Ok(trimmed.parse()?)
}

fn field<'a>(value: &'a Value, name: &str) -> Option<&'a Value> {
    value.get(name)
}

fn component_size_variant(krate: &str, name: &str) -> Option<String> {
    let variant = match name {
        "sm" => "Sm",
        "md" => "Md",
        "lg" => "Lg",
        "xl" => "Xl",
        _ => return None,
    };
    Some(format!("{krate}::ComponentSize::{variant}"))
}

fn button_hierarchy_variant(krate: &str, name: &str) -> Option<String> {
    let variant = match name {
        "primary" => "Primary",
        "secondary_color" => "SecondaryColor",
        "secondary_gray" => "SecondaryGray",
        "outline" => "Outline",
        "tertiary_color" => "TertiaryColor",
        "tertiary_gray" => "TertiaryGray",
        "link_color" => "LinkColor",
        "link_gray" => "LinkGray",
        "destructive" => "Destructive",
        _ => return None,
    };
    Some(format!("{krate}::ButtonHierarchy::{variant}"))
}

fn badge_tone_variant(krate: &str, name: &str) -> Option<String> {
    let variant = match name {
        "brand" => "Brand",
        "gray" => "Gray",
        "success" => "Success",
        "warning" => "Warning",
        "error" => "Error",
        "blue" => "Blue",
        "orange" => "Orange",
        _ => return None,
    };
    Some(format!("{krate}::BadgeTone::{variant}"))
}

fn card_pattern_variant(krate: &str, name: &str) -> Option<String> {
    let variant = match name {
        "plain" => "Plain",
        "raised" => "Raised",
        "tinted" => "Tinted",
        "elevated" => "Elevated",
        _ => return None,
    };
    Some(format!("{krate}::CardPattern::{variant}"))
}

fn feature_icon_tone_variant(krate: &str, name: &str) -> Option<String> {
    let variant = match name {
        "brand" => "Brand",
        "gray" => "Gray",
        "blue" => "Blue",
        "orange" => "Orange",
        _ => return None,
    };
    Some(format!("{krate}::FeatureIconTone::{variant}"))
}

fn parse_duration_ms(value: &str) -> Result<u64> {
    let trimmed = value.trim();
    if let Some(ms) = trimmed.strip_suffix("ms") {
        return Ok(ms.trim().parse()?);
    }
    if let Some(s) = trimmed.strip_suffix('s') {
        return Ok((s.trim().parse::<f32>()? * 1000.0).round() as u64);
    }
    Ok(trimmed.parse()?)
}

fn color_expr(krate: &str, r: u8, g: u8, b: u8, a: u8) -> String {
    format!("{krate}::Color {{ r: {r}, g: {g}, b: {b}, a: {a} }}")
}

fn box_shadow_expr(krate: &str, layer: &ShadowLayer) -> String {
    format!(
        "{krate}::BoxShadow {{ color: {}, offset: ({}, {}), blur_radius: {}, spread_radius: {}, inset: {} }}",
        color_expr(
            krate,
            layer.color.0,
            layer.color.1,
            layer.color.2,
            layer.color.3
        ),
        f32_lit(layer.offset_x),
        f32_lit(layer.offset_y),
        f32_lit(layer.blur_radius),
        f32_lit(layer.spread_radius),
        layer.inset
    )
}

fn shadow_layer_expr(krate: &str, layer: &ShadowLayer) -> String {
    format!(
        "{krate}::ShadowLayer {{ color: {}, offset: ({}, {}), blur_radius: {}, spread_radius: {}, inset: {} }}",
        color_expr(krate, layer.color.0, layer.color.1, layer.color.2, layer.color.3),
        f32_lit(layer.offset_x),
        f32_lit(layer.offset_y),
        f32_lit(layer.blur_radius),
        f32_lit(layer.spread_radius),
        layer.inset
    )
}

fn easing_expr(krate: &str, value: &str) -> String {
    let value = value.trim();
    if let Some(inner) = value
        .strip_prefix("cubic-bezier(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let nums = inner
            .split(',')
            .filter_map(|n| n.trim().parse::<f32>().ok())
            .collect::<Vec<_>>();
        if nums.len() == 4 {
            return format!(
                "{krate}::EasingCurve::CubicBezier({}, {}, {}, {})",
                f32_lit(nums[0]),
                f32_lit(nums[1]),
                f32_lit(nums[2]),
                f32_lit(nums[3])
            );
        }
    }
    match value {
        "linear" => format!("{krate}::EasingCurve::Linear"),
        "ease" => format!("{krate}::EasingCurve::Ease"),
        _ => format!("{krate}::EasingCurve::Named({})", rust_string(value)),
    }
}

fn asset_expr(krate: &str, item: &Value) -> String {
    let id = item
        .get("id")
        .and_then(Value::as_str)
        .or_else(|| item.get("family").and_then(Value::as_str))
        .unwrap_or("");
    let path = item.get("path").and_then(Value::as_str).unwrap_or("");
    let format = item.get("format").and_then(Value::as_str).unwrap_or("");
    format!(
        "{krate}::DesignAsset {{ id: {}, path: {}, format: {} }}",
        rust_string(id),
        rust_string(path),
        rust_string(format)
    )
}

fn font_asset_expr(krate: &str, item: &Value) -> Result<String> {
    let family = required_string(item, "family", "font asset")?;
    let path = required_string(item, "path", "font asset")?;
    let format = item
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("truetype");
    let weight = font_weight(item)?;
    let style = font_style_expr(krate, item)?;
    let axes = font_axes(item)?
        .into_iter()
        .map(|(tag, value)| {
            format!(
                "{krate}::FontVariationAxis {{ tag: {:?}, value: {} }}",
                tag.as_bytes(),
                f32_lit(value)
            )
        })
        .collect::<Vec<_>>();
    Ok(format!(
        "{krate}::DesignFontAsset {{ family: {}, weight: {weight}, style: {style}, path: {}, format: {}, axes: vec![{}] }}",
        rust_string(family),
        rust_string(path),
        rust_string(format),
        axes.join(",")
    ))
}

fn packaged_font_expr(krate: &str, root: &Path, item: &Value) -> Result<String> {
    let family = required_string(item, "family", "font asset")?;
    let weight = font_weight(item)?;
    let style = font_style_expr(krate, item)?;
    let format = item
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("truetype");
    let path = resolve_font_path(root, item)?;
    let axes = font_axes(item)?
        .into_iter()
        .map(|(tag, value)| {
            format!(
                "{krate}::FontVariationAxis {{ tag: {:?}, value: {} }}",
                tag.as_bytes(),
                f32_lit(value)
            )
        })
        .collect::<Vec<_>>();
    Ok(format!(
        "{krate}::PackagedFont {{ family: {:?}, weight: {weight}, style: {style}, format: {:?}, data: include_bytes!({:?}), axes: &[{}] }}",
        family,
        format,
        path.display().to_string(),
        axes.join(",")
    ))
}

fn resolve_font_path(root: &Path, item: &Value) -> Result<PathBuf> {
    let relative = required_string(item, "path", "font asset")?;
    let path = root.join(relative);
    path.canonicalize()
        .with_context(|| format!("font asset does not exist: {}", path.display()))
}

fn font_weight(item: &Value) -> Result<u16> {
    let weight = item.get("weight").and_then(Value::as_u64).unwrap_or(400);
    u16::try_from(weight)
        .ok()
        .filter(|weight| (1..=1000).contains(weight))
        .ok_or_else(|| anyhow!("font weight must be in the range 1..=1000, got {weight}"))
}

fn font_style_expr(krate: &str, item: &Value) -> Result<String> {
    let style = item
        .get("style")
        .and_then(Value::as_str)
        .unwrap_or("normal");
    let variant = match style {
        "normal" => "Normal",
        "italic" => "Italic",
        "oblique" => "Oblique",
        other => bail!("unsupported font style '{other}'"),
    };
    Ok(format!("{krate}::PackagedFontStyle::{variant}"))
}

fn font_axes(item: &Value) -> Result<Vec<(String, f32)>> {
    let Some(axes) = item.get("axes") else {
        return Ok(Vec::new());
    };
    let mut parsed = Vec::new();
    match axes {
        Value::Object(values) => {
            for (tag, value) in values {
                validate_axis_tag(tag)?;
                let value = value
                    .as_f64()
                    .ok_or_else(|| anyhow!("font axis '{tag}' must be numeric"))?;
                parsed.push((tag.clone(), value as f32));
            }
        }
        Value::Array(values) => {
            for axis in values {
                let tag = required_string(axis, "tag", "font axis")?;
                validate_axis_tag(tag)?;
                let value = axis
                    .get("value")
                    .and_then(Value::as_f64)
                    .ok_or_else(|| anyhow!("font axis '{tag}' must have a numeric value"))?;
                parsed.push((tag.to_string(), value as f32));
            }
        }
        _ => bail!("font axes must be an object or array"),
    }
    Ok(parsed)
}

fn validate_axis_tag(tag: &str) -> Result<()> {
    if tag.len() != 4 || !tag.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
        bail!("font variation axis tags must contain four printable ASCII bytes: '{tag}'");
    }
    Ok(())
}

fn required_string<'a>(item: &'a Value, field: &str, kind: &str) -> Result<&'a str> {
    item.get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("{kind} requires non-empty '{field}'"))
}

fn rust_string(value: &str) -> String {
    format!("{:?}.to_string()", value)
}

fn f32_lit(value: f32) -> String {
    if value.is_finite() && value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        let mut out = value.to_string();
        if !out.contains('.') && !out.contains('e') && !out.contains('E') {
            out.push_str(".0");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn token_groups_can_have_a_child_token_named_value() {
        let raw = json!({
            "color": {
                "light": {
                    "primary": { "value": "#2563EB", "type": "color" },
                    "value": { "value": "#A6E22E", "type": "color" },
                    "value_text": { "value": "#365314", "type": "color" }
                }
            }
        });

        let tokens = TokenStore::from_value(&raw).unwrap();

        assert!(tokens.contains("color.light.primary"));
        assert!(tokens.contains("color.light.value"));
        assert!(tokens.contains("color.light.value_text"));
        assert_eq!(tokens.resolve("color.light.primary").unwrap(), "#2563EB");
        assert_eq!(tokens.resolve("color.light.value").unwrap(), "#A6E22E");
    }

    #[test]
    fn legacy_scalar_value_without_type_still_counts_as_token() {
        let raw = json!({
            "spacing": {
                "small": { "value": "4px" }
            }
        });

        let tokens = TokenStore::from_value(&raw).unwrap();

        assert!(tokens.contains("spacing.small"));
        assert_eq!(tokens.resolve("spacing.small").unwrap(), "4px");
    }
}
