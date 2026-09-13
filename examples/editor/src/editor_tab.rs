use crate::layout::{ACTIVE_INDICATOR_THICKNESS, TAB_BAR_HEIGHT, TAB_CLOSE_SIZE};
use crate::model::{on_close_tab, on_select_tab, CloseTab, EditorState, SelectTab, TabInfo};
use crate::palette::EditorPalette;
use fission::prelude::*;
use fission::widgets::{HStack, Spacer};

pub(crate) struct EditorTab {
    pub index: usize,
    pub tab: TabInfo,
    pub active: bool,
}

impl From<EditorTab> for Widget {
    fn from(tab: EditorTab) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let palette = EditorPalette::from_theme(&view.env().theme);
        let tokens = &view.env().theme.tokens;
        let background = if tab.active {
            palette.tab_active_bg
        } else {
            palette.tab_inactive_bg
        };
        let text_color = if tab.active {
            tokens.colors.text_primary
        } else {
            palette.tab_inactive_text
        };
        let title = if tab.tab.is_dirty {
            format!("* {}", tab.tab.title)
        } else {
            tab.tab.title
        };

        let accent = Container::new(Spacer::default())
            .height(ACTIVE_INDICATOR_THICKNESS)
            .bg(if tab.active {
                palette.tab_accent
            } else {
                palette.transparent
            });

        let content = HStack {
            spacing: Some(tokens.spacing.s),
            children: widgets![
                Text::new(title)
                    .size(tokens.typography.font_size_sm)
                    .color(text_color),
                Button {
                    variant: ButtonVariant::Ghost,
                    child: Some(
                        Text::new("x")
                            .size(tokens.typography.body_medium_size)
                            .color(palette.tab_inactive_text)
                            .into(),
                    ),
                    on_press: Some(ctx.bind(CloseTab(tab.index), reduce_with!(on_close_tab))),
                    width: Some(TAB_CLOSE_SIZE),
                    height: Some(TAB_CLOSE_SIZE),
                    padding: Some([tokens.spacing.none; 4]),
                    ..Default::default()
                },
            ],
        };

        Button {
            variant: ButtonVariant::Ghost,
            content_align: ButtonContentAlign::Start,
            child: Some(
                Column {
                    children: widgets![
                        accent,
                        Container::new(content)
                            .bg(background)
                            .padding_all(tokens.spacing.s)
                            .flex_grow(1.0),
                    ],
                    ..Default::default()
                }
                .into(),
            ),
            on_press: Some(ctx.bind(SelectTab(tab.index), reduce_with!(on_select_tab))),
            height: Some(TAB_BAR_HEIGHT),
            padding: Some([tokens.spacing.none; 4]),
            ..Default::default()
        }
        .into()
    }
}
