use crate::git_status_item::GitStatusItem;
use crate::layout::PANEL_ACTION_HEIGHT;
use crate::model::{EditorState, OpenFile, RefreshGitStatus};
use crate::palette::{DIM_TEXT, PANEL_TEXT, SURFACE_BG};
use fission::prelude::*;
use fission::widgets::{HStack, Spacer, VStack};

pub struct GitPanel;

impl From<GitPanel> for Widget {
    fn from(_component: GitPanel) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;

        let refresh = ctx.bind(
            RefreshGitStatus,
            reduce_with!((|s: &mut EditorState, _, _| s.refresh_git_status())),
        );

        let open_id = ctx
            .bind(
                OpenFile(String::new()),
                reduce_with!((|s: &mut EditorState, a: OpenFile, _| s.open_file(a.0))),
            )
            .id;

        let mut children = vec![HStack {
            spacing: Some(tokens.spacing.xs),
            children: widgets![
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                },
                Button {
                    variant: ButtonVariant::Ghost,
                    child: Some(
                        Text::new("Refresh")
                            .size(tokens.typography.font_size_xs)
                            .color(PANEL_TEXT)
                            .into(),
                    ),
                    on_press: Some(refresh),
                    height: Some(PANEL_ACTION_HEIGHT),
                    padding: Some([
                        tokens.spacing.xs,
                        tokens.spacing.xs,
                        tokens.spacing.none,
                        tokens.spacing.none,
                    ]),
                    ..Default::default()
                },
            ],
        }
        .into()];

        if view.state().git_status_lines.is_empty() {
            children.push(
                Text::new("No changes detected.\nClick ↻ to refresh.")
                    .size(tokens.typography.font_size_sm)
                    .color(DIM_TEXT)
                    .into(),
            );
        } else {
            let items = view
                .state()
                .git_status_lines
                .iter()
                .cloned()
                .map(|entry| GitStatusItem { entry, open_id }.into())
                .collect();

            children.push(
                Scroll {
                    direction: FlexDirection::Column,
                    child: Some(
                        VStack {
                            spacing: Some(tokens.spacing.none),
                            children: items,
                        }
                        .into(),
                    ),
                    show_scrollbar: true,
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    ..Default::default()
                }
                .into(),
            );
        }

        Container::new(Column {
            children,
            gap: Some(tokens.spacing.s),
            flex_grow: 1.0,
            justify_content: fission::core::op::JustifyContent::Start,
            ..Default::default()
        })
        .padding_all(tokens.spacing.s)
        .bg(SURFACE_BG)
        .flex_grow(1.0)
        .into()
    }
}
