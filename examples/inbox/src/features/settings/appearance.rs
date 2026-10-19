use super::section::SectionHeading;
use crate::features::settings_theme_preview::SettingsThemePreview;
use crate::model::settings::{set_density, set_theme, set_zoom_level};
use crate::model::{InboxState, SetDensity, SetTheme, SetZoomLevel};
use fission::prelude::*;
use fission::theme::{DesignMode, DesignSystem, FissionFluent2DesignSystem};
use serde::{Deserialize, Serialize};

const MIN_ZOOM: f32 = 0.75;
const MAX_ZOOM: f32 = 1.25;

#[derive(Clone, Copy, Serialize, Deserialize)]
enum AppearanceSelect {
    Theme,
    Density,
}

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

fission_component! {
/// Theme, density, zoom and the theme previews.
    pub(super) AppearanceSettings<InboxState> |_settings, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        VStack(
            spacing: tokens.spacing.m,
            children: [
                SectionHeading("settings.appearance"),
                SettingSelect(
                    "theme_select",
                    Some("settings.theme"),
                    "settings.theme.label",
                    "settings.theme.placeholder",
                    None,
                    &THEMES[..],
                    state.theme_mode.clone(),
                    state.show_theme_select,
                    AppearanceSelect::Theme,
                    THEMES
                        .iter()
                        .map(|(value, ..)| {
                            cx.bind(SetTheme((*value).into()), reduce_with!(set_theme))
                        })
                        .collect::<Vec<_>>(),
                ),
                SettingSelect(
                    "density_select",
                    None,
                    "settings.density.label",
                    "settings.density.placeholder",
                    Some("settings.density.helper"),
                    &DENSITIES[..],
                    state.density_mode.clone(),
                    state.show_density_select,
                    AppearanceSelect::Density,
                    DENSITIES
                        .iter()
                        .map(|(value, ..)| {
                            cx.bind(SetDensity((*value).into()), reduce_with!(set_density))
                        })
                        .collect::<Vec<_>>(),
                ),
                ZoomSetting(),
                Grid(
                    columns: vec![GridTrack::Fr(1.0), GridTrack::Fr(1.0)],
                    rows: vec![GridTrack::Auto],
                    column_gap: tokens.spacing.s,
                    row_gap: tokens.spacing.s,
                    padding: [tokens.spacing.none, tokens.spacing.none, tokens.spacing.none, tokens.spacing.none],
                    children: [
                        GridItem(ThemePreviewCard(
                            "settings.theme.preview_light",
                            "Light",
                            DesignMode::Light,
                        )),
                        GridItem(ThemePreviewCard(
                            "settings.theme.preview_dark",
                            "Dark",
                            DesignMode::Dark,
                        )),
                    ],
                ),
            ],
        )
}

fission_component! {
/// A labelled select over a table of `(value, label key, semantics identifier)`
/// options, with one bound action per option in the same order.
    SettingSelect<InboxState> {
        id: &'static str,
        trigger_identifier: Option<&'static str>,
        label_key: &'static str,
        placeholder_key: &'static str,
        helper_key: Option<&'static str>,
        options: &'static [(&'static str, &'static str, &'static str)],
        current: String,
        is_open: bool,
        kind: AppearanceSelect,
        on_select: Vec<ActionEnvelope>,
    } |select, _state, cx| {
        let selected_key = select
            .options
            .iter()
            .find(|(value, ..)| select.current == *value)
            .unwrap_or(&select.options[0])
            .1;
    } =>
        FormControl(
            label: Some(cx.tr(select.label_key)),
            helper: select.helper_key.map(|key| cx.tr(key)),
            child: Select(
                id: WidgetId::explicit(select.id),
                trigger_semantics_identifier: select.trigger_identifier.map(Into::into),
                selected_label: Some(cx.tr(selected_key)),
                placeholder: cx.tr(select.placeholder_key),
                is_open: select.is_open,
                    on_toggle: |select, state, _cx| match select.kind {
                        AppearanceSelect::Theme => {
                            state.show_theme_select = !state.show_theme_select;
                            if state.show_theme_select {
                                state.show_inbox_type_select = false;
                                state.show_density_select = false;
                            }
                        }
                        AppearanceSelect::Density => {
                            state.show_density_select = !state.show_density_select;
                            if state.show_density_select {
                                state.show_inbox_type_select = false;
                                state.show_theme_select = false;
                            }
                        }
                    },
                items: select
                    .options
                    .iter()
                    .zip(select.on_select.iter())
                    .map(|((_, key, identifier), on_select)| SelectItem {
                        label: cx.tr(key),
                        icon: None,
                        on_select: on_select.clone(),
                        semantics_identifier: Some((*identifier).into()),
                    })
                    .collect::<Vec<_>>(),
            ),
        )
}

fission_component! {
/// The reading zoom, from 75% to 125%.
    ZoomSetting<InboxState> |_setting, state, cx| {
        let zoom = state.zoom_level;
    } =>
        FormControl(
            label: Some(cx.tr("settings.zoom.label")),
            helper: Some(cx.tr("settings.zoom.helper")),
            child: Slider(
                semantics_identifier: Some("inbox.settings.zoom_level".into()),
                value: zoom,
                min: MIN_ZOOM,
                max: MAX_ZOOM,
                on_change: cx.bind(SetZoomLevel(zoom), reduce_with!(set_zoom_level)),
            ),
        )
}

fission_component! {
/// A captioned preview of the inbox's design system in one mode.
///
/// The swatch colours come from the design system itself, so the preview
/// matches what choosing it produces.
    ThemePreviewCard<InboxState> {
        caption_key: &'static str,
        theme_name: &'static str,
        mode: DesignMode,
    } |card, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let preview = FissionFluent2DesignSystem::theme(card.mode);
    } =>
        Card(
            child: VStack(
                spacing: tokens.spacing.s,
                children: [
                    Text(TextContent::Key(card.caption_key.into()))
                        .size(tokens.typography.font_size_xs),
                    SettingsThemePreview(
                        cx.bind(SetTheme(card.theme_name.into()), reduce_with!(set_theme)),
                        card.theme_name,
                        preview.tokens.colors.background,
                        preview.tokens.colors.primary,
                        state.theme_mode == card.theme_name,
                    ),
                ],
            ),
        )
}
