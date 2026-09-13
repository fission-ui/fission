use super::section::SectionHeading;
use crate::features::settings_theme_preview::SettingsThemePreview;
use crate::model::settings::{
    set_density, set_density_select_open, set_theme, set_theme_select_open, set_zoom_level,
};
use crate::model::{
    InboxState, SetDensity, SetDensitySelectOpen, SetTheme, SetThemeSelectOpen, SetZoomLevel,
};
use fission::core::op::GridTrack;
use fission::core::ui::{Grid, GridItem, Text, TextContent, Widget};
use fission::core::{reduce_with, WidgetId};
use fission::theme::{DesignMode, DesignSystem, FissionFluent2DesignSystem};
use fission::widgets::{Card, FormControl, Select, SelectItem, Slider, VStack};

/// Theme values, with each one's label key and semantics identifier.
const THEMES: [(&str, &str, &str); 3] = [
    ("Light", "settings.theme.light", "settings.theme.light"),
    ("Dark", "settings.theme.dark", "settings.theme.dark"),
    ("System", "settings.theme.system", "settings.theme.system"),
];

/// Density values, with each one's label key and semantics identifier.
const DENSITIES: [(&str, &str, &str); 3] = [
    (
        "Comfortable",
        "settings.density.comfortable",
        "settings.density.comfortable",
    ),
    (
        "Compact",
        "settings.density.compact",
        "settings.density.compact",
    ),
    ("Cozy", "settings.density.cozy", "settings.density.cozy"),
];

/// Theme, density, zoom and the theme previews.
pub(super) struct AppearanceSettings;

impl From<AppearanceSettings> for Widget {
    fn from(_: AppearanceSettings) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let state = view.state();
        let tokens = &view.env().theme.tokens;
        let theme_label = THEMES
            .iter()
            .find(|(value, ..)| state.theme_mode == *value)
            .unwrap_or(&THEMES[0])
            .1;
        let density_label = DENSITIES
            .iter()
            .find(|(value, ..)| state.density_mode == *value)
            .unwrap_or(&DENSITIES[0])
            .1;

        VStack {
            spacing: Some(tokens.spacing.m),
            children: vec![
                SectionHeading {
                    key: "settings.appearance",
                }
                .into(),
                FormControl {
                    id: None,
                    label: Some(view.tr("settings.theme.label")),
                    required: false,
                    error: None,
                    helper: None,
                    child: Select {
                        id: WidgetId::explicit("theme_select"),
                        trigger_semantics_identifier: Some("settings.theme".into()),
                        selected_label: Some(view.tr(theme_label)),
                        placeholder: view.tr("settings.theme.placeholder"),
                        is_open: state.show_theme_select,
                        on_toggle: Some(ctx.bind(
                            SetThemeSelectOpen(!state.show_theme_select),
                            reduce_with!(set_theme_select_open),
                        )),
                        items: THEMES
                            .iter()
                            .map(|(value, key, identifier)| SelectItem {
                                label: view.tr(key),
                                icon: None,
                                on_select: ctx
                                    .bind(SetTheme((*value).into()), reduce_with!(set_theme)),
                                semantics_identifier: Some((*identifier).into()),
                            })
                            .collect(),
                        ..Default::default()
                    }
                    .into(),
                }
                .into(),
                FormControl {
                    id: None,
                    label: Some(view.tr("settings.density.label")),
                    required: false,
                    error: None,
                    helper: Some(view.tr("settings.density.helper")),
                    child: Select {
                        id: WidgetId::explicit("density_select"),
                        selected_label: Some(view.tr(density_label)),
                        placeholder: view.tr("settings.density.placeholder"),
                        is_open: state.show_density_select,
                        on_toggle: Some(ctx.bind(
                            SetDensitySelectOpen(!state.show_density_select),
                            reduce_with!(set_density_select_open),
                        )),
                        items: DENSITIES
                            .iter()
                            .map(|(value, key, identifier)| SelectItem {
                                label: view.tr(key),
                                icon: None,
                                on_select: ctx
                                    .bind(SetDensity((*value).into()), reduce_with!(set_density)),
                                semantics_identifier: Some((*identifier).into()),
                            })
                            .collect(),
                        ..Default::default()
                    }
                    .into(),
                }
                .into(),
                FormControl {
                    id: None,
                    label: Some(view.tr("settings.zoom.label")),
                    required: false,
                    error: None,
                    helper: Some(view.tr("settings.zoom.helper")),
                    child: Slider {
                        id: None,
                        semantics_identifier: Some("inbox.settings.zoom_level".into()),
                        value: state.zoom_level,
                        min: 0.75,
                        max: 1.25,
                        on_change: Some(
                            ctx.bind(SetZoomLevel(state.zoom_level), reduce_with!(set_zoom_level)),
                        ),
                        ..Default::default()
                    }
                    .into(),
                }
                .into(),
                Grid {
                    id: None,
                    columns: vec![GridTrack::Fr(1.0), GridTrack::Fr(1.0)],
                    rows: vec![GridTrack::Auto],
                    column_gap: Some(tokens.spacing.s),
                    row_gap: Some(tokens.spacing.s),
                    padding: [0.0; 4],
                    children: vec![
                        GridItem::new(ThemePreviewCard {
                            caption_key: "settings.theme.preview_light",
                            theme_name: "Light",
                            mode: DesignMode::Light,
                        })
                        .into(),
                        GridItem::new(ThemePreviewCard {
                            caption_key: "settings.theme.preview_dark",
                            theme_name: "Dark",
                            mode: DesignMode::Dark,
                        })
                        .into(),
                    ],
                }
                .into(),
            ],
        }
        .into()
    }
}

/// A captioned preview of the inbox's design system in one mode.
///
/// The swatch colours come from the design system itself, so the preview
/// matches what choosing it produces.
struct ThemePreviewCard {
    caption_key: &'static str,
    theme_name: &'static str,
    mode: DesignMode,
}

impl From<ThemePreviewCard> for Widget {
    fn from(card: ThemePreviewCard) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let tokens = &view.env().theme.tokens;
        let preview = FissionFluent2DesignSystem::theme(card.mode);

        Card {
            child: VStack {
                spacing: Some(tokens.spacing.s),
                children: vec![
                    Text::new(TextContent::Key(card.caption_key.into()))
                        .size(tokens.typography.font_size_xs)
                        .into(),
                    SettingsThemePreview {
                        on_press: ctx
                            .bind(SetTheme(card.theme_name.into()), reduce_with!(set_theme)),
                        theme_name: card.theme_name,
                        background: preview.tokens.colors.background,
                        accent: preview.tokens.colors.primary,
                        is_active: view.state().theme_mode == card.theme_name,
                    }
                    .into(),
                ],
            }
            .into(),
            ..Default::default()
        }
        .into()
    }
}
