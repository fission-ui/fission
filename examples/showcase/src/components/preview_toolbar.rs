use super::design_system_picker::DesignSystemPicker;
use crate::state::{
    on_reset_preview, on_set_locale, on_set_preview_viewport, on_set_theme, PreviewViewport,
    ResetPreview, SetLocale, SetPreviewViewport, SetTheme, ShowcaseState,
};
use fission::icons::material;
use fission::op::{AlignItems, FlexWrap};
use fission::prelude::*;
use fission::widgets::SegmentedControl;
use std::sync::Arc;

/// Locale codes and their names, written in their own language so they are not translated.
const LOCALES: [(&str, &str); 2] = [("en-US", "English"), ("es-ES", "Español")];
const VIEWPORTS: [PreviewViewport; 2] = [PreviewViewport::Desktop, PreviewViewport::Mobile];
const MODES: [DesignMode; 2] = [DesignMode::Light, DesignMode::Dark];

/// The preview's controls, placed directly above the preview they change:
/// reset, viewport, design system, light or dark, and language.
#[derive(Clone, Debug)]
pub(crate) struct PreviewToolbar;

impl From<PreviewToolbar> for Widget {
    fn from(_component: PreviewToolbar) -> Self {
        let (ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        let state = view.state();

        let viewport_actions: Vec<ActionEnvelope> = VIEWPORTS
            .iter()
            .map(|viewport| {
                with_reducer!(ctx, SetPreviewViewport(*viewport), on_set_preview_viewport)
            })
            .collect();
        let mode_actions: Vec<ActionEnvelope> = MODES
            .iter()
            .map(|mode| with_reducer!(ctx, SetTheme(*mode), on_set_theme))
            .collect();
        let locale_actions: Vec<ActionEnvelope> = LOCALES
            .iter()
            .map(|(code, _)| with_reducer!(ctx, SetLocale((*code).to_string()), on_set_locale))
            .collect();

        let viewport_index = VIEWPORTS
            .iter()
            .position(|viewport| *viewport == state.preview_viewport)
            .unwrap_or(0);
        let mode_index = MODES
            .iter()
            .position(|mode| *mode == state.theme_mode)
            .unwrap_or(0);
        let locale_index = LOCALES
            .iter()
            .position(|(code, _)| *code == state.locale.0.as_str())
            .unwrap_or(0);

        let reset: Widget = Button {
            variant: ButtonVariant::Outline,
            size: ComponentSize::Sm,
            content: Some(
                ButtonContent::new(TextContent::Key("showcase.workbench.reset".into()))
                    .leading_icon(Icon::svg(material::device::restart_alt::round())),
            ),
            on_press: Some(with_reducer!(ctx, ResetPreview, on_reset_preview)),
            ..Default::default()
        }
        .semantics_identifier("showcase.preview.reset")
        .into();

        // The container spans the pane so the bar's ground and rule reach the edge; the
        // toolbar semantics sit inside it, around the controls.
        Container::new(
            SemanticsRegion::new(Row {
                gap: Some(tokens.spacing.s),
                wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                children: vec![
                    reset,
                    Choice {
                        options: vec![
                            view.tr("showcase.workbench.viewport.desktop"),
                            view.tr("showcase.workbench.viewport.mobile"),
                        ],
                        selected: viewport_index,
                        actions: viewport_actions,
                        label: view.tr("showcase.workbench.viewport"),
                        identifier: "showcase.preview.viewport",
                    }
                    .into(),
                    DesignSystemPicker.into(),
                    Choice {
                        options: vec![
                            view.tr("showcase.workbench.light"),
                            view.tr("showcase.workbench.dark"),
                        ],
                        selected: mode_index,
                        actions: mode_actions,
                        label: view.tr("showcase.workbench.theme"),
                        identifier: "showcase.preview.theme",
                    }
                    .into(),
                    Choice {
                        options: LOCALES
                            .iter()
                            .map(|(_, name)| (*name).to_string())
                            .collect(),
                        selected: locale_index,
                        actions: locale_actions,
                        label: view.tr("showcase.workbench.locale"),
                        identifier: "showcase.preview.locale",
                    }
                    .into(),
                ],
                ..Default::default()
            })
            .role(Role::Toolbar)
            .label(view.tr("showcase.workbench.preview"))
            .identifier("showcase.preview.toolbar"),
        )
        .width_length(Length::percent(100.0))
        .padding_lengths(Length::symmetric(
            Length::points(tokens.spacing.l),
            Length::points(tokens.spacing.s),
        ))
        .bg(tokens.colors.surface)
        .border_bottom(tokens.colors.border, tokens.sizing.border_hairline)
        .into()
    }
}

/// One of a few options, shown as a segmented control and announced as a named group.
struct Choice {
    options: Vec<String>,
    selected: usize,
    actions: Vec<ActionEnvelope>,
    label: String,
    identifier: &'static str,
}

impl From<Choice> for Widget {
    fn from(choice: Choice) -> Self {
        let actions = choice.actions;
        SemanticsRegion::new(SegmentedControl {
            options: choice.options,
            selected_index: choice.selected,
            on_change: Some(Arc::new(move |index| actions[index].clone())),
        })
        .role(Role::Group)
        .label(choice.label)
        .identifier(choice.identifier)
        .into()
    }
}
