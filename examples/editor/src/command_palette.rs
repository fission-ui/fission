use crate::command_palette_item::CommandPaletteItem;
use crate::flyout_overlay::FillParent;
use crate::layout::{
    COMMAND_PALETTE_MAX_WIDTH, COMMAND_PALETTE_MIN_WIDTH, COMMAND_PALETTE_VERTICAL_RESERVE,
    COMMAND_RESULTS_MAX_HEIGHT, COMMAND_RESULTS_MIN_HEIGHT, OVERLAY_HORIZONTAL_RESERVE,
};
use crate::model::{
    on_dismiss_command_palette, on_run_command, on_update_command_query, DismissCommandPalette,
    EditorState, RunCommand, UpdateCommandQuery, PALETTE_COMMANDS,
};
use crate::palette::EditorPalette;
use fission::core::op::FlexDirection;
use fission::core::ui::widgets::scroll::Scroll;
use fission::core::ui::{Align, Container, GestureDetector, Positioned, TextInput, Widget, ZStack};
use fission::core::{reduce_with, PortalLayer, WidgetId};
use fission::widgets::{Spacer, VStack};

/// A VS Code-style command list that drops from the top of the window.
pub struct CommandPalette;

impl From<CommandPalette> for Widget {
    fn from(_component: CommandPalette) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        if view.state().show_command_palette {
            let palette = EditorPalette::from_theme(&view.env().theme);
            let tokens = &view.env().theme.tokens;
            let backdrop = GestureDetector {
                on_tap: Some(ctx.bind(
                    DismissCommandPalette,
                    reduce_with!(on_dismiss_command_palette),
                )),
                child: Container::new(Spacer::default())
                    .bg(palette.modal_backdrop)
                    .flex_grow(1.0)
                    .into(),
                ..Default::default()
            };
            let overlay = Container::new(ZStack {
                children: vec![
                    FillParent(backdrop.into()).into(),
                    Positioned {
                        top: Some(tokens.spacing.xl + tokens.spacing.s),
                        left: Some(tokens.spacing.none),
                        right: Some(tokens.spacing.none),
                        child: Some(Align::new(Widget::from(CommandPaletteDropdown)).into()),
                        ..Default::default()
                    }
                    .into(),
                ],
                ..Default::default()
            })
            .flex_grow(1.0);
            ctx.register_portal_with_layer(
                PortalLayer::Modal,
                Some(WidgetId::explicit("command_palette")),
                FillParent(overlay.into()).into(),
            );
        }

        Spacer {
            height: Some(0.0),
            ..Default::default()
        }
        .into()
    }
}

/// The query field and the commands matching it.
struct CommandPaletteDropdown;

impl From<CommandPaletteDropdown> for Widget {
    fn from(_dropdown: CommandPaletteDropdown) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let palette = EditorPalette::from_theme(&view.env().theme);
        let tokens = &view.env().theme.tokens;
        let state = view.state();
        let query = state.command_query.to_lowercase();
        let results = PALETTE_COMMANDS
            .iter()
            .filter(|command| command.matches(&query))
            .map(|command| {
                CommandPaletteItem {
                    label: command.label(),
                    description: command.description(),
                    action: ctx.bind(RunCommand(*command), reduce_with!(on_run_command)),
                }
                .into()
            })
            .collect();

        let viewport = view.viewport_size();
        let width = (viewport.width - OVERLAY_HORIZONTAL_RESERVE)
            .clamp(COMMAND_PALETTE_MIN_WIDTH, COMMAND_PALETTE_MAX_WIDTH);
        let results_height = (viewport.height - COMMAND_PALETTE_VERTICAL_RESERVE)
            .clamp(COMMAND_RESULTS_MIN_HEIGHT, COMMAND_RESULTS_MAX_HEIGHT);

        let dropdown = Container::new(VStack {
            spacing: Some(tokens.spacing.none),
            children: vec![
                Container::new(TextInput {
                    id: Some(WidgetId::explicit("editor_command_palette_input")),
                    value: state.command_query.clone(),
                    placeholder: Some("Type a command...".into()),
                    on_input: Some(
                        ctx.bind(UpdateCommandQuery, reduce_with!(on_update_command_query)),
                    ),
                    ..Default::default()
                })
                .padding_all(tokens.spacing.s)
                .into(),
                Container::new(Scroll {
                    direction: FlexDirection::Column,
                    child: Some(
                        VStack {
                            spacing: Some(tokens.spacing.none),
                            children: results,
                        }
                        .into(),
                    ),
                    height: Some(results_height),
                    show_scrollbar: true,
                    ..Default::default()
                })
                .padding_all(tokens.spacing.xs)
                .into(),
            ],
        })
        .width(width)
        .bg(palette.flyout_bg)
        .border(palette.flyout_border, 1.0)
        .border_radius(tokens.radii.small)
        .flex_shrink(1.0);

        if let Some(shadow) = tokens.elevations.level3 {
            dropdown.shadow(shadow).into()
        } else {
            dropdown.into()
        }
    }
}
