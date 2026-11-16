use crate::command_palette_item::CommandPaletteItem;
use crate::flyout_overlay::FillParent;
use crate::layout::{
    COMMAND_PALETTE_MAX_WIDTH, COMMAND_PALETTE_MIN_WIDTH, COMMAND_PALETTE_VERTICAL_RESERVE,
    COMMAND_RESULTS_MAX_HEIGHT, COMMAND_RESULTS_MIN_HEIGHT, OVERLAY_HORIZONTAL_RESERVE,
};
use crate::model::{on_run_command, EditorState, RunCommand, PALETTE_COMMANDS};
use crate::palette::EditorPalette;
use fission::prelude::*;

fission_component! {
/// A VS Code-style command list that drops from the top of the window.
    pub CommandPalette<EditorState> |_palette, state, cx| {
        if state.show_command_palette {
            cx.register_portal_with_layer(
                PortalLayer::Modal,
                Some(WidgetId::explicit("command_palette")),
                CommandPaletteOverlay::new().into(),
            );
        }
    } =>
    Spacer(height: 0.0)
}

fission_component! {
    CommandPaletteOverlay<EditorState> |_overlay, _state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
    } =>
    FillParent(
        Container(
            ZStack(
                children: [
                    FillParent(
                        GestureDetector(
                            on_tap: |_overlay, state, _cx| {
                                state.show_command_palette = false;
                                state.command_query.clear();
                            },
                            child: Container(Spacer())
                                .bg(palette.modal_backdrop)
                                .flex_grow(1.0),
                        ),
                    ),
                    Positioned(
                        top: Some(tokens.spacing.xl + tokens.spacing.s),
                        left: Some(tokens.spacing.none),
                        right: Some(tokens.spacing.none),
                        child: Some(Align::new(CommandPaletteDropdown::new()).into()),
                    ),
                ],
            ),
        ).flex_grow(1.0),
    )
}

fission_component! {
/// The query field and the commands matching it.
    CommandPaletteDropdown<EditorState> |_dropdown, state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
        let query = state.command_query.to_lowercase();
        let viewport = cx.view.viewport_size();
        let width = (viewport.width - OVERLAY_HORIZONTAL_RESERVE)
            .clamp(COMMAND_PALETTE_MIN_WIDTH, COMMAND_PALETTE_MAX_WIDTH);
        let results_height = (viewport.height - COMMAND_PALETTE_VERTICAL_RESERVE)
            .clamp(COMMAND_RESULTS_MIN_HEIGHT, COMMAND_RESULTS_MAX_HEIGHT);
    } =>
        Container(
            VStack(
            spacing: tokens.spacing.none,
            children: [
                Container(
                    TextInput(
                    id: Some(WidgetId::explicit("editor_command_palette_input")),
                    placeholder: Some("Type a command...".into()),
                    bind: state.command_query,
                    ),
                ).padding_all(tokens.spacing.s),
                Container(
                    Scroll(
                    direction: FlexDirection::Column,
                    child: VStack(
                        spacing: tokens.spacing.none,
                        children: [
                            for command in PALETTE_COMMANDS.iter().filter(|command| command.matches(&query)) {
                                CommandPaletteItem(
                                    command.label(),
                                    command.description(),
                                    cx.bind(RunCommand(*command), reduce_with!(on_run_command)),
                                    key: command.label(),
                                )
                            },
                        ],
                    ),
                    height: results_height,
                    show_scrollbar: true,
                    ),
                ).padding_all(tokens.spacing.xs),
            ],
            ),
            shadow: tokens.elevations.level3.clone(),
        )
        .width(width)
        .bg(palette.flyout_bg)
        .border(palette.flyout_border, 1.0)
        .border_radius(tokens.radii.small)
        .flex_shrink(1.0)
}
