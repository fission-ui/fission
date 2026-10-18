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

fission_component! {
/// The preview's controls, placed directly above the preview they change:
/// reset, viewport, design system, and light or dark.
///
/// Language lives in the app header because it changes the whole showcase; that
/// keeps these controls to one row on a wide window. When a narrow window has no
/// room for them, they wrap rather than hide.
    #[derive(Clone, Debug)] pub(crate) PreviewToolbar<ShowcaseState> {}
    |_component, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let viewport_actions: Vec<ActionEnvelope> = VIEWPORTS
            .iter()
            .map(|viewport| {
                with_reducer!(cx.build, SetPreviewViewport(*viewport), on_set_preview_viewport)
            })
            .collect();
        let mode_actions: Vec<ActionEnvelope> = MODES
            .iter()
            .map(|mode| with_reducer!(cx.build, SetTheme(*mode), on_set_theme))
            .collect();

        let viewport_index = VIEWPORTS
            .iter()
            .position(|viewport| *viewport == state.preview_viewport)
            .unwrap_or(0);
        let mode_index = MODES
            .iter()
            .position(|mode| *mode == state.theme_mode)
            .unwrap_or(0);

        // The container spans the pane so the bar's ground and rule reach the edge; the
        // toolbar semantics sit inside it, around the controls.
    } =>
        Container(
            SemanticsRegion(Row(
                gap: tokens.spacing.s,
                wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                children: [
                    Button(
                        variant: ButtonVariant::Outline,
                        size: ComponentSize::Sm,
                        content: ButtonContent::new(TextContent::Key("showcase.workbench.reset".into()))
                            .leading_icon(Icon::svg(material::device::restart_alt::round())),
                        on_press: with_reducer!(cx.build, ResetPreview, on_reset_preview),
                    )
                    .semantics_identifier("showcase.preview.reset"),
                    Choice(
                        vec![
                            cx.view.tr("showcase.workbench.viewport.desktop"),
                            cx.view.tr("showcase.workbench.viewport.mobile"),
                        ],
                        viewport_index,
                        viewport_actions,
                        cx.view.tr("showcase.workbench.viewport"),
                        "showcase.preview.viewport",
                    ),
                    DesignSystemPicker(),
                    Choice(
                        vec![
                            cx.view.tr("showcase.workbench.light"),
                            cx.view.tr("showcase.workbench.dark"),
                        ],
                        mode_index,
                        mode_actions,
                        cx.view.tr("showcase.workbench.theme"),
                        "showcase.preview.theme",
                    ),
                ],
            ))
            .role(Role::Toolbar)
            .label(cx.view.tr("showcase.workbench.preview"))
            .identifier("showcase.preview.toolbar"),
        )
        .width_length(Length::percent(100.0))
        .padding_lengths(Length::symmetric(
            Length::points(tokens.spacing.l),
            Length::points(tokens.spacing.s),
        ))
        .bg(tokens.colors.surface)
        .border_bottom(tokens.colors.border, tokens.sizing.border_hairline)
}

fission_component! {
/// One of a few options, shown as a segmented control and announced as a named group.
    Choice<ShowcaseState> {
        options: Vec<String>,
        selected: usize,
        actions: Vec<ActionEnvelope>,
        label: String,
        identifier: &'static str,
    }
    |choice, _state, _cx| {
        let actions = choice.actions.clone();
    } =>
        SemanticsRegion(SegmentedControl(
            options: choice.options,
            selected_index: choice.selected,
            on_change: Arc::new(move |index| actions[index].clone()),
        ))
        .role(Role::Group)
        .label(choice.label.clone())
        .identifier(choice.identifier)
}
