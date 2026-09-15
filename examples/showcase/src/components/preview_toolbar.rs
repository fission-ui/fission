use super::design_system_picker::DesignSystemPicker;
use crate::state::{
    on_reset_preview, on_set_preview_viewport, on_set_theme, PreviewViewport, ResetPreview,
    SetPreviewViewport, SetTheme, ShowcaseState,
};
use fission::icons::material;
use fission::op::{AlignItems, FlexWrap};
use fission::prelude::*;
use fission::widgets::SegmentedControl;
use std::sync::Arc;

const VIEWPORTS: [PreviewViewport; 2] = [PreviewViewport::Desktop, PreviewViewport::Mobile];
const MODES: [DesignMode; 2] = [DesignMode::Light, DesignMode::Dark];

/// The preview's controls, placed directly above the preview they change:
/// reset, viewport, design system, and light or dark.
///
/// Language lives in the app header because it changes the whole showcase; that
/// keeps these controls to one row on a wide window. When a narrow window has no
/// room for them, they wrap rather than hide.
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

        let viewport_index = VIEWPORTS
            .iter()
            .position(|viewport| *viewport == state.preview_viewport)
            .unwrap_or(0);
        let mode_index = MODES
            .iter()
            .position(|mode| *mode == state.theme_mode)
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
