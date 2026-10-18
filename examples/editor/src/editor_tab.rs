use crate::layout::{ACTIVE_INDICATOR_THICKNESS, TAB_BAR_HEIGHT, TAB_CLOSE_SIZE};
use crate::model::{on_close_tab, on_select_tab, CloseTab, EditorState, SelectTab, TabInfo};
use crate::palette::EditorPalette;
use fission::prelude::*;

fission_component! {
    pub(crate) EditorTab<EditorState> {
        index: usize,
        tab: TabInfo,
        active: bool,
    } |tab, _state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
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
            tab.tab.title.clone()
        };
    } =>
        Button(
            variant: ButtonVariant::Ghost,
            content_align: ButtonContentAlign::Start,
            on_press: cx.bind(SelectTab(tab.index), reduce_with!(on_select_tab)),
            height: TAB_BAR_HEIGHT,
            padding: [tokens.spacing.none, tokens.spacing.none, tokens.spacing.none, tokens.spacing.none],
            child: Column(
                children: [
                    Container(Spacer())
                        .height(ACTIVE_INDICATOR_THICKNESS)
                        .bg(if tab.active { palette.tab_accent } else { palette.transparent }),
                    Container(
                        HStack(
                            spacing: tokens.spacing.s,
                            children: [
                                Text(title)
                                    .size(tokens.typography.font_size_sm)
                                    .color(text_color),
                                Button(
                                    variant: ButtonVariant::Ghost,
                                    child: Text("×")
                                        .size(tokens.typography.body_medium_size)
                                        .color(palette.tab_inactive_text),
                                    on_press: cx.bind(
                                        CloseTab(tab.index),
                                        reduce_with!(on_close_tab),
                                    ),
                                    width: TAB_CLOSE_SIZE,
                                    height: TAB_CLOSE_SIZE,
                                    padding: [tokens.spacing.none, tokens.spacing.none, tokens.spacing.none, tokens.spacing.none],
                                    semantics: Semantics {
                                        label: Some(format!("Close {}", tab.tab.title)),
                                        ..Default::default()
                                    },
                                ),
                            ],
                        ),
                    )
                    .bg(background)
                    .padding_all(tokens.spacing.s)
                    .flex_grow(1.0),
                ],
            ),
        )
}
