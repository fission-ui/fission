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
use fission::core::{reduce_with, ActionEnvelope, WidgetId};
use fission::theme::{DesignMode, DesignSystem, FissionFluent2DesignSystem};
use fission::widgets::{Card, FormControl, Select, SelectItem, Slider, VStack};

const MIN_ZOOM: f32 = 0.75;
const MAX_ZOOM: f32 = 1.25;

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

        VStack {
            spacing: Some(tokens.spacing.m),
            children: vec![
                SectionHeading {
                    key: "settings.appearance",
                }
                .into(),
                SettingSelect {
                    id: "theme_select",
                    trigger_identifier: Some("settings.theme"),
                    label_key: "settings.theme.label",
                    placeholder_key: "settings.theme.placeholder",
                    helper_key: None,
                    options: &THEMES,
                    current: state.theme_mode.clone(),
                    is_open: state.show_theme_select,
                    on_toggle: ctx.bind(
                        SetThemeSelectOpen(!state.show_theme_select),
                        reduce_with!(set_theme_select_open),
                    ),
                    on_select: THEMES
                        .iter()
                        .map(|(value, ..)| {
                            ctx.bind(SetTheme((*value).into()), reduce_with!(set_theme))
                        })
                        .collect(),
                }
                .into(),
                SettingSelect {
                    id: "density_select",
                    trigger_identifier: None,
                    label_key: "settings.density.label",
                    placeholder_key: "settings.density.placeholder",
                    helper_key: Some("settings.density.helper"),
                    options: &DENSITIES,
                    current: state.density_mode.clone(),
                    is_open: state.show_density_select,
                    on_toggle: ctx.bind(
                        SetDensitySelectOpen(!state.show_density_select),
                        reduce_with!(set_density_select_open),
                    ),
                    on_select: DENSITIES
                        .iter()
                        .map(|(value, ..)| {
                            ctx.bind(SetDensity((*value).into()), reduce_with!(set_density))
                        })
                        .collect(),
                }
                .into(),
                ZoomSetting.into(),
                Grid {
                    id: None,
                    columns: vec![GridTrack::Fr(1.0), GridTrack::Fr(1.0)],
                    rows: vec![GridTrack::Auto],
                    column_gap: Some(tokens.spacing.s),
                    row_gap: Some(tokens.spacing.s),
                    padding: [tokens.spacing.none; 4],
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

/// A labelled select over a table of `(value, label key, semantics identifier)`
/// options, with one bound action per option in the same order.
struct SettingSelect {
    id: &'static str,
    trigger_identifier: Option<&'static str>,
    label_key: &'static str,
    placeholder_key: &'static str,
    helper_key: Option<&'static str>,
    options: &'static [(&'static str, &'static str, &'static str)],
    current: String,
    is_open: bool,
    on_toggle: ActionEnvelope,
    on_select: Vec<ActionEnvelope>,
}

impl From<SettingSelect> for Widget {
    fn from(select: SettingSelect) -> Self {
        let (_, view) = fission::build::current::<InboxState>();
        let selected_key = select
            .options
            .iter()
            .find(|(value, ..)| select.current == *value)
            .unwrap_or(&select.options[0])
            .1;
        FormControl {
            id: None,
            label: Some(view.tr(select.label_key)),
            required: false,
            error: None,
            helper: select.helper_key.map(|key| view.tr(key)),
            child: Select {
                id: WidgetId::explicit(select.id),
                trigger_semantics_identifier: select.trigger_identifier.map(Into::into),
                selected_label: Some(view.tr(selected_key)),
                placeholder: view.tr(select.placeholder_key),
                is_open: select.is_open,
                on_toggle: Some(select.on_toggle),
                items: select
                    .options
                    .iter()
                    .zip(select.on_select)
                    .map(|((_, key, identifier), on_select)| SelectItem {
                        label: view.tr(key),
                        icon: None,
                        on_select,
                        semantics_identifier: Some((*identifier).into()),
                    })
                    .collect(),
                ..Default::default()
            }
            .into(),
        }
        .into()
    }
}

/// The reading zoom, from 75% to 125%.
struct ZoomSetting;

impl From<ZoomSetting> for Widget {
    fn from(_: ZoomSetting) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let zoom = view.state().zoom_level;
        FormControl {
            id: None,
            label: Some(view.tr("settings.zoom.label")),
            required: false,
            error: None,
            helper: Some(view.tr("settings.zoom.helper")),
            child: Slider {
                id: None,
                semantics_identifier: Some("inbox.settings.zoom_level".into()),
                value: zoom,
                min: MIN_ZOOM,
                max: MAX_ZOOM,
                on_change: Some(ctx.bind(SetZoomLevel(zoom), reduce_with!(set_zoom_level))),
                ..Default::default()
            }
            .into(),
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
