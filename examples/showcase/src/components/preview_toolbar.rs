use super::design_system_picker::DesignSystemPicker;
use crate::semantics::ShowcaseSemantics;
use crate::state::{
    on_reset_preview, on_set_locale, on_set_preview_viewport, on_set_theme, PreviewViewport,
    ResetPreview, SetLocale, SetPreviewViewport, SetTheme, ShowcaseState,
};
use fission::icons::material;
use fission::op::{AlignItems, Fill, FlexWrap};
use fission::prelude::*;

/// The preview's controls: reset, viewport, locale, design system and mode.
#[derive(Clone, Debug)]
pub(crate) struct PreviewToolbar;

impl From<PreviewToolbar> for Widget {
    fn from(_component: PreviewToolbar) -> Self {
        let (ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        let state = view.state();
        let viewport = state.preview_viewport;
        let locale = state.locale.0.as_str();

        Container::new(Row {
            gap: Some(tokens.spacing.xs),
            wrap: FlexWrap::Wrap,
            align_items: AlignItems::Center,
            children: widgets![
                ResetButton,
                ToolbarToggle::icon(
                    material::hardware::computer::round(),
                    view.tr("showcase.workbench.viewport.desktop"),
                    "showcase.preview.viewport.desktop",
                )
                .selected(viewport == PreviewViewport::Desktop)
                .on_press(with_reducer!(
                    ctx,
                    SetPreviewViewport(PreviewViewport::Desktop),
                    on_set_preview_viewport
                )),
                ToolbarToggle::icon(
                    material::hardware::smartphone::round(),
                    view.tr("showcase.workbench.viewport.mobile"),
                    "showcase.preview.viewport.mobile",
                )
                .selected(viewport == PreviewViewport::Mobile)
                .on_press(with_reducer!(
                    ctx,
                    SetPreviewViewport(PreviewViewport::Mobile),
                    on_set_preview_viewport
                )),
                // Language names are written in their own language, so they are
                // not translated.
                ToolbarToggle::text("EN", "English", "showcase.preview.locale.english")
                    .selected(locale == "en-US")
                    .on_press(with_reducer!(ctx, SetLocale("en-US".into()), on_set_locale)),
                ToolbarToggle::text("ES", "Español", "showcase.preview.locale.spanish")
                    .selected(locale == "es-ES")
                    .on_press(with_reducer!(ctx, SetLocale("es-ES".into()), on_set_locale)),
                DesignSystemPicker,
                ToolbarToggle::icon(
                    material::image::wb_sunny::round(),
                    view.tr("showcase.workbench.light"),
                    "showcase.preview.theme.light",
                )
                .selected(state.theme_mode == DesignMode::Light)
                .on_press(with_reducer!(
                    ctx,
                    SetTheme(DesignMode::Light),
                    on_set_theme
                )),
                ToolbarToggle::icon(
                    material::image::brightness_3::round(),
                    view.tr("showcase.workbench.dark"),
                    "showcase.preview.theme.dark",
                )
                .selected(state.theme_mode == DesignMode::Dark)
                .on_press(with_reducer!(
                    ctx,
                    SetTheme(DesignMode::Dark),
                    on_set_theme
                )),
            ],
            ..Default::default()
        })
        .padding_lengths(Length::all(Length::points(tokens.spacing.s)))
        .bg_fill(Fill::Solid(tokens.colors.surface_sunken))
        .border_radius(tokens.radii.large)
        .into()
    }
}

/// Restores the preview to its initial state.
struct ResetButton;

impl From<ResetButton> for Widget {
    fn from(_button: ResetButton) -> Self {
        let (ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        Button {
            variant: ButtonVariant::TertiaryGray,
            size: ComponentSize::Sm,
            child: Some(
                Row {
                    gap: Some(tokens.spacing.xs),
                    align_items: AlignItems::Center,
                    children: widgets![
                        Icon::svg(material::device::restart_alt::round())
                            .size(tokens.typography.font_size_lg),
                        Text::new(view.tr("showcase.workbench.reset")),
                    ],
                    ..Default::default()
                }
                .into(),
            ),
            on_press: Some(with_reducer!(ctx, ResetPreview, on_reset_preview)),
            semantics: Some(
                Semantics::button(view.tr("showcase.workbench.reset"))
                    .identifier("showcase.preview.reset"),
            ),
            ..Default::default()
        }
        .into()
    }
}

enum ToggleFace {
    Icon(&'static str),
    Text(&'static str),
}

/// A toolbar button that shows whether its option is the active one.
struct ToolbarToggle {
    face: ToggleFace,
    label: String,
    identifier: &'static str,
    selected: bool,
    action: Option<ActionEnvelope>,
}

impl ToolbarToggle {
    fn icon(icon: &'static str, label: String, identifier: &'static str) -> Self {
        Self::new(ToggleFace::Icon(icon), label, identifier)
    }

    fn text(text: &'static str, label: &str, identifier: &'static str) -> Self {
        Self::new(ToggleFace::Text(text), label.to_string(), identifier)
    }

    fn new(face: ToggleFace, label: String, identifier: &'static str) -> Self {
        Self {
            face,
            label,
            identifier,
            selected: false,
            action: None,
        }
    }

    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn on_press(mut self, action: ActionEnvelope) -> Self {
        self.action = Some(action);
        self
    }
}

impl From<ToolbarToggle> for Widget {
    fn from(toggle: ToolbarToggle) -> Self {
        let (_, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        let face: Widget = match toggle.face {
            ToggleFace::Icon(icon) => Icon::svg(icon).size(tokens.typography.font_size_lg).into(),
            ToggleFace::Text(text) => Text::new(text).into(),
        };
        Button {
            variant: if toggle.selected {
                ButtonVariant::SecondaryColor
            } else {
                ButtonVariant::TertiaryGray
            },
            size: ComponentSize::Sm,
            child: Some(face),
            on_press: toggle.action,
            semantics: Some(Semantics::button(toggle.label).identifier(toggle.identifier)),
            ..Default::default()
        }
        .into()
    }
}
